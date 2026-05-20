"use client";

import { useState, useMemo, useEffect } from "react";
import { useParams, useRouter } from "next/navigation";
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
};

export default function GovernanceOrgDetailPage() {
  const params = useParams();
  const router = useRouter();
  const orgId = params.orgId as string;

  const { metrics, loading, error } = useGovernanceOrg(orgId);
  const { events: timelineEvents } = useGovernanceTimeline(orgId);
  const [orgName, setOrgName] = useState<string>(orgId);
  const [tab, setTab] = useState<
    "leadership" | "tax" | "diplomacy" | "timeline"
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

      {/* Stats Cards */}
      <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 xl:grid-cols-4">
        <div className="rounded-xl border border-blue-500/20 bg-blue-500/5 p-4 space-y-1">
          <p className="text-sm text-zinc-400">选举次数</p>
          <p className="text-2xl font-bold text-blue-400">
            {metrics.election_count}
          </p>
          <p className="text-xs text-zinc-500">
            平均 {metrics.avg_candidate_count.toFixed(1)} 名候选人
          </p>
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
        <div className="rounded-xl border border-purple-500/20 bg-purple-500/5 p-4 space-y-1">
          <p className="text-sm text-zinc-400">外交关系</p>
          <p className="text-2xl font-bold text-purple-400">
            {metrics.active_relations_count}
          </p>
          <p className="text-xs text-zinc-500">活跃外交伙伴</p>
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

      {tab === "timeline" && (
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
      )}
    </div>
  );
}
