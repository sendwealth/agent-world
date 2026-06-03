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

function MoneyIcon() {
  return (
    <svg className="h-5 w-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5}>
      <path strokeLinecap="round" strokeLinejoin="round" d="M12 6v12m-3-2.818.879.659c1.171.879 3.07.879 4.242 0 1.172-.879 1.172-2.303 0-3.182C13.536 12.219 12.768 12 12 12c-.725 0-1.45-.22-2.003-.659-1.106-.879-1.106-2.303 0-3.182s2.9-.879 4.006 0l.415.33M21 12a9 9 0 1 1-18 0 9 9 0 0 1 18 0Z" />
    </svg>
  );
}

function TaskIcon() {
  return (
    <svg className="h-5 w-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5}>
      <path strokeLinecap="round" strokeLinejoin="round" d="M9 12h3.75M9 15h3.75M9 18h3.75m3 .75H18a2.25 2.25 0 0 0 2.25-2.25V6.108c0-1.135-.845-2.098-1.976-2.192a48.424 48.424 0 0 0-1.123-.08m-5.801 0c-.065.21-.1.433-.1.664 0 .414.336.75.75.75h4.5a.75.75 0 0 0 .75-.75 2.25 2.25 0 0 0-.1-.664m-5.8 0A2.251 2.251 0 0 1 13.5 2.25H15c1.012 0 1.867.668 2.15 1.586m-5.8 0c-.376.023-.75.05-1.124.08C9.095 4.01 8.25 4.973 8.25 6.108V8.25m0 0H4.875c-.621 0-1.125.504-1.125 1.125v11.25c0 .621.504 1.125 1.125 1.125h9.75c.621 0 1.125-.504 1.125-1.125V9.375c0-.621-.504-1.125-1.125-1.125H8.25ZM6.75 12h.008v.008H6.75V12Zm0 3h.008v.008H6.75V15Zm0 3h.008v.008H6.75V18Z" />
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
        title="总货币"
        value={stats?.totalMoney != null ? `$${stats.totalMoney.toLocaleString()}` : "—"}
        subtitle={stats ? `总 Token: ${stats.totalTokens.toLocaleString()}` : "加载中..."}
        icon={<MoneyIcon />}
        color="green"
      />
      <StatCard
        title="任务数"
        value={stats?.taskCount ?? "—"}
        subtitle={stats ? `Tick #${stats.tick}` : "加载中..."}
        icon={<TaskIcon />}
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
