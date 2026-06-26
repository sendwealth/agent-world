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
import { transformSSEEvent } from "@/lib/transform-sse-event";

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
const RECONNECT_BASE_MS = 1000;
const RECONNECT_MAX_MS = 30000;
let reconnectAttemptCount = 0;

export function SSEProvider({ children }: { children: React.ReactNode }) {
  const [events, setEvents] = useState<WorldEvent[]>([]);
  const [connected, setConnected] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const listenersRef = useRef<Set<(event: WorldEvent) => void>>(new Set());
  const abortRef = useRef<AbortController | null>(null);
  const reconnectTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  /** Cleanup from the currently active connect() — used to tear down old connections during reconnect. */
  const cleanupRef = useRef<(() => void) | null>(null);
  const connect = useCallback(() => {
    // Properly tear down the previous connection before starting a new one.
    // Without this, each reconnect creates a new AbortController while the
    // old one's fetch stream and reader are left dangling — that's the leak.
    if (cleanupRef.current) {
      cleanupRef.current();
      cleanupRef.current = null;
    }

    // Also cancel any pending reconnect timer so we don't race with a new one.
    if (reconnectTimer.current) {
      clearTimeout(reconnectTimer.current);
      reconnectTimer.current = null;
    }

    const controller = new AbortController();
    abortRef.current = controller;

    const url = sseEndpoint("/api/v1/world/events");

    fetch(url, { signal: controller.signal })
      .then(async (res) => {
        if (!res.ok) throw new Error(`SSE connect failed: ${res.status}`);

        setConnected(true);
        setError(null);
        // Reset backoff counter on successful connection
        reconnectAttemptCount = 0;

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
                const raw = JSON.parse(line.slice(6));
                const event: WorldEvent = transformSSEEvent(raw);
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
        const delay = Math.min(
          RECONNECT_BASE_MS * 2 ** reconnectAttemptCount,
          RECONNECT_MAX_MS,
        );
        reconnectAttemptCount++;
        reconnectTimer.current = setTimeout(() => {
          connect();
        }, delay);
      });

    // Store the cleanup function so reconnects can call it first,
    // and so the useEffect cleanup can also invoke it.
    const cleanup = () => {
      controller.abort();
    };
    cleanupRef.current = cleanup;

    return cleanup;
  }, []);

  useEffect(() => {
    const cleanup = connect();
    return () => {
      cleanup();
      if (reconnectTimer.current) clearTimeout(reconnectTimer.current);
      cleanupRef.current = null;
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
