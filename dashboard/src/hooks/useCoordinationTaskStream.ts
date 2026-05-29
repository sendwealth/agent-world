"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import type { CoordinationTask, WorldEvent } from "@/types/world";
import { fetchJSON } from "@/lib/api";
import { useSSEContext } from "@/components/SSEProvider";

const COORD_TASK_EVENT_TYPES = new Set<WorldEvent["type"]>([
  "coordination_task_created",
  "coordination_task_agent_joined",
  "coordination_task_agent_submitted",
  "coordination_task_completed",
  "coordination_task_cancelled",
  "coordination_task_expired",
]);

interface UseCoordinationTaskStreamOptions {
  pollInterval?: number;
  debounceDelay?: number;
}

/**
 * Hook that maintains a real-time list of coordination (team) tasks.
 *
 * Strategy:
 * 1. Initial REST fetch to get full coordination task list.
 * 2. SSE events trigger a debounced REST refresh.
 * 3. Falls back to periodic polling if SSE is not connected.
 */
export function useCoordinationTaskStream(
  options: UseCoordinationTaskStreamOptions = {},
) {
  const { pollInterval = 5000, debounceDelay = 500 } = options;

  const [tasks, setTasks] = useState<CoordinationTask[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const sse = useSSEContext();
  const refreshPending = useRef(false);
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const loadTasks = useCallback(async () => {
    try {
      const data = await fetchJSON<CoordinationTask[]>(
        "/api/v1/coordination-tasks",
      );
      setTasks(data);
      setError(null);
    } catch {
      setError("无法加载团队任务");
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    (async () => {
      await loadTasks();
    })();
  }, [loadTasks]);

  useEffect(() => {
    function onEvent(event: WorldEvent) {
      if (!COORD_TASK_EVENT_TYPES.has(event.type)) return;
      if (refreshPending.current) return;

      refreshPending.current = true;
      debounceRef.current = setTimeout(() => {
        loadTasks();
        refreshPending.current = false;
      }, debounceDelay);
    }

    const unsubscribe = sse.subscribe(onEvent);
    return () => {
      unsubscribe();
      if (debounceRef.current) {
        clearTimeout(debounceRef.current);
        debounceRef.current = null;
      }
      refreshPending.current = false;
    };
  }, [sse, loadTasks, debounceDelay]);

  useEffect(() => {
    if (sse.connected) return;

    const interval = setInterval(loadTasks, pollInterval);
    return () => clearInterval(interval);
  }, [sse, pollInterval, loadTasks]);

  return {
    tasks,
    loading,
    error,
    sseConnected: sse.connected,
    refresh: loadTasks,
  };
}
