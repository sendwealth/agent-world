"use client";

import { useMemo, useState } from "react";
import Link from "next/link";
import type { WorldEvent, EventType } from "@/types/world";

const eventTypeConfig: Record<string, { label: string; color: string; dot: string; icon: string }> = {
  agent_spawn: { label: "诞生", color: "text-green-400", dot: "bg-green-400", icon: "👶" },
  agent_death: { label: "死亡", color: "text-red-400", dot: "bg-red-400", icon: "💀" },
  trade: { label: "交易", color: "text-amber-400", dot: "bg-amber-400", icon: "💰" },
  task_created: { label: "发布任务", color: "text-blue-400", dot: "bg-blue-400", icon: "📋" },
  task_claimed: { label: "认领任务", color: "text-cyan-400", dot: "bg-cyan-400", icon: "✋" },
  task_completed: { label: "完成任务", color: "text-emerald-400", dot: "bg-emerald-400", icon: "✅" },
  message: { label: "消息", color: "text-violet-400", dot: "bg-violet-400", icon: "💬" },
  skill_up: { label: "技能提升", color: "text-purple-400", dot: "bg-purple-400", icon: "⬆" },
  reputation_change: { label: "信誉变化", color: "text-yellow-400", dot: "bg-yellow-400", icon: "⭐" },
  investment: { label: "投资", color: "text-teal-400", dot: "bg-teal-400", icon: "🏦" },
  tax: { label: "税收", color: "text-orange-400", dot: "bg-orange-400", icon: "🏛" },
  inflation: { label: "通胀", color: "text-rose-400", dot: "bg-rose-400", icon: "📈" },
};

function formatDate(iso: string): string {
  const d = new Date(iso);
  const now = new Date();
  const diff = now.getTime() - d.getTime();
  const minutes = Math.floor(diff / 60_000);
  if (minutes < 1) return "刚刚";
  if (minutes < 60) return `${minutes} 分钟前`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours} 小时前`;
  const days = Math.floor(hours / 24);
  if (days < 30) return `${days} 天前`;
  return d.toLocaleDateString("zh-CN");
}


interface ActivityTimelineProps {
  agentId: string;
  events: WorldEvent[];
}

export default function ActivityTimeline({ agentId, events }: ActivityTimelineProps) {
  const [filter, setFilter] = useState<EventType | "all">("all");
  const [search, setSearch] = useState("");
  const [expanded, setExpanded] = useState(false);

  const agentEvents = useMemo(
    () =>
      events
        .filter((e) => e.agentId === agentId || e.targetId === agentId)
        .sort((a, b) => new Date(b.timestamp).getTime() - new Date(a.timestamp).getTime()),
    [events, agentId]
  );

  // Count per type
  const typeCounts = useMemo(() => {
    const counts: Record<string, number> = {};
    for (const e of agentEvents) {
      counts[e.type] = (counts[e.type] ?? 0) + 1;
    }
    return counts;
  }, [agentEvents]);

  // Filter and search
  const filteredEvents = useMemo(() => {
    let result = agentEvents;

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

    return expanded ? result : result.slice(0, 20);
  }, [agentEvents, filter, search, expanded]);

  // Group events by date
  const groupedEvents = useMemo(() => {
    const groups: { date: string; events: WorldEvent[] }[] = [];
    let currentDate = "";

    for (const event of filteredEvents) {
      const date = new Date(event.timestamp).toLocaleDateString("zh-CN");
      if (date !== currentDate) {
        currentDate = date;
        groups.push({ date, events: [] });
      }
      groups[groups.length - 1].events.push(event);
    }

    return groups;
  }, [filteredEvents]);

  if (agentEvents.length === 0) {
    return (
      <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4">
        <h2 className="text-sm font-semibold text-zinc-200">活动时间线</h2>
        <p className="mt-2 text-sm text-zinc-600">暂无活动记录</p>
      </div>
    );
  }

  const usedTypes = Object.keys(typeCounts) as EventType[];

  return (
    <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4 space-y-4">
      <div className="flex items-center justify-between">
        <h2 className="text-sm font-semibold text-zinc-200">活动时间线</h2>
        <span className="text-xs text-zinc-500">
          {agentEvents.length} 条记录
          {filter !== "all" && ` · 筛选 ${filteredEvents.length} 条`}
        </span>
      </div>

      {/* Filter chips */}
      <div className="flex flex-wrap items-center gap-1.5">
        <button
          onClick={() => setFilter("all")}
          className={`rounded-full px-2.5 py-0.5 text-[10px] font-medium transition-colors ${
            filter === "all"
              ? "bg-blue-500/20 text-blue-400 border border-blue-500/30"
              : "bg-zinc-800/50 text-zinc-500 border border-zinc-800 hover:text-zinc-300"
          }`}
        >
          全部
        </button>
        {usedTypes.map((type) => {
          const cfg = eventTypeConfig[type];
          if (!cfg) return null;
          return (
            <button
              key={type}
              onClick={() => setFilter(type)}
              className={`flex items-center gap-1 rounded-full px-2.5 py-0.5 text-[10px] font-medium transition-colors ${
                filter === type
                  ? `${cfg.color} border border-current/30 bg-current/10`
                  : "bg-zinc-800/50 text-zinc-500 border border-zinc-800 hover:text-zinc-300"
              }`}
            >
              <span className="text-[10px]">{cfg.icon}</span>
              {cfg.label}
              <span className="tabular-nums opacity-60">{typeCounts[type]}</span>
            </button>
          );
        })}
      </div>

      {/* Search */}
      <input
        type="text"
        value={search}
        onChange={(e) => setSearch(e.target.value)}
        placeholder="搜索活动..."
        className="w-full rounded-lg border border-zinc-800 bg-zinc-900/50 px-3 py-1.5 text-xs text-zinc-200 placeholder-zinc-600 outline-none transition-colors focus:border-blue-500/40"
      />

      {/* Timeline */}
      <div className="max-h-[500px] overflow-y-auto scrollbar-thin space-y-4">
        {groupedEvents.map((group) => (
          <div key={group.date}>
            {/* Date separator */}
            <div className="sticky top-0 z-10 flex items-center gap-2 bg-zinc-900/80 py-1 backdrop-blur-sm">
              <div className="h-px flex-1 bg-zinc-800" />
              <span className="text-[10px] font-medium text-zinc-500">{group.date}</span>
              <div className="h-px flex-1 bg-zinc-800" />
            </div>

            <div className="space-y-0 mt-1">
              {group.events.map((event, idx) => {
                const meta = eventTypeConfig[event.type] ?? {
                  label: event.type,
                  color: "text-zinc-400",
                  dot: "bg-zinc-400",
                  icon: "•",
                };
                const isLast = idx === group.events.length - 1;
                const isActor = event.agentId === agentId;
                const isTarget = event.targetId === agentId;

                return (
                  <div key={event.id} className="relative flex gap-3 pb-3">
                    {/* Timeline line */}
                    {!isLast && (
                      <div className="absolute left-[5px] top-3 h-full w-px bg-zinc-800" />
                    )}
                    {/* Dot */}
                    <div
                      className={`relative mt-1.5 h-2.5 w-2.5 shrink-0 rounded-full ${meta.dot}`}
                    />
                    {/* Content */}
                    <div className="min-w-0 flex-1 space-y-0.5">
                      <div className="flex items-center gap-2">
                        <span className={`text-[10px] font-medium ${meta.color}`}>
                          {meta.label}
                        </span>
                        <span className="text-[10px] text-zinc-600">
                          Tick #{event.tick}
                        </span>
                        <span className="text-[10px] text-zinc-700">
                          {formatDate(event.timestamp)}
                        </span>
                      </div>
                      <p className="text-xs text-zinc-400 leading-relaxed">
                        {event.description}
                        {event.amount != null && (
                          <span className="ml-1 tabular-nums font-medium text-amber-400">
                            ({event.amount > 0 ? "+" : ""}
                            {event.amount.toLocaleString()})
                          </span>
                        )}
                      </p>
                      {/* Related agent links */}
                      {(isActor || isTarget) && event.agentId && event.targetId && event.agentId !== event.targetId && (
                        <div className="flex items-center gap-1 text-[10px]">
                          {isActor && event.targetName ? (
                            <Link
                              href={`/agents/${event.targetId}`}
                              className="text-blue-400/70 hover:text-blue-300 transition-colors"
                            >
                              → {event.targetName}
                            </Link>
                          ) : isTarget && event.agentName ? (
                            <Link
                              href={`/agents/${event.agentId}`}
                              className="text-blue-400/70 hover:text-blue-300 transition-colors"
                            >
                              ← {event.agentName}
                            </Link>
                          ) : null}
                        </div>
                      )}
                    </div>
                  </div>
                );
              })}
            </div>
          </div>
        ))}
      </div>

      {/* Show more/less */}
      {agentEvents.length > 20 && (
        <button
          onClick={() => setExpanded(!expanded)}
          className="w-full rounded-lg border border-zinc-800 bg-zinc-900/30 py-1.5 text-xs text-zinc-500 transition-colors hover:bg-zinc-800/50 hover:text-zinc-300"
        >
          {expanded
            ? "收起"
            : `显示全部 ${agentEvents.length} 条记录`}
        </button>
      )}
    </div>
  );
}
