"use client";

import { useEffect, useState } from "react";
import {
  AreaChart,
  Area,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ResponsiveContainer,
} from "recharts";
import { loadMetrics, loadEvents } from "@/lib/data";
import type { MetricSeries, EmergenceEvent } from "@/types/demo";

const METRIC_CONFIG = [
  { key: "culturalDiversity", title: "Cultural Diversity", color: "#f59e0b", format: (v: number) => `${(v * 100).toFixed(0)}%` },
  { key: "organizations", title: "Organizations", color: "#3b82f6", format: (v: number) => v.toString() },
  { key: "economy", title: "Trade Volume", color: "#22c55e", format: (v: number) => v.toLocaleString() },
  { key: "governance", title: "Governance Events", color: "#a855f7", format: (v: number) => v.toString() },
];

const CATEGORY_COLORS: Record<string, string> = {
  organization: "#3b82f6",
  economic: "#22c55e",
  governance: "#a855f7",
  culture: "#f59e0b",
  milestone: "#06b6d4",
};

function CustomTooltip({ active, payload, label }: { active?: boolean; payload?: Array<{ value: number; dataKey: string }>; label?: number }) {
  if (!active || !payload?.length) return null;
  return (
    <div className="rounded-lg border border-zinc-700 bg-zinc-900 px-3 py-2 text-xs shadow-xl">
      <div className="text-zinc-400 mb-1 font-mono tabular-nums">Tick {label?.toLocaleString()}</div>
      {payload.map((p) => (
        <div key={p.dataKey} className="text-zinc-200 tabular-nums">
          {p.dataKey}: {p.value.toLocaleString()}
        </div>
      ))}
    </div>
  );
}

function MetricChart({
  series,
  format,
}: {
  series: MetricSeries;
  format: (v: number) => string;
}) {
  const data = series.points.map((p) => ({
    tick: p.tick,
    value: p.value,
  }));

  return (
    <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4">
      <div className="flex items-center justify-between mb-4">
        <h3 className="text-sm font-semibold text-zinc-300">{series.name}</h3>
        <span className="text-xs text-zinc-600 tabular-nums">
          {format(data[data.length - 1]?.value ?? 0)}
        </span>
      </div>
      <ResponsiveContainer width="100%" height={200}>
        <AreaChart data={data} margin={{ top: 5, right: 5, left: -20, bottom: 0 }}>
          <defs>
            <linearGradient id={`grad-${series.name}`} x1="0" y1="0" x2="0" y2="1">
              <stop offset="5%" stopColor={series.color} stopOpacity={0.2} />
              <stop offset="95%" stopColor={series.color} stopOpacity={0} />
            </linearGradient>
          </defs>
          <CartesianGrid strokeDasharray="3 3" stroke="#27272a" />
          <XAxis
            dataKey="tick"
            tick={{ fontSize: 10, fill: "#71717a" }}
            tickFormatter={(v: number) => `${(v / 1000).toFixed(0)}k`}
            axisLine={{ stroke: "#27272a" }}
          />
          <YAxis
            tick={{ fontSize: 10, fill: "#71717a" }}
            tickFormatter={(v: number) => format(v)}
            axisLine={{ stroke: "#27272a" }}
          />
          <Tooltip content={<CustomTooltip />} />
          <Area
            type="monotone"
            dataKey="value"
            stroke={series.color}
            strokeWidth={2}
            fill={`url(#grad-${series.name})`}
          />
        </AreaChart>
      </ResponsiveContainer>
    </div>
  );
}

export default function DashboardPage() {
  const [metrics, setMetrics] = useState<Record<string, MetricSeries>>({});
  const [events, setEvents] = useState<EmergenceEvent[]>([]);

  useEffect(() => {
    Promise.all([loadMetrics(), loadEvents()]).then(([m, e]) => {
      setMetrics(m);
      setEvents(e);
    });
  }, []);

  return (
    <div className="min-h-screen">
      <div className="border-b border-zinc-800 bg-zinc-950/80 backdrop-blur-md px-4 md:px-6 py-4">
        <h1 className="text-xl font-bold text-zinc-100">Emergence Dashboard</h1>
        <p className="text-sm text-zinc-500 mt-1">Key metrics from 5,000 ticks of civilization</p>
      </div>

      <div className="max-w-6xl mx-auto px-4 md:px-6 py-6">
        {/* Metric charts */}
        <div className="grid grid-cols-1 md:grid-cols-2 gap-6 mb-8">
          {METRIC_CONFIG.map((cfg) => {
            const series = metrics[cfg.key];
            if (!series) return null;
            return (
              <MetricChart key={cfg.key} series={series} format={cfg.format} />
            );
          })}
        </div>

        {/* Key Events Timeline */}
        <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4 md:p-6">
          <h2 className="text-sm font-semibold text-zinc-400 uppercase tracking-wider mb-4">
            Key Events Timeline ({events.length} events)
          </h2>
          <div className="relative">
            <div className="absolute top-3 left-0 right-0 h-px bg-zinc-800" />

            <div className="flex justify-between relative min-h-16">
              {events.map((ev) => {
                const left = `${(ev.tick / 5000) * 100}%`;
                const dotColor = CATEGORY_COLORS[ev.category] ?? "#71717a";
                return (
                  <div
                    key={ev.id}
                    className="group relative flex flex-col items-center"
                    style={{ position: "absolute", left, transform: "translateX(-50%)" }}
                  >
                    <div
                      className="w-2.5 h-2.5 rounded-full border-2 border-zinc-900 z-10 transition-transform group-hover:scale-150"
                      style={{ backgroundColor: dotColor }}
                    />
                    <div className="absolute top-6 left-1/2 -translate-x-1/2 hidden group-hover:block z-20 w-48">
                      <div className="rounded-lg border border-zinc-700 bg-zinc-900 p-2 text-xs shadow-xl">
                        <div className="font-semibold text-zinc-200 mb-0.5">{ev.title}</div>
                        <div className="text-zinc-500 font-mono tabular-nums">Tick {ev.tick.toLocaleString()}</div>
                        {ev.agentsDetail.length > 0 && (
                          <div className="text-zinc-600 mt-0.5">{ev.agentsDetail.map((a) => a.name).join(", ")}</div>
                        )}
                      </div>
                    </div>
                  </div>
                );
              })}
            </div>

            <div className="flex justify-between mt-8 text-xs text-zinc-600">
              <span>Tick 0</span>
              <span className="text-blue-500/50">Exploration</span>
              <span className="text-green-500/50">Organization</span>
              <span className="text-purple-500/50">Governance</span>
              <span>Tick 5,000</span>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
