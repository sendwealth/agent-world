"use client";

import { StatCards } from "@/components/StatCards";
import { EventStream } from "@/components/EventStream";
import { LeaderboardSection } from "@/components/Leaderboard";
import { useWorldState } from "@/hooks/useWorldState";

export default function DashboardPage() {
  const { stats, events, connected, error } = useWorldState();

  return (
    <div className="p-6 space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold text-zinc-100">世界概览</h1>
          <p className="text-sm text-zinc-500">
            {stats
              ? `Tick #${stats.tick} · ${stats.agentCount} Agents`
              : "正在连接世界引擎..."}
          </p>
        </div>
        {error && (
          <div className="rounded-lg bg-red-500/10 border border-red-500/20 px-3 py-1.5 text-xs text-red-400">
            {error}
          </div>
        )}
      </div>

      {/* Stat Cards */}
      <StatCards stats={stats} />

      {/* Event Stream + Quick Actions */}
      <div className="grid grid-cols-1 gap-6 xl:grid-cols-3">
        <div className="xl:col-span-2">
          <EventStream events={events} connected={connected} />
        </div>
        <div className="space-y-4">
          <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4">
            <h2 className="mb-3 text-sm font-semibold text-zinc-200">快速操作</h2>
            <div className="space-y-2">
              <button className="w-full rounded-lg bg-blue-500/10 border border-blue-500/20 px-4 py-2.5 text-sm font-medium text-blue-400 transition-colors hover:bg-blue-500/20">
                🤖 创建 Agent
              </button>
              <button className="w-full rounded-lg bg-purple-500/10 border border-purple-500/20 px-4 py-2.5 text-sm font-medium text-purple-400 transition-colors hover:bg-purple-500/20">
                📋 发布任务
              </button>
              <button className="w-full rounded-lg bg-emerald-500/10 border border-emerald-500/20 px-4 py-2.5 text-sm font-medium text-emerald-400 transition-colors hover:bg-emerald-500/20">
                ⏩ 快进 100 Tick
              </button>
            </div>
          </div>
        </div>
      </div>

      {/* Leaderboards */}
      <LeaderboardSection statsTick={stats?.tick} />
    </div>
  );
}
