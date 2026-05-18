"use client";

import { useEffect, useState, useMemo } from "react";
import { useParams, useRouter } from "next/navigation";
import type { Agent, WorldEvent } from "@/types/world";
import { fetchJSON } from "@/lib/api";
import { formatDate, phaseLabels } from "@/lib/format";
import SkillTree from "@/components/agent/SkillTree";
import MemoryStats from "@/components/agent/MemoryStats";
import RelationshipGraph from "@/components/agent/RelationshipGraph";
import ActivityTimeline from "@/components/agent/ActivityTimeline";

function formatMoney(v: number): string {
  if (v >= 1_000_000) return `$${(v / 1_000_000).toFixed(1)}M`;
  if (v >= 1_000) return `$${(v / 1_000).toFixed(1)}K`;
  return `$${v.toFixed(0)}`;
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

const TABS = [
  { key: "skills" as const, label: "技能树", icon: "🌳" },
  { key: "memory" as const, label: "记忆统计", icon: "🧠" },
  { key: "relations" as const, label: "关系图", icon: "🕸️" },
  { key: "activity" as const, label: "活动时间线", icon: "📜" },
];

export default function AgentDetailPage() {
  const params = useParams();
  const router = useRouter();
  const rawId = params.id;
  const agentId = Array.isArray(rawId) ? rawId[0] : rawId;

  const [agent, setAgent] = useState<Agent | null>(null);
  const [events, setEvents] = useState<WorldEvent[]>([]);
  const [allAgents, setAllAgents] = useState<Agent[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [activeTab, setActiveTab] = useState<"skills" | "memory" | "relations" | "activity">("skills");

  useEffect(() => {
    if (!agentId) return;
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
        .sort((a, b) => new Date(b.timestamp).getTime() - new Date(a.timestamp).getTime()),
    [events, agentId]
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

  if (loading) {
    return (
      <div className="flex h-full items-center justify-center">
        <div className="text-sm text-zinc-600">正在加载 Agent 数据...</div>
      </div>
    );
  }

  if (error || !agent || !agentId) {
    return (
      <div className="p-6 space-y-4">
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
        <div className="flex items-center gap-2 md:gap-4">
          <button
            onClick={() => router.push("/agents")}
            className="flex min-h-[44px] min-w-[44px] items-center justify-center rounded-lg p-1.5 text-zinc-400 hover:bg-zinc-800 hover:text-zinc-200 transition-colors"
          >
            <svg className="h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M15 19l-7-7 7-7" />
            </svg>
          </button>
          <div>
            <div className="flex flex-wrap items-center gap-2 md:gap-3">
              <h1 className="text-xl md:text-2xl font-bold text-zinc-100">{agent.name}</h1>
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

        {/* Action buttons in header */}
        <div className="flex items-center gap-2">
          <button className="rounded-lg bg-blue-500/10 border border-blue-500/20 px-3 py-1.5 text-xs font-medium text-blue-400 transition-colors hover:bg-blue-500/20">
            投资
          </button>
          <button className="rounded-lg bg-purple-500/10 border border-purple-500/20 px-3 py-1.5 text-xs font-medium text-purple-400 transition-colors hover:bg-purple-500/20">
            发布任务
          </button>
          <button className="rounded-lg bg-emerald-500/10 border border-emerald-500/20 px-3 py-1.5 text-xs font-medium text-emerald-400 transition-colors hover:bg-emerald-500/20">
            发消息
          </button>
        </div>
      </div>

      {/* Economic Dashboard: Token / Money */}
      <div className="grid grid-cols-2 gap-3 md:grid-cols-2 xl:grid-cols-4 md:gap-4">
        <div className="rounded-xl border border-blue-500/20 bg-blue-500/5 p-3 md:p-4 space-y-2">
          <div className="flex items-center justify-between">
            <span className="text-xs md:text-sm font-medium text-zinc-400">Token 余额</span>
            <span className="text-xs text-blue-400">
              <svg className="inline h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M13 10V3L4 14h7v7l9-11h-7z" />
              </svg>
            </span>
          </div>
          <p className="text-xl md:text-2xl font-bold tabular-nums text-zinc-100">{agent.tokens.toLocaleString()}</p>
          <BalanceBar value={agent.tokens} max={maxTokens} color="bg-blue-400" />
        </div>

        <div className="rounded-xl border border-green-500/20 bg-green-500/5 p-3 md:p-4 space-y-2">
          <div className="flex items-center justify-between">
            <span className="text-xs md:text-sm font-medium text-zinc-400">Money 余额</span>
            <span className="text-xs text-green-400">$</span>
          </div>
          <p className="text-xl md:text-2xl font-bold tabular-nums text-zinc-100">{formatMoney(agent.money)}</p>
          <BalanceBar value={agent.money} max={maxMoney} color="bg-green-400" />
        </div>

        <div className="rounded-xl border border-amber-500/20 bg-amber-500/5 p-3 md:p-4 space-y-2">
          <div className="flex items-center justify-between">
            <span className="text-xs md:text-sm font-medium text-zinc-400">信誉值</span>
            <span className="text-xs text-amber-400">&#9733;</span>
          </div>
          <p className="text-xl md:text-2xl font-bold tabular-nums text-zinc-100">{agent.reputation.toFixed(1)}</p>
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
          <p className="text-2xl font-bold tabular-nums text-zinc-100">
            {Object.keys(agent.skills).length}
          </p>
          <div className="h-2 w-full rounded-full bg-zinc-800">
            <div
              className="h-2 rounded-full bg-purple-400 transition-all duration-500"
              style={{ width: `${Math.min((Object.keys(agent.skills).length / 10) * 100, 100)}%` }}
            />
          </div>
        </div>
      </div>

      {/* Tab navigation */}
      <div className="flex items-center gap-1 border-b border-zinc-800">
        {TABS.map((tab) => (
          <button
            key={tab.key}
            onClick={() => setActiveTab(tab.key)}
            className={`flex items-center gap-1.5 border-b-2 px-4 py-2.5 text-sm font-medium transition-colors ${
              activeTab === tab.key
                ? "border-blue-400 text-blue-400"
                : "border-transparent text-zinc-500 hover:text-zinc-300"
            }`}
          >
            <span className="text-sm">{tab.icon}</span>
            {tab.label}
          </button>
        ))}
      </div>

      {/* Tab content */}
      <div>
        {activeTab === "skills" && <SkillTree skills={agent.skills} />}
        {activeTab === "memory" && (
          <MemoryStats agentId={agentId} events={events} />
        )}
        {activeTab === "relations" && (
          <RelationshipGraph
            agent={agent}
            allAgents={allAgents}
            agentEvents={agentEvents}
          />
        )}
        {activeTab === "activity" && (
          <ActivityTimeline
            agentId={agentId}
            events={events}
          />
        )}
      </div>
    </div>
  );
}
