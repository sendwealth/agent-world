"use client";

import { useEffect, useState, useMemo, useCallback } from "react";
import { useParams, useRouter } from "next/navigation";
import Link from "next/link";
import type { Organization } from "@/types/world";
import { fetchJSON } from "@/lib/api";
import { useSSEContext } from "@/components/SSEProvider";

const orgTypeLabels: Record<string, string> = {
  company: "公司",
  guild: "公会",
  alliance: "联盟",
  university: "大学",
};

const roleLabels: Record<string, string> = {
  founder: "创始人",
  leader: "领导者",
  member: "成员",
};

const roleColors: Record<string, string> = {
  founder: "bg-amber-500/10 text-amber-400",
  leader: "bg-blue-500/10 text-blue-400",
  member: "bg-zinc-700/50 text-zinc-400",
};

export default function OrganizationDetailPage() {
  const params = useParams();
  const router = useRouter();
  const orgId = params.id as string;

  const [org, setOrg] = useState<Organization | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [tab, setTab] = useState<"members" | "finance" | "history">("members");

  const sse = useSSEContext();

  const loadOrg = useCallback(async () => {
    try {
      const data = await fetchJSON<Organization>(`/api/v1/orgs/${orgId}`);
      setOrg(data);
      setError(null);
    } catch {
      setError("无法加载组织数据");
    } finally {
      setLoading(false);
    }
  }, [orgId]);

  useEffect(() => {
    (async () => {
      await loadOrg();
    })();
    const interval = setInterval(loadOrg, 10000);
    return () => clearInterval(interval);
  }, [loadOrg]);

  useEffect(() => {
    function onEvent() {
      loadOrg();
    }
    const unsubscribe = sse.subscribe(onEvent);
    return unsubscribe;
  }, [sse, loadOrg]);

  const sortedMembers = useMemo(() => {
    if (!org) return [];
    const roleOrder: Record<string, number> = { founder: 0, leader: 1, member: 2 };
    return [...org.members].sort(
      (a, b) => (roleOrder[a.role] ?? 3) - (roleOrder[b.role] ?? 3)
    );
  }, [org]);

  // Simulated profit data based on treasury history
  const profitData = useMemo(() => {
    if (!org) return [];
    const data = [];
    for (let i = 10; i >= 0; i--) {
      const tick = org.last_activity_tick - i * 5;
      const step = (10 - i);
      const treasuryDelta = step * ((org.treasury * 0.02) + (step * org.member_count * 7));
      const memberDelta = Math.min(step, Math.floor(step * 0.3));
      data.push({
        tick,
        treasury: Math.max(0, org.treasury - Math.floor(treasuryDelta)),
        members: Math.max(1, org.member_count - memberDelta),
      });
    }
    // Ensure last entry matches current state
    data[data.length - 1] = {
      tick: org.last_activity_tick,
      treasury: org.treasury,
      members: org.member_count,
    };
    return data;
  }, [org]);

  // Generate proposal history from key events (placeholder)
  const proposals = useMemo(() => {
    if (!org) return [];
    return [
      {
        id: "p1",
        title: "成立组织",
        proposer: org.members.find((m) => m.role === "founder")?.agent_name ?? "未知",
        status: "passed",
        tick: org.created_tick,
      },
      {
        id: "p2",
        title: "调整利润分配",
        proposer: org.members.find((m) => m.role === "leader")?.agent_name ?? "未知",
        status: org.member_count > 2 ? "passed" : "pending",
        tick: org.created_tick + 10,
      },
      {
        id: "p3",
        title: "成员扩张计划",
        proposer: org.members[0]?.agent_name ?? "未知",
        status: org.member_count > 3 ? "passed" : "pending",
        tick: org.last_activity_tick - 5,
      },
    ];
  }, [org]);

  if (loading) {
    return (
      <div className="flex h-full items-center justify-center">
        <div className="text-sm text-zinc-600">正在加载组织数据...</div>
      </div>
    );
  }

  if (error || !org) {
    return (
      <div className="p-4 md:p-6 space-y-4">
        <button
          onClick={() => router.back()}
          className="text-sm text-zinc-400 hover:text-zinc-200 transition-colors"
        >
          &larr; 返回
        </button>
        <div className="rounded-lg bg-red-500/10 border border-red-500/20 px-4 py-3 text-sm text-red-400">
          {error ?? "组织不存在"}
        </div>
      </div>
    );
  }

  return (
    <div className="p-4 md:p-6 space-y-6">
      {/* Header */}
      <div className="flex items-center gap-2 sm:gap-4">
        <button
          onClick={() => router.push("/organizations")}
          className="rounded-lg p-1.5 text-zinc-400 hover:bg-zinc-800 hover:text-zinc-200 transition-colors"
        >
          <svg className="h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M15 19l-7-7 7-7" />
          </svg>
        </button>
        <div>
          <div className="flex flex-wrap items-center gap-2 sm:gap-3">
            <h1 className="text-xl sm:text-2xl font-bold text-zinc-100">{org.name}</h1>
            <span className="rounded-full bg-blue-500/10 border border-blue-500/20 px-2 py-0.5 text-[10px] font-medium text-blue-400">
              {orgTypeLabels[org.type] ?? org.type}
            </span>
            <span className={`rounded-full px-2 py-0.5 text-[10px] font-medium ${
              org.status === "active" ? "bg-green-500/10 text-green-400" : "bg-red-500/10 text-red-400"
            }`}>
              {org.status === "active" ? "活跃" : org.status === "inactive" ? "不活跃" : "已解散"}
            </span>
          </div>
          <p className="text-sm text-zinc-500">
            创建于 Tick #{org.created_tick} · 最后活动 Tick #{org.last_activity_tick}
          </p>
        </div>
      </div>

      {/* Stats Cards */}
      <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 xl:grid-cols-4">
        <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4 space-y-1">
          <p className="text-sm text-zinc-400">成员数量</p>
          <p className="text-2xl font-bold text-zinc-100">{org.member_count}</p>
        </div>
        <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4 space-y-1">
          <p className="text-sm text-zinc-400">国库资金</p>
          <p className="text-2xl font-bold text-zinc-100">${org.treasury.toLocaleString()}</p>
        </div>
        <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4 space-y-1">
          <p className="text-sm text-zinc-400">负债</p>
          <p className="text-2xl font-bold text-red-400">${org.debts.toLocaleString()}</p>
        </div>
        <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4 space-y-1">
          <p className="text-sm text-zinc-400">净资产</p>
          <p className="text-2xl font-bold text-emerald-400">${(org.treasury - org.debts).toLocaleString()}</p>
        </div>
      </div>

      {/* Tab Navigation */}
      <div className="flex items-center gap-1 border-b border-zinc-800">
        {(["members", "finance", "history"] as const).map((t) => {
          const labels = { members: "成员列表", finance: "财务报表", history: "提案历史" };
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
      {tab === "members" && (
        <div className="rounded-xl border border-zinc-800 bg-zinc-900/50">
          <div className="overflow-x-auto">
            <table className="w-full">
              <thead>
                <tr className="border-b border-zinc-800">
                  <th className="px-4 py-3 text-left text-xs font-semibold text-zinc-400">名称</th>
                  <th className="px-4 py-3 text-left text-xs font-semibold text-zinc-400">角色</th>
                  <th className="px-4 py-3 text-right text-xs font-semibold text-zinc-400">股份</th>
                  <th className="px-4 py-3 text-right text-xs font-semibold text-zinc-400">加入时间</th>
                </tr>
              </thead>
              <tbody>
                {sortedMembers.map((m) => (
                  <tr key={m.agent_id} className="border-b border-zinc-800/50 last:border-0">
                    <td className="px-4 py-3">
                      <Link
                        href={`/agents/${m.agent_id}`}
                        className="text-sm font-medium text-zinc-200 hover:text-blue-400 transition-colors"
                      >
                        {m.agent_name}
                      </Link>
                    </td>
                    <td className="px-4 py-3">
                      <span className={`rounded-full px-2 py-0.5 text-[10px] font-medium ${roleColors[m.role] ?? ""}`}>
                        {roleLabels[m.role] ?? m.role}
                      </span>
                    </td>
                    <td className="px-4 py-3 text-right text-sm text-zinc-300 tabular-nums">
                      {(m.share * 100).toFixed(1)}%
                    </td>
                    <td className="px-4 py-3 text-right text-sm text-zinc-500 tabular-nums">
                      Tick #{m.joined_tick}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      )}

      {tab === "finance" && (
        <div className="space-y-4">
          {/* Treasury trend chart */}
          <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4 space-y-3">
            <h3 className="text-sm font-semibold text-zinc-200">国库走势</h3>
            <div className="h-48 flex items-end gap-1">
              {profitData.map((d, i) => {
                const maxTreasury = Math.max(...profitData.map((p) => p.treasury), 1);
                const height = (d.treasury / maxTreasury) * 100;
                return (
                  <div key={i} className="flex-1 flex flex-col items-center gap-1">
                    <div
                      className="w-full rounded-t bg-gradient-to-t from-blue-600 to-blue-400 transition-all duration-500 min-h-[2px]"
                      style={{ height: `${Math.max(height, 1)}%` }}
                      title={`Tick #${d.tick}: $${d.treasury.toLocaleString()}`}
                    />
                    {i % 3 === 0 && (
                      <span className="text-[8px] text-zinc-600">#{d.tick}</span>
                    )}
                  </div>
                );
              })}
            </div>
          </div>

          {/* Financial summary */}
          <div className="grid grid-cols-1 gap-4 sm:grid-cols-3">
            <div className="rounded-xl border border-green-500/20 bg-green-500/5 p-4">
              <p className="text-sm text-zinc-400">总收入</p>
              <p className="text-xl font-bold text-green-400">${Math.floor(org.treasury * 0.7).toLocaleString()}</p>
            </div>
            <div className="rounded-xl border border-red-500/20 bg-red-500/5 p-4">
              <p className="text-sm text-zinc-400">总支出</p>
              <p className="text-xl font-bold text-red-400">${Math.floor(org.treasury * 0.3 + org.debts).toLocaleString()}</p>
            </div>
            <div className="rounded-xl border border-blue-500/20 bg-blue-500/5 p-4">
              <p className="text-sm text-zinc-400">人均资产</p>
              <p className="text-xl font-bold text-blue-400">
                ${org.member_count > 0 ? Math.floor(org.treasury / org.member_count).toLocaleString() : 0}
              </p>
            </div>
          </div>
        </div>
      )}

      {tab === "history" && (
        <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4 space-y-3">
          <h3 className="text-sm font-semibold text-zinc-200">提案记录</h3>
          {proposals.length === 0 ? (
            <p className="text-sm text-zinc-600">暂无提案记录</p>
          ) : (
            <div className="space-y-2">
              {proposals.map((p) => (
                <div
                  key={p.id}
                  className="flex items-center justify-between rounded-lg border border-zinc-800 bg-zinc-900/30 px-4 py-3"
                >
                  <div className="space-y-0.5">
                    <p className="text-sm text-zinc-200">{p.title}</p>
                    <p className="text-xs text-zinc-500">提案人: {p.proposer} · Tick #{p.tick}</p>
                  </div>
                  <span
                    className={`rounded-full px-2 py-0.5 text-[10px] font-medium ${
                      p.status === "passed"
                        ? "bg-green-500/10 text-green-400"
                        : p.status === "pending"
                        ? "bg-amber-500/10 text-amber-400"
                        : "bg-red-500/10 text-red-400"
                    }`}
                  >
                    {p.status === "passed" ? "已通过" : p.status === "pending" ? "待投票" : "已否决"}
                  </span>
                </div>
              ))}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
