"use client";

import { useState, useMemo } from "react";
import { getAgents } from "@/lib/data";
import { AgentCard } from "@/components/AgentCard";
import { AgentDetail } from "@/components/AgentDetail";
import type { DemoAgent } from "@/types/demo";

type FilterKey = "all" | "alive" | "dead";

const ORG_OPTIONS = ["all", "星辰商会", "铁匠公会", "探索者联盟", "学院派", "none"] as const;
type OrgFilter = (typeof ORG_OPTIONS)[number];

const ORG_LABELS: Record<string, string> = {
  all: "全部",
  none: "无组织",
};

export default function AgentsPage() {
  const agents = getAgents();
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [statusFilter, setStatusFilter] = useState<FilterKey>("all");
  const [orgFilter, setOrgFilter] = useState<OrgFilter>("all");
  const [search, setSearch] = useState("");

  const filtered = useMemo(() => {
    let result = agents;

    if (statusFilter === "alive") result = result.filter((a) => a.alive);
    else if (statusFilter === "dead") result = result.filter((a) => !a.alive);

    if (orgFilter !== "all") {
      if (orgFilter === "none") {
        result = result.filter((a) => !a.organization);
      } else {
        result = result.filter((a) => a.organization === orgFilter);
      }
    }

    if (search.trim()) {
      const q = search.toLowerCase();
      result = result.filter(
        (a) =>
          a.name.toLowerCase().includes(q) ||
          a.traits.some((t) => t.includes(q))
      );
    }

    return result;
  }, [agents, statusFilter, orgFilter, search]);

  const selectedAgent: DemoAgent | undefined = selectedId
    ? agents.find((a) => a.id === selectedId)
    : undefined;

  return (
    <div className="mx-auto max-w-7xl px-4 py-8">
      <h1 className="text-2xl font-bold text-white md:text-3xl">Agent 画廊</h1>
      <p className="mt-2 text-zinc-400">
        {agents.length} 个 Agent 在虚拟世界中生存、合作、竞争。
      </p>

      {/* Filters */}
      <div className="mt-6 flex flex-col gap-3 sm:flex-row sm:items-center">
        <input
          type="search"
          placeholder="搜索名字或特质..."
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          className="h-10 w-full rounded-lg border border-zinc-800 bg-zinc-900 px-3 text-sm text-white placeholder-zinc-500 focus:border-blue-500 focus:outline-none sm:w-64"
        />

        <div className="flex gap-2">
          {(["all", "alive", "dead"] as const).map((f) => (
            <button
              key={f}
              onClick={() => setStatusFilter(f)}
              className={`rounded-lg px-3 py-2 text-xs font-medium transition-colors ${
                statusFilter === f
                  ? "bg-blue-600 text-white"
                  : "bg-zinc-800 text-zinc-400 hover:bg-zinc-700"
              }`}
            >
              {f === "all" ? "全部" : f === "alive" ? "存活" : "已消亡"}
            </button>
          ))}
        </div>

        <select
          value={orgFilter}
          onChange={(e) => setOrgFilter(e.target.value as OrgFilter)}
          className="h-10 rounded-lg border border-zinc-800 bg-zinc-900 px-3 text-sm text-zinc-300 focus:border-blue-500 focus:outline-none"
        >
          {ORG_OPTIONS.map((org) => (
            <option key={org} value={org}>
              {ORG_LABELS[org] ?? org}
            </option>
          ))}
        </select>
      </div>

      <div className="mt-2 text-xs text-zinc-500">
        显示 {filtered.length} / {agents.length} 个 Agent
      </div>

      {/* Grid + Detail */}
      <div className="mt-6 flex flex-col gap-6 lg:flex-row">
        {/* Grid */}
        <div className="grid flex-1 gap-4 sm:grid-cols-2 lg:grid-cols-3">
          {filtered.map((agent) => (
            <AgentCard
              key={agent.id}
              agent={agent}
              isSelected={selectedId === agent.id}
              onClick={() =>
                setSelectedId(selectedId === agent.id ? null : agent.id)
              }
            />
          ))}
          {filtered.length === 0 && (
            <div className="col-span-full py-12 text-center text-sm text-zinc-500">
              没有匹配的 Agent
            </div>
          )}
        </div>

        {/* Detail panel */}
        {selectedAgent && (
          <div className="w-full lg:w-96">
            <AgentDetail
              agent={selectedAgent}
              onClose={() => setSelectedId(null)}
            />
          </div>
        )}
      </div>
    </div>
  );
}
