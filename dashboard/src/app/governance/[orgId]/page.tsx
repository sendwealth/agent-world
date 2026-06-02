"use client";

import { useState, useMemo, useEffect } from "react";
import { useParams, useRouter } from "next/navigation";
import {
  BarChart,
  Bar,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ResponsiveContainer,
} from "recharts";
import {
  useGovernanceOrg,
  useGovernanceTimeline,
} from "@/hooks/useGovernanceStream";
import { fetchJSON } from "@/lib/api";
import { EVENT_TYPE_CONFIG } from "@/lib/event-types";
import type { Organization } from "@/types/world";

const eventTypeLabels: Record<string, string> = {
  tax_collected: "税收征收",
  treasury_distributed: "国库分配",
  leadership_election_started: "选举开始",
  leadership_changed: "领导更替",
  treaty_proposed: "条约提议",
  treaty_signed: "条约签署",
  treaty_broken: "条约撕毁",
  relation_changed: "关系变化",
  soft_rule_proposed: "法案提议",
  soft_rule_activated: "法案生效",
  soft_rule_expired: "法案过期",
  soft_rule_repealed: "法案废除",
};

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

export default function GovernanceOrgDetailPage() {
  const params = useParams();
  const router = useRouter();
  const orgId = params.orgId as string;

  const { metrics, loading, error } = useGovernanceOrg(orgId);
  const { events: timelineEvents } = useGovernanceTimeline(orgId);
  const [orgName, setOrgName] = useState<string>(orgId);
  const [tab, setTab] = useState<
    "leadership" | "tax" | "diplomacy" | "legislation" | "timeline"
  >("leadership");

  // Fetch org name from /api/v1/orgs/:id
  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const org = await fetchJSON<Organization>(`/api/v1/orgs/${orgId}`);
        if (!cancelled) setOrgName(org.name);
      } catch {
        // keep using orgId as fallback name
      }
    })();
    return () => { cancelled = true; };
  }, [orgId]);

  // Timeline events grouped for display
  const groupedTimeline = useMemo(() => {
    const grouped: Record<string, typeof timelineEvents> = {};
    for (const event of timelineEvents) {
      const type = event.event_type;
      if (!grouped[type]) grouped[type] = [];
      grouped[type].push(event);
    }
    return grouped;
  }, [timelineEvents]);

  // Governance activity time series: bin events by tick intervals
  const activityTimeSeries = useMemo(() => {
    if (timelineEvents.length === 0) return [];
    const sorted = [...timelineEvents].sort((a, b) => a.tick - b.tick);
    const minTick = sorted[0].tick;
    const maxTick = sorted[sorted.length - 1].tick;
    const bucketSize = Math.max(1, Math.ceil((maxTick - minTick) / 30));
    const buckets: Record<number, Record<string, number>> = {};
    for (const event of sorted) {
      const bucket = Math.floor(event.tick / bucketSize) * bucketSize;
      if (!buckets[bucket]) buckets[bucket] = {};
      const type = event.event_type;
      buckets[bucket][type] = (buckets[bucket][type] ?? 0) + 1;
    }
    return Object.entries(buckets)
      .sort(([a], [b]) => Number(a) - Number(b))
      .map(([tick, counts]) => ({
        tick: `T${tick}`,
        ...counts,
      }));
  }, [timelineEvents]);

  if (loading) {
    return (
      <div className="flex h-full items-center justify-center">
        <div className="text-sm text-zinc-600">正在加载治理数据...</div>
      </div>
    );
  }

  if (error || !metrics) {
    return (
      <div className="p-4 md:p-6 space-y-4">
        <button
          onClick={() => router.back()}
          className="text-sm text-zinc-400 hover:text-zinc-200 transition-colors"
        >
          &larr; 返回
        </button>
        <div className="rounded-lg bg-red-500/10 border border-red-500/20 px-4 py-3 text-sm text-red-400">
          {error ?? "组织治理数据不存在"}
        </div>
      </div>
    );
  }

  return (
    <div className="p-4 md:p-6 space-y-6">
      {/* Header */}
      <div className="flex items-center gap-2 sm:gap-4">
        <button
          onClick={() => router.push("/governance")}
          className="rounded-lg p-1.5 text-zinc-400 hover:bg-zinc-800 hover:text-zinc-200 transition-colors"
        >
          <svg
            className="h-5 w-5"
            fill="none"
            stroke="currentColor"
            viewBox="0 0 24 24"
          >
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              strokeWidth={2}
              d="M15 19l-7-7 7-7"
            />
          </svg>
        </button>
        <div>
          <div className="flex flex-wrap items-center gap-2 sm:gap-3">
            <h1 className="text-xl sm:text-2xl font-bold text-zinc-100">
              {orgName}
            </h1>
            <span className="rounded-full bg-purple-500/10 border border-purple-500/20 px-2 py-0.5 text-[10px] font-medium text-purple-400">
              治理详情
            </span>
          </div>
          <p className="text-sm text-zinc-500">
            稳定性 {(metrics.governance_stability_score * 100).toFixed(0)}% ·{" "}
            {metrics.member_count} 成员
          </p>
        </div>
      </div>

      {/* Stats Cards — now 6 cards */}
      <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 xl:grid-cols-3 2xl:grid-cols-6">
        <div className="rounded-xl border border-blue-500/20 bg-blue-500/5 p-4 space-y-1">
          <p className="text-sm text-zinc-400">选举次数</p>
          <p className="text-2xl font-bold text-blue-400">
            {metrics.election_count}
          </p>
          <p className="text-xs text-zinc-500">
            平均 {metrics.avg_candidate_count.toFixed(1)} 名候选人
          </p>
        </div>
        <div className="rounded-xl border border-violet-500/20 bg-violet-500/5 p-4 space-y-1">
          <p className="text-sm text-zinc-400">投票参与率</p>
          <p className="text-2xl font-bold text-violet-400">
            {(metrics.avg_participation_rate * 100).toFixed(1)}%
          </p>
          <p className="text-xs text-zinc-500">选举参与水平</p>
        </div>
        <div className="rounded-xl border border-orange-500/20 bg-orange-500/5 p-4 space-y-1">
          <p className="text-sm text-zinc-400">税收总额</p>
          <p className="text-2xl font-bold text-orange-400">
            ${metrics.total_tax_collected.toLocaleString()}
          </p>
          <p className="text-xs text-zinc-500">
            人均 ${metrics.tax_per_member.toLocaleString()}
          </p>
        </div>
        <div className="rounded-xl border border-emerald-500/20 bg-emerald-500/5 p-4 space-y-1">
          <p className="text-sm text-zinc-400">条约签署</p>
          <p className="text-2xl font-bold text-emerald-400">
            {metrics.treaties_signed}
          </p>
          <p className="text-xs text-zinc-500">
            撕毁 {metrics.treaties_broken}
          </p>
        </div>
        <div className="rounded-xl border border-sky-500/20 bg-sky-500/5 p-4 space-y-1">
          <p className="text-sm text-zinc-400">立法通过率</p>
          <p className="text-2xl font-bold text-sky-400">
            {(metrics.legislation_success_rate * 100).toFixed(0)}%
          </p>
          <p className="text-xs text-zinc-500">
            {metrics.rules_activated}/{metrics.rules_proposed} 项生效
          </p>
        </div>
        <div className="rounded-xl border border-purple-500/20 bg-purple-500/5 p-4 space-y-1">
          <p className="text-sm text-zinc-400">外交关系</p>
          <p className="text-2xl font-bold text-purple-400">
            {metrics.active_relations_count}
          </p>
          <p className="text-xs text-zinc-500">活跃外交伙伴</p>
        </div>
      </div>

      {/* Governance Health Score Breakdown */}
      <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4 space-y-3">
        <h3 className="text-sm font-semibold text-zinc-200">
          治理健康度评分
        </h3>
        <div className="grid grid-cols-1 gap-3 sm:grid-cols-4">
          <div className="space-y-1">
            <p className="text-xs text-zinc-500">综合评分</p>
            <div className="flex items-center gap-2">
              <span className="text-lg font-bold text-zinc-100">
                {(metrics.governance_stability_score * 100).toFixed(0)}
              </span>
              <span className="text-sm text-zinc-500">/ 100</span>
            </div>
          </div>
          <div className="space-y-1">
            <p className="text-xs text-zinc-500">外交健康 (40%)</p>
            <div className="h-2 rounded-full bg-zinc-800">
              <div
                className="h-full rounded-full bg-emerald-500 transition-all duration-500"
                style={{
                  width: `${(metrics.treaties_signed + metrics.treaties_broken > 0
                    ? metrics.treaties_signed / (metrics.treaties_signed + metrics.treaties_broken)
                    : 0.5
                  ) * 100}%`,
                }}
              />
            </div>
          </div>
          <div className="space-y-1">
            <p className="text-xs text-zinc-500">领导稳定性 (40%)</p>
            <div className="h-2 rounded-full bg-zinc-800">
              <div
                className="h-full rounded-full bg-blue-500 transition-all duration-500"
                style={{
                  width: `${(metrics.election_count > 0
                    ? Math.min(1, metrics.election_count / Math.max(metrics.election_count, 1))
                    : 1
                  ) * 100}%`,
                }}
              />
            </div>
          </div>
          <div className="space-y-1">
            <p className="text-xs text-zinc-500">立法通过率 (20%)</p>
            <div className="h-2 rounded-full bg-zinc-800">
              <div
                className="h-full rounded-full bg-sky-500 transition-all duration-500"
                style={{ width: `${metrics.legislation_success_rate * 100}%` }}
              />
            </div>
          </div>
        </div>
      </div>

      {/* Tab Navigation */}
      <div className="flex items-center gap-1 border-b border-zinc-800 overflow-x-auto">
        {(
          ["leadership", "tax", "diplomacy", "legislation", "timeline"] as const
        ).map((t) => {
          const labels = {
            leadership: "领导变更",
            tax: "税收趋势",
            diplomacy: "外交关系",
            legislation: "立法追踪",
            timeline: "事件流",
          };
          return (
            <button
              key={t}
              onClick={() => setTab(t)}
              className={`px-4 py-2.5 text-sm font-medium transition-colors border-b-2 whitespace-nowrap ${
                tab === t
                  ? "text-blue-400 border-blue-400"
                  : "text-zinc-400 border-transparent hover:text-zinc-200"
              }`}
            >
              {labels[t]}
            </button>
          );
        })}
      </div>

      {/* Tab Content */}
      {tab === "leadership" && (
        <div className="space-y-4">
          <div className="grid grid-cols-1 gap-4 sm:grid-cols-3">
            <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4">
              <p className="text-sm text-zinc-400">平均任期</p>
              <p className="text-xl font-bold text-zinc-100">
                {metrics.avg_term_length_ticks.toFixed(1)} ticks
              </p>
            </div>
            <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4">
              <p className="text-sm text-zinc-400">投票参与率</p>
              <p className="text-xl font-bold text-zinc-100">
                {(metrics.avg_participation_rate * 100).toFixed(1)}%
              </p>
            </div>
            <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4">
              <p className="text-sm text-zinc-400">平均候选人数</p>
              <p className="text-xl font-bold text-zinc-100">
                {metrics.avg_candidate_count.toFixed(1)}
              </p>
            </div>
          </div>

          {/* Leadership change timeline */}
          <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4 space-y-3">
            <h3 className="text-sm font-semibold text-zinc-200">
              领导变更时间线
            </h3>
            {(groupedTimeline["leadership_changed"] ?? []).length === 0 ? (
              <p className="text-sm text-zinc-600">暂无领导变更记录</p>
            ) : (
              <div className="space-y-2">
                {(groupedTimeline["leadership_changed"] ?? [])
                  .slice(0, 20)
                  .map((event, idx) => (
                    <div
                      key={`${event.event_type}-${event.tick}-${idx}`}
                      className="flex items-center justify-between rounded-lg border border-zinc-800 bg-zinc-900/30 px-4 py-3"
                    >
                      <div className="flex items-center gap-3">
                        <span className="text-base">👑</span>
                        <p className="text-sm text-zinc-200">
                          {event.summary}
                        </p>
                      </div>
                      <span className="text-xs text-zinc-500 tabular-nums">
                        Tick #{event.tick}
                      </span>
                    </div>
                  ))}
              </div>
            )}
          </div>
        </div>
      )}

      {tab === "tax" && (
        <div className="space-y-4">
          <div className="grid grid-cols-1 gap-4 sm:grid-cols-3">
            <div className="rounded-xl border border-green-500/20 bg-green-500/5 p-4">
              <p className="text-sm text-zinc-400">国库余额</p>
              <p className="text-xl font-bold text-green-400">
                ${metrics.treasury_balance.toLocaleString()}
              </p>
            </div>
            <div className="rounded-xl border border-blue-500/20 bg-blue-500/5 p-4">
              <p className="text-sm text-zinc-400">征收次数</p>
              <p className="text-xl font-bold text-blue-400">
                {metrics.tax_collection_count}
              </p>
            </div>
            <div className="rounded-xl border border-amber-500/20 bg-amber-500/5 p-4">
              <p className="text-sm text-zinc-400">人均税收</p>
              <p className="text-xl font-bold text-amber-400">
                ${metrics.tax_per_member.toLocaleString()}
              </p>
            </div>
          </div>
        </div>
      )}

      {tab === "diplomacy" && (
        <div className="space-y-4">
          <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
            <div className="rounded-xl border border-emerald-500/20 bg-emerald-500/5 p-4">
              <p className="text-sm text-zinc-400">条约签署</p>
              <p className="text-xl font-bold text-emerald-400">
                {metrics.treaties_signed}
              </p>
            </div>
            <div className="rounded-xl border border-red-500/20 bg-red-500/5 p-4">
              <p className="text-sm text-zinc-400">条约撕毁</p>
              <p className="text-xl font-bold text-red-400">
                {metrics.treaties_broken}
              </p>
            </div>
          </div>

          {/* Treaty events */}
          <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4 space-y-3">
            <h3 className="text-sm font-semibold text-zinc-200">条约事件</h3>
            {[
              ...(groupedTimeline["treaty_signed"] ?? []),
              ...(groupedTimeline["treaty_broken"] ?? []),
            ]
              .sort((a, b) => b.tick - a.tick)
              .slice(0, 20)
              .length === 0 ? (
              <p className="text-sm text-zinc-600">暂无条约事件</p>
            ) : (
              <div className="space-y-2">
                {[
                  ...(groupedTimeline["treaty_signed"] ?? []),
                  ...(groupedTimeline["treaty_broken"] ?? []),
                ]
                  .sort((a, b) => b.tick - a.tick)
                  .slice(0, 20)
                  .map((event, idx) => {
                    const isBroken =
                      event.event_type === "treaty_broken";
                    const config = EVENT_TYPE_CONFIG[event.event_type];
                    return (
                      <div
                        key={`${event.event_type}-${event.tick}-${idx}`}
                        className="flex items-center justify-between rounded-lg border border-zinc-800 bg-zinc-900/30 px-4 py-3"
                      >
                        <div className="flex items-center gap-3">
                          <span className="text-base">
                            {config?.icon ?? (isBroken ? "💔" : "🤝")}
                          </span>
                          <p className="text-sm text-zinc-200">
                            {event.summary}
                          </p>
                        </div>
                        <span className="text-xs text-zinc-500 tabular-nums">
                          Tick #{event.tick}
                        </span>
                      </div>
                    );
                  })}
              </div>
            )}
          </div>
        </div>
      )}

      {tab === "legislation" && (
        <div className="space-y-4">
          {/* Legislation summary cards */}
          <div className="grid grid-cols-1 gap-4 sm:grid-cols-4">
            <div className="rounded-xl border border-sky-500/20 bg-sky-500/5 p-4">
              <p className="text-sm text-zinc-400">提案数</p>
              <p className="text-xl font-bold text-sky-400">
                {metrics.rules_proposed}
              </p>
            </div>
            <div className="rounded-xl border border-green-500/20 bg-green-500/5 p-4">
              <p className="text-sm text-zinc-400">已生效</p>
              <p className="text-xl font-bold text-green-400">
                {metrics.rules_activated}
              </p>
            </div>
            <div className="rounded-xl border border-zinc-500/20 bg-zinc-500/5 p-4">
              <p className="text-sm text-zinc-400">已过期</p>
              <p className="text-xl font-bold text-zinc-400">
                {metrics.rules_expired}
              </p>
            </div>
            <div className="rounded-xl border border-red-500/20 bg-red-500/5 p-4">
              <p className="text-sm text-zinc-400">已废除</p>
              <p className="text-xl font-bold text-red-400">
                {metrics.rules_repealed}
              </p>
            </div>
          </div>

          {/* Legislation pass/veto rate bar */}
          <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4 space-y-3">
            <h3 className="text-sm font-semibold text-zinc-200">
              法案通过/否决率
            </h3>
            {metrics.rules_proposed === 0 ? (
              <p className="text-sm text-zinc-600">暂无法案数据</p>
            ) : (
              <div className="space-y-2">
                <div className="flex items-center gap-2">
                  <span className="text-xs text-zinc-500 w-16">通过</span>
                  <div className="flex-1 h-3 rounded-full bg-zinc-800 overflow-hidden">
                    <div
                      className="h-full bg-green-500 rounded-full"
                      style={{
                        width: `${(metrics.rules_activated / metrics.rules_proposed) * 100}%`,
                      }}
                    />
                  </div>
                  <span className="text-xs text-zinc-400 tabular-nums w-12 text-right">
                    {metrics.rules_activated}/{metrics.rules_proposed}
                  </span>
                </div>
                <div className="flex items-center gap-2">
                  <span className="text-xs text-zinc-500 w-16">过期</span>
                  <div className="flex-1 h-3 rounded-full bg-zinc-800 overflow-hidden">
                    <div
                      className="h-full bg-zinc-500 rounded-full"
                      style={{
                        width: `${(metrics.rules_expired / metrics.rules_proposed) * 100}%`,
                      }}
                    />
                  </div>
                  <span className="text-xs text-zinc-400 tabular-nums w-12 text-right">
                    {metrics.rules_expired}/{metrics.rules_proposed}
                  </span>
                </div>
                <div className="flex items-center gap-2">
                  <span className="text-xs text-zinc-500 w-16">废除</span>
                  <div className="flex-1 h-3 rounded-full bg-zinc-800 overflow-hidden">
                    <div
                      className="h-full bg-red-500 rounded-full"
                      style={{
                        width: `${(metrics.rules_repealed / metrics.rules_proposed) * 100}%`,
                      }}
                    />
                  </div>
                  <span className="text-xs text-zinc-400 tabular-nums w-12 text-right">
                    {metrics.rules_repealed}/{metrics.rules_proposed}
                  </span>
                </div>
              </div>
            )}
          </div>

          {/* Legislation events timeline */}
          <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4 space-y-3">
            <h3 className="text-sm font-semibold text-zinc-200">
              法案事件记录
            </h3>
            {[
              ...(groupedTimeline["soft_rule_proposed"] ?? []),
              ...(groupedTimeline["soft_rule_activated"] ?? []),
              ...(groupedTimeline["soft_rule_expired"] ?? []),
              ...(groupedTimeline["soft_rule_repealed"] ?? []),
            ]
              .sort((a, b) => b.tick - a.tick)
              .slice(0, 30)
              .length === 0 ? (
              <p className="text-sm text-zinc-600">暂无法案事件</p>
            ) : (
              <div className="space-y-2">
                {[
                  ...(groupedTimeline["soft_rule_proposed"] ?? []),
                  ...(groupedTimeline["soft_rule_activated"] ?? []),
                  ...(groupedTimeline["soft_rule_expired"] ?? []),
                  ...(groupedTimeline["soft_rule_repealed"] ?? []),
                ]
                  .sort((a, b) => b.tick - a.tick)
                  .slice(0, 30)
                  .map((event, idx) => {
                    const config = EVENT_TYPE_CONFIG[event.event_type];
                    return (
                      <div
                        key={`${event.event_type}-${event.tick}-${idx}`}
                        className="flex items-center justify-between rounded-lg border border-zinc-800 bg-zinc-900/30 px-4 py-3"
                      >
                        <div className="flex items-center gap-3 min-w-0">
                          <span className="text-base">
                            {config?.icon ?? "📜"}
                          </span>
                          <div className="min-w-0">
                            <p className="text-sm text-zinc-200 truncate">
                              {event.summary}
                            </p>
                            <p className="text-[10px] text-zinc-500">
                              {eventTypeLabels[event.event_type] ??
                                event.event_type}
                            </p>
                          </div>
                        </div>
                        <span className="text-xs text-zinc-500 tabular-nums shrink-0 ml-3">
                          Tick #{event.tick}
                        </span>
                      </div>
                    );
                  })}
              </div>
            )}
          </div>
        </div>
      )}

      {tab === "timeline" && (
        <div className="space-y-4">
          {/* Governance activity time series chart */}
          {activityTimeSeries.length > 0 && (
            <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4 space-y-3">
              <h3 className="text-sm font-semibold text-zinc-200">
                治理活跃度时间序列
              </h3>
              <ResponsiveContainer width="100%" height={200}>
                <BarChart data={activityTimeSeries}>
                  <CartesianGrid strokeDasharray="3 3" stroke="#27272a" />
                  <XAxis
                    dataKey="tick"
                    stroke="#52525b"
                    tick={{ fontSize: 9 }}
                  />
                  <YAxis stroke="#52525b" tick={{ fontSize: 9 }} />
                  <Tooltip content={<CustomTooltip />} />
                  <Bar
                    dataKey="tax_collected"
                    stackId="events"
                    fill="#f97316"
                    name="税收"
                  />
                  <Bar
                    dataKey="leadership_changed"
                    stackId="events"
                    fill="#3b82f6"
                    name="领导变更"
                  />
                  <Bar
                    dataKey="treaty_signed"
                    stackId="events"
                    fill="#22c55e"
                    name="条约签署"
                  />
                  <Bar
                    dataKey="soft_rule_proposed"
                    stackId="events"
                    fill="#0ea5e9"
                    name="法案提议"
                  />
                  <Bar
                    dataKey="soft_rule_activated"
                    stackId="events"
                    fill="#10b981"
                    name="法案生效"
                  />
                </BarChart>
              </ResponsiveContainer>
            </div>
          )}

          {/* Event stream */}
          <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4 space-y-3">
            <h3 className="text-sm font-semibold text-zinc-200">
              治理事件时间线
            </h3>
            {timelineEvents.length === 0 ? (
              <p className="text-sm text-zinc-600">暂无治理事件</p>
            ) : (
              <div className="space-y-2">
                {timelineEvents.slice(0, 50).map((event, idx) => {
                  const config = EVENT_TYPE_CONFIG[event.event_type];
                  return (
                    <div
                      key={`${event.event_type}-${event.tick}-${idx}`}
                      className="flex items-center justify-between rounded-lg border border-zinc-800 bg-zinc-900/30 px-4 py-3"
                    >
                      <div className="flex items-center gap-3 min-w-0">
                        <span
                          className={`shrink-0 h-2 w-2 rounded-full ${
                            config?.dot ?? "bg-zinc-400"
                          }`}
                        />
                        <div className="min-w-0">
                          <p className="text-sm text-zinc-200 truncate">
                            {event.summary}
                          </p>
                          <p className="text-[10px] text-zinc-500">
                            {eventTypeLabels[event.event_type] ??
                              event.event_type}
                          </p>
                        </div>
                      </div>
                      <span className="text-xs text-zinc-500 tabular-nums shrink-0 ml-3">
                        Tick #{event.tick}
                      </span>
                    </div>
                  );
                })}
              </div>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
