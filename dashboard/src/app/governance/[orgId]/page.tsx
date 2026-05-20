"use client";

import { useState, useMemo } from "react";
import { useParams, useRouter } from "next/navigation";
import {
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ResponsiveContainer,
  BarChart,
  Bar,
} from "recharts";
import {
  useGovernanceOrg,
  useGovernanceTimeline,
} from "@/hooks/useGovernanceStream";
import { EVENT_TYPE_CONFIG } from "@/lib/event-types";

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

const eventTypeLabels: Record<string, string> = {
  tax_collected: "税收征收",
  treasury_distributed: "国库分配",
  leadership_election_started: "选举开始",
  leadership_changed: "领导更替",
  treaty_proposed: "条约提议",
  treaty_signed: "条约签署",
  treaty_broken: "条约撕毁",
  relation_changed: "关系变化",
};

const relationLevelLabels: Record<number, string> = {
  "-3": "敌对",
  "-2": "紧张",
  "-1": "冷淡",
  0: "中立",
  1: "友好",
  2: "亲密",
  3: "盟友",
};

export default function GovernanceOrgDetailPage() {
  const params = useParams();
  const router = useRouter();
  const orgId = params.orgId as string;

  const { metrics, loading, error } = useGovernanceOrg(orgId);
  const { events: timelineEvents } = useGovernanceTimeline(orgId);
  const [tab, setTab] = useState<
    "leadership" | "tax" | "diplomacy" | "timeline"
  >("leadership");

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

  // Tax distribution strategy chart data
  const taxStrategyData = useMemo(() => {
    if (!metrics) return [];
    return Object.entries(metrics.tax.distribution_strategies ?? {}).map(
      ([strategy, count]) => ({
        name: strategy,
        count,
      })
    );
  }, [metrics]);

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
              {metrics.org_name}
            </h1>
            <span className="rounded-full bg-purple-500/10 border border-purple-500/20 px-2 py-0.5 text-[10px] font-medium text-purple-400">
              治理详情
            </span>
          </div>
          <p className="text-sm text-zinc-500">
            稳定性 {(metrics.health.stability_score * 100).toFixed(0)}% ·{" "}
            {metrics.health.member_count} 成员
          </p>
        </div>
      </div>

      {/* Stats Cards */}
      <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 xl:grid-cols-4">
        <div className="rounded-xl border border-blue-500/20 bg-blue-500/5 p-4 space-y-1">
          <p className="text-sm text-zinc-400">选举次数</p>
          <p className="text-2xl font-bold text-blue-400">
            {metrics.election.election_count}
          </p>
          <p className="text-xs text-zinc-500">
            平均 {metrics.election.avg_candidates.toFixed(1)} 名候选人
          </p>
        </div>
        <div className="rounded-xl border border-orange-500/20 bg-orange-500/5 p-4 space-y-1">
          <p className="text-sm text-zinc-400">税收总额</p>
          <p className="text-2xl font-bold text-orange-400">
            ${metrics.tax.total_collected.toLocaleString()}
          </p>
          <p className="text-xs text-zinc-500">
            人均 ${metrics.tax.per_capita.toLocaleString()}
          </p>
        </div>
        <div className="rounded-xl border border-emerald-500/20 bg-emerald-500/5 p-4 space-y-1">
          <p className="text-sm text-zinc-400">条约签署</p>
          <p className="text-2xl font-bold text-emerald-400">
            {metrics.diplomacy.treaties_signed}
          </p>
          <p className="text-xs text-zinc-500">
            撕毁 {metrics.diplomacy.treaties_broken}
          </p>
        </div>
        <div className="rounded-xl border border-purple-500/20 bg-purple-500/5 p-4 space-y-1">
          <p className="text-sm text-zinc-400">外交活跃度</p>
          <p className="text-2xl font-bold text-purple-400">
            {metrics.diplomacy.diplomatic_activity}
          </p>
          <p className="text-xs text-zinc-500">外交事件总数</p>
        </div>
      </div>

      {/* Tab Navigation */}
      <div className="flex items-center gap-1 border-b border-zinc-800">
        {(
          ["leadership", "tax", "diplomacy", "timeline"] as const
        ).map((t) => {
          const labels = {
            leadership: "领导变更",
            tax: "税收趋势",
            diplomacy: "外交关系",
            timeline: "事件流",
          };
          return (
            <button
              key={t}
              onClick={() => setTab(t)}
              className={`px-4 py-2.5 text-sm font-medium transition-colors border-b-2 ${
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
                {metrics.election.avg_term_length.toFixed(1)} ticks
              </p>
            </div>
            <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4">
              <p className="text-sm text-zinc-400">投票参与率</p>
              <p className="text-xl font-bold text-zinc-100">
                {(metrics.election.avg_voter_participation * 100).toFixed(1)}%
              </p>
            </div>
            <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4">
              <p className="text-sm text-zinc-400">平均候选人数</p>
              <p className="text-xl font-bold text-zinc-100">
                {metrics.election.avg_candidates.toFixed(1)}
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
                  .map((event) => (
                    <div
                      key={event.id}
                      className="flex items-center justify-between rounded-lg border border-zinc-800 bg-zinc-900/30 px-4 py-3"
                    >
                      <div className="flex items-center gap-3">
                        <span className="text-base">👑</span>
                        <p className="text-sm text-zinc-200">
                          {event.description}
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
                ${metrics.tax.treasury_balance.toLocaleString()}
              </p>
            </div>
            <div className="rounded-xl border border-blue-500/20 bg-blue-500/5 p-4">
              <p className="text-sm text-zinc-400">征收次数</p>
              <p className="text-xl font-bold text-blue-400">
                {metrics.tax.collection_count}
              </p>
            </div>
            <div className="rounded-xl border border-amber-500/20 bg-amber-500/5 p-4">
              <p className="text-sm text-zinc-400">人均税收</p>
              <p className="text-xl font-bold text-amber-400">
                ${metrics.tax.per_capita.toLocaleString()}
              </p>
            </div>
          </div>

          {/* Distribution Strategy Chart */}
          <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4 space-y-3">
            <h3 className="text-sm font-semibold text-zinc-200">
              分配策略使用分布
            </h3>
            {taxStrategyData.length === 0 ? (
              <p className="text-sm text-zinc-600 h-48 flex items-center justify-center">
                暂无数据
              </p>
            ) : (
              <ResponsiveContainer width="100%" height={240}>
                <BarChart data={taxStrategyData}>
                  <CartesianGrid strokeDasharray="3 3" stroke="#27272a" />
                  <XAxis
                    dataKey="name"
                    stroke="#52525b"
                    tick={{ fontSize: 10 }}
                  />
                  <YAxis stroke="#52525b" tick={{ fontSize: 10 }} />
                  <Tooltip content={<CustomTooltip />} />
                  <Bar
                    dataKey="count"
                    fill="#f97316"
                    name="使用次数"
                    radius={[4, 4, 0, 0]}
                  />
                </BarChart>
              </ResponsiveContainer>
            )}
          </div>
        </div>
      )}

      {tab === "diplomacy" && (
        <div className="space-y-4">
          <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4 space-y-3">
            <h3 className="text-sm font-semibold text-zinc-200">关系等级分布</h3>
            {Object.keys(metrics.diplomacy.relation_levels ?? {}).length ===
            0 ? (
              <p className="text-sm text-zinc-600">暂无外交关系数据</p>
            ) : (
              <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
                {Object.entries(metrics.diplomacy.relation_levels ?? {}).map(
                  ([level, count]) => {
                    const levelNum = parseInt(level, 10);
                    const label =
                      relationLevelLabels[levelNum] ?? `等级 ${level}`;
                    const colors: Record<string, string> = {
                      "-3": "text-red-400 bg-red-500/10 border-red-500/20",
                      "-2": "text-orange-400 bg-orange-500/10 border-orange-500/20",
                      "-1": "text-amber-400 bg-amber-500/10 border-amber-500/20",
                      "0": "text-zinc-400 bg-zinc-500/10 border-zinc-500/20",
                      "1": "text-cyan-400 bg-cyan-500/10 border-cyan-500/20",
                      "2": "text-green-400 bg-green-500/10 border-green-500/20",
                      "3": "text-emerald-400 bg-emerald-500/10 border-emerald-500/20",
                    };
                    const colorClass =
                      colors[level] ?? "text-zinc-400 bg-zinc-500/10 border-zinc-500/20";
                    return (
                      <div
                        key={level}
                        className={`rounded-lg border px-4 py-3 ${colorClass}`}
                      >
                        <div className="flex items-center justify-between">
                          <span className="text-sm font-medium">{label}</span>
                          <span className="text-lg font-bold">{count}</span>
                        </div>
                      </div>
                    );
                  }
                )}
              </div>
            )}
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
                  .map((event) => {
                    const isBroken =
                      event.event_type === "treaty_broken";
                    const config = EVENT_TYPE_CONFIG[event.event_type];
                    return (
                      <div
                        key={event.id}
                        className="flex items-center justify-between rounded-lg border border-zinc-800 bg-zinc-900/30 px-4 py-3"
                      >
                        <div className="flex items-center gap-3">
                          <span className="text-base">
                            {config?.icon ?? (isBroken ? "💔" : "🤝")}
                          </span>
                          <p className="text-sm text-zinc-200">
                            {event.description}
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

      {tab === "timeline" && (
        <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4 space-y-3">
          <h3 className="text-sm font-semibold text-zinc-200">
            治理事件时间线
          </h3>
          {timelineEvents.length === 0 ? (
            <p className="text-sm text-zinc-600">暂无治理事件</p>
          ) : (
            <div className="space-y-2">
              {timelineEvents.slice(0, 50).map((event) => {
                const config = EVENT_TYPE_CONFIG[event.event_type];
                return (
                  <div
                    key={event.id}
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
                          {event.description}
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
      )}
    </div>
  );
}
