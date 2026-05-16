"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import type { WorldEvent, WorldStats } from "@/types/world";
import { fetchJSON, sseEndpoint } from "@/lib/api";

const MAX_EVENTS = 100;

interface WorldState {
  stats: WorldStats | null;
  events: WorldEvent[];
  connected: boolean;
  error: string | null;
}

export function useWorldState() {
  const [state, setState] = useState<WorldState>({
    stats: null,
    events: [],
    connected: false,
    error: null,
  });

  const reconnectTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const abortRef = useRef<AbortController | null>(null);
  const connectRef = useRef<() => () => void>(() => () => {});

  // Fetch initial stats
  useEffect(() => {
    let cancelled = false;

    async function load() {
      try {
        const stats = await fetchJSON<WorldStats>("/api/v1/world/stats");
        if (!cancelled) {
          setState((s) => ({ ...s, stats }));
        }
      } catch {
        // Backend may not be running yet; we'll retry via SSE
      }
    }

    load();
    return () => {
      cancelled = true;
    };
  }, []);

  // SSE connection with auto-reconnect
  const connect = useCallback(() => {
    if (abortRef.current) {
      abortRef.current.abort();
    }

    const controller = new AbortController();
    abortRef.current = controller;

    const url = sseEndpoint("/api/v1/world/events");

    fetch(url, { signal: controller.signal })
      .then(async (res) => {
        if (!res.ok) throw new Error(`SSE connect failed: ${res.status}`);

        setState((s) => ({ ...s, connected: true, error: null }));

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
                setState((s) => ({
                  ...s,
                  events: [event, ...s.events].slice(0, MAX_EVENTS),
                  // Update stats from events if they carry tick info
                  stats: s.stats
                    ? { ...s.stats, tick: event.tick }
                    : s.stats,
                }));
              } catch {
                // Ignore malformed JSON
              }
            }
          }
        }
      })
      .catch((err) => {
        if (controller.signal.aborted) return;
        setState((s) => ({
          ...s,
          connected: false,
          error: err instanceof Error ? err.message : "SSE disconnected",
        }));
        // Reconnect after delay
        reconnectTimer.current = setTimeout(() => connectRef.current(), 3000);
      });

    return () => {
      controller.abort();
    };
  }, []);

  useEffect(() => {
    connectRef.current = connect;
  }, [connect]);

  useEffect(() => {
    const cleanup = connect();
    return () => {
      cleanup();
      if (reconnectTimer.current) clearTimeout(reconnectTimer.current);
    };
  }, [connect]);

  // Periodic stats refresh (every 5 seconds)
  useEffect(() => {
    const interval = setInterval(async () => {
      try {
        const stats = await fetchJSON<WorldStats>("/api/v1/world/stats");
        setState((s) => ({ ...s, stats }));
      } catch {
        // Silently ignore — SSE will reconnect
      }
    }, 5000);
    return () => clearInterval(interval);
  }, []);

  return state;
}
