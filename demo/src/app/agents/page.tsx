"use client";

import { useEffect, useState } from "react";
import { loadAgents } from "@/lib/data";
import type { DemoAgent, PersonalityTraits, ValueProfile } from "@/types/demo";

const PHASE_STYLES: Record<string, { bg: string; text: string; label: string }> = {
  exploration: { bg: "bg-blue-500/10", text: "text-blue-400", label: "Exploration" },
  organization: { bg: "bg-green-500/10", text: "text-green-400", label: "Organization" },
  governance: { bg: "bg-purple-500/10", text: "text-purple-400", label: "Governance" },
};

function RadarChart({ data, size = 120 }: { data: Record<string, number>; size?: number }) {
  const keys = Object.keys(data);
  const values = Object.values(data);
  const cx = size / 2;
  const cy = size / 2;
  const r = size / 2 - 16;
  const n = keys.length;

  const points = values.map((v, i) => {
    const angle = (Math.PI * 2 * i) / n - Math.PI / 2;
    return {
      x: cx + r * v * Math.cos(angle),
      y: cy + r * v * Math.sin(angle),
    };
  });

  const pathD = points.map((p, i) => `${i === 0 ? "M" : "L"}${p.x},${p.y}`).join(" ") + "Z";

  return (
    <svg width={size} height={size} viewBox={`0 0 ${size} ${size}`} className="mx-auto">
      {/* Grid rings */}
      {[0.25, 0.5, 0.75, 1].map((scale) => (
        <polygon
          key={scale}
          points={keys.map((_, i) => {
            const angle = (Math.PI * 2 * i) / n - Math.PI / 2;
            return `${cx + r * scale * Math.cos(angle)},${cy + r * scale * Math.sin(angle)}`;
          }).join(" ")}
          fill="none"
          stroke="#27272a"
          strokeWidth="0.5"
        />
      ))}
      {/* Axes */}
      {keys.map((_, i) => {
        const angle = (Math.PI * 2 * i) / n - Math.PI / 2;
        return (
          <line
            key={i}
            x1={cx}
            y1={cy}
            x2={cx + r * Math.cos(angle)}
            y2={cy + r * Math.sin(angle)}
            stroke="#27272a"
            strokeWidth="0.5"
          />
        );
      })}
      {/* Data area */}
      <path d={pathD} fill="rgba(59,130,246,0.15)" stroke="#3b82f6" strokeWidth="1.5" />
      {/* Data points */}
      {points.map((p, i) => (
        <circle key={i} cx={p.x} cy={p.y} r="2.5" fill="#3b82f6" />
      ))}
      {/* Labels */}
      {keys.map((key, i) => {
        const angle = (Math.PI * 2 * i) / n - Math.PI / 2;
        const labelR = r + 12;
        return (
          <text
            key={key}
            x={cx + labelR * Math.cos(angle)}
            y={cy + labelR * Math.sin(angle)}
            textAnchor="middle"
            dominantBaseline="central"
            className="fill-zinc-500"
            fontSize="8"
          >
            {key.slice(0, 4)}
          </text>
        );
      })}
    </svg>
  );
}

function AgentCard({
  agent,
  onSelect,
}: {
  agent: DemoAgent;
  onSelect: (agent: DemoAgent) => void;
}) {
  const phaseStyle = PHASE_STYLES[agent.phase] ?? PHASE_STYLES.exploration;
  const topSkills = Object.entries(agent.skills)
    .sort(([, a], [, b]) => b - a)
    .slice(0, 3);

  return (
    <button
      onClick={() => onSelect(agent)}
      className="w-full text-left rounded-xl border border-zinc-800 bg-zinc-900/50 p-4 hover:border-zinc-700 hover:bg-zinc-800/50 transition-all cursor-pointer"
    >
      <div className="flex items-center gap-3 mb-3">
        <div className="w-10 h-10 rounded-full bg-zinc-800 flex items-center justify-center text-lg flex-shrink-0">
          {agent.emoji}
        </div>
        <div className="min-w-0">
          <div className="font-semibold text-zinc-100 truncate">{agent.name}</div>
          <div className="flex items-center gap-2">
            <span className={`text-[10px] px-1.5 py-0.5 rounded ${phaseStyle.bg} ${phaseStyle.text}`}>
              {phaseStyle.label}
            </span>
            {agent.status === "dead" && (
              <span className="text-[10px] px-1.5 py-0.5 rounded bg-red-500/10 text-red-400">
                Dead
              </span>
            )}
          </div>
        </div>
      </div>

      {/* Top skills */}
      <div className="flex gap-1.5 flex-wrap">
        {topSkills.map(([name, level]) => (
          <span key={name} className="text-[10px] px-1.5 py-0.5 rounded bg-zinc-800/60 text-zinc-400">
            {name} {level}
          </span>
        ))}
      </div>

      {/* Stats */}
      <div className="grid grid-cols-3 gap-2 mt-3 text-center">
        <div>
          <div className="text-xs text-zinc-500">Rep</div>
          <div className="text-sm font-medium tabular-nums">{agent.reputation}</div>
        </div>
        <div>
          <div className="text-xs text-zinc-500">Wealth</div>
          <div className="text-sm font-medium tabular-nums">{agent.money.toLocaleString()}</div>
        </div>
        <div>
          <div className="text-xs text-zinc-500">Age</div>
          <div className="text-sm font-medium tabular-nums">{agent.age}</div>
        </div>
      </div>
    </button>
  );
}

function AgentDetail({
  agent,
  onClose,
}: {
  agent: DemoAgent;
  onClose: () => void;
}) {
  const phaseStyle = PHASE_STYLES[agent.phase] ?? PHASE_STYLES.exploration;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/60 backdrop-blur-sm" onClick={onClose}>
      <div
        className="w-full max-w-2xl max-h-[85vh] overflow-y-auto rounded-2xl border border-zinc-800 bg-zinc-950 p-6 scrollbar-thin"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-start justify-between mb-6">
          <div className="flex items-center gap-4">
            <div className="w-14 h-14 rounded-full bg-zinc-800 flex items-center justify-center text-2xl">
              {agent.emoji}
            </div>
            <div>
              <h2 className="text-xl font-bold text-zinc-100">{agent.name}</h2>
              <div className="flex items-center gap-2 mt-1">
                <span className={`text-xs px-2 py-0.5 rounded ${phaseStyle.bg} ${phaseStyle.text}`}>
                  {phaseStyle.label} Phase
                </span>
                <span className={`text-xs px-2 py-0.5 rounded ${agent.status === "alive" ? "bg-green-500/10 text-green-400" : "bg-red-500/10 text-red-400"}`}>
                  {agent.status === "alive" ? "Alive" : "Dead"}
                </span>
              </div>
            </div>
          </div>
          <button onClick={onClose} className="p-2 text-zinc-500 hover:text-zinc-300 transition-colors" aria-label="Close detail">
            <svg width="20" height="20" viewBox="0 0 20 20" fill="none" stroke="currentColor" strokeWidth="1.5">
              <path d="M5 5l10 10M15 5L5 15" />
            </svg>
          </button>
        </div>

        {/* Personality radar */}
        <div className="mb-6">
          <h3 className="text-sm font-semibold text-zinc-400 uppercase tracking-wider mb-3">Personality</h3>
          <RadarChart data={{ ...agent.personality }} size={180} />
        </div>

        {/* Values */}
        <div className="mb-6">
          <h3 className="text-sm font-semibold text-zinc-400 uppercase tracking-wider mb-3">Values</h3>
          <div className="grid grid-cols-2 sm:grid-cols-3 gap-3">
            {Object.entries(agent.values).map(([key, val]) => (
              <div key={key}>
                <div className="flex justify-between text-xs mb-1">
                  <span className="text-zinc-400 capitalize">{key}</span>
                  <span className="text-zinc-500 tabular-nums">{(val * 100).toFixed(0)}%</span>
                </div>
                <div className="h-1.5 rounded-full bg-zinc-800">
                  <div
                    className="h-full rounded-full bg-blue-500/60"
                    style={{ width: `${val * 100}%` }}
                  />
                </div>
              </div>
            ))}
          </div>
        </div>

        {/* Skills */}
        <div className="mb-6">
          <h3 className="text-sm font-semibold text-zinc-400 uppercase tracking-wider mb-3">Skills</h3>
          <div className="grid grid-cols-2 gap-3">
            {Object.entries(agent.skills)
              .sort(([, a], [, b]) => b - a)
              .map(([key, val]) => (
                <div key={key}>
                  <div className="flex justify-between text-xs mb-1">
                    <span className="text-zinc-400 capitalize">{key}</span>
                    <span className="text-zinc-500 tabular-nums">{val}/10</span>
                  </div>
                  <div className="h-1.5 rounded-full bg-zinc-800">
                    <div
                      className="h-full rounded-full bg-emerald-500/60"
                      style={{ width: `${val * 10}%` }}
                    />
                  </div>
                </div>
              ))}
          </div>
        </div>

        {/* Relationships */}
        {agent.relationships.length > 0 && (
          <div className="mb-6">
            <h3 className="text-sm font-semibold text-zinc-400 uppercase tracking-wider mb-3">Relationships</h3>
            <div className="space-y-2">
              {agent.relationships.map((rel, i) => (
                <div key={i} className="flex items-center justify-between text-sm">
                  <span className="text-zinc-300">{rel.targetName}</span>
                  <span className="text-xs text-zinc-500">{rel.type.replace("_", " ")}</span>
                  <span className="text-xs text-zinc-600 tabular-nums">{(rel.strength * 100).toFixed(0)}%</span>
                </div>
              ))}
            </div>
          </div>
        )}

        {/* Stats summary */}
        <div className="grid grid-cols-4 gap-3">
          <div className="text-center p-3 rounded-lg bg-zinc-900/30">
            <div className="text-xs text-zinc-500">Wealth</div>
            <div className="text-sm font-semibold tabular-nums text-amber-400">{agent.money.toLocaleString()}</div>
          </div>
          <div className="text-center p-3 rounded-lg bg-zinc-900/30">
            <div className="text-xs text-zinc-500">Tokens</div>
            <div className="text-sm font-semibold tabular-nums">{agent.tokens}</div>
          </div>
          <div className="text-center p-3 rounded-lg bg-zinc-900/30">
            <div className="text-xs text-zinc-500">Memories</div>
            <div className="text-sm font-semibold tabular-nums">{agent.memoryCount}</div>
          </div>
          <div className="text-center p-3 rounded-lg bg-zinc-900/30">
            <div className="text-xs text-zinc-500">Age</div>
            <div className="text-sm font-semibold tabular-nums">{agent.age}</div>
          </div>
        </div>
      </div>
    </div>
  );
}

type FilterKey = "all" | "alive" | "dead";

export default function AgentsPage() {
  const [agents, setAgents] = useState<DemoAgent[]>([]);
  const [selected, setSelected] = useState<DemoAgent | null>(null);
  const [filter, setFilter] = useState<FilterKey>("all");
  const [search, setSearch] = useState("");

  useEffect(() => {
    loadAgents().then(setAgents);
  }, []);

  const filtered = agents.filter((a) => {
    if (filter === "alive" && a.status !== "alive") return false;
    if (filter === "dead" && a.status !== "dead") return false;
    if (search && !a.name.toLowerCase().includes(search.toLowerCase())) return false;
    return true;
  });

  return (
    <div className="min-h-screen">
      <div className="border-b border-zinc-800 bg-zinc-950/80 backdrop-blur-md px-4 md:px-6 py-4">
        <h1 className="text-xl font-bold text-zinc-100">Agent Gallery</h1>
        <p className="text-sm text-zinc-500 mt-1">{agents.length} agents across 3 phases of civilization</p>
      </div>

      <div className="max-w-6xl mx-auto px-4 md:px-6 py-6">
        {/* Filters */}
        <div className="flex flex-col sm:flex-row gap-3 mb-6">
          <input
            type="text"
            placeholder="Search agents..."
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            className="flex-1 px-3 py-2 rounded-lg bg-zinc-900/50 border border-zinc-800 text-sm text-zinc-300 placeholder-zinc-600 focus:outline-none focus:border-zinc-600"
          />
          <div className="flex gap-1">
            {(["all", "alive", "dead"] as FilterKey[]).map((f) => (
              <button
                key={f}
                onClick={() => setFilter(f)}
                className={`px-3 py-2 rounded-lg text-sm transition-colors capitalize ${
                  filter === f
                    ? "bg-blue-500/10 text-blue-400 border border-blue-500/20"
                    : "text-zinc-400 hover:text-zinc-200 border border-transparent hover:border-zinc-800"
                }`}
              >
                {f}
              </button>
            ))}
          </div>
        </div>

        {/* Grid */}
        <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4">
          {filtered.map((agent) => (
            <AgentCard key={agent.id} agent={agent} onSelect={setSelected} />
          ))}
        </div>

        {filtered.length === 0 && (
          <div className="text-center text-zinc-500 py-20">No agents match your filters.</div>
        )}
      </div>

      {selected && <AgentDetail agent={selected} onClose={() => setSelected(null)} />}
    </div>
  );
}
