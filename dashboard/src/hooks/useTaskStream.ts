"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import type { Task, WorldEvent } from "@/types/world";
import { fetchJSON } from "@/lib/api";

interface UseTaskStreamOptions {
  /** Polling interval in ms when SSE is not connected (default: 5000) */
  pollInterval?: number;
}

/**
 * Hook that maintains a real-time list of tasks.
 *
 * Strategy:
 * 1. Initial REST fetch to get full task list.
 * 2. SSE events (task_created, task_claimed, task_completed) trigger an immediate
 *    REST refresh to pick up the latest task state.
 * 3. Falls back to periodic polling if SSE is not connected.
 */
export function useTaskStream(options: UseTaskStreamOptions = {}) {
  const { pollInterval = 5000 } = options;

  const [tasks, setTasks] = useState<Task[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [sseConnected, setSseConnected] = useState(false);

  const refreshPending = useRef(false);
  const abortRef = useRef<AbortController | null>(null);
  const reconnectTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

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

  // Initial load
  useEffect(() => {
    let cancelled = false;

    async function load() {
      try {
        const data = await fetchJSON<Task[]>("/api/v1/tasks");
        if (!cancelled) {
          setTasks(data);
          setError(null);
        }
      } catch {
        if (!cancelled) setError("无法连接到世界引擎");
      } finally {
        if (!cancelled) setLoading(false);
      }
    }

    load();
    return () => {
      cancelled = true;
    };
  }, []);

  // SSE connection: listen for task-related events, then refresh
  useEffect(() => {
    const controller = new AbortController();
    abortRef.current = controller;

    function connect() {
      const url = `${process.env.NEXT_PUBLIC_API_URL ?? ""}/api/v1/world/events`;

      fetch(url, { signal: controller.signal })
        .then(async (res) => {
          if (!res.ok) throw new Error(`SSE connect failed: ${res.status}`);
          setSseConnected(true);

          const reader = res.body?.getReader();
          if (!reader) return;

          const decoder = new TextDecoder();
          let buffer = "";

          while (true) {
            const { done, value } = await reader.read();
            if (done) break;

            buffer += decoder.decode(value, { stream: true });
            const lines = buffer.split("\n");
            buffer = lines.pop() ?? "";

            for (const line of lines) {
              if (line.startsWith("data: ")) {
                try {
                  const event: WorldEvent = JSON.parse(line.slice(6));
                  if (
                    event.type === "task_created" ||
                    event.type === "task_claimed" ||
                    event.type === "task_completed"
                  ) {
                    if (!refreshPending.current) {
                      refreshPending.current = true;
                      setTimeout(() => {
                        loadTasks();
                        refreshPending.current = false;
                      }, 500);
                    }
                  }
                } catch {
                  // Ignore malformed JSON
                }
            }
            }
          }
        })
        .catch(() => {
          if (controller.signal.aborted) return;
          setSseConnected(false);
          reconnectTimer.current = setTimeout(connect, 3000);
        });
    }

    connect();

    return () => {
      controller.abort();
      if (reconnectTimer.current) clearTimeout(reconnectTimer.current);
    };
  }, [loadTasks]);

  // Fallback polling when SSE is not connected
  useEffect(() => {
    if (sseConnected) return;

    const interval = setInterval(loadTasks, pollInterval);
    return () => clearInterval(interval);
  }, [sseConnected, pollInterval, loadTasks]);

  return { tasks, loading, error, sseConnected, refresh: loadTasks };
}
