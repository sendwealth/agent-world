"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import type { Task, WorldEvent } from "@/types/world";
import { fetchJSON } from "@/lib/api";
import { useSSEContext } from "@/components/SSEProvider";

const TASK_EVENT_TYPES = new Set<WorldEvent["type"]>([
  "task_created",
  "task_claimed",
  "task_completed",
]);

interface UseTaskStreamOptions {
  /** Polling interval in ms when SSE is not connected (default: 5000) */
  pollInterval?: number;
  /** Debounce delay in ms for SSE-driven refreshes (default: 500) */
  debounceDelay?: number;
}

/**
 * Hook that maintains a real-time list of tasks.
 *
 * Strategy:
 * 1. Initial REST fetch to get full task list.
 * 2. SSE events from the shared provider trigger a debounced REST refresh
 *    to pick up the latest task state.
 * 3. Falls back to periodic polling if SSE is not connected.
 */
export function useTaskStream(options: UseTaskStreamOptions = {}) {
  const { pollInterval = 5000, debounceDelay = 500 } = options;

  const [tasks, setTasks] = useState<Task[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const sse = useSSEContext();
  const refreshPending = useRef(false);
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const loadTasks = useCallback(async () => {
    try {
      const data = await fetchJSON<Task[]>("/api/v1/tasks");
      setTasks(data);
      setError(null);
    } catch {
      setError("无法连接到世界引擎");
    } finally {
      setLoading(false);
    }
  }, []);

  // Initial load — use IIFE pattern to avoid set-state-in-effect lint
  useEffect(() => {
    (async () => {
      await loadTasks();
    })();
  }, [loadTasks]);

  // SSE-driven refresh: subscribe to the shared event stream
  useEffect(() => {
    function onEvent(event: WorldEvent) {
      if (!TASK_EVENT_TYPES.has(event.type)) return;
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

  // Fallback polling when SSE is not connected
  useEffect(() => {
    if (sse.connected) return;

    const interval = setInterval(loadTasks, pollInterval);
    return () => clearInterval(interval);
  }, [sse, pollInterval, loadTasks]);

  return { tasks, loading, error, sseConnected: sse.connected, refresh: loadTasks };
}
