"use client";

import { useEffect, useState, useMemo } from "react";
import Link from "next/link";
import type { Agent } from "@/types/world";
import { fetchJSON } from "@/lib/api";

type StatusFilter = "all" | "alive" | "dead";

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

function formatSkills(skills: Record<string, number>): string {
  const entries = Object.entries(skills);
  if (entries.length === 0) return "-";
  return entries
    .sort((a, b) => b[1] - a[1])
    .slice(0, 3)
    .map(([name, val]) => `${name} ${val}`)
    .join(", ");
}

function AgentCard({ agent }: { agent: Agent }) {
  return (
    <Link
      href={`/agents/${agent.id}`}
      className="block rounded-xl border border-zinc-800 bg-zinc-900/50 p-4 transition-colors hover:bg-zinc-800/50"
    >
      <div className="flex items-center justify-between mb-2">
        <span className="text-sm font-medium text-zinc-200">{agent.name}</span>
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
      </div>
      <div className="flex flex-wrap items-center gap-x-3 gap-y-1 text-xs text-zinc-400">
        <span>{phaseLabels[agent.phase] ?? agent.phase}</span>
        <span>{agent.tokens.toLocaleString()} Token</span>
        <span>{formatMoney(agent.money)}</span>
        <span>信誉 {agent.reputation.toFixed(1)}</span>
        <span>{agent.age} Tick</span>
      </div>
      <p className="mt-1.5 text-xs text-zinc-500 truncate">
        {formatSkills(agent.skills)}
      </p>
    </Link>
  );
}

export default function AgentsPage() {
  const [agents, setAgents] = useState<Agent[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [statusFilter, setStatusFilter] = useState<StatusFilter>("all");
  const [search, setSearch] = useState("");

  useEffect(() => {
    let cancelled = false;

    async function load() {
      try {
        const data = await fetchJSON<Agent[]>("/api/v1/agents");
        if (!cancelled) {
          setAgents(data);
          setError(null);
        }
      } catch {
        if (!cancelled) {
          setError("无法连接到世界引擎");
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
  }, []);

  const filtered = useMemo(() => {
    let result = agents;

    if (statusFilter === "alive") {
      result = result.filter((a) => a.alive);
    } else if (statusFilter === "dead") {
      result = result.filter((a) => !a.alive);
    }

    if (search.trim()) {
      const q = search.trim().toLowerCase();
      result = result.filter((a) => a.name.toLowerCase().includes(q));
    }

    return result;
  }, [agents, statusFilter, search]);

  const aliveCount = agents.filter((a) => a.alive).length;
  const deadCount = agents.length - aliveCount;

  const filterButtons: { value: StatusFilter; label: string; count: number }[] = [
    { value: "all", label: "全部", count: agents.length },
    { value: "alive", label: "存活", count: aliveCount },
    { value: "dead", label: "死亡", count: deadCount },
  ];

  return (
    <div className="p-4 md:p-6 space-y-6">
      {/* Header */}
      <div className="flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between">
        <div>
          <h1 className="text-2xl font-bold text-zinc-100">Agent 列表</h1>
          <p className="text-sm text-zinc-500">
            {loading
              ? "正在加载..."
              : `共 ${agents.length} 个 Agent · ${aliveCount} 存活 · ${deadCount} 死亡`}
          </p>
        </div>
        {error && (
          <div className="rounded-lg bg-red-500/10 border border-red-500/20 px-3 py-1.5 text-xs text-red-400">
            {error}
          </div>
        )}
      </div>

      {/* Filters & Search */}
      <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
        <div className="flex flex-wrap items-center gap-1.5">
          {filterButtons.map((btn) => (
            <button
              key={btn.value}
              onClick={() => setStatusFilter(btn.value)}
              className={`rounded-lg px-3 py-1.5 text-xs font-medium transition-colors ${
                statusFilter === btn.value
                  ? "bg-blue-500/15 text-blue-400 border border-blue-500/30"
                  : "bg-zinc-800/50 text-zinc-400 border border-zinc-800 hover:bg-zinc-800 hover:text-zinc-300"
              }`}
            >
              {btn.label} ({btn.count})
            </button>
          ))}
        </div>
        <div className="relative">
          <input
            type="text"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            placeholder="搜索 Agent 名称..."
            className="w-full rounded-lg border border-zinc-800 bg-zinc-900/50 px-3 py-2 pl-8 text-sm text-zinc-200 placeholder-zinc-600 outline-none transition-colors focus:border-zinc-700 focus:ring-1 focus:ring-zinc-700 sm:w-64"
          />
          <svg
            className="absolute left-2.5 top-2.5 h-3.5 w-3.5 text-zinc-600"
            fill="none"
            stroke="currentColor"
            viewBox="0 0 24 24"
          >
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              strokeWidth={2}
              d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z"
            />
          </svg>
        </div>
      </div>

      {loading ? (
        <div className="flex h-48 items-center justify-center text-sm text-zinc-600">
          正在加载 Agent 数据...
        </div>
      ) : filtered.length === 0 ? (
        <div className="flex h-48 items-center justify-center text-sm text-zinc-600">
          {agents.length === 0 ? "暂无 Agent 数据" : "没有匹配的 Agent"}
        </div>
      ) : (
        <>
          {/* Mobile card layout */}
          <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:hidden">
            {filtered.map((agent) => (
              <AgentCard key={agent.id} agent={agent} />
            ))}
          </div>

          {/* Desktop table layout */}
          <div className="hidden lg:block rounded-xl border border-zinc-800 bg-zinc-900/50">
            <div className="overflow-x-auto">
              <table className="w-full">
                <thead>
                  <tr className="border-b border-zinc-800">
                    <th className="px-4 py-3 text-left text-xs font-semibold text-zinc-400">
                      名称
                    </th>
                    <th className="px-4 py-3 text-left text-xs font-semibold text-zinc-400">
                      状态
                    </th>
                    <th className="px-4 py-3 text-left text-xs font-semibold text-zinc-400">
                      阶段
                    </th>
                    <th className="px-4 py-3 text-right text-xs font-semibold text-zinc-400">
                      Token
                    </th>
                    <th className="px-4 py-3 text-right text-xs font-semibold text-zinc-400">
                      Money
                    </th>
                    <th className="px-4 py-3 text-right text-xs font-semibold text-zinc-400">
                      信誉
                    </th>
                    <th className="px-4 py-3 text-left text-xs font-semibold text-zinc-400">
                      技能
                    </th>
                    <th className="px-4 py-3 text-right text-xs font-semibold text-zinc-400">
                      年龄
                    </th>
                  </tr>
                </thead>
                <tbody>
                  {filtered.map((agent) => (
                    <tr key={agent.id}>
                      <td className="px-4 py-3">
                        <Link
                          href={`/agents/${agent.id}`}
                          className="text-sm font-medium text-zinc-200 hover:text-blue-400 transition-colors"
                        >
                          {agent.name}
                        </Link>
                      </td>
                      <td className="px-4 py-3">
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
                      </td>
                      <td className="px-4 py-3 text-sm text-zinc-300">
                        {phaseLabels[agent.phase] ?? agent.phase}
                      </td>
                      <td className="px-4 py-3 text-right text-sm text-zinc-300 tabular-nums">
                        {agent.tokens.toLocaleString()}
                      </td>
                      <td className="px-4 py-3 text-right text-sm text-zinc-300 tabular-nums">
                        {formatMoney(agent.money)}
                      </td>
                      <td className="px-4 py-3 text-right text-sm text-zinc-300 tabular-nums">
                        {agent.reputation.toFixed(1)}
                      </td>
                      <td className="px-4 py-3 text-sm text-zinc-400 max-w-[200px] truncate">
                        {formatSkills(agent.skills)}
                      </td>
                      <td className="px-4 py-3 text-right text-sm text-zinc-500 tabular-nums">
                        {agent.age} Tick
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </div>
        </>
      )}
    </div>
  );
}
