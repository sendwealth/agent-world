"use client";

import { useEffect, useState } from "react";
import type { Leaderboard, LeaderboardEntry } from "@/types/world";
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

export function LeaderboardSection({ statsTick }: LeaderboardProps) {
  const [leaderboard, setLeaderboard] = useState<Leaderboard | null>(null);

  useEffect(() => {
    async function load() {
      try {
        const data = await fetchJSON<Leaderboard>("/api/v1/world/leaderboard");
        setLeaderboard(data);
      } catch {
        // Backend may not be available
      }
    }
    load();
    const interval = setInterval(load, 10000);
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
    </div>
  );
}
