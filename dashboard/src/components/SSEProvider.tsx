"use client";

import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useRef,
  useState,
} from "react";
import type { WorldEvent } from "@/types/world";
import { sseEndpoint } from "@/lib/api";

interface SSEContextValue {
  /** All events received from the SSE stream (most recent first, capped). */
  events: WorldEvent[];
  /** Whether the SSE connection is currently active. */
  connected: boolean;
  /** Last error message, if any. */
  error: string | null;
  /** Register a callback invoked for each incoming SSE event. Returns an unsubscribe function. */
  subscribe: (cb: (event: WorldEvent) => void) => () => void;
}

const SSEContext = createContext<SSEContextValue | null>(null);

const MAX_EVENTS = 200;
const RECONNECT_DELAY = 3000;

export function SSEProvider({ children }: { children: React.ReactNode }) {
  const [events, setEvents] = useState<WorldEvent[]>([]);
  const [connected, setConnected] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const listenersRef = useRef<Set<(event: WorldEvent) => void>>(new Set());
  const abortRef = useRef<AbortController | null>(null);
  const reconnectTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const connectFnRef = useRef<() => () => void>(() => () => {});

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

        setConnected(true);
        setError(null);

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
                setEvents((prev) => [event, ...prev].slice(0, MAX_EVENTS));
                // Notify all listeners
                for (const cb of listenersRef.current) {
                  cb(event);
                }
              } catch {
                // Ignore malformed JSON
              }
            }
          }
        }
      })
      .catch((err) => {
        if (controller.signal.aborted) return;
        setConnected(false);
        setError(err instanceof Error ? err.message : "SSE disconnected");
        reconnectTimer.current = setTimeout(
          () => connectFnRef.current(),
          RECONNECT_DELAY
        );
      });

    return () => {
      controller.abort();
    };
  }, []);

  useEffect(() => {
    connectFnRef.current = connect;
  }, [connect]);

  useEffect(() => {
    const cleanup = connect();
    return () => {
      cleanup();
      if (reconnectTimer.current) clearTimeout(reconnectTimer.current);
    };
  }, [connect]);

  const subscribe = useCallback((cb: (event: WorldEvent) => void) => {
    listenersRef.current.add(cb);
    return () => {
      listenersRef.current.delete(cb);
    };
  }, []);

  return (
    <SSEContext.Provider value={{ events, connected, error, subscribe }}>
      {children}
    </SSEContext.Provider>
  );
}

/**
 * Hook to access the shared SSE connection.
 * Must be used within an <SSEProvider>.
 */
export function useSSEContext(): SSEContextValue {
  const ctx = useContext(SSEContext);
  if (!ctx) {
    throw new Error("useSSEContext must be used within an SSEProvider");
  }
  return ctx;
}
