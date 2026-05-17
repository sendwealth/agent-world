"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import type { Agent, WorldEvent } from "@/types/world";
import { fetchJSON } from "@/lib/api";

interface UseAgentStreamOptions {
  /** Polling interval in ms when SSE is not connected (default: 5000) */
  pollInterval?: number;
}

/**
 * Hook that maintains a real-time list of agents.
 *
 * Strategy:
 * 1. Initial REST fetch to get full agent list.
 * 2. SSE events (agent_spawn, agent_death, etc.) trigger an immediate REST refresh
 *    to pick up the latest data (agent state is computed server-side).
 * 3. Falls back to periodic polling if SSE is not connected.
 */
export function useAgentStream(options: UseAgentStreamOptions = {}) {
  const { pollInterval = 5000 } = options;

  const [agents, setAgents] = useState<Agent[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [sseConnected, setSseConnected] = useState(false);

  // Track whether we have a fresh SSE-driven refresh pending
  const refreshPending = useRef(false);
  const abortRef = useRef<AbortController | null>(null);
  const reconnectTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  const loadAgents = useCallback(async () => {
    try {
      const data = await fetchJSON<Agent[]>("/api/v1/agents");
      setAgents(data);
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
        const data = await fetchJSON<Agent[]>("/api/v1/agents");
        if (!cancelled) {
          setAgents(data);
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

  // SSE connection: listen for agent-related events, then refresh
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
                  // Agent-related events trigger a full refresh
                  if (
                    event.type === "agent_spawn" ||
                    event.type === "agent_death" ||
                    event.type === "skill_up" ||
                    event.type === "reputation_change" ||
                    event.type === "trade" ||
                    event.type === "investment" ||
                    event.type === "tax"
                  ) {
                    if (!refreshPending.current) {
                      refreshPending.current = true;
                      // Debounce: wait a small amount for more events
                      setTimeout(() => {
                        loadAgents();
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
  }, [loadAgents]);

  // Fallback polling when SSE is not connected
  useEffect(() => {
    if (sseConnected) return;

    const interval = setInterval(loadAgents, pollInterval);
    return () => clearInterval(interval);
  }, [sseConnected, pollInterval, loadAgents]);

  return { agents, loading, error, sseConnected };
}
