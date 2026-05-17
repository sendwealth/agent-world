"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import { sseEndpoint } from "@/lib/api";

interface SSEState<T> {
  data: T[];
  connected: boolean;
  error: string | null;
}

interface UseSSEOptions {
  /** Maximum number of items to keep in state (default: 100) */
  maxItems?: number;
  /** Reconnect delay in ms (default: 3000) */
  reconnectDelay?: number;
}

/**
 * Reusable SSE hook that connects to a Server-Sent Events endpoint,
 * parses incoming `data:` lines as JSON, and manages auto-reconnection.
 *
 * @param path - API path (e.g. "/api/v1/world/events")
 * @param options - Configuration for max items and reconnect delay
 */
export function useSSE<T>(path: string, options: UseSSEOptions = {}) {
  const { maxItems = 100, reconnectDelay = 3000 } = options;

  const [state, setState] = useState<SSEState<T>>({
    data: [],
    connected: false,
    error: null,
  });

  const reconnectTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const abortRef = useRef<AbortController | null>(null);
  const connectRef = useRef<() => () => void>(() => () => {});

  // Stable callback ref so consumers can react to individual events
  const onEventRef = useRef<((event: T) => void) | null>(null);

  const connect = useCallback(() => {
    if (abortRef.current) {
      abortRef.current.abort();
    }

    const controller = new AbortController();
    abortRef.current = controller;

    const url = sseEndpoint(path);

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
                const event: T = JSON.parse(line.slice(6));
                setState((s) => ({
                  ...s,
                  data: [event, ...s.data].slice(0, maxItems),
                }));
                // Notify event listener
                onEventRef.current?.(event);
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
        reconnectTimer.current = setTimeout(
          () => connectRef.current(),
          reconnectDelay
        );
      });

    return () => {
      controller.abort();
    };
  }, [path, maxItems, reconnectDelay]);

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

  return state;
}
