"use client";

import type { DemoAgent } from "@/types/demo";

interface AgentCardProps {
  agent: DemoAgent;
  isSelected: boolean;
  onClick: () => void;
}

const PHASE_LABELS: Record<string, string> = {
  survive: "生存",
  explore: "探索",
  build: "建设",
  trade: "贸易",
  govern: "治理",
  create: "创造",
  transcend: "超越",
};

export function AgentCard({ agent, isSelected, onClick }: AgentCardProps) {
  return (
    <button
      onClick={onClick}
      className={`group w-full rounded-xl border p-4 text-left transition-all ${
        isSelected
          ? "border-blue-500 bg-zinc-800/80 ring-1 ring-blue-500/30"
          : "border-zinc-800 bg-zinc-900/50 hover:border-zinc-700 hover:bg-zinc-800/50"
      }`}
    >
      <div className="flex items-start gap-3">
        <div className="flex h-12 w-12 flex-shrink-0 items-center justify-center rounded-lg bg-zinc-800 text-2xl">
          {agent.emoji}
        </div>
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2">
            <span className="truncate font-semibold text-white">{agent.name}</span>
            {!agent.alive && (
              <span className="rounded bg-zinc-700 px-1.5 py-0.5 text-[10px] text-zinc-400">
                已消亡
              </span>
            )}
          </div>
          <div className="mt-1 flex flex-wrap items-center gap-2 text-xs text-zinc-400">
            <span className="rounded bg-zinc-800 px-2 py-0.5">
              {PHASE_LABELS[agent.phase] ?? agent.phase}
            </span>
            {agent.organization && (
              <span className="rounded bg-blue-900/30 px-2 py-0.5 text-blue-300">
                {agent.organization}
              </span>
            )}
          </div>
          <div className="mt-2 flex flex-wrap gap-1">
            {agent.traits.map((t) => (
              <span
                key={t}
                className="rounded bg-zinc-800 px-1.5 py-0.5 text-[10px] text-zinc-500"
              >
                {t}
              </span>
            ))}
          </div>
        </div>
      </div>

      {/* Quick stats */}
      <div className="mt-3 grid grid-cols-3 gap-2 border-t border-zinc-800 pt-3 text-center text-xs">
        <div>
          <div className="font-medium text-white">{agent.money.toLocaleString()}</div>
          <div className="text-zinc-500">金币</div>
        </div>
        <div>
          <div className="font-medium text-white">{agent.reputation}</div>
          <div className="text-zinc-500">声望</div>
        </div>
        <div>
          <div className="font-medium text-white">{agent.age}</div>
          <div className="text-zinc-500">Tick</div>
        </div>
      </div>
    </button>
  );
}
