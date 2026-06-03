"use client";

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import Link from "next/link";
import type { EventType } from "@/types/world";
import { useSSEContext } from "@/components/SSEProvider";

// Event type config with icons, colors, and labels
const eventTypeConfig: Record<
  EventType,
  { label: string; color: string; bg: string; border: string; dot: string; icon: string }
> = {
  tick_advanced: { label: "Tick 推进", color: "text-zinc-400", bg: "bg-zinc-500/10", border: "border-zinc-500/20", dot: "bg-zinc-400", icon: "⏱" },
  agent_spawned: { label: "诞生", color: "text-green-400", bg: "bg-green-500/10", border: "border-green-500/20", dot: "bg-green-400", icon: "👶" },
  agent_dying: { label: "濒死", color: "text-orange-400", bg: "bg-orange-500/10", border: "border-orange-500/20", dot: "bg-orange-400", icon: "⚠" },
  agent_died: { label: "死亡", color: "text-red-400", bg: "bg-red-500/10", border: "border-red-500/20", dot: "bg-red-400", icon: "💀" },
  agent_rescued: { label: "营救", color: "text-emerald-400", bg: "bg-emerald-500/10", border: "border-emerald-500/20", dot: "bg-emerald-400", icon: "🆘" },
  transaction_completed: { label: "交易", color: "text-blue-400", bg: "bg-blue-500/10", border: "border-blue-500/20", dot: "bg-blue-400", icon: "💰" },
  balance_changed: { label: "余额变更", color: "text-yellow-400", bg: "bg-yellow-500/10", border: "border-yellow-500/20", dot: "bg-yellow-400", icon: "💳" },
  phase_changed: { label: "阶段变更", color: "text-blue-400", bg: "bg-blue-500/10", border: "border-blue-500/20", dot: "bg-blue-400", icon: "🔄" },
  rule_violated: { label: "违规", color: "text-red-400", bg: "bg-red-500/10", border: "border-red-500/20", dot: "bg-red-400", icon: "🚫" },
  snapshot_taken: { label: "快照", color: "text-zinc-400", bg: "bg-zinc-500/10", border: "border-zinc-500/20", dot: "bg-zinc-400", icon: "📸" },
  escrow_created: { label: "托管创建", color: "text-cyan-400", bg: "bg-cyan-500/10", border: "border-cyan-500/20", dot: "bg-cyan-400", icon: "🔒" },
  escrow_claimed: { label: "托管认领", color: "text-blue-400", bg: "bg-blue-500/10", border: "border-blue-500/20", dot: "bg-blue-400", icon: "✋" },
  escrow_released: { label: "托管释放", color: "text-green-400", bg: "bg-green-500/10", border: "border-green-500/20", dot: "bg-green-400", icon: "🔓" },
  escrow_refunded: { label: "托管退款", color: "text-amber-400", bg: "bg-amber-500/10", border: "border-amber-500/20", dot: "bg-amber-400", icon: "↩" },
  escrow_frozen: { label: "托管冻结", color: "text-blue-400", bg: "bg-blue-500/10", border: "border-blue-500/20", dot: "bg-blue-400", icon: "❄" },
  task_created: { label: "新任务", color: "text-purple-400", bg: "bg-purple-500/10", border: "border-purple-500/20", dot: "bg-purple-400", icon: "📋" },
  task_claimed: { label: "认领", color: "text-indigo-400", bg: "bg-indigo-500/10", border: "border-indigo-500/20", dot: "bg-indigo-400", icon: "✋" },
  task_started: { label: "开始", color: "text-blue-400", bg: "bg-blue-500/10", border: "border-blue-500/20", dot: "bg-blue-400", icon: "▶" },
  task_submitted: { label: "提交", color: "text-cyan-400", bg: "bg-cyan-500/10", border: "border-cyan-500/20", dot: "bg-cyan-400", icon: "📤" },
  task_reviewed: { label: "审核", color: "text-yellow-400", bg: "bg-yellow-500/10", border: "border-yellow-500/20", dot: "bg-yellow-400", icon: "🔍" },
  task_completed: { label: "完成", color: "text-emerald-400", bg: "bg-emerald-500/10", border: "border-emerald-500/20", dot: "bg-emerald-400", icon: "✅" },
  task_expired: { label: "过期", color: "text-zinc-400", bg: "bg-zinc-500/10", border: "border-zinc-500/20", dot: "bg-zinc-400", icon: "⏰" },
  reward_distributed: { label: "奖励发放", color: "text-amber-400", bg: "bg-amber-500/10", border: "border-amber-500/20", dot: "bg-amber-400", icon: "🎁" },
  reputation_changed: { label: "信誉更新", color: "text-orange-400", bg: "bg-orange-500/10", border: "border-orange-500/20", dot: "bg-orange-400", icon: "⭐" },
  skill_level_up: { label: "技能提升", color: "text-yellow-400", bg: "bg-yellow-500/10", border: "border-yellow-500/20", dot: "bg-yellow-400", icon: "⬆" },
  skill_mutated: { label: "技能变异", color: "text-purple-400", bg: "bg-purple-500/10", border: "border-purple-500/20", dot: "bg-purple-400", icon: "🧬" },
  tax_collected: { label: "税收征收", color: "text-orange-400", bg: "bg-orange-500/10", border: "border-orange-500/20", dot: "bg-orange-400", icon: "🏛" },
  treasury_distributed: { label: "国库分配", color: "text-emerald-400", bg: "bg-emerald-500/10", border: "border-emerald-500/20", dot: "bg-emerald-400", icon: "💰" },
  leadership_election_started: { label: "选举开始", color: "text-blue-400", bg: "bg-blue-500/10", border: "border-blue-500/20", dot: "bg-blue-400", icon: "🗳" },
  leadership_changed: { label: "领导更替", color: "text-indigo-400", bg: "bg-indigo-500/10", border: "border-indigo-500/20", dot: "bg-indigo-400", icon: "👑" },
  treaty_proposed: { label: "条约提议", color: "text-cyan-400", bg: "bg-cyan-500/10", border: "border-cyan-500/20", dot: "bg-cyan-400", icon: "📝" },
  treaty_signed: { label: "条约签署", color: "text-green-400", bg: "bg-green-500/10", border: "border-green-500/20", dot: "bg-green-400", icon: "🤝" },
  treaty_broken: { label: "条约撕毁", color: "text-red-400", bg: "bg-red-500/10", border: "border-red-500/20", dot: "bg-red-400", icon: "💔" },
  relation_changed: { label: "关系变化", color: "text-purple-400", bg: "bg-purple-500/10", border: "border-purple-500/20", dot: "bg-purple-400", icon: "🔄" },
  coordination_task_created: { label: "团队任务创建", color: "text-violet-400", bg: "bg-violet-500/10", border: "border-violet-500/20", dot: "bg-violet-400", icon: "🎯" },
  coordination_task_agent_joined: { label: "加入团队任务", color: "text-blue-400", bg: "bg-blue-500/10", border: "border-blue-500/20", dot: "bg-blue-400", icon: "👥" },
  coordination_task_agent_submitted: { label: "提交团队任务", color: "text-teal-400", bg: "bg-teal-500/10", border: "border-teal-500/20", dot: "bg-teal-400", icon: "📤" },
  coordination_task_completed: { label: "团队任务完成", color: "text-emerald-400", bg: "bg-emerald-500/10", border: "border-emerald-500/20", dot: "bg-emerald-400", icon: "✅" },
  coordination_task_cancelled: { label: "团队任务取消", color: "text-red-400", bg: "bg-red-500/10", border: "border-red-500/20", dot: "bg-red-400", icon: "🚫" },
  coordination_task_expired: { label: "团队任务过期", color: "text-zinc-400", bg: "bg-zinc-500/10", border: "border-zinc-500/20", dot: "bg-zinc-400", icon: "⏰" },
  soft_rule_proposed: { label: "法案提议", color: "text-sky-400", bg: "bg-sky-500/10", border: "border-sky-500/20", dot: "bg-sky-400", icon: "📜" },
  soft_rule_activated: { label: "法案生效", color: "text-green-400", bg: "bg-green-500/10", border: "border-green-500/20", dot: "bg-green-400", icon: "✅" },
  soft_rule_expired: { label: "法案过期", color: "text-zinc-400", bg: "bg-zinc-500/10", border: "border-zinc-500/20", dot: "bg-zinc-400", icon: "⏰" },
  soft_rule_repealed: { label: "法案废除", color: "text-red-400", bg: "bg-red-500/10", border: "border-red-500/20", dot: "bg-red-400", icon: "❌" },
  investment_product_created: { label: "投资产品", color: "text-teal-400", bg: "bg-teal-500/10", border: "border-teal-500/20", dot: "bg-teal-400", icon: "📊" },
  investment_purchased: { label: "投资", color: "text-teal-400", bg: "bg-teal-500/10", border: "border-teal-500/20", dot: "bg-teal-400", icon: "🏦" },
  investment_sold: { label: "卖出", color: "text-amber-400", bg: "bg-amber-500/10", border: "border-amber-500/20", dot: "bg-amber-400", icon: "💸" },
  investment_dividend: { label: "分红", color: "text-green-400", bg: "bg-green-500/10", border: "border-green-500/20", dot: "bg-green-400", icon: "💰" },
  feed_post_created: { label: "动态发布", color: "text-blue-400", bg: "bg-blue-500/10", border: "border-blue-500/20", dot: "bg-blue-400", icon: "📝" },
  feed_post_liked: { label: "点赞", color: "text-pink-400", bg: "bg-pink-500/10", border: "border-pink-500/20", dot: "bg-pink-400", icon: "❤" },
  feed_comment_created: { label: "评论", color: "text-indigo-400", bg: "bg-indigo-500/10", border: "border-indigo-500/20", dot: "bg-indigo-400", icon: "💬" },
  feed_comment_liked: { label: "评论点赞", color: "text-pink-400", bg: "bg-pink-500/10", border: "border-pink-500/20", dot: "bg-pink-400", icon: "❤" },
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
  // SSE events via shared context
  const sse = useSSEContext();
  const events = sse.events;
  const connected = sse.connected;
  const error = sse.error;

  const [filter, setFilter] = useState<EventType | "all">("all");
  const [search, setSearch] = useState("");
  const scrollRef = useRef<HTMLDivElement>(null);
  const [autoScroll, setAutoScroll] = useState(true);

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
            <h1 className="text-xl md:text-2xl font-bold text-zinc-100">事件时间线</h1>
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
