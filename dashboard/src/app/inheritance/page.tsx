"use client";

import { useEffect, useState, useCallback } from "react";
import { fetchJSON } from "@/lib/api";
import { useSSEContext } from "@/components/SSEProvider";
import type { InheritanceStats, Will, WorldEvent } from "@/types/world";

export default function InheritancePage() {
  const [stats, setStats] = useState<InheritanceStats | null>(null);
  const [wills, setWills] = useState<Will[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [agentFilter, setAgentFilter] = useState("");

  const sse = useSSEContext();

  const loadData = useCallback(async () => {
    try {
      const statsData = await fetchJSON<InheritanceStats>("/api/v1/inheritance/stats").catch(() => null);
      setStats(statsData);
      setError(null);
    } catch {
      setError("无法加载继承数据");
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
      if (event.type === "agent_died") {
        loadData();
      }
    }
    const unsubscribe = sse.subscribe(onEvent);
    return unsubscribe;
  }, [sse, loadData]);

  const loadWill = useCallback(async () => {
    if (!agentFilter.trim()) return;
    try {
      const data = await fetchJSON<Will>(`/api/v1/inheritance/will/${agentFilter.trim()}`).catch(() => null);
      setWills(data ? [data] : []);
    } catch {
      setWills([]);
    }
  }, [agentFilter]);

  useEffect(() => {
    if (agentFilter.trim()) {
      loadWill();
    }
  }, [agentFilter, loadWill]);

  if (loading) {
    return (
      <div className="flex h-full items-center justify-center">
        <div className="text-sm text-zinc-600">正在加载继承数据...</div>
      </div>
    );
  }

  return (
    <div className="p-4 md:p-6 space-y-6">
      <div>
        <h1 className="text-xl md:text-2xl font-bold text-zinc-100">遗产继承</h1>
        <p className="text-sm text-zinc-500">
          {stats
            ? `${stats.total_wills} 份遗嘱 · ${stats.executed_wills} 份已执行 · 平均 ${stats.avg_beneficiaries?.toFixed(1) ?? 0} 位受益人`
            : "Agent 遗嘱与遗产分配"}
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
          <p className="text-sm text-zinc-400">总遗嘱数</p>
          <p className="text-2xl font-bold text-blue-400">{stats?.total_wills ?? 0}</p>
        </div>
        <div className="rounded-xl border border-green-500/20 bg-green-500/5 p-4 space-y-1">
          <p className="text-sm text-zinc-400">已执行</p>
          <p className="text-2xl font-bold text-green-400">{stats?.executed_wills ?? 0}</p>
        </div>
        <div className="rounded-xl border border-amber-500/20 bg-amber-500/5 p-4 space-y-1">
          <p className="text-sm text-zinc-400">待执行</p>
          <p className="text-2xl font-bold text-amber-400">{stats?.pending_wills ?? 0}</p>
        </div>
        <div className="rounded-xl border border-purple-500/20 bg-purple-500/5 p-4 space-y-1">
          <p className="text-sm text-zinc-400">总继承额</p>
          <p className="text-2xl font-bold text-purple-400">${stats?.total_inherited?.toLocaleString() ?? 0}</p>
        </div>
      </div>

      {/* Will Lookup */}
      <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4 space-y-4">
        <h2 className="text-sm font-semibold text-zinc-200">查询遗嘱</h2>
        <div className="flex items-center gap-2">
          <input
            type="text"
            value={agentFilter}
            onChange={(e) => setAgentFilter(e.target.value)}
            placeholder="输入 Agent ID 查询遗嘱..."
            className="flex-1 rounded-lg border border-zinc-800 bg-zinc-900/50 px-3 py-2 text-sm text-zinc-200 placeholder:text-zinc-600 focus:border-blue-500/50 focus:outline-none"
          />
          <button
            onClick={loadWill}
            disabled={!agentFilter.trim()}
            className="rounded-lg bg-blue-500/15 px-4 py-2 text-sm font-medium text-blue-400 border border-blue-500/30 hover:bg-blue-500/20 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
          >
            查询
          </button>
        </div>

        {wills.length > 0 ? (
          wills.map((will) => (
            <div key={will.testator_id} className="space-y-3">
              <div className="grid grid-cols-2 gap-3 text-xs">
                <div>
                  <span className="text-zinc-500">立遗嘱者:</span>{" "}
                  <span className="text-zinc-300 font-mono">{will.testator_id.slice(0, 8)}</span>
                </div>
                <div>
                  <span className="text-zinc-500">状态:</span>{" "}
                  <span className={will.executed ? "text-green-400" : "text-amber-400"}>
                    {will.executed ? "已执行" : "待执行"}
                  </span>
                </div>
                <div>
                  <span className="text-zinc-500">创建:</span>{" "}
                  <span className="text-zinc-300">Tick #{will.created_tick}</span>
                </div>
                {will.executed_tick && (
                  <div>
                    <span className="text-zinc-500">执行:</span>{" "}
                    <span className="text-zinc-300">Tick #{will.executed_tick}</span>
                  </div>
                )}
              </div>

              {/* Beneficiaries */}
              <div className="space-y-2">
                <h3 className="text-xs font-semibold text-zinc-400">受益人</h3>
                {will.beneficiaries.map((b, i) => (
                  <div key={i} className="flex items-center justify-between rounded-lg border border-zinc-800 bg-zinc-800/30 px-3 py-2">
                    <span className="text-sm font-mono text-zinc-300">{b.agent_id.slice(0, 8)}</span>
                    <div className="flex items-center gap-2">
                      <span className="text-sm text-zinc-200">{(b.share * 100).toFixed(1)}%</span>
                      <div className="w-24 h-1.5 rounded-full bg-zinc-800 overflow-hidden">
                        <div
                          className="h-full rounded-full bg-blue-400"
                          style={{ width: `${b.share * 100}%` }}
                        />
                      </div>
                    </div>
                  </div>
                ))}
              </div>
            </div>
          ))
        ) : (
          agentFilter.trim() && (
            <div className="text-sm text-zinc-600 text-center py-4">
              该 Agent 尚未立遗嘱
            </div>
          )
        )}
      </div>
    </div>
  );
}
