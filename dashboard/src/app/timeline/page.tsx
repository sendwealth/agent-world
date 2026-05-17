"use client";

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import Link from "next/link";
import type { WorldEvent, EventType } from "@/types/world";
import { fetchJSON, sseEndpoint } from "@/lib/api";

const MAX_EVENTS = 500;

// Event type config with icons, colors, and labels
const eventTypeConfig: Record<
  EventType,
  { label: string; color: string; bg: string; border: string; dot: string; icon: string }
> = {
  agent_spawn: {
    label: "诞生",
    color: "text-green-400",
    bg: "bg-green-500/10",
    border: "border-green-500/20",
    dot: "bg-green-400",
    icon: "👶",
  },
  agent_death: {
    label: "死亡",
    color: "text-red-400",
    bg: "bg-red-500/10",
    border: "border-red-500/20",
    dot: "bg-red-400",
    icon: "💀",
  },
  trade: {
    label: "交易",
    color: "text-blue-400",
    bg: "bg-blue-500/10",
    border: "border-blue-500/20",
    dot: "bg-blue-400",
    icon: "💰",
  },
  task_created: {
    label: "新任务",
    color: "text-purple-400",
    bg: "bg-purple-500/10",
    border: "border-purple-500/20",
    dot: "bg-purple-400",
    icon: "📋",
  },
  task_claimed: {
    label: "认领",
    color: "text-indigo-400",
    bg: "bg-indigo-500/10",
    border: "border-indigo-500/20",
    dot: "bg-indigo-400",
    icon: "✋",
  },
  task_completed: {
    label: "完成",
    color: "text-emerald-400",
    bg: "bg-emerald-500/10",
    border: "border-emerald-500/20",
    dot: "bg-emerald-400",
    icon: "✅",
  },
  message: {
    label: "消息",
    color: "text-sky-400",
    bg: "bg-sky-500/10",
    border: "border-sky-500/20",
    dot: "bg-sky-400",
    icon: "💬",
  },
  skill_up: {
    label: "技能提升",
    color: "text-yellow-400",
    bg: "bg-yellow-500/10",
    border: "border-yellow-500/20",
    dot: "bg-yellow-400",
    icon: "⬆",
  },
  reputation_change: {
    label: "信誉变化",
    color: "text-orange-400",
    bg: "bg-orange-500/10",
    border: "border-orange-500/20",
    dot: "bg-orange-400",
    icon: "⭐",
  },
  inflation: {
    label: "通胀",
    color: "text-amber-400",
    bg: "bg-amber-500/10",
    border: "border-amber-500/20",
    dot: "bg-amber-400",
    icon: "📈",
  },
  investment: {
    label: "投资",
    color: "text-teal-400",
    bg: "bg-teal-500/10",
    border: "border-teal-500/20",
    dot: "bg-teal-400",
    icon: "🏦",
  },
  tax: {
    label: "税收",
    color: "text-rose-400",
    bg: "bg-rose-500/10",
    border: "border-rose-500/20",
    dot: "bg-rose-400",
    icon: "🏛",
  },
};

const defaultConfig = {
  label: "未知",
  color: "text-zinc-400",
  bg: "bg-zinc-500/10",
  border: "border-zinc-500/20",
  dot: "bg-zinc-400",
  icon: "•",
};

function formatTime(ts: string): string {
  const d = new Date(ts);
  return d.toLocaleTimeString("zh-CN", {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
}

function formatDate(ts: string): string {
  const d = new Date(ts);
  return d.toLocaleDateString("zh-CN", {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  });
}

export default function TimelinePage() {
  const [events, setEvents] = useState<WorldEvent[]>([]);
  const [connected, setConnected] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [filter, setFilter] = useState<EventType | "all">("all");
  const [search, setSearch] = useState("");
  const reconnectTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const abortRef = useRef<AbortController | null>(null);
  const connectRef = useRef<() => () => void>(() => () => {});
  const scrollRef = useRef<HTMLDivElement>(null);
  const [autoScroll, setAutoScroll] = useState(true);

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

  // Also load initial events from REST API
  useEffect(() => {
    let cancelled = false;
    async function load() {
      try {
        const data = await fetchJSON<WorldEvent[]>("/api/v1/world/events");
        if (!cancelled && Array.isArray(data)) {
          setEvents((prev) => {
            const existing = new Set(prev.map((e) => e.id));
            const merged = [...data.filter((e) => !existing.has(e.id)), ...prev];
            return merged
              .sort(
                (a, b) =>
                  new Date(b.timestamp).getTime() - new Date(a.timestamp).getTime()
              )
              .slice(0, MAX_EVENTS);
          });
        }
      } catch {
        // Backend may not have REST endpoint for events, SSE will work
      }
    }
    load();
    return () => {
      cancelled = true;
    };
  }, []);

  // Auto-scroll to top on new events
  useEffect(() => {
    if (autoScroll && scrollRef.current) {
      scrollRef.current.scrollTop = 0;
    }
  }, [events, autoScroll]);

  // Detect manual scroll
  const handleScroll = useCallback(() => {
    if (!scrollRef.current) return;
    const { scrollTop } = scrollRef.current;
    setAutoScroll(scrollTop < 50);
  }, []);

  // Filter and search
  const filteredEvents = useMemo(() => {
    let result = events;

    if (filter !== "all") {
      result = result.filter((e) => e.type === filter);
    }

    if (search.trim()) {
      const q = search.toLowerCase();
      result = result.filter(
        (e) =>
          e.description.toLowerCase().includes(q) ||
          e.agentName?.toLowerCase().includes(q) ||
          e.targetName?.toLowerCase().includes(q)
      );
    }

    return result;
  }, [events, filter, search]);

  // Count per type
  const typeCounts = useMemo(() => {
    const counts: Partial<Record<EventType, number>> = {};
    for (const e of events) {
      counts[e.type] = (counts[e.type] ?? 0) + 1;
    }
    return counts;
  }, [events]);

  return (
    <div className="flex h-full flex-col">
      {/* Header */}
      <div className="shrink-0 border-b border-zinc-800 bg-zinc-950/80 px-4 md:px-6 py-4 backdrop-blur">
        <div className="flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between">
          <div>
            <h1 className="text-2xl font-bold text-zinc-100">事件时间线</h1>
            <p className="text-sm text-zinc-500">
              共 {events.length} 条事件
              {filter !== "all" && ` · 已筛选 ${filteredEvents.length} 条`}
            </p>
          </div>
          <div className="flex items-center gap-3">
            <div className="flex items-center gap-1.5">
              <span
                className={`inline-block h-2 w-2 rounded-full ${
                  connected ? "bg-green-400 animate-pulse" : "bg-red-400"
                }`}
              />
              <span className="text-xs text-zinc-500">
                {connected ? "SSE 已连接" : "断开连接"}
              </span>
            </div>
            {error && (
              <span className="text-xs text-red-400">{error}</span>
            )}
          </div>
        </div>

        {/* Filter bar */}
        <div className="mt-3 flex flex-wrap items-center gap-2">
          <button
            onClick={() => setFilter("all")}
            className={`rounded-full px-3 py-1 text-xs font-medium transition-colors ${
              filter === "all"
                ? "bg-blue-500/20 text-blue-400 border border-blue-500/30"
                : "bg-zinc-800/50 text-zinc-400 border border-zinc-800 hover:bg-zinc-800 hover:text-zinc-200"
            }`}
          >
            全部
          </button>
          {(Object.keys(eventTypeConfig) as EventType[]).map((type) => {
            const cfg = eventTypeConfig[type];
            const count = typeCounts[type] ?? 0;
            return (
              <button
                key={type}
                onClick={() => setFilter(type)}
                className={`flex items-center gap-1.5 rounded-full px-3 py-1 text-xs font-medium transition-colors ${
                  filter === type
                    ? `${cfg.bg} ${cfg.color} ${cfg.border} border`
                    : "bg-zinc-800/50 text-zinc-400 border border-zinc-800 hover:bg-zinc-800 hover:text-zinc-200"
                }`}
              >
                <span className="text-sm leading-none">{cfg.icon}</span>
                {cfg.label}
                {count > 0 && (
                  <span className="text-[10px] tabular-nums opacity-60">
                    {count}
                  </span>
                )}
              </button>
            );
          })}
        </div>

        {/* Search */}
        <div className="mt-3">
          <input
            type="text"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            placeholder="搜索事件描述、Agent 名称..."
            className="w-full max-w-md rounded-lg border border-zinc-800 bg-zinc-900/50 px-3 py-1.5 text-sm text-zinc-200 placeholder-zinc-600 outline-none transition-colors focus:border-blue-500/40"
          />
        </div>
      </div>

      {/* Timeline */}
      <div
        ref={scrollRef}
        onScroll={handleScroll}
        className="flex-1 overflow-y-auto scrollbar-thin"
      >
        {filteredEvents.length === 0 ? (
          <div className="flex h-full items-center justify-center">
            <div className="text-center space-y-2">
              <p className="text-lg text-zinc-600">
                {events.length === 0 ? "等待事件..." : "没有匹配的事件"}
              </p>
              <p className="text-xs text-zinc-700">
                {events.length === 0
                  ? "连接世界引擎后将实时推送事件"
                  : "尝试调整筛选条件"}
              </p>
            </div>
          </div>
        ) : (
          <div className="p-4 md:p-6">
            <div className="mx-auto max-w-3xl space-y-0">
              {filteredEvents.map((event, idx) => {
                const config = eventTypeConfig[event.type] ?? defaultConfig;
                const isLast = idx === filteredEvents.length - 1;

                return (
                  <div key={event.id} className="relative flex gap-4 pb-0">
                    {/* Timeline connector line */}
                    {!isLast && (
                      <div className="absolute left-[11px] top-6 h-full w-px bg-zinc-800" />
                    )}

                    {/* Event dot */}
                    <div className="relative mt-1.5 flex shrink-0 items-center justify-center">
                      <div
                        className={`h-5 w-5 rounded-full ${config.bg} ${config.border} border flex items-center justify-center`}
                      >
                        <span className="text-[10px] leading-none">{config.icon}</span>
                      </div>
                    </div>

                    {/* Event content card */}
                    <div className="min-w-0 flex-1 pb-4">
                      <div className="rounded-lg border border-zinc-800/60 bg-zinc-900/30 px-4 py-3 transition-colors hover:bg-zinc-800/30">
                        {/* Header row */}
                        <div className="flex items-center justify-between gap-2">
                          <div className="flex items-center gap-2">
                            <span
                              className={`inline-block rounded px-1.5 py-0.5 text-[10px] font-medium ${config.bg} ${config.color}`}
                            >
                              {config.label}
                            </span>
                            <span className="text-[10px] tabular-nums text-zinc-600">
                              Tick #{event.tick}
                            </span>
                          </div>
                          <span className="shrink-0 text-[10px] text-zinc-600">
                            {formatTime(event.timestamp)}
                          </span>
                        </div>

                        {/* Description */}
                        <p className="mt-1 text-sm text-zinc-300 leading-relaxed">
                          {event.description}
                        </p>

                        {/* Metadata row */}
                        {(event.agentName || event.targetName || event.amount != null) && (
                          <div className="mt-2 flex flex-wrap items-center gap-2 text-xs text-zinc-500">
                            {event.agentName && (
                              <Link
                                href={`/agents/${event.agentId}`}
                                className="text-blue-400 hover:text-blue-300 transition-colors"
                              >
                                {event.agentName}
                              </Link>
                            )}
                            {event.agentName && event.targetName && (
                              <span className="text-zinc-700">→</span>
                            )}
                            {event.targetName && (
                              <Link
                                href={`/agents/${event.targetId}`}
                                className="text-blue-400 hover:text-blue-300 transition-colors"
                              >
                                {event.targetName}
                              </Link>
                            )}
                            {event.amount != null && (
                              <span className="tabular-nums font-medium text-amber-400">
                                {event.amount > 0 ? "+" : ""}
                                {event.amount.toLocaleString()}
                              </span>
                            )}
                            <span className="text-zinc-700">
                              {formatDate(event.timestamp)}
                            </span>
                          </div>
                        )}
                      </div>
                    </div>
                  </div>
                );
              })}
            </div>
          </div>
        )}
      </div>

      {/* Scroll-to-top button */}
      {!autoScroll && (
        <button
          onClick={() => {
            scrollRef.current?.scrollTo({ top: 0, behavior: "smooth" });
            setAutoScroll(true);
          }}
          className="fixed bottom-4 right-4 sm:absolute sm:bottom-6 sm:right-6 rounded-full bg-blue-500/20 border border-blue-500/30 px-3 py-1.5 text-xs font-medium text-blue-400 transition-colors hover:bg-blue-500/30"
        >
          ↓ 回到最新
        </button>
      )}
    </div>
  );
}
