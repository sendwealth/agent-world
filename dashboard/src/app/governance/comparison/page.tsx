"use client";

import { useEffect, useState } from "react";
import Link from "next/link";
import {
  RadarChart,
  PolarGrid,
  PolarAngleAxis,
  PolarRadiusAxis,
  Radar,
  Legend,
  ResponsiveContainer,
  BarChart,
  Bar,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
} from "recharts";
import { useGovernanceComparison } from "@/hooks/useGovernanceStream";
import { fetchJSON } from "@/lib/api";
import type { Organization } from "@/types/world";

const ORG_COLORS = [
  "#3b82f6",
  "#f97316",
  "#22c55e",
  "#a855f7",
  "#ef4444",
  "#06b6d4",
  "#eab308",
  "#ec4899",
];

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

export default function GovernanceComparisonPage() {
  const [orgNames, setOrgNames] = useState<Record<string, string>>({});
  const [orgIds, setOrgIds] = useState<string[] | undefined>(undefined);

  // Fetch org list to get IDs and names
  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const orgs = await fetchJSON<Organization[]>("/api/v1/orgs");
        if (cancelled) return;
        const activeOrgs = orgs.filter((o) => o.status === "active");
        const nameMap: Record<string, string> = {};
        activeOrgs.forEach((o) => { nameMap[o.id] = o.name; });
        setOrgNames(nameMap);
        setOrgIds(activeOrgs.map((o) => o.id));
      } catch {
        // will show empty state
      }
    })();
    return () => { cancelled = true; };
  }, []);

  const { orgs, loading, error } = useGovernanceComparison(orgIds);

  // Get display name for an org
  const getOrgName = (orgId: string) => orgNames[orgId] ?? orgId.slice(0, 8);

  if (loading || orgIds === undefined) {
    return (
      <div className="flex h-full items-center justify-center">
        <div className="text-sm text-zinc-600">正在加载治理对比数据...</div>
      </div>
    );
  }

  if (error && orgs.length === 0) {
    return (
      <div className="p-4 md:p-6 space-y-4">
        <h1 className="text-xl md:text-2xl font-bold text-zinc-100">
          治理模式对比
        </h1>
        <div className="rounded-lg bg-red-500/10 border border-red-500/20 px-4 py-3 text-sm text-red-400">
          {error}
        </div>
      </div>
    );
  }

  if (orgs.length === 0) {
    return (
      <div className="p-4 md:p-6 space-y-4">
        <h1 className="text-xl md:text-2xl font-bold text-zinc-100">
          治理模式对比
        </h1>
        <p className="text-sm text-zinc-500">
          暂无组织数据。请等待组织形成治理结构。
        </p>
      </div>
    );
  }

  // Radar chart: normalized governance metrics per org
  const radarMetrics = [
    "选举活跃度",
    "投票参与率",
    "税收效率",
    "外交活跃度",
    "立法通过率",
    "治理稳定性",
    "成员规模",
  ];

  // Normalize values for radar chart (0-100 scale)
  const maxElections = Math.max(...orgs.map((o) => o.election_count), 1);
  const maxTax = Math.max(...orgs.map((o) => o.total_tax_collected), 1);
  const maxRelations = Math.max(...orgs.map((o) => o.active_relations_count), 1);
  const maxMembers = Math.max(...orgs.map((o) => o.member_count), 1);

  const radarData = radarMetrics.map((metric) => {
    const entry: Record<string, string | number> = { metric };
    orgs.forEach((org) => {
      let value = 0;
      switch (metric) {
        case "选举活跃度":
          value = (org.election_count / maxElections) * 100;
          break;
        case "投票参与率":
          value = org.avg_participation_rate * 100;
          break;
        case "税收效率":
          value = (org.total_tax_collected / maxTax) * 100;
          break;
        case "外交活跃度":
          value = (org.active_relations_count / maxRelations) * 100;
          break;
        case "立法通过率":
          value = org.legislation_success_rate * 100;
          break;
        case "治理稳定性":
          value = org.governance_stability_score * 100;
          break;
        case "成员规模":
          value = (org.member_count / maxMembers) * 100;
          break;
      }
      entry[getOrgName(org.org_id)] = Math.round(value);
    });
    return entry;
  });

  // Bar chart: side-by-side comparison
  const comparisonBarData = orgs.map((org) => {
    const name = getOrgName(org.org_id);
    return {
      name: name.length > 10 ? name.slice(0, 10) + "…" : name,
      elections: org.election_count,
      tax: org.total_tax_collected,
      treaties: org.treaties_signed,
      stability: Math.round(org.governance_stability_score * 100),
      legislation: Math.round(org.legislation_success_rate * 100),
    };
  });

  return (
    <div className="p-4 md:p-6 space-y-6">
      {/* Header */}
      <div>
        <h1 className="text-xl md:text-2xl font-bold text-zinc-100">
          治理模式对比
        </h1>
        <p className="text-sm text-zinc-500">
          并排比较 {orgs.length} 个组织的治理效率指标
        </p>
      </div>

      {error && (
        <div className="rounded-lg bg-red-500/10 border border-red-500/20 px-3 py-1.5 text-xs text-red-400">
          {error}
        </div>
      )}

      {/* Radar Chart — Governance Efficiency */}
      <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4 space-y-3">
        <h2 className="text-sm font-semibold text-zinc-200">
          治理效率雷达图
        </h2>
        <ResponsiveContainer width="100%" height={360}>
          <RadarChart data={radarData}>
            <PolarGrid stroke="#27272a" />
            <PolarAngleAxis
              dataKey="metric"
              stroke="#71717a"
              tick={{ fontSize: 11 }}
            />
            <PolarRadiusAxis stroke="#52525b" tick={{ fontSize: 8 }} />
            {orgs.slice(0, 6).map((org, i) => {
              const name = getOrgName(org.org_id);
              return (
                <Radar
                  key={org.org_id}
                  name={name}
                  dataKey={name}
                  stroke={ORG_COLORS[i % ORG_COLORS.length]}
                  fill={ORG_COLORS[i % ORG_COLORS.length]}
                  fillOpacity={0.1}
                />
              );
            })}
            <Legend
              wrapperStyle={{ fontSize: 11 }}
              formatter={(value: string) => (
                <span className="text-zinc-300">{value}</span>
              )}
            />
          </RadarChart>
        </ResponsiveContainer>
      </div>

      {/* Bar Charts Row */}
      <div className="grid grid-cols-1 gap-6 xl:grid-cols-3">
        {/* Elections Comparison */}
        <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4 space-y-3">
          <h2 className="text-sm font-semibold text-zinc-200">选举次数对比</h2>
          <ResponsiveContainer width="100%" height={240}>
            <BarChart data={comparisonBarData}>
              <CartesianGrid strokeDasharray="3 3" stroke="#27272a" />
              <XAxis dataKey="name" stroke="#52525b" tick={{ fontSize: 10 }} />
              <YAxis stroke="#52525b" tick={{ fontSize: 10 }} />
              <Tooltip content={<CustomTooltip />} />
              <Bar
                dataKey="elections"
                fill="#3b82f6"
                name="选举次数"
                radius={[4, 4, 0, 0]}
              />
            </BarChart>
          </ResponsiveContainer>
        </div>

        {/* Stability Comparison */}
        <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4 space-y-3">
          <h2 className="text-sm font-semibold text-zinc-200">
            治理稳定性对比
          </h2>
          <ResponsiveContainer width="100%" height={240}>
            <BarChart data={comparisonBarData}>
              <CartesianGrid strokeDasharray="3 3" stroke="#27272a" />
              <XAxis dataKey="name" stroke="#52525b" tick={{ fontSize: 10 }} />
              <YAxis stroke="#52525b" tick={{ fontSize: 10 }} domain={[0, 100]} />
              <Tooltip content={<CustomTooltip />} />
              <Bar
                dataKey="stability"
                fill="#22c55e"
                name="稳定性 %"
                radius={[4, 4, 0, 0]}
              />
            </BarChart>
          </ResponsiveContainer>
        </div>

        {/* Legislation Comparison */}
        <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4 space-y-3">
          <h2 className="text-sm font-semibold text-zinc-200">
            立法通过率对比
          </h2>
          <ResponsiveContainer width="100%" height={240}>
            <BarChart data={comparisonBarData}>
              <CartesianGrid strokeDasharray="3 3" stroke="#27272a" />
              <XAxis dataKey="name" stroke="#52525b" tick={{ fontSize: 10 }} />
              <YAxis stroke="#52525b" tick={{ fontSize: 10 }} domain={[0, 100]} />
              <Tooltip content={<CustomTooltip />} />
              <Bar
                dataKey="legislation"
                fill="#0ea5e9"
                name="立法通过率 %"
                radius={[4, 4, 0, 0]}
              />
            </BarChart>
          </ResponsiveContainer>
        </div>
      </div>

      {/* Detailed Comparison Table */}
      <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4 space-y-3">
        <h2 className="text-sm font-semibold text-zinc-200">详细指标对比</h2>
        <div className="overflow-x-auto">
          <table className="w-full">
            <thead>
              <tr className="border-b border-zinc-800">
                <th className="px-4 py-3 text-left text-xs font-semibold text-zinc-400">
                  组织
                </th>
                <th className="px-4 py-3 text-right text-xs font-semibold text-zinc-400">
                  选举次数
                </th>
                <th className="px-4 py-3 text-right text-xs font-semibold text-zinc-400">
                  参与率
                </th>
                <th className="px-4 py-3 text-right text-xs font-semibold text-zinc-400">
                  税收总额
                </th>
                <th className="px-4 py-3 text-right text-xs font-semibold text-zinc-400">
                  人均税收
                </th>
                <th className="px-4 py-3 text-right text-xs font-semibold text-zinc-400">
                  条约
                </th>
                <th className="px-4 py-3 text-right text-xs font-semibold text-zinc-400">
                  立法
                </th>
                <th className="px-4 py-3 text-right text-xs font-semibold text-zinc-400">
                  稳定性
                </th>
                <th className="px-4 py-3 text-right text-xs font-semibold text-zinc-400">
                  成员
                </th>
              </tr>
            </thead>
            <tbody>
              {orgs.map((org) => (
                <tr
                  key={org.org_id}
                  className="border-b border-zinc-800/50 last:border-0"
                >
                  <td className="px-4 py-3">
                    <Link
                      href={`/governance/${org.org_id}`}
                      className="text-sm font-medium text-zinc-200 hover:text-blue-400 transition-colors"
                    >
                      {getOrgName(org.org_id)}
                    </Link>
                  </td>
                  <td className="px-4 py-3 text-right text-sm text-zinc-300 tabular-nums">
                    {org.election_count}
                  </td>
                  <td className="px-4 py-3 text-right text-sm text-zinc-300 tabular-nums">
                    {(org.avg_participation_rate * 100).toFixed(0)}%
                  </td>
                  <td className="px-4 py-3 text-right text-sm text-zinc-300 tabular-nums">
                    ${org.total_tax_collected.toLocaleString()}
                  </td>
                  <td className="px-4 py-3 text-right text-sm text-zinc-300 tabular-nums">
                    ${org.tax_per_member.toLocaleString()}
                  </td>
                  <td className="px-4 py-3 text-right text-sm text-zinc-300 tabular-nums">
                    {org.treaties_signed} /{" "}
                    <span className="text-red-400">
                      {org.treaties_broken}
                    </span>
                  </td>
                  <td className="px-4 py-3 text-right text-sm tabular-nums">
                    <span className="text-zinc-300">
                      {org.rules_activated}/{org.rules_proposed}
                    </span>
                    {" "}
                    <span className="text-sky-400">
                      ({(org.legislation_success_rate * 100).toFixed(0)}%)
                    </span>
                  </td>
                  <td className="px-4 py-3 text-right">
                    <span
                      className={`text-sm font-medium tabular-nums ${
                        org.governance_stability_score >= 0.7
                          ? "text-green-400"
                          : org.governance_stability_score >= 0.4
                          ? "text-amber-400"
                          : "text-red-400"
                      }`}
                    >
                      {(org.governance_stability_score * 100).toFixed(0)}%
                    </span>
                  </td>
                  <td className="px-4 py-3 text-right text-sm text-zinc-300 tabular-nums">
                    {org.member_count}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );
}
