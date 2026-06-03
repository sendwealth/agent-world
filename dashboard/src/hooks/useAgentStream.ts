"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import type { Agent, WorldEvent } from "@/types/world";
import { fetchJSON } from "@/lib/api";
import { useSSEContext } from "@/components/SSEProvider";

const AGENT_EVENT_TYPES = new Set<WorldEvent["type"]>([
  "agent_spawned",
  "agent_died",
  "skill_level_up",
  "reputation_changed",
  "transaction_completed",
  "investment_purchased",
  "tax_collected",
]);

interface UseAgentStreamOptions {
  /** Polling interval in ms when SSE is not connected (default: 5000) */
  pollInterval?: number;
  /** Debounce delay in ms for SSE-driven refreshes (default: 500) */
  debounceDelay?: number;
}

/**
 * Hook that maintains a real-time list of agents.
 *
 * Strategy:
 * 1. Initial REST fetch to get full agent list.
 * 2. SSE events from the shared provider trigger a debounced REST refresh
 *    to pick up the latest data (agent state is computed server-side).
 * 3. Falls back to periodic polling if SSE is not connected.
 */
export function useAgentStream(options: UseAgentStreamOptions = {}) {
  const { pollInterval = 5000, debounceDelay = 500 } = options;

  const [agents, setAgents] = useState<Agent[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const sse = useSSEContext();
  const refreshPending = useRef(false);
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const mountedRef = useRef(false);

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

  // Initial load — use IIFE pattern to avoid set-state-in-effect lint
  useEffect(() => {
    mountedRef.current = true;
    (async () => {
      await loadAgents();
    })();
  }, [loadAgents]);

  // SSE-driven refresh: subscribe to the shared event stream
  useEffect(() => {
    function onEvent(event: WorldEvent) {
      if (!AGENT_EVENT_TYPES.has(event.type)) return;
      if (refreshPending.current) return;

      refreshPending.current = true;
      debounceRef.current = setTimeout(() => {
        loadAgents();
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
  }, [sse, loadAgents, debounceDelay]);

  // Fallback polling when SSE is not connected
  useEffect(() => {
    if (sse.connected) return;

    const interval = setInterval(loadAgents, pollInterval);
    return () => clearInterval(interval);
  }, [sse, pollInterval, loadAgents]);

  return { agents, loading, error, sseConnected: sse.connected };
}
