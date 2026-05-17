"use client";

import { useEffect, useState, useMemo } from "react";
import Link from "next/link";
import { useParams, useRouter } from "next/navigation";
import type { Agent, WorldEvent } from "@/types/world";
import { fetchJSON } from "@/lib/api";

const phaseLabels: Record<string, string> = {
  newborn: "新生",
  child: "幼年",
  adult: "成年",
  elder: "老年",
};

function formatMoney(v: number): string {
  if (v >= 1_000_000) return `$${(v / 1_000_000).toFixed(1)}M`;
  if (v >= 1_000) return `$${(v / 1_000).toFixed(1)}K`;
  return `$${v.toFixed(0)}`;
}

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

// Mini bar chart component for balance visualization
function BalanceBar({ value, max, color }: { value: number; max: number; color: string }) {
  const pct = max > 0 ? Math.min((value / max) * 100, 100) : 0;
  return (
    <div className="h-2 w-full rounded-full bg-zinc-800">
      <div
        className={`h-2 rounded-full transition-all duration-500 ${color}`}
        style={{ width: `${pct}%` }}
      />
    </div>
  );
}

// Skill progress bar component
function SkillBar({ name, level }: { name: string; level: number }) {
  const maxLevel = 10;
  const pct = Math.min((level / maxLevel) * 100, 100);
  const color =
    level >= 8 ? "bg-purple-400" : level >= 5 ? "bg-blue-400" : level >= 3 ? "bg-green-400" : "bg-zinc-500";

  return (
    <div className="space-y-1">
      <div className="flex items-center justify-between">
        <span className="text-sm text-zinc-300">{name}</span>
        <span className="text-xs font-medium tabular-nums text-zinc-500">
          Lv.{level}
        </span>
      </div>
      <div className="h-1.5 w-full rounded-full bg-zinc-800">
        <div
          className={`h-1.5 rounded-full transition-all duration-500 ${color}`}
          style={{ width: `${pct}%` }}
        />
      </div>
    </div>
  );
}

// Event type labels and colors
const eventTypeLabels: Record<string, { label: string; color: string; dot: string }> = {
  agent_spawn: { label: "诞生", color: "text-green-400", dot: "bg-green-400" },
  agent_death: { label: "死亡", color: "text-red-400", dot: "bg-red-400" },
  trade: { label: "交易", color: "text-amber-400", dot: "bg-amber-400" },
  task_created: { label: "发布任务", color: "text-blue-400", dot: "bg-blue-400" },
  task_claimed: { label: "认领任务", color: "text-cyan-400", dot: "bg-cyan-400" },
  task_completed: { label: "完成任务", color: "text-emerald-400", dot: "bg-emerald-400" },
  message: { label: "消息", color: "text-violet-400", dot: "bg-violet-400" },
  skill_up: { label: "技能提升", color: "text-purple-400", dot: "bg-purple-400" },
  reputation_change: { label: "信誉变化", color: "text-yellow-400", dot: "bg-yellow-400" },
  investment: { label: "投资", color: "text-teal-400", dot: "bg-teal-400" },
  tax: { label: "税收", color: "text-orange-400", dot: "bg-orange-400" },
};

export default function AgentDetailPage() {
  const params = useParams();
  const router = useRouter();
  const agentId = params.id as string;

  const [agent, setAgent] = useState<Agent | null>(null);
  const [events, setEvents] = useState<WorldEvent[]>([]);
  const [allAgents, setAllAgents] = useState<Agent[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;

    async function load() {
      try {
        const [agentData, eventsData, agentsData] = await Promise.all([
          fetchJSON<Agent>(`/api/v1/agents/${agentId}`),
          fetchJSON<WorldEvent[]>("/api/v1/world/events").catch(() => []),
          fetchJSON<Agent[]>("/api/v1/agents").catch(() => []),
        ]);
        if (!cancelled) {
          setAgent(agentData);
          setEvents(eventsData);
          setAllAgents(agentsData);
          setError(null);
        }
      } catch {
        if (!cancelled) {
          setError("无法加载 Agent 数据");
        }
      } finally {
        if (!cancelled) setLoading(false);
      }
    }

    load();
    const interval = setInterval(load, 5000);
    return () => {
      cancelled = true;
      clearInterval(interval);
    };
  }, [agentId]);

  // Filter events for this agent
  const agentEvents = useMemo(
    () =>
      events
        .filter((e) => e.agentId === agentId || e.targetId === agentId)
        .sort((a, b) => new Date(b.timestamp).getTime() - new Date(a.timestamp).getTime())
        .slice(0, 50),
    [events, agentId]
  );

  // Related agents from events
  const relatedAgentIds = useMemo(() => {
    const ids = new Set<string>();
    for (const e of agentEvents) {
      if (e.agentId && e.agentId !== agentId) ids.add(e.agentId);
      if (e.targetId && e.targetId !== agentId) ids.add(e.targetId);
    }
    return ids;
  }, [agentEvents, agentId]);

  const relatedAgents = useMemo(
    () => allAgents.filter((a) => relatedAgentIds.has(a.id)),
    [allAgents, relatedAgentIds]
  );

  // Compute max values for bar charts
  const maxMoney = useMemo(
    () => Math.max(agent?.money ?? 0, ...allAgents.map((a) => a.money), 1),
    [agent, allAgents]
  );
  const maxTokens = useMemo(
    () => Math.max(agent?.tokens ?? 0, ...allAgents.map((a) => a.tokens), 1),
    [agent, allAgents]
  );

  // Sorted skills
  const sortedSkills = useMemo(() => {
    if (!agent) return [];
    return Object.entries(agent.skills).sort(([, a], [, b]) => b - a);
  }, [agent]);

  if (loading) {
    return (
      <div className="flex h-full items-center justify-center">
        <div className="text-sm text-zinc-600">正在加载 Agent 数据...</div>
      </div>
    );
  }

  if (error || !agent) {
    return (
      <div className="p-4 md:p-6 space-y-4">
        <button
          onClick={() => router.back()}
          className="text-sm text-zinc-400 hover:text-zinc-200 transition-colors"
        >
          &larr; 返回
        </button>
        <div className="rounded-lg bg-red-500/10 border border-red-500/20 px-4 py-3 text-sm text-red-400">
          {error ?? "Agent 不存在"}
        </div>
      </div>
    );
  }

  return (
    <div className="p-4 md:p-6 space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2 sm:gap-4">
          <button
            onClick={() => router.push("/agents")}
            className="rounded-lg p-1.5 text-zinc-400 hover:bg-zinc-800 hover:text-zinc-200 transition-colors"
          >
            <svg className="h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M15 19l-7-7 7-7" />
            </svg>
          </button>
          <div>
            <div className="flex flex-wrap items-center gap-2 sm:gap-3">
              <h1 className="text-xl sm:text-2xl font-bold text-zinc-100">{agent.name}</h1>
              <span
                className={`inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-[10px] font-medium ${
                  agent.alive
                    ? "bg-green-500/10 text-green-400"
                    : "bg-red-500/10 text-red-400"
                }`}
              >
                <span
                  className={`inline-block h-1.5 w-1.5 rounded-full ${
                    agent.alive ? "bg-green-400" : "bg-red-400"
                  }`}
                />
                {agent.alive ? "存活" : "死亡"}
              </span>
              <span className="rounded-full bg-zinc-800 px-2 py-0.5 text-[10px] font-medium text-zinc-400">
                {phaseLabels[agent.phase] ?? agent.phase}
              </span>
            </div>
            <p className="text-sm text-zinc-500">
              创建于 {formatDate(agent.createdAt)} · {agent.age} Tick · 信誉 {agent.reputation.toFixed(1)}
            </p>
          </div>
        </div>
      </div>

      {/* Economic Dashboard: Token / Money */}
      <div className="grid grid-cols-1 gap-4 md:grid-cols-2 xl:grid-cols-4">
        <div className="rounded-xl border border-blue-500/20 bg-blue-500/5 p-4 space-y-2">
          <div className="flex items-center justify-between">
            <span className="text-sm font-medium text-zinc-400">Token 余额</span>
            <span className="text-xs text-blue-400">
              <svg className="inline h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M13 10V3L4 14h7v7l9-11h-7z" />
              </svg>
            </span>
          </div>
          <p className="text-2xl font-bold tabular-nums text-zinc-100">{agent.tokens.toLocaleString()}</p>
          <BalanceBar value={agent.tokens} max={maxTokens} color="bg-blue-400" />
        </div>

        <div className="rounded-xl border border-green-500/20 bg-green-500/5 p-4 space-y-2">
          <div className="flex items-center justify-between">
            <span className="text-sm font-medium text-zinc-400">Money 余额</span>
            <span className="text-xs text-green-400">$</span>
          </div>
          <p className="text-2xl font-bold tabular-nums text-zinc-100">{formatMoney(agent.money)}</p>
          <BalanceBar value={agent.money} max={maxMoney} color="bg-green-400" />
        </div>

        <div className="rounded-xl border border-amber-500/20 bg-amber-500/5 p-4 space-y-2">
          <div className="flex items-center justify-between">
            <span className="text-sm font-medium text-zinc-400">信誉值</span>
            <span className="text-xs text-amber-400">&#9733;</span>
          </div>
          <p className="text-2xl font-bold tabular-nums text-zinc-100">{agent.reputation.toFixed(1)}</p>
          <BalanceBar value={agent.reputation} max={100} color="bg-amber-400" />
        </div>

        <div className="rounded-xl border border-purple-500/20 bg-purple-500/5 p-4 space-y-2">
          <div className="flex items-center justify-between">
            <span className="text-sm font-medium text-zinc-400">技能数量</span>
            <span className="text-xs text-purple-400">
              <svg className="inline h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9.663 17h4.673M12 3v1m6.364 1.636l-.707.707M21 12h-1M4 12H3m3.343-5.657l-.707-.707m2.828 9.9a5 5 0 117.072 0l-.548.547A3.374 3.374 0 0014 18.469V19a2 2 0 11-4 0v-.531c0-.895-.356-1.754-.988-2.386l-.548-.547z" />
              </svg>
            </span>
          </div>
          <p className="text-2xl font-bold tabular-nums text-zinc-100">{sortedSkills.length}</p>
          <div className="h-2 w-full rounded-full bg-zinc-800">
            <div
              className="h-2 rounded-full bg-purple-400 transition-all duration-500"
              style={{ width: `${Math.min((sortedSkills.length / 10) * 100, 100)}%` }}
            />
          </div>
        </div>
      </div>

      {/* Main content grid: Skills + Relations | Activity + Actions */}
      <div className="grid grid-cols-1 gap-6 xl:grid-cols-2">
        {/* Left column */}
        <div className="space-y-6">
          {/* Skill Tree */}
          <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4 space-y-4">
            <h2 className="text-sm font-semibold text-zinc-200">技能树</h2>
            {sortedSkills.length === 0 ? (
              <p className="text-sm text-zinc-600">暂无技能数据</p>
            ) : (
              <div className="space-y-3">
                {sortedSkills.map(([name, level]) => (
                  <SkillBar key={name} name={name} level={level} />
                ))}
              </div>
            )}
          </div>

          {/* Relationship Graph */}
          <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4 space-y-4">
            <h2 className="text-sm font-semibold text-zinc-200">关系图</h2>
            {relatedAgents.length === 0 ? (
              <p className="text-sm text-zinc-600">暂无关系数据</p>
            ) : (
              <div className="space-y-2">
                {relatedAgents.map((related) => {
                  const interactionCount = agentEvents.filter(
                    (e) =>
                      (e.agentId === related.id && e.targetId === agentId) ||
                      (e.agentId === agentId && e.targetId === related.id)
                  ).length;
                  return (
                    <Link
                      key={related.id}
                      href={`/agents/${related.id}`}
                      className="flex items-center justify-between rounded-lg border border-zinc-800 bg-zinc-900/30 px-3 py-2 transition-colors hover:bg-zinc-800/50"
                    >
                      <div className="flex items-center gap-2">
                        <span
                          className={`inline-block h-2 w-2 rounded-full ${
                            related.alive ? "bg-green-400" : "bg-red-400"
                          }`}
                        />
                        <span className="text-sm text-zinc-200">{related.name}</span>
                        <span className="rounded-full bg-zinc-800 px-1.5 py-0.5 text-[10px] text-zinc-500">
                          {phaseLabels[related.phase] ?? related.phase}
                        </span>
                      </div>
                      <div className="flex items-center gap-3">
                        <span className="text-xs text-zinc-500">{interactionCount} 次互动</span>
                        <svg className="h-4 w-4 text-zinc-600" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 5l7 7-7 7" />
                        </svg>
                      </div>
                    </Link>
                  );
                })}
              </div>
            )}
          </div>
        </div>

        {/* Right column */}
        <div className="space-y-6">
          {/* Action Buttons */}
          <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4 space-y-4">
            <h2 className="text-sm font-semibold text-zinc-200">操作</h2>
            <div className="grid grid-cols-1 gap-2 sm:grid-cols-3">
              <button className="rounded-lg bg-blue-500/10 border border-blue-500/20 px-4 py-2.5 text-sm font-medium text-blue-400 transition-colors hover:bg-blue-500/20">
                投资
              </button>
              <button className="rounded-lg bg-purple-500/10 border border-purple-500/20 px-4 py-2.5 text-sm font-medium text-purple-400 transition-colors hover:bg-purple-500/20">
                发布任务
              </button>
              <button className="rounded-lg bg-emerald-500/10 border border-emerald-500/20 px-4 py-2.5 text-sm font-medium text-emerald-400 transition-colors hover:bg-emerald-500/20">
                发消息
              </button>
            </div>
          </div>

          {/* Activity History */}
          <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4 space-y-4">
            <h2 className="text-sm font-semibold text-zinc-200">活动历史</h2>
            {agentEvents.length === 0 ? (
              <p className="text-sm text-zinc-600">暂无活动记录</p>
            ) : (
              <div className="max-h-[480px] overflow-y-auto scrollbar-thin space-y-0">
                {agentEvents.map((event, idx) => {
                  const meta = eventTypeLabels[event.type] ?? {
                    label: event.type,
                    color: "text-zinc-400",
                    dot: "bg-zinc-400",
                  };
                  return (
                    <div key={event.id} className="relative flex gap-3 pb-4">
                      {/* Timeline line */}
                      {idx < agentEvents.length - 1 && (
                        <div className="absolute left-[5px] top-3 h-full w-px bg-zinc-800" />
                      )}
                      {/* Dot */}
                      <div className={`relative mt-1.5 h-2.5 w-2.5 shrink-0 rounded-full ${meta.dot}`} />
                      {/* Content */}
                      <div className="min-w-0 flex-1 space-y-0.5">
                        <div className="flex items-center gap-2">
                          <span className={`text-xs font-medium ${meta.color}`}>{meta.label}</span>
                          <span className="text-[10px] text-zinc-600">
                            Tick #{event.tick} · {formatDate(event.timestamp)}
                          </span>
                        </div>
                        <p className="text-sm text-zinc-400 leading-relaxed">
                          {event.description}
                          {event.amount != null && (
                            <span className="ml-1 tabular-nums font-medium text-amber-400">
                              ({event.amount > 0 ? "+" : ""}
                              {event.amount.toLocaleString()})
                            </span>
                          )}
                        </p>
                      </div>
                    </div>
                  );
                })}
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
