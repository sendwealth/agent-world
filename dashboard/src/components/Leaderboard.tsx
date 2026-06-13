"use client";

import { useEffect, useState } from "react";
import type { Agent, Leaderboard, LeaderboardEntry, ReputationRankingEntry } from "@/types/world";
import { fetchJSON } from "@/lib/api";

interface LeaderboardProps {
  statsTick: number | undefined;
}

function LeaderboardTable({
  title,
  icon,
  entries,
  valueFormatter,
}: {
  title: string;
  icon: string;
  entries: LeaderboardEntry[];
  valueFormatter: (v: number) => string;
}) {
  return (
    <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4">
      <h3 className="mb-3 flex items-center gap-2 text-sm font-semibold text-zinc-200">
        <span>{icon}</span>
        {title}
      </h3>
      {entries.length === 0 ? (
        <div className="flex h-24 items-center justify-center text-xs text-zinc-600">
          暂无数据
        </div>
      ) : (
        <div className="space-y-1.5">
          {entries.map((entry) => (
            <div
              key={entry.agentId}
              className="flex items-center justify-between rounded-lg bg-zinc-800/40 px-3 py-2"
            >
              <div className="flex items-center gap-2.5">
                <span
                  className={`flex h-5 w-5 items-center justify-center rounded text-[10px] font-bold ${
                    entry.rank === 1
                      ? "bg-yellow-500/20 text-yellow-400"
                      : entry.rank === 2
                        ? "bg-zinc-400/20 text-zinc-300"
                        : entry.rank === 3
                          ? "bg-amber-600/20 text-amber-500"
                          : "bg-zinc-700/30 text-zinc-500"
                  }`}
                >
                  {entry.rank}
                </span>
                <span className="text-xs font-medium text-zinc-300">
                  {entry.agentName}
                </span>
              </div>
              <span className="text-xs font-semibold text-zinc-100">
                {valueFormatter(entry.value)}
              </span>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

function ReputationRankingTable({
  entries,
}: {
  entries: ReputationRankingEntry[];
}) {
  return (
    <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4">
      <h3 className="mb-3 flex items-center gap-2 text-sm font-semibold text-zinc-200">
        <span>⭐</span>
        信誉排名（来自任务市场）
      </h3>
      {entries.length === 0 ? (
        <div className="flex h-24 items-center justify-center text-xs text-zinc-600">
          暂无数据
        </div>
      ) : (
        <div className="space-y-1.5">
          {entries.map((entry) => (
            <div
              key={entry.agent_id}
              className="flex items-center justify-between rounded-lg bg-zinc-800/40 px-3 py-2"
            >
              <div className="flex items-center gap-2.5">
                <span
                  className={`flex h-5 w-5 items-center justify-center rounded text-[10px] font-bold ${
                    entry.rank === 1
                      ? "bg-yellow-500/20 text-yellow-400"
                      : entry.rank === 2
                        ? "bg-zinc-400/20 text-zinc-300"
                        : entry.rank === 3
                          ? "bg-amber-600/20 text-amber-500"
                          : "bg-zinc-700/30 text-zinc-500"
                  }`}
                >
                  {entry.rank}
                </span>
                <span className="text-xs font-medium text-zinc-300">
                  {entry.agent_id}
                </span>
              </div>
              <span
                className={`text-xs font-semibold ${
                  entry.reputation >= 0 ? "text-emerald-400" : "text-red-400"
                }`}
              >
                {entry.reputation >= 0 ? "+" : ""}
                {entry.reputation.toFixed(1)}
              </span>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

// ─── Client-side leaderboard derivation ─────────────────────
// Builds a Leaderboard object from the /api/v1/agents response so the
// homepage rankings sections stay populated even when the dedicated
// /api/v1/world/leaderboard endpoint is unavailable or empty.

function topN<T>(items: T[], n: number): T[] {
  return items.slice(0, n);
}

function rankEntries(
  values: { agent: Agent; value: number }[],
): LeaderboardEntry[] {
  return topN(
    values
      .filter((v) => typeof v.value === "number" && !Number.isNaN(v.value))
      .sort((a, b) => b.value - a.value),
    10,
  ).map((v, i) => ({
    agentId: v.agent.id,
    agentName: v.agent.name ?? v.agent.id,
    value: v.value,
    rank: i + 1,
  }));
}

function deriveLeaderboardFromAgents(agents: Agent[]): Leaderboard {
  const richest = rankEntries(
    agents.map((agent) => ({ agent, value: agent.money ?? 0 })),
  );
  const longestLived = rankEntries(
    agents.map((agent) => ({
      agent,
      value: agent.ticks_survived ?? agent.age ?? 0,
    })),
  );
  const highestSkill = rankEntries(
    agents.map((agent) => ({
      agent,
      value: Object.values(agent.skills ?? {}).reduce(
        (sum, lvl) => sum + (typeof lvl === "number" ? lvl : 0),
        0,
      ),
    })),
  );
  const highestReputation = rankEntries(
    agents.map((agent) => ({ agent, value: agent.reputation ?? 0 })),
  );
  return { richest, longestLived, highestSkill, highestReputation };
}

export function LeaderboardSection({ statsTick }: LeaderboardProps) {
  const [leaderboard, setLeaderboard] = useState<Leaderboard | null>(null);
  const [reputationRankings, setReputationRankings] = useState<ReputationRankingEntry[]>([]);

  useEffect(() => {
    let cancelled = false;

    async function load() {
      // Try the dedicated leaderboard endpoint first. If it returns nothing
      // (or the endpoint is not implemented), fall back to deriving rankings
      // from the /api/v1/agents list.
      let data: Leaderboard | null = null;
      try {
        data = await fetchJSON<Leaderboard>("/api/v1/world/leaderboard");
      } catch {
        // Backend may not be available
      }

      const hasAny =
        !!data &&
        (data.richest?.length ||
          data.longestLived?.length ||
          data.highestSkill?.length ||
          data.highestReputation?.length);

      if (!hasAny) {
        // Derive from agents list. Each agent has {name, money, tokens,
        // ticks_survived, reputation, skills}. Map to LeaderboardEntry shape.
        try {
          const agents = await fetchJSON<Agent[]>("/api/v1/agents");
          if (cancelled) return;
          const list = Array.isArray(agents) ? agents : [];
          data = deriveLeaderboardFromAgents(list);
        } catch {
          // Both endpoints unavailable — leave leaderboard empty
        }
      }

      if (!cancelled) setLeaderboard(data);
    }

    load();
    const interval = setInterval(load, 10000);
    return () => {
      cancelled = true;
      clearInterval(interval);
    };
  }, [statsTick]);

  useEffect(() => {
    async function loadReputation() {
      try {
        const data = await fetchJSON<ReputationRankingEntry[]>("/api/v1/reputation/rankings");
        setReputationRankings(data);
      } catch {
        // Reputation API may not be configured
      }
    }
    loadReputation();
    const interval = setInterval(loadReputation, 10000);
    return () => clearInterval(interval);
  }, [statsTick]);

  return (
    <div className="grid grid-cols-1 gap-4 lg:grid-cols-2">
      <LeaderboardTable
        title="最富有"
        icon="💰"
        entries={leaderboard?.richest ?? []}
        valueFormatter={(v) => `$${v.toLocaleString()}`}
      />
      <LeaderboardTable
        title="最长寿"
        icon="🕐"
        entries={leaderboard?.longestLived ?? []}
        valueFormatter={(v) => `${v.toLocaleString()} Tick`}
      />
      <LeaderboardTable
        title="最高技能"
        icon="⚡"
        entries={leaderboard?.highestSkill ?? []}
        valueFormatter={(v) => `${v.toFixed(1)}`}
      />
      <LeaderboardTable
        title="最高信誉"
        icon="⭐"
        entries={leaderboard?.highestReputation ?? []}
        valueFormatter={(v) => `${v.toFixed(1)}`}
      />
      {reputationRankings.length > 0 && (
        <div className="lg:col-span-2">
          <ReputationRankingTable entries={reputationRankings} />
        </div>
      )}
    </div>
  );
}
