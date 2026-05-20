"use client";

import { useState, useCallback, useRef, useEffect } from "react";
import type { EmergenceEvent, TimelineSnapshot } from "@/types/demo";
import { EMERGENCE_COLORS, EMERGENCE_LABELS } from "@/types/demo";

interface TimelineViewProps {
  events: EmergenceEvent[];
  snapshots: TimelineSnapshot[];
}

export function TimelineView({ events, snapshots }: TimelineViewProps) {
  const [selectedTick, setSelectedTick] = useState<number>(0);
  const [hoveredEvent, setHoveredEvent] = useState<EmergenceEvent | null>(null);
  const scrollRef = useRef<HTMLDivElement>(null);

  const ticks = snapshots.map((s) => s.tick);
  const maxTick = Math.max(...ticks);

  const currentSnapshot = snapshots.find((s) => s.tick === selectedTick) ?? snapshots[0];

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "ArrowRight" || e.key === "ArrowDown") {
        e.preventDefault();
        setSelectedTick((t) => Math.min(t + 100, maxTick));
      } else if (e.key === "ArrowLeft" || e.key === "ArrowUp") {
        e.preventDefault();
        setSelectedTick((t) => Math.max(t - 100, 0));
      }
    },
    [maxTick]
  );

  // Auto-scroll timeline bar
  useEffect(() => {
    if (scrollRef.current) {
      const progress = selectedTick / maxTick;
      const scrollTarget = progress * scrollRef.current.scrollWidth;
      scrollRef.current.scrollLeft = scrollTarget - scrollRef.current.clientWidth / 2;
    }
  }, [selectedTick, maxTick]);

  return (
    <div
      className="flex flex-col gap-6"
      onKeyDown={handleKeyDown}
      tabIndex={0}
      role="slider"
      aria-label="Timeline navigation"
      aria-valuemin={0}
      aria-valuemax={maxTick}
      aria-valuenow={selectedTick}
    >
      {/* Slider */}
      <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4">
        <div className="mb-2 flex items-center justify-between text-sm">
          <span className="text-zinc-400">Tick: <span className="font-mono text-white">{selectedTick}</span></span>
          <span className="text-zinc-500">{Math.round((selectedTick / maxTick) * 100)}%</span>
        </div>
        <input
          type="range"
          min={0}
          max={maxTick}
          step={100}
          value={selectedTick}
          onChange={(e) => setSelectedTick(Number(e.target.value))}
          className="w-full accent-blue-500"
          aria-label="Tick slider"
        />
        {/* Quick-jump buttons */}
        <div className="mt-3 flex flex-wrap gap-2">
          {[0, 1000, 2000, 3000, 4000, 5000].map((tick) => (
            <button
              key={tick}
              onClick={() => setSelectedTick(tick)}
              className={`rounded-md px-3 py-1 text-xs font-medium transition-colors ${
                selectedTick === tick
                  ? "bg-blue-600 text-white"
                  : "bg-zinc-800 text-zinc-400 hover:bg-zinc-700 hover:text-zinc-200"
              }`}
            >
              Tick {tick}
            </button>
          ))}
        </div>
      </div>

      {/* Snapshot info */}
      <div className="grid gap-4 md:grid-cols-3">
        <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4">
          <div className="text-xs text-zinc-500">人口</div>
          <div className="mt-1 text-2xl font-bold text-white">{currentSnapshot.population}</div>
        </div>
        <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4">
          <div className="text-xs text-zinc-500">GDP</div>
          <div className="mt-1 text-2xl font-bold text-white">{currentSnapshot.gdp.toLocaleString()}</div>
        </div>
        <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4">
          <div className="text-xs text-zinc-500">组织数</div>
          <div className="mt-1 text-2xl font-bold text-white">{currentSnapshot.organizations}</div>
        </div>
      </div>

      {/* Scrollable timeline with events */}
      <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4">
        <h3 className="mb-4 text-sm font-semibold text-zinc-300">涌现事件时间轴</h3>
        <div
          ref={scrollRef}
          className="scrollbar-thin flex gap-3 overflow-x-auto pb-4"
        >
          {events.map((event) => {
            const isPast = event.tick <= selectedTick;
            const isHovered = hoveredEvent?.id === event.id;
            return (
              <button
                key={event.id}
                className={`flex-shrink-0 rounded-lg border p-3 text-left transition-all ${
                  isHovered
                    ? "border-zinc-600 bg-zinc-800"
                    : isPast
                      ? "border-zinc-700 bg-zinc-800/80"
                      : "border-zinc-800 bg-zinc-900/30 opacity-40"
                }`}
                style={{ borderLeftWidth: "3px", borderLeftColor: EMERGENCE_COLORS[event.category] }}
                onMouseEnter={() => setHoveredEvent(event)}
                onMouseLeave={() => setHoveredEvent(null)}
                onClick={() => setSelectedTick(event.tick)}
              >
                <div className="flex items-center gap-2">
                  <span
                    className="inline-block h-2 w-2 rounded-full"
                    style={{ backgroundColor: EMERGENCE_COLORS[event.category] }}
                  />
                  <span className="text-xs text-zinc-400">
                    Tick {event.tick}
                  </span>
                </div>
                <div className="mt-1 text-sm font-medium text-zinc-200">
                  {event.title}
                </div>
                <div className="mt-1 text-xs text-zinc-500">
                  {EMERGENCE_LABELS[event.category]}
                </div>
              </button>
            );
          })}
        </div>
        {hoveredEvent && (
          <div className="mt-2 rounded-lg border border-zinc-700 bg-zinc-800 p-3 text-sm">
            <div className="font-medium text-white">{hoveredEvent.title}</div>
            <div className="mt-1 text-zinc-400">{hoveredEvent.description}</div>
            <div className="mt-2 text-xs text-zinc-500">
              参与者: {hoveredEvent.agents.join(", ")}
            </div>
          </div>
        )}
      </div>

      {/* Legend */}
      <div className="flex flex-wrap items-center gap-4 text-xs text-zinc-400">
        {(Object.entries(EMERGENCE_COLORS) as [EmergenceEvent["category"], string][]).map(
          ([cat, color]) => (
            <div key={cat} className="flex items-center gap-1.5">
              <span
                className="inline-block h-2.5 w-2.5 rounded-full"
                style={{ backgroundColor: color }}
              />
              {EMERGENCE_LABELS[cat]}
            </div>
          )
        )}
        <span className="text-zinc-600">|</span>
        <span>← → 键盘导航</span>
      </div>
    </div>
  );
}
