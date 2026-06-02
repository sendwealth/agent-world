"use client";

import {
  AreaChart,
  Area,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ResponsiveContainer,
} from "recharts";
import type { MetricSeries } from "@/types/demo";

function CustomTooltip({
  active,
  payload,
  label,
}: {
  active?: boolean;
  payload?: Array<{ value: number; dataKey: string }>;
  label?: number;
}) {
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

export function MetricChart({
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
