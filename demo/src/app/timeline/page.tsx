"use client";

import { useEffect, useState, useCallback } from "react";
import { loadEvents, loadSnapshots } from "@/lib/data";
import type { EmergenceEvent, WorldSnapshot } from "@/types/demo";

const CATEGORY_CONFIG: Record<string, { color: string; bg: string; border: string; label: string }> = {
  organization: { color: "text-blue-400", bg: "bg-blue-500/10", border: "border-blue-500/20", label: "Organization" },
  economic: { color: "text-green-400", bg: "bg-green-500/10", border: "border-green-500/20", label: "Economic" },
  governance: { color: "text-purple-400", bg: "bg-purple-500/10", border: "border-purple-500/20", label: "Governance" },
  culture: { color: "text-amber-400", bg: "bg-amber-500/10", border: "border-amber-500/20", label: "Culture" },
  milestone: { color: "text-cyan-400", bg: "bg-cyan-500/10", border: "border-cyan-500/20", label: "Milestone" },
};

const CATEGORY_DOT_COLORS: Record<string, string> = {
  organization: "#3b82f6",
  economic: "#22c55e",
  governance: "#a855f7",
  culture: "#f59e0b",
  milestone: "#06b6d4",
};

const PHASE_LABELS: Record<string, string> = {
  exploration: "Exploration",
  organization: "Organization",
  governance: "Governance",
};

function getPhaseColor(phase: string) {
  switch (phase) {
    case "exploration": return "text-blue-400";
    case "organization": return "text-green-400";
    case "governance": return "text-purple-400";
    default: return "text-zinc-400";
  }
}

export default function TimelinePage() {
  const [events, setEvents] = useState<EmergenceEvent[]>([]);
  const [snapshots, setSnapshots] = useState<WorldSnapshot[]>([]);
  const [selectedTick, setSelectedTick] = useState<number | null>(null);
  const [scrubTick, setScrubTick] = useState(0);

  useEffect(() => {
    Promise.all([loadEvents(), loadSnapshots()]).then(([ev, sn]) => {
      setEvents(ev);
      setSnapshots(sn);
      if (sn.length > 0) setScrubTick(sn[sn.length - 1].tick);
    });
  }, []);

  const currentSnapshot = snapshots.find((s) => s.tick <= scrubTick) ?? snapshots[0] ?? null;
  const nearbyEvents = events.filter((e) => Math.abs(e.tick - scrubTick) < 200);

  const handleSliderChange = useCallback((e: React.ChangeEvent<HTMLInputElement>) => {
    const val = Number(e.target.value);
    setScrubTick(val);
    setSelectedTick(null);
  }, []);

  return (
    <div className="min-h-screen">
      {/* Header */}
      <div className="border-b border-zinc-800 bg-zinc-950/80 backdrop-blur-md px-4 md:px-6 py-4">
        <h1 className="text-xl font-bold text-zinc-100">Civilization Timeline</h1>
        <p className="text-sm text-zinc-500 mt-1">Drag the slider to explore 5,000 ticks of emergent civilization</p>
      </div>

      <div className="max-w-6xl mx-auto px-4 md:px-6 py-6">
        {/* Slider */}
        <div className="mb-8 rounded-xl border border-zinc-800 bg-zinc-900/50 p-4 md:p-6">
          <div className="flex items-center justify-between mb-3">
            <span className="text-xs text-zinc-500">Tick 0</span>
            <div className="flex items-center gap-3">
              <span className={`text-sm font-semibold ${getPhaseColor(currentSnapshot?.phase ?? "exploration")}`}>
                {PHASE_LABELS[currentSnapshot?.phase ?? "exploration"]}
              </span>
              <span className="text-sm tabular-nums text-zinc-300 font-mono">
                Tick {scrubTick.toLocaleString()}
              </span>
            </div>
            <span className="text-xs text-zinc-500">Tick 5,000</span>
          </div>
          <input
            type="range"
            min={0}
            max={5000}
            step={10}
            value={scrubTick}
            onChange={handleSliderChange}
            className="w-full h-2 rounded-full appearance-none bg-zinc-800 cursor-pointer
              [&::-webkit-slider-thumb]:appearance-none [&::-webkit-slider-thumb]:w-4 [&::-webkit-slider-thumb]:h-4
              [&::-webkit-slider-thumb]:rounded-full [&::-webkit-slider-thumb]:bg-blue-400
              [&::-webkit-slider-thumb]:shadow-[0_0_10px_rgba(59,130,246,0.5)]"
            aria-label="Timeline scrubber"
          />

          {/* Event markers */}
          <div className="relative h-6 mt-2">
            {events.map((ev) => (
              <button
                key={ev.id}
                className={`absolute top-1 w-2 h-2 rounded-full -translate-x-1/2 cursor-pointer transition-transform hover:scale-150 ${
                  CATEGORY_CONFIG[ev.category]?.bg ?? "bg-zinc-500/20"
                } ${CATEGORY_CONFIG[ev.category]?.border ?? "border-zinc-500/20"} border ${
                  Math.abs(ev.tick - scrubTick) < 100 ? "scale-150" : ""
                }`}
                style={{ left: `${(ev.tick / 5000) * 100}%`, backgroundColor: CATEGORY_DOT_COLORS[ev.category] }}
                onClick={() => {
                  setScrubTick(ev.tick);
                  setSelectedTick(ev.tick);
                }}
                title={ev.title}
                aria-label={`${ev.title} at tick ${ev.tick}`}
              />
            ))}
          </div>
        </div>

        <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
          {/* World State */}
          <div className="lg:col-span-1">
            <h2 className="text-sm font-semibold text-zinc-400 uppercase tracking-wider mb-3">World State</h2>
            {currentSnapshot && (
              <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4 space-y-3">
                <div className="grid grid-cols-2 gap-3">
                  <div>
                    <div className="text-xs text-zinc-500">Active Agents</div>
                    <div className="text-lg font-semibold tabular-nums">{currentSnapshot.aliveAgents}</div>
                  </div>
                  <div>
                    <div className="text-xs text-zinc-500">Total Population</div>
                    <div className="text-lg font-semibold tabular-nums">{currentSnapshot.totalPopulation}</div>
                  </div>
                  <div>
                    <div className="text-xs text-zinc-500">GDP</div>
                    <div className="text-lg font-semibold tabular-nums">{currentSnapshot.gdp.toLocaleString()}</div>
                  </div>
                  <div>
                    <div className="text-xs text-zinc-500">Gini Index</div>
                    <div className="text-lg font-semibold tabular-nums">{currentSnapshot.giniCoefficient.toFixed(2)}</div>
                  </div>
                </div>

                {/* Top skills */}
                {currentSnapshot.skillDistribution.length > 0 && (
                  <div className="mt-3 pt-3 border-t border-zinc-800">
                    <div className="text-xs text-zinc-500 mb-2">Top Skills</div>
                    <div className="space-y-1.5">
                      {currentSnapshot.skillDistribution.slice(0, 5).map((s) => (
                        <div key={s.skill_name} className="flex items-center justify-between text-xs">
                          <span className="text-zinc-400 capitalize">{s.skill_name}</span>
                          <span className="text-zinc-500 tabular-nums">{s.agent_count} agents (lv {s.avg_level.toFixed(1)})</span>
                        </div>
                      ))}
                    </div>
                  </div>
                )}
              </div>
            )}
          </div>

          {/* Emergence Events */}
          <div className="lg:col-span-2">
            <h2 className="text-sm font-semibold text-zinc-400 uppercase tracking-wider mb-3">
              Emergence Events
              {nearbyEvents.length > 0 && (
                <span className="text-zinc-600 font-normal ml-2">near tick {scrubTick}</span>
              )}
            </h2>
            <div className="space-y-3">
              {(selectedTick !== null
                ? events.filter((e) => e.tick === selectedTick)
                : nearbyEvents.length > 0
                  ? nearbyEvents
                  : events
              ).map((ev) => {
                const cfg = CATEGORY_CONFIG[ev.category] ?? CATEGORY_CONFIG.milestone;
                return (
                  <div
                    key={ev.id}
                    className={`rounded-xl border ${cfg.border} ${cfg.bg} p-4 transition-all`}
                  >
                    <div className="flex items-start justify-between gap-3">
                      <div>
                        <div className="flex items-center gap-2 mb-1">
                          <span className={`text-xs font-medium ${cfg.color} uppercase`}>
                            {cfg.label}
                          </span>
                          <span className="text-xs text-zinc-600">|</span>
                          <span className="text-xs text-zinc-500 font-mono tabular-nums">
                            Tick {ev.tick.toLocaleString()}
                          </span>
                        </div>
                        <h3 className="text-sm font-semibold text-zinc-100">{ev.title}</h3>
                        <p className="text-xs text-zinc-400 mt-1">{ev.description}</p>
                      </div>
                    </div>
                    <div className="flex flex-wrap gap-1 mt-2">
                      {ev.agentsDetail.map((a) => (
                        <span key={a.id} className="text-[10px] px-1.5 py-0.5 rounded bg-zinc-800/60 text-zinc-400">
                          {a.name}
                        </span>
                      ))}
                      {ev.orgsDetail.map((o) => (
                        <span key={o.id} className="text-[10px] px-1.5 py-0.5 rounded bg-amber-500/10 text-amber-400">
                          {o.name}
                        </span>
                      ))}
                    </div>
                  </div>
                );
              })}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
