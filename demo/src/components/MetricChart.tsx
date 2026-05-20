"use client";

import {
  LineChart,
  Line,
  XAxis,
  YAxis,
  Tooltip,
  ResponsiveContainer,
  CartesianGrid,
} from "recharts";
import type { MetricSeries } from "@/types/demo";

interface MetricChartProps {
  title: string;
  data: MetricSeries[];
  color: string;
  valueLabel: string;
}

function CustomTooltip({
  active,
  payload,
  label,
  valueLabel,
}: {
  active?: boolean;
  payload?: Array<{ value: number }>;
  label?: number;
  valueLabel: string;
}) {
  if (!active || !payload?.length || label === undefined) return null;
  return (
    <div className="rounded-lg border border-zinc-700 bg-zinc-800 px-3 py-2 text-sm shadow-lg">
      <div className="text-xs text-zinc-400">Tick {label}</div>
      <div className="mt-1 font-medium text-white">
        {valueLabel}: {payload[0].value.toLocaleString()}
      </div>
    </div>
  );
}

export function MetricChart({ title, data, color, valueLabel }: MetricChartProps) {
  return (
    <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4">
      <h3 className="mb-4 text-sm font-semibold text-zinc-300">{title}</h3>
      <ResponsiveContainer width="100%" height={200}>
        <LineChart data={data}>
          <CartesianGrid strokeDasharray="3 3" stroke="#27272a" />
          <XAxis
            dataKey="tick"
            stroke="#52525b"
            tick={{ fontSize: 10, fill: "#71717a" }}
            tickFormatter={(v: number) => String(v)}
          />
          <YAxis
            stroke="#52525b"
            tick={{ fontSize: 10, fill: "#71717a" }}
            tickFormatter={(v: number) => {
              if (v >= 1000) return `${(v / 1000).toFixed(0)}k`;
              return String(v);
            }}
          />
          <Tooltip content={<CustomTooltip valueLabel={valueLabel} />} />
          <Line
            type="monotone"
            dataKey="value"
            stroke={color}
            strokeWidth={2}
            dot={false}
            activeDot={{ r: 4, strokeWidth: 0 }}
          />
        </LineChart>
      </ResponsiveContainer>
    </div>
  );
}
