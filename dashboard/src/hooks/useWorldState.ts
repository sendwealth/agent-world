"use client";

import { useEffect, useMemo, useState } from "react";
import type { WorldEvent, WorldStats } from "@/types/world";
import { fetchJSON } from "@/lib/api";
import { useSSEContext } from "@/components/SSEProvider";

interface WorldState {
  stats: WorldStats | null;
  events: WorldEvent[];
  connected: boolean;
  error: string | null;
}

export function useWorldState() {
  const [stats, setStats] = useState<WorldStats | null>(null);

  const sse = useSSEContext();

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
    if (!stats || sse.events.length === 0) return stats;
    const latestEvent = sse.events[0];
    return { ...stats, tick: latestEvent.tick };
  }, [stats, sse.events]);

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
    events: sse.events,
    connected: sse.connected,
    error: sse.error,
  };

  return state;
}
