"use client";

import { useEffect, useState, useCallback } from "react";
import { fetchJSON } from "@/lib/api";
import { useSSEContext } from "@/components/SSEProvider";
import type {
  FederatedWorld,
  CrossWorldTreaty,
  FederationSummary,
  WorldEvent,
} from "@/types/world";

const TREATY_TYPE_LABELS: Record<string, { label: string; color: string }> = {
  trade: { label: "贸易", color: "text-green-400" },
  non_aggression: { label: "互不侵犯", color: "text-blue-400" },
  alliance: { label: "同盟", color: "text-purple-400" },
  cultural: { label: "文化", color: "text-amber-400" },
  technology: { label: "科技", color: "text-cyan-400" },
  migration: { label: "移民", color: "text-pink-400" },
};

const TREATY_STATUS_LABELS: Record<string, { label: string; color: string }> = {
  proposed: { label: "待批准", color: "text-amber-400" },
  active: { label: "生效中", color: "text-green-400" },
  rejected: { label: "已拒绝", color: "text-red-400" },
  broken: { label: "已撕毁", color: "text-red-500" },
  expired: { label: "已过期", color: "text-zinc-500" },
};

const RELATION_LABELS: Record<string, { label: string; color: string }> = {
  neutral: { label: "中立", color: "text-zinc-400" },
  friendly: { label: "友好", color: "text-green-400" },
  hostile: { label: "敌对", color: "text-red-400" },
  allied: { label: "结盟", color: "text-blue-400" },
  at_war: { label: "交战", color: "text-red-500" },
};

export default function DiplomacyPage() {
  const [worlds, setWorlds] = useState<FederatedWorld[]>([]);
  const [treaties, setTreaties] = useState<CrossWorldTreaty[]>([]);
  const [summary, setSummary] = useState<FederationSummary | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [tab, setTab] = useState<"worlds" | "treaties">("worlds");
  const [statusFilter, setStatusFilter] = useState<string>("all");

  const sse = useSSEContext();

  const loadData = useCallback(async () => {
    try {
      const [worldsData, treatiesData, summaryData] = await Promise.all([
        fetchJSON<FederatedWorld[]>("/api/v1/federation/worlds").catch(() => []),
        fetchJSON<CrossWorldTreaty[]>("/api/v1/federation/treaties").catch(() => []),
        fetchJSON<FederationSummary>("/api/v1/federation/summary").catch(() => null),
      ]);
      setWorlds(worldsData);
      setTreaties(treatiesData);
      setSummary(summaryData);
      setError(null);
    } catch {
      setError("无法加载外交数据");
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    /* eslint-disable react-hooks/set-state-in-effect */
    loadData();
    const interval = setInterval(loadData, 10000);
    return () => clearInterval(interval);
  }, [loadData]);

  useEffect(() => {
    function onEvent(event: WorldEvent) {
      if (
        event.type === "treaty_proposed" ||
        event.type === "treaty_signed" ||
        event.type === "treaty_broken" ||
        event.type === "relation_changed"
      ) {
        loadData();
      }
    }
    const unsubscribe = sse.subscribe(onEvent);
    return unsubscribe;
  }, [sse, loadData]);

  const filteredTreaties = treaties.filter(
    (t) => statusFilter === "all" || t.status === statusFilter
  );

  if (loading) {
    return (
      <div className="flex h-full items-center justify-center">
        <div className="text-sm text-zinc-600">正在加载外交数据...</div>
      </div>
    );
  }

  return (
    <div className="p-4 md:p-6 space-y-6">
      <div>
        <h1 className="text-xl md:text-2xl font-bold text-zinc-100">联邦外交</h1>
        <p className="text-sm text-zinc-500">
          {summary
            ? `${summary.total_worlds} 个世界 · ${summary.active_treaties} 个活跃条约 · ${summary.wars} 场战争`
            : "跨世界外交关系管理"}
        </p>
      </div>

      {error && (
        <div className="rounded-lg bg-red-500/10 border border-red-500/20 px-3 py-1.5 text-xs text-red-400">
          {error}
        </div>
      )}

      {/* Key Indicators */}
      <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 xl:grid-cols-4">
        <div className="rounded-xl border border-blue-500/20 bg-blue-500/5 p-4 space-y-1">
          <p className="text-sm text-zinc-400">已注册世界</p>
          <p className="text-2xl font-bold text-blue-400">{summary?.total_worlds ?? worlds.length}</p>
        </div>
        <div className="rounded-xl border border-green-500/20 bg-green-500/5 p-4 space-y-1">
          <p className="text-sm text-zinc-400">活跃条约</p>
          <p className="text-2xl font-bold text-green-400">{summary?.active_treaties ?? 0}</p>
        </div>
        <div className="rounded-xl border border-amber-500/20 bg-amber-500/5 p-4 space-y-1">
          <p className="text-sm text-zinc-400">待处理条约</p>
          <p className="text-2xl font-bold text-amber-400">{summary?.pending_treaties ?? 0}</p>
        </div>
        <div className="rounded-xl border border-red-500/20 bg-red-500/5 p-4 space-y-1">
          <p className="text-sm text-zinc-400">活跃战争</p>
          <p className="text-2xl font-bold text-red-400">{summary?.wars ?? 0}</p>
        </div>
      </div>

      {/* Tab Switcher */}
      <div className="flex items-center gap-2">
        {(["worlds", "treaties"] as const).map((t) => (
          <button
            key={t}
            onClick={() => setTab(t)}
            className={`rounded-lg px-3 py-1 text-xs font-medium transition-colors ${
              tab === t
                ? "bg-blue-500/15 text-blue-400 border border-blue-500/30"
                : "bg-zinc-800/50 text-zinc-400 border border-zinc-800 hover:bg-zinc-800"
            }`}
          >
            {t === "worlds" ? "世界列表" : "条约列表"}
          </button>
        ))}
      </div>

      {/* Worlds Tab */}
      {tab === "worlds" && (
        <>
          {worlds.length === 0 ? (
            <div className="flex h-48 items-center justify-center text-sm text-zinc-600">
              暂无已注册的联邦世界
            </div>
          ) : (
            <div className="rounded-xl border border-zinc-800 bg-zinc-900/50">
              <div className="overflow-x-auto">
                <table className="w-full">
                  <thead>
                    <tr className="border-b border-zinc-800">
                      <th className="px-4 py-3 text-left text-xs font-semibold text-zinc-400">世界 ID</th>
                      <th className="px-4 py-3 text-left text-xs font-semibold text-zinc-400">名称</th>
                      <th className="px-4 py-3 text-left text-xs font-semibold text-zinc-400">端点</th>
                      <th className="px-4 py-3 text-left text-xs font-semibold text-zinc-400">关系</th>
                      <th className="px-4 py-3 text-right text-xs font-semibold text-zinc-400">注册 Tick</th>
                    </tr>
                  </thead>
                  <tbody>
                    {worlds.map((world) => {
                      const relation = world.relation_status
                        ? RELATION_LABELS[world.relation_status] ?? { label: world.relation_status, color: "text-zinc-400" }
                        : { label: "—", color: "text-zinc-600" };
                      return (
                        <tr
                          key={world.id}
                          className="border-b border-zinc-800/50 last:border-0 hover:bg-zinc-800/30 transition-colors"
                        >
                          <td className="px-4 py-3 text-sm font-mono text-zinc-300">{world.id.slice(0, 8)}</td>
                          <td className="px-4 py-3 text-sm text-zinc-200 font-medium">{world.name}</td>
                          <td className="px-4 py-3 text-sm text-zinc-400">{world.endpoint}</td>
                          <td className={`px-4 py-3 text-sm font-medium ${relation.color}`}>{relation.label}</td>
                          <td className="px-4 py-3 text-right text-sm text-zinc-400">#{world.registered_tick}</td>
                        </tr>
                      );
                    })}
                  </tbody>
                </table>
              </div>
            </div>
          )}
        </>
      )}

      {/* Treaties Tab */}
      {tab === "treaties" && (
        <>
          {/* Status Filter */}
          <div className="flex items-center gap-2 flex-wrap">
            <span className="text-xs text-zinc-500">状态:</span>
            {["all", "proposed", "active", "rejected", "broken", "expired"].map((s) => (
              <button
                key={s}
                onClick={() => setStatusFilter(s)}
                className={`rounded-lg px-3 py-1 text-xs font-medium transition-colors ${
                  statusFilter === s
                    ? "bg-blue-500/15 text-blue-400 border border-blue-500/30"
                    : "bg-zinc-800/50 text-zinc-400 border border-zinc-800 hover:bg-zinc-800"
                }`}
              >
                {s === "all" ? "全部" : TREATY_STATUS_LABELS[s]?.label ?? s}
              </button>
            ))}
          </div>

          {filteredTreaties.length === 0 ? (
            <div className="flex h-48 items-center justify-center text-sm text-zinc-600">
              暂无条约数据
            </div>
          ) : (
            <div className="rounded-xl border border-zinc-800 bg-zinc-900/50">
              <div className="overflow-x-auto">
                <table className="w-full">
                  <thead>
                    <tr className="border-b border-zinc-800">
                      <th className="px-4 py-3 text-left text-xs font-semibold text-zinc-400">条约 ID</th>
                      <th className="px-4 py-3 text-left text-xs font-semibold text-zinc-400">类型</th>
                      <th className="px-4 py-3 text-left text-xs font-semibold text-zinc-400">发起方</th>
                      <th className="px-4 py-3 text-left text-xs font-semibold text-zinc-400">目标方</th>
                      <th className="px-4 py-3 text-left text-xs font-semibold text-zinc-400">状态</th>
                      <th className="px-4 py-3 text-left text-xs font-semibold text-zinc-400">条款</th>
                      <th className="px-4 py-3 text-right text-xs font-semibold text-zinc-400">持续 Tick</th>
                    </tr>
                  </thead>
                  <tbody>
                    {filteredTreaties.map((treaty) => {
                      const typeInfo = TREATY_TYPE_LABELS[treaty.treaty_type] ?? { label: treaty.treaty_type, color: "text-zinc-400" };
                      const statusInfo = TREATY_STATUS_LABELS[treaty.status] ?? { label: treaty.status, color: "text-zinc-400" };
                      return (
                        <tr
                          key={treaty.id}
                          className="border-b border-zinc-800/50 last:border-0 hover:bg-zinc-800/30 transition-colors"
                        >
                          <td className="px-4 py-3 text-sm font-mono text-zinc-300">{treaty.id.slice(0, 8)}</td>
                          <td className={`px-4 py-3 text-sm font-medium ${typeInfo.color}`}>{typeInfo.label}</td>
                          <td className="px-4 py-3 text-sm text-zinc-300">{treaty.proposer_world_id.slice(0, 8)}</td>
                          <td className="px-4 py-3 text-sm text-zinc-300">{treaty.target_world_id.slice(0, 8)}</td>
                          <td className={`px-4 py-3 text-sm font-medium ${statusInfo.color}`}>{statusInfo.label}</td>
                          <td className="px-4 py-3 text-sm text-zinc-400 max-w-48 truncate">{treaty.terms}</td>
                          <td className="px-4 py-3 text-right text-sm text-zinc-400">{treaty.duration_ticks}</td>
                        </tr>
                      );
                    })}
                  </tbody>
                </table>
              </div>
            </div>
          )}
        </>
      )}
    </div>
  );
}
