"use client";

import { useEffect, useState, useCallback } from "react";
import { fetchJSON } from "@/lib/api";
import { useSSEContext } from "@/components/SSEProvider";
import type { TrustStats, TrustRelationship, WorldEvent } from "@/types/world";

export default function TrustPage() {
  const [stats, setStats] = useState<TrustStats | null>(null);
  const [relationships, setRelationships] = useState<TrustRelationship[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [agentFilter, setAgentFilter] = useState("");
  const [tab, setTab] = useState<"stats" | "relationships">("stats");

  const sse = useSSEContext();

  const loadData = useCallback(async () => {
    try {
      const statsData = await fetchJSON<TrustStats>("/api/v1/trust/stats").catch(() => null);
      setStats(statsData);
      setError(null);
    } catch {
      setError("无法加载信任网络数据");
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    loadData();
    const interval = setInterval(loadData, 10000);
    return () => clearInterval(interval);
  }, [loadData]);

  useEffect(() => {
    function onEvent(event: WorldEvent) {
      if (event.type === "relation_changed") {
        loadData();
      }
    }
    const unsubscribe = sse.subscribe(onEvent);
    return unsubscribe;
  }, [sse, loadData]);

  const loadRelationships = useCallback(async () => {
    if (!agentFilter.trim()) return;
    try {
      const data = await fetchJSON<TrustRelationship[]>(
        `/api/v1/trust/relationships/${agentFilter.trim()}`
      ).catch(() => []);
      setRelationships(data);
    } catch {
      setRelationships([]);
    }
  }, [agentFilter]);

  useEffect(() => {
    if (agentFilter.trim()) {
      loadRelationships();
    }
  }, [agentFilter, loadRelationships]);

  if (loading) {
    return (
      <div className="flex h-full items-center justify-center">
        <div className="text-sm text-zinc-600">正在加载信任网络...</div>
      </div>
    );
  }

  return (
    <div className="p-4 md:p-6 space-y-6">
      <div>
        <h1 className="text-xl md:text-2xl font-bold text-zinc-100">信任网络</h1>
        <p className="text-sm text-zinc-500">
          {stats
            ? `${stats.total_relationships ?? 0} 条关系 · 平均信任分 ${stats.avg_trust_score?.toFixed(2) ?? "—"} · ${stats.allies_count ?? 0} 个盟友 · ${stats.enemies_count ?? 0} 个敌人`
            : "Agent 间的信任关系图谱"}
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
          <p className="text-sm text-zinc-400">总关系数</p>
          <p className="text-2xl font-bold text-blue-400">{stats?.total_relationships ?? 0}</p>
        </div>
        <div className="rounded-xl border border-purple-500/20 bg-purple-500/5 p-4 space-y-1">
          <p className="text-sm text-zinc-400">平均信任分</p>
          <p className="text-2xl font-bold text-purple-400">{stats?.avg_trust_score?.toFixed(2) ?? "—"}</p>
        </div>
        <div className="rounded-xl border border-green-500/20 bg-green-500/5 p-4 space-y-1">
          <p className="text-sm text-zinc-400">盟友对数</p>
          <p className="text-2xl font-bold text-green-400">{stats?.allies_count ?? 0}</p>
        </div>
        <div className="rounded-xl border border-red-500/20 bg-red-500/5 p-4 space-y-1">
          <p className="text-sm text-zinc-400">敌人对数</p>
          <p className="text-2xl font-bold text-red-400">{stats?.enemies_count ?? 0}</p>
        </div>
      </div>

      {/* Tab Switcher */}
      <div className="flex items-center gap-2">
        {(["stats", "relationships"] as const).map((t) => (
          <button
            key={t}
            onClick={() => setTab(t)}
            className={`rounded-lg px-3 py-1 text-xs font-medium transition-colors ${
              tab === t
                ? "bg-blue-500/15 text-blue-400 border border-blue-500/30"
                : "bg-zinc-800/50 text-zinc-400 border border-zinc-800 hover:bg-zinc-800"
            }`}
          >
            {t === "stats" ? "全局概览" : "查询关系"}
          </button>
        ))}
      </div>

      {/* Stats Tab */}
      {tab === "stats" && stats && (
        <div className="grid grid-cols-1 gap-6 xl:grid-cols-2">
          {/* Top Allies */}
          <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4 space-y-3">
            <h2 className="text-sm font-semibold text-zinc-200">最强盟友</h2>
            {(stats.top_allies ?? []).length === 0 ? (
              <p className="text-sm text-zinc-600 h-32 flex items-center justify-center">暂无数据</p>
            ) : (
              <div className="space-y-2">
                {(stats.top_allies ?? []).slice(0, 10).map((rel, i) => (
                  <div key={i} className="flex items-center justify-between rounded-lg border border-zinc-800 bg-zinc-800/30 px-3 py-2">
                    <div className="text-xs text-zinc-300">
                      <span className="font-mono">{rel.from_agent.slice(0, 8)}</span>
                      <span className="text-zinc-500 mx-2">&rarr;</span>
                      <span className="font-mono">{rel.to_agent.slice(0, 8)}</span>
                    </div>
                    <div className="flex items-center gap-3">
                      <span className="text-xs text-zinc-500">{rel.interaction_count} 次互动</span>
                      <span className="text-sm font-medium text-green-400">{rel.score.toFixed(2)}</span>
                    </div>
                  </div>
                ))}
              </div>
            )}
          </div>

          {/* Top Enemies */}
          <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4 space-y-3">
            <h2 className="text-sm font-semibold text-zinc-200">最强敌人</h2>
            {(stats.top_enemies ?? []).length === 0 ? (
              <p className="text-sm text-zinc-600 h-32 flex items-center justify-center">暂无数据</p>
            ) : (
              <div className="space-y-2">
                {(stats.top_enemies ?? []).slice(0, 10).map((rel, i) => (
                  <div key={i} className="flex items-center justify-between rounded-lg border border-zinc-800 bg-zinc-800/30 px-3 py-2">
                    <div className="text-xs text-zinc-300">
                      <span className="font-mono">{rel.from_agent.slice(0, 8)}</span>
                      <span className="text-zinc-500 mx-2">&rarr;</span>
                      <span className="font-mono">{rel.to_agent.slice(0, 8)}</span>
                    </div>
                    <div className="flex items-center gap-3">
                      <span className="text-xs text-zinc-500">{rel.interaction_count} 次互动</span>
                      <span className="text-sm font-medium text-red-400">{rel.score.toFixed(2)}</span>
                    </div>
                  </div>
                ))}
              </div>
            )}
          </div>
        </div>
      )}

      {/* Relationships Tab */}
      {tab === "relationships" && (
        <div className="space-y-4">
          <div className="flex items-center gap-2">
            <input
              type="text"
              value={agentFilter}
              onChange={(e) => setAgentFilter(e.target.value)}
              placeholder="输入 Agent ID 查询信任关系..."
              className="flex-1 rounded-lg border border-zinc-800 bg-zinc-900/50 px-3 py-2 text-sm text-zinc-200 placeholder:text-zinc-600 focus:border-blue-500/50 focus:outline-none"
            />
            <button
              onClick={loadRelationships}
              disabled={!agentFilter.trim()}
              className="rounded-lg bg-blue-500/15 px-4 py-2 text-sm font-medium text-blue-400 border border-blue-500/30 hover:bg-blue-500/20 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
            >
              查询
            </button>
          </div>

          {relationships.length === 0 ? (
            <div className="flex h-48 items-center justify-center text-sm text-zinc-600">
              {agentFilter.trim() ? "未找到信任关系" : "输入 Agent ID 开始查询"}
            </div>
          ) : (
            <div className="rounded-xl border border-zinc-800 bg-zinc-900/50">
              <div className="overflow-x-auto">
                <table className="w-full">
                  <thead>
                    <tr className="border-b border-zinc-800">
                      <th className="px-4 py-3 text-left text-xs font-semibold text-zinc-400">对方 Agent</th>
                      <th className="px-4 py-3 text-right text-xs font-semibold text-zinc-400">信任分</th>
                      <th className="px-4 py-3 text-right text-xs font-semibold text-zinc-400">互动次数</th>
                      <th className="px-4 py-3 text-right text-xs font-semibold text-zinc-400">最后互动</th>
                    </tr>
                  </thead>
                  <tbody>
                    {relationships.map((rel, i) => (
                      <tr key={i} className="border-b border-zinc-800/50 last:border-0">
                        <td className="px-4 py-3 text-sm font-mono text-zinc-300">{rel.to_agent.slice(0, 8)}</td>
                        <td className={`px-4 py-3 text-right text-sm font-medium ${rel.score >= 0 ? "text-green-400" : "text-red-400"}`}>
                          {rel.score.toFixed(2)}
                        </td>
                        <td className="px-4 py-3 text-right text-sm text-zinc-400">{rel.interaction_count}</td>
                        <td className="px-4 py-3 text-right text-sm text-zinc-400">#{rel.last_interaction_tick}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
