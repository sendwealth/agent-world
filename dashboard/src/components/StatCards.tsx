"use client";

import { StatCard } from "./StatCard";
import type { WorldStats } from "@/types/world";

interface StatCardsProps {
  stats: WorldStats | null;
}

// SVG icons as inline components
function AgentsIcon() {
  return (
    <svg className="h-5 w-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5}>
      <path strokeLinecap="round" strokeLinejoin="round" d="M15 19.128a9.38 9.38 0 0 0 2.625.372 9.337 9.337 0 0 0 4.121-.952 4.125 4.125 0 0 0-7.533-2.493M15 19.128v-.003c0-1.113-.285-2.16-.786-3.07M15 19.128v.106A12.318 12.318 0 0 1 8.624 21c-2.331 0-4.512-.645-6.374-1.766l-.001-.109a6.375 6.375 0 0 1 11.964-3.07M12 6.375a3.375 3.375 0 1 1-6.75 0 3.375 3.375 0 0 1 6.75 0Zm8.25 2.25a2.625 2.625 0 1 1-5.25 0 2.625 2.625 0 0 1 5.25 0Z" />
    </svg>
  );
}

function GdpIcon() {
  return (
    <svg className="h-5 w-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5}>
      <path strokeLinecap="round" strokeLinejoin="round" d="M12 6v12m-3-2.818.879.659c1.171.879 3.07.879 4.242 0 1.172-.879 1.172-2.303 0-3.182C13.536 12.219 12.768 12 12 12c-.725 0-1.45-.22-2.003-.659-1.106-.879-1.106-2.303 0-3.182s2.9-.879 4.006 0l.415.33M21 12a9 9 0 1 1-18 0 9 9 0 0 1 18 0Z" />
    </svg>
  );
}

function InflationIcon() {
  return (
    <svg className="h-5 w-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5}>
      <path strokeLinecap="round" strokeLinejoin="round" d="M2.25 18 9 11.25l4.306 4.306a11.95 11.95 0 0 1 5.814-5.518l2.74-1.22m0 0-5.94-2.281m5.94 2.28-2.28 5.941" />
    </svg>
  );
}

function DeathsIcon() {
  return (
    <svg className="h-5 w-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5}>
      <path strokeLinecap="round" strokeLinejoin="round" d="M12 6a3.939 3.939 0 0 0-3.939 3.939c0 2.178 1.768 3.939 3.939 3.939a3.939 3.939 0 0 0 0-7.878Z" />
      <path strokeLinecap="round" strokeLinejoin="round" d="M12 2C6.477 2 2 6.477 2 12s4.477 10 10 10 10-4.477 10-10S17.523 2 12 2Z" />
      <path strokeLinecap="round" strokeLinejoin="round" d="M9 10h.01M15 10h.01" />
      <path strokeLinecap="round" strokeLinejoin="round" d="M9.5 15.5c.833-1 2.667-1 3.5 0" />
    </svg>
  );
}

export function StatCards({ stats }: StatCardsProps) {
  return (
    <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 xl:grid-cols-4">
      <StatCard
        title="Agent 总数"
        value={stats?.agentCount ?? "—"}
        subtitle={stats ? `存活: ${stats.aliveCount}` : "加载中..."}
        icon={<AgentsIcon />}
        color="blue"
      />
      <StatCard
        title="世界 GDP"
        value={stats?.gdp != null ? `$${stats.gdp.toLocaleString()}` : "—"}
        subtitle={stats ? `总货币: $${stats.totalMoney.toLocaleString()}` : "加载中..."}
        icon={<GdpIcon />}
        color="green"
      />
      <StatCard
        title="通胀率"
        value={stats?.inflationRate != null ? `${stats.inflationRate.toFixed(2)}%` : "—"}
        subtitle="每 Tick"
        icon={<InflationIcon />}
        color="amber"
      />
      <StatCard
        title="死亡数"
        value={stats?.deadCount ?? "—"}
        subtitle={stats ? `Tick #${stats.tick}` : "加载中..."}
        icon={<DeathsIcon />}
        color="red"
      />
    </div>
  );
}
