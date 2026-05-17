"use client";

import { useEffect, useMemo, useState } from "react";
import type { WorldEvent, WorldStats } from "@/types/world";
import { fetchJSON } from "@/lib/api";
import { useSSE } from "./useSSE";

const MAX_EVENTS = 100;

interface WorldState {
  stats: WorldStats | null;
  events: WorldEvent[];
  connected: boolean;
  error: string | null;
}

export function useWorldState() {
  const [stats, setStats] = useState<WorldStats | null>(null);

  // SSE events via shared hook
  const sse = useSSE<WorldEvent>("/api/v1/world/events", {
    maxItems: MAX_EVENTS,
  });

  // Fetch initial stats
  useEffect(() => {
    let cancelled = false;

    async function load() {
      try {
        const data = await fetchJSON<WorldStats>("/api/v1/world/stats");
        if (!cancelled) setStats(data);
      } catch {
        // Backend may not be running yet; we'll retry via periodic refresh
      }
    }

    load();
    return () => {
      cancelled = true;
    };
  }, []);

  // Derive stats with tick update from SSE events (no setState in effect)
  const statsWithTick = useMemo(() => {
    if (!stats || sse.data.length === 0) return stats;
    const latestEvent = sse.data[0];
    return { ...stats, tick: latestEvent.tick };
  }, [stats, sse.data]);

  // Periodic stats refresh (every 5 seconds)
  useEffect(() => {
    const interval = setInterval(async () => {
      try {
        const data = await fetchJSON<WorldStats>("/api/v1/world/stats");
        setStats(data);
      } catch {
        // Silently ignore — SSE will reconnect
      }
    }, 5000);
    return () => clearInterval(interval);
  }, []);

  const state: WorldState = {
    stats: statsWithTick,
    events: sse.data,
    connected: sse.connected,
    error: sse.error,
  };

  return state;
}
