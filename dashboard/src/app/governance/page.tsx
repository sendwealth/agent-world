"use client";

import Link from "next/link";
import {
  BarChart,
  Bar,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ResponsiveContainer,
  RadarChart,
  PolarGrid,
  PolarAngleAxis,
  PolarRadiusAxis,
  Radar,
  PieChart,
  Pie,
  Cell,
} from "recharts";
import { useGovernanceOverview } from "@/hooks/useGovernanceStream";
import type { OrgMetrics } from "@/types/world";

type OrgWithMetrics = OrgMetrics & { org_name: string };

const CustomTooltip = ({
  active,
  payload,
  label,
}: {
  active?: boolean;
  payload?: Array<{ value: number; name: string; color: string }>;
  label?: string;
}) => {
  if (!active || !payload) return null;
  return (
    <div className="rounded-lg border border-zinc-700 bg-zinc-800/95 px-3 py-2 text-xs shadow-xl backdrop-blur-sm">
      <p className="text-zinc-400 mb-1">{label}</p>
      {payload.map((p, i) => (
        <p
          key={i}
          style={{ color: p.color }}
          className="font-medium tabular-nums"
        >
          {p.name}:{" "}
          {typeof p.value === "number"
            ? p.value.toLocaleString(undefined, { maximumFractionDigits: 2 })
            : p.value}
        </p>
      ))}
    </div>
  );
};

function StabilityBar({ score }: { score: number }) {
  const pct = Math.min(100, Math.max(0, score * 100));
  const color =
    pct >= 70 ? "bg-green-500" : pct >= 40 ? "bg-amber-500" : "bg-red-500";
  return (
    <div className="flex items-center gap-2">
      <div className="h-2 flex-1 rounded-full bg-zinc-800">
        <div
          className={`h-full rounded-full ${color} transition-all duration-500`}
          style={{ width: `${pct}%` }}
        />
      </div>
      <span className="text-xs text-zinc-400 tabular-nums w-10 text-right">
        {pct.toFixed(0)}%
      </span>
    </div>
  );
}

export default function GovernancePage() {
  const { orgMetrics, summary, loading, error } = useGovernanceOverview();

  if (loading) {
    return (
      <div className="flex h-full items-center justify-center">
        <div className="text-sm text-zinc-600">正在加载治理数据...</div>
      </div>
    );
  }

  if (error && !summary) {
    return (
      <div className="p-4 md:p-6 space-y-4">
        <h1 className="text-xl md:text-2xl font-bold text-zinc-100">
          治理面板
        </h1>
        <div className="rounded-lg bg-red-500/10 border border-red-500/20 px-4 py-3 text-sm text-red-400">
          {error}
        </div>
      </div>
    );
  }

  if (!summary) {
    return (
      <div className="p-4 md:p-6 space-y-4">
        <h1 className="text-xl md:text-2xl font-bold text-zinc-100">
          治理面板
        </h1>
        <p className="text-sm text-zinc-500">暂无治理数据</p>
      </div>
    );
  }

  // Chart data: tax collection per org
  const taxChartData = orgMetrics.map((o: OrgWithMetrics) => ({
    name:
      o.org_name.length > 8 ? o.org_name.slice(0, 8) + "…" : o.org_name,
    totalTax: o.total_tax_collected,
    perCapita: o.tax_per_member,
  }));

  // Chart data: legislation status per org
  const legislationBarData = orgMetrics.map((o: OrgWithMetrics) => ({
    name:
      o.org_name.length > 8 ? o.org_name.slice(0, 8) + "…" : o.org_name,
    activated: o.rules_activated,
    expired: o.rules_expired,
    repealed: o.rules_repealed,
  }));

  // World legislation pie chart data
  const legislationPieData = [
    { name: "已生效", value: summary.total_rules_activated, color: "#22c55e" },
    {
      name: "已过期",
      value:
        summary.total_rules_proposed - summary.total_rules_activated > 0
          ? summary.total_rules_proposed - summary.total_rules_activated
          : 0,
      color: "#71717a",
    },
  ].filter((d) => d.value > 0);

  // Radar data with legislation
  const diplomacyRadarData = [
    {
      metric: "条约签署",
      value: summary.total_treaties,
      fullMark: Math.max(summary.total_treaties * 1.5, 10),
    },
    {
      metric: "选举活跃度",
      value: Math.round(summary.election_activity_rate * 100),
      fullMark: 100,
    },
    {
      metric: "立法通过率",
      value: Math.round(summary.avg_legislation_success_rate * 100),
      fullMark: 100,
    },
    {
      metric: "税收总额",
      value: summary.total_tax_collected,
      fullMark: Math.max(summary.total_tax_collected * 1.5, 100),
    },
    {
      metric: "组织总数",
      value: summary.total_orgs,
      fullMark: Math.max(summary.total_orgs * 1.5, 5),
    },
    {
      metric: "平均稳定性",
      value: Math.round(summary.avg_stability * 100),
      fullMark: 100,
    },
  ];

  // Compute avg participation rate across orgs
  const avgParticipation =
    orgMetrics.length > 0
      ? orgMetrics.reduce((sum, o) => sum + o.avg_participation_rate, 0) /
        orgMetrics.length
      : 0;

  return (
    <div className="p-4 md:p-6 space-y-6">
      {/* Header */}
      <div>
        <h1 className="text-xl md:text-2xl font-bold text-zinc-100">
          治理面板
        </h1>
        <p className="text-sm text-zinc-500">
          世界治理概览 — {summary.total_orgs} 个组织
        </p>
      </div>

      {error && (
        <div className="rounded-lg bg-red-500/10 border border-red-500/20 px-3 py-1.5 text-xs text-red-400">
          {error}
        </div>
      )}

      {/* Key Indicators */}
      <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 xl:grid-cols-3 2xl:grid-cols-6">
        <div className="rounded-xl border border-blue-500/20 bg-blue-500/5 p-4 space-y-1">
          <p className="text-sm text-zinc-400">组织总数</p>
          <p className="text-2xl font-bold text-blue-400">
            {summary.total_orgs}
          </p>
          <p className="text-xs text-zinc-500">已形成治理结构</p>
        </div>
        <div className="rounded-xl border border-indigo-500/20 bg-indigo-500/5 p-4 space-y-1">
          <p className="text-sm text-zinc-400">选举活跃度</p>
          <p className="text-2xl font-bold text-indigo-400">
            {(summary.election_activity_rate * 100).toFixed(0)}%
          </p>
          <p className="text-xs text-zinc-500">有选举的组织占比</p>
        </div>
        <div className="rounded-xl border border-violet-500/20 bg-violet-500/5 p-4 space-y-1">
          <p className="text-sm text-zinc-400">投票参与率</p>
          <p className="text-2xl font-bold text-violet-400">
            {(avgParticipation * 100).toFixed(1)}%
          </p>
          <p className="text-xs text-zinc-500">平均选举参与</p>
        </div>
        <div className="rounded-xl border border-orange-500/20 bg-orange-500/5 p-4 space-y-1">
          <p className="text-sm text-zinc-400">税收总额</p>
          <p className="text-2xl font-bold text-orange-400">
            ${summary.total_tax_collected.toLocaleString()}
          </p>
          <p className="text-xs text-zinc-500">累计征收</p>
        </div>
        <div className="rounded-xl border border-sky-500/20 bg-sky-500/5 p-4 space-y-1">
          <p className="text-sm text-zinc-400">立法通过率</p>
          <p className="text-2xl font-bold text-sky-400">
            {(summary.avg_legislation_success_rate * 100).toFixed(0)}%
          </p>
          <p className="text-xs text-zinc-500">
            {summary.total_rules_proposed} 项提案 / {summary.total_rules_activated} 项生效
          </p>
        </div>
        <div className="rounded-xl border border-emerald-500/20 bg-emerald-500/5 p-4 space-y-1">
          <p className="text-sm text-zinc-400">治理健康度</p>
          <p className="text-2xl font-bold text-emerald-400">
            {(summary.avg_stability * 100).toFixed(0)}%
          </p>
          <p className="text-xs text-zinc-500">世界平均稳定性</p>
        </div>
      </div>

      {/* Charts Row */}
      <div className="grid grid-cols-1 gap-6 xl:grid-cols-2">
        {/* Tax Distribution Bar Chart */}
        <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4 space-y-3">
          <h2 className="text-sm font-semibold text-zinc-200">
            组织税收对比
          </h2>
          {taxChartData.length === 0 ? (
            <p className="text-sm text-zinc-600 h-48 flex items-center justify-center">
              暂无数据
            </p>
          ) : (
            <ResponsiveContainer width="100%" height={240}>
              <BarChart data={taxChartData}>
                <CartesianGrid strokeDasharray="3 3" stroke="#27272a" />
                <XAxis
                  dataKey="name"
                  stroke="#52525b"
                  tick={{ fontSize: 10 }}
                />
                <YAxis
                  stroke="#52525b"
                  tick={{ fontSize: 10 }}
                  tickFormatter={(v: number) =>
                    v >= 1000 ? `${(v / 1000).toFixed(0)}K` : `${v}`
                  }
                />
                <Tooltip content={<CustomTooltip />} />
                <Bar
                  dataKey="totalTax"
                  fill="#f97316"
                  name="税收总额"
                  radius={[4, 4, 0, 0]}
                />
              </BarChart>
            </ResponsiveContainer>
          )}
        </div>

        {/* Governance Radar */}
        <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4 space-y-3">
          <h2 className="text-sm font-semibold text-zinc-200">
            治理活跃度雷达
          </h2>
          {diplomacyRadarData.length === 0 ? (
            <p className="text-sm text-zinc-600 h-48 flex items-center justify-center">
              暂无数据
            </p>
          ) : (
            <ResponsiveContainer width="100%" height={240}>
              <RadarChart data={diplomacyRadarData}>
                <PolarGrid stroke="#27272a" />
                <PolarAngleAxis
                  dataKey="metric"
                  stroke="#71717a"
                  tick={{ fontSize: 10 }}
                />
                <PolarRadiusAxis stroke="#52525b" tick={{ fontSize: 8 }} />
                <Radar
                  name="治理指标"
                  dataKey="value"
                  stroke="#3b82f6"
                  fill="#3b82f6"
                  fillOpacity={0.2}
                />
              </RadarChart>
            </ResponsiveContainer>
          )}
        </div>
      </div>

      {/* Legislation Row */}
      <div className="grid grid-cols-1 gap-6 xl:grid-cols-2">
        {/* Legislation status per org — stacked bar */}
        <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4 space-y-3">
          <h2 className="text-sm font-semibold text-zinc-200">
            组织立法状态对比
          </h2>
          {legislationBarData.length === 0 ? (
            <p className="text-sm text-zinc-600 h-48 flex items-center justify-center">
              暂无立法数据
            </p>
          ) : (
            <ResponsiveContainer width="100%" height={240}>
              <BarChart data={legislationBarData}>
                <CartesianGrid strokeDasharray="3 3" stroke="#27272a" />
                <XAxis
                  dataKey="name"
                  stroke="#52525b"
                  tick={{ fontSize: 10 }}
                />
                <YAxis stroke="#52525b" tick={{ fontSize: 10 }} />
                <Tooltip content={<CustomTooltip />} />
                <Bar
                  dataKey="activated"
                  stackId="legislation"
                  fill="#22c55e"
                  name="已生效"
                  radius={[0, 0, 0, 0]}
                />
                <Bar
                  dataKey="expired"
                  stackId="legislation"
                  fill="#71717a"
                  name="已过期"
                />
                <Bar
                  dataKey="repealed"
                  stackId="legislation"
                  fill="#ef4444"
                  name="已废除"
                  radius={[4, 4, 0, 0]}
                />
              </BarChart>
            </ResponsiveContainer>
          )}
        </div>

        {/* World legislation pie — pass vs fail */}
        <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4 space-y-3">
          <h2 className="text-sm font-semibold text-zinc-200">
            世界立法总览
          </h2>
          {summary.total_rules_proposed === 0 ? (
            <p className="text-sm text-zinc-600 h-48 flex items-center justify-center">
              暂无立法数据
            </p>
          ) : (
            <div className="flex items-center justify-center gap-6">
              <ResponsiveContainer width="50%" height={220}>
                <PieChart>
                  <Pie
                    data={legislationPieData}
                    cx="50%"
                    cy="50%"
                    innerRadius={50}
                    outerRadius={80}
                    dataKey="value"
                    stroke="none"
                  >
                    {legislationPieData.map((entry, idx) => (
                      <Cell key={idx} fill={entry.color} />
                    ))}
                  </Pie>
                  <Tooltip content={<CustomTooltip />} />
                </PieChart>
              </ResponsiveContainer>
              <div className="space-y-3">
                <div>
                  <p className="text-xs text-zinc-500">总提案</p>
                  <p className="text-lg font-bold text-zinc-100">
                    {summary.total_rules_proposed}
                  </p>
                </div>
                <div>
                  <p className="text-xs text-zinc-500">已生效</p>
                  <p className="text-lg font-bold text-green-400">
                    {summary.total_rules_activated}
                  </p>
                </div>
                <div>
                  <p className="text-xs text-zinc-500">通过率</p>
                  <p className="text-lg font-bold text-sky-400">
                    {(summary.avg_legislation_success_rate * 100).toFixed(1)}%
                  </p>
                </div>
              </div>
            </div>
          )}
        </div>
      </div>

      {/* Organization Governance Health Ranking */}
      <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4 space-y-3">
        <div className="flex items-center justify-between">
          <h2 className="text-sm font-semibold text-zinc-200">
            组织治理健康度排行
          </h2>
          <Link
            href="/governance/comparison"
            className="text-xs text-blue-400 hover:text-blue-300 transition-colors"
          >
            查看对比 →
          </Link>
        </div>
        {orgMetrics.length === 0 ? (
          <p className="text-sm text-zinc-600">暂无组织数据</p>
        ) : (
          <div className="space-y-3">
            {[...orgMetrics]
              .sort(
                (a, b) =>
                  b.governance_stability_score - a.governance_stability_score
              )
              .slice(0, 10)
              .map((org: OrgWithMetrics, idx: number) => (
                <div key={org.org_id}>
                  <Link
                    href={`/governance/${org.org_id}`}
                    className="flex items-center gap-3 rounded-lg px-3 py-2 hover:bg-zinc-800/50 transition-colors"
                  >
                    <span className="text-xs font-bold text-zinc-500 w-5 text-right">
                      #{idx + 1}
                    </span>
                    <div className="flex-1 min-w-0">
                      <div className="flex items-center gap-2">
                        <span className="text-sm font-medium text-zinc-200 truncate">
                          {org.org_name}
                        </span>
                        <span className="text-[10px] text-zinc-500">
                          {org.member_count} 成员
                        </span>
                      </div>
                      <StabilityBar score={org.governance_stability_score} />
                    </div>
                    <div className="text-right shrink-0">
                      <p className="text-xs text-zinc-400">
                        {org.election_count} 次选举 · 参与{" "}
                        {(org.avg_participation_rate * 100).toFixed(0)}%
                      </p>
                      <p className="text-[10px] text-zinc-500">
                        ${org.total_tax_collected.toLocaleString()} 税收 ·{" "}
                        {org.rules_activated}/{org.rules_proposed} 法案通过
                      </p>
                    </div>
                  </Link>
                </div>
              ))}
          </div>
        )}
      </div>
    </div>
  );
}
