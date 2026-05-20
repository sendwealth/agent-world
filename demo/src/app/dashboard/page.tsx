"use client";

import { MetricChart } from "@/components/MetricChart";
import { getDashboardMetrics, getEmergenceEvents } from "@/lib/data";
import { EMERGENCE_COLORS, EMERGENCE_LABELS } from "@/types/demo";
import type { EmergenceCategory } from "@/types/demo";

export default function DashboardPage() {
  const metrics = getDashboardMetrics();
  const events = getEmergenceEvents();

  const charts = [
    {
      title: "文化多样性指数",
      data: metrics.culturalDiversity,
      color: "#f97316",
      valueLabel: "多样性",
    },
    {
      title: "组织数量",
      data: metrics.organizationCount,
      color: "#3b82f6",
      valueLabel: "组织数",
    },
    {
      title: "经济活动总量",
      data: metrics.economicActivity,
      color: "#22c55e",
      valueLabel: "GDP",
    },
    {
      title: "治理事件累计",
      data: metrics.governanceEvents,
      color: "#a855f7",
      valueLabel: "事件数",
    },
  ];

  return (
    <div className="mx-auto max-w-7xl px-4 py-8">
      <h1 className="text-2xl font-bold text-white md:text-3xl">涌现仪表盘</h1>
      <p className="mt-2 text-zinc-400">
        5000 Tick 内四个维度的涌现指标变化趋势。
      </p>

      {/* Charts grid */}
      <div className="mt-8 grid gap-6 md:grid-cols-2">
        {charts.map((chart) => (
          <MetricChart
            key={chart.title}
            title={chart.title}
            data={chart.data}
            color={chart.color}
            valueLabel={chart.valueLabel}
          />
        ))}
      </div>

      {/* Event timeline summary */}
      <div className="mt-8 rounded-xl border border-zinc-800 bg-zinc-900/50 p-6">
        <h2 className="mb-4 text-sm font-semibold text-zinc-300">关键事件时间线</h2>
        <div className="space-y-3">
          {events.map((event) => (
            <div key={event.id} className="flex items-start gap-3">
              <div className="flex flex-col items-center">
                <span
                  className="mt-1 inline-block h-2.5 w-2.5 rounded-full"
                  style={{ backgroundColor: EMERGENCE_COLORS[event.category as EmergenceCategory] }}
                />
              </div>
              <div className="flex-1">
                <div className="flex items-center gap-2">
                  <span className="text-sm font-medium text-zinc-200">
                    {event.title}
                  </span>
                  <span className="rounded bg-zinc-800 px-1.5 py-0.5 text-[10px] text-zinc-500">
                    {EMERGENCE_LABELS[event.category as EmergenceCategory]}
                  </span>
                </div>
                <div className="mt-0.5 text-xs text-zinc-500">
                  Tick {event.tick} — {event.description}
                </div>
              </div>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
