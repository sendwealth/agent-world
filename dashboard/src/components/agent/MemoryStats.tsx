"use client";

import { useMemo } from "react";
import type { WorldEvent } from "@/types/world";

interface MemoryStatsProps {
  agentId: string;
  events: WorldEvent[];
}

export default function MemoryStats({ agentId, events }: MemoryStatsProps) {
  const stats = useMemo(() => {
    const agentEvents = events.filter(
      (e) => e.agentId === agentId || e.targetId === agentId
    );

    // Interaction memory: unique agents interacted with
    const interactedAgents = new Set<string>();
    for (const e of agentEvents) {
      if (e.agentId && e.agentId !== agentId) interactedAgents.add(e.agentId);
      if (e.targetId && e.targetId !== agentId) interactedAgents.add(e.targetId);
    }

    // Event type breakdown
    const typeCounts: Record<string, number> = {};
    for (const e of agentEvents) {
      typeCounts[e.type] = (typeCounts[e.type] ?? 0) + 1;
    }

    // Activity intensity: events per tick bucket (last 100 ticks)
    const maxTick = Math.max(...agentEvents.map((e) => e.tick), 0);
    const bucketSize = 10;
    const bucketCount = 10;
    const activityBuckets: number[] = new Array(bucketCount).fill(0);
    for (const e of agentEvents) {
      const bucketIdx = Math.floor((maxTick - e.tick) / bucketSize);
      if (bucketIdx >= 0 && bucketIdx < bucketCount) {
        activityBuckets[bucketIdx]++;
      }
    }

    // Trade volume
    const tradeEvents = agentEvents.filter((e) => e.type === "trade");
    const tradeVolume = tradeEvents.reduce((sum, e) => sum + (e.amount ?? 0), 0);

    // Task stats
    const tasksCreated = agentEvents.filter((e) => e.type === "task_created" && e.agentId === agentId).length;
    const tasksCompleted = agentEvents.filter((e) => e.type === "task_completed" && e.agentId === agentId).length;
    const tasksClaimed = agentEvents.filter((e) => e.type === "task_claimed" && e.agentId === agentId).length;

    // Skill-up events
    const skillUpEvents = agentEvents.filter((e) => e.type === "skill_up" && e.agentId === agentId);

    return {
      totalEvents: agentEvents.length,
      interactedAgents: interactedAgents.size,
      typeCounts,
      activityBuckets: activityBuckets.reverse(),
      tradeVolume,
      tasksCreated,
      tasksCompleted,
      tasksClaimed,
      skillUps: skillUpEvents.length,
    };
  }, [agentId, events]);

  const maxBucket = Math.max(...stats.activityBuckets, 1);

  const eventTypeLabels: Record<string, { label: string; color: string }> = {
    agent_spawn: { label: "诞生", color: "bg-green-400" },
    agent_death: { label: "死亡", color: "bg-red-400" },
    trade: { label: "交易", color: "bg-amber-400" },
    task_created: { label: "任务创建", color: "bg-blue-400" },
    task_claimed: { label: "任务认领", color: "bg-cyan-400" },
    task_completed: { label: "任务完成", color: "bg-emerald-400" },
    message: { label: "消息", color: "bg-violet-400" },
    skill_up: { label: "技能提升", color: "bg-purple-400" },
    reputation_change: { label: "信誉变化", color: "bg-yellow-400" },
    investment: { label: "投资", color: "bg-teal-400" },
    tax: { label: "税收", color: "bg-orange-400" },
    inflation: { label: "通胀", color: "bg-rose-400" },
  };

  return (
    <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4 space-y-4">
      <div className="flex items-center justify-between">
        <h2 className="text-sm font-semibold text-zinc-200">记忆统计</h2>
        <span className="text-xs text-zinc-500">{stats.totalEvents} 条记录</span>
      </div>

      {/* Memory summary cards */}
      <div className="grid grid-cols-2 gap-2 sm:grid-cols-4">
        <div className="rounded-lg border border-zinc-800 bg-zinc-900/30 px-3 py-2">
          <p className="text-[10px] text-zinc-500">互动 Agent</p>
          <p className="text-lg font-bold tabular-nums text-zinc-100">{stats.interactedAgents}</p>
        </div>
        <div className="rounded-lg border border-zinc-800 bg-zinc-900/30 px-3 py-2">
          <p className="text-[10px] text-zinc-500">交易量</p>
          <p className="text-lg font-bold tabular-nums text-amber-400">{stats.tradeVolume.toLocaleString()}</p>
        </div>
        <div className="rounded-lg border border-zinc-800 bg-zinc-900/30 px-3 py-2">
          <p className="text-[10px] text-zinc-500">任务完成</p>
          <p className="text-lg font-bold tabular-nums text-emerald-400">
            {stats.tasksCompleted}
            <span className="text-xs text-zinc-500">/{stats.tasksClaimed}</span>
          </p>
        </div>
        <div className="rounded-lg border border-zinc-800 bg-zinc-900/30 px-3 py-2">
          <p className="text-[10px] text-zinc-500">技能升级</p>
          <p className="text-lg font-bold tabular-nums text-purple-400">{stats.skillUps}</p>
        </div>
      </div>

      {/* Activity histogram */}
      <div>
        <p className="mb-2 text-[10px] font-medium uppercase tracking-wider text-zinc-500">
          活动热度 (近 {stats.activityBuckets.length * 10} Tick)
        </p>
        <div className="flex items-end gap-1" style={{ height: "40px" }}>
          {stats.activityBuckets.map((count, i) => (
            <div
              key={i}
              className="flex-1 rounded-t bg-blue-400/60 transition-all duration-300"
              style={{ height: `${Math.max((count / maxBucket) * 100, 2)}%` }}
              title={`${count} events`}
            />
          ))}
        </div>
        <div className="mt-1 flex justify-between text-[9px] text-zinc-600">
          <span>旧</span>
          <span>新</span>
        </div>
      </div>

      {/* Event type breakdown */}
      <div>
        <p className="mb-2 text-[10px] font-medium uppercase tracking-wider text-zinc-500">
          事件类型分布
        </p>
        <div className="space-y-1.5">
          {Object.entries(stats.typeCounts)
            .sort(([, a], [, b]) => b - a)
            .slice(0, 6)
            .map(([type, count]) => {
              const cfg = eventTypeLabels[type] ?? { label: type, color: "bg-zinc-400" };
              const pct = stats.totalEvents > 0 ? (count / stats.totalEvents) * 100 : 0;
              return (
                <div key={type} className="flex items-center gap-2">
                  <span className="w-16 text-[10px] text-zinc-500 truncate">{cfg.label}</span>
                  <div className="h-1.5 flex-1 rounded-full bg-zinc-800">
                    <div
                      className={`h-1.5 rounded-full ${cfg.color} transition-all duration-500`}
                      style={{ width: `${pct}%` }}
                    />
                  </div>
                  <span className="w-8 text-right text-[10px] tabular-nums text-zinc-500">
                    {count}
                  </span>
                </div>
              );
            })}
        </div>
      </div>
    </div>
  );
}
