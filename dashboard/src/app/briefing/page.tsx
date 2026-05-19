"use client";

import { useState, useEffect, useCallback } from "react";
import { fetchJSON, postJSON } from "@/lib/api";
import type { WorldSnapshotData } from "@/types/world";

function formatTimestamp(ts: number): string {
  return new Date(ts * 1000).toLocaleString("zh-CN");
}

export default function BriefingPage() {
  const [snapshots, setSnapshots] = useState<WorldSnapshotData[]>([]);
  const [selectedSnapshot, setSelectedSnapshot] = useState<WorldSnapshotData | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [creating, setCreating] = useState(false);

  const fetchSnapshots = useCallback(async () => {
    try {
      setLoading(true);
      const data = await fetchJSON<WorldSnapshotData[]>("/api/v1/snapshots?limit=50");
      setSnapshots(data);
      if (data.length > 0 && !selectedSnapshot) {
        setSelectedSnapshot(data[data.length - 1]);
      }
      setError(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to fetch snapshots");
    } finally {
      setLoading(false);
    }
  }, [selectedSnapshot]);

  useEffect(() => {
    (async () => {
      await fetchSnapshots();
    })();
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  const handleCreateSnapshot = async () => {
    try {
      setCreating(true);
      await postJSON<WorldSnapshotData>("/api/v1/snapshots", {});
      await fetchSnapshots();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to create snapshot");
    } finally {
      setCreating(false);
    }
  };

  const handleExportJSON = () => {
    window.open("/api/v1/snapshots/export/json", "_blank");
  };

  const handleExportCSV = () => {
    window.open("/api/v1/snapshots/export/csv", "_blank");
  };

  return (
    <div className="p-4 md:p-6 space-y-6">
      {/* Header */}
      <div className="flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between">
        <div>
          <h1 className="text-xl md:text-2xl font-bold text-zinc-100">世界简报</h1>
          <p className="text-sm text-zinc-500">
            时间胶囊 — 世界快照回放与数据导出
          </p>
        </div>
        <div className="flex gap-2 flex-wrap">
          <button
            onClick={handleCreateSnapshot}
            disabled={creating}
            className="min-h-[44px] rounded-lg bg-blue-500/10 border border-blue-500/20 px-4 py-2 text-sm font-medium text-blue-400 transition-colors hover:bg-blue-500/20 disabled:opacity-50"
          >
            {creating ? "生成中..." : "手动快照"}
          </button>
          <button
            onClick={handleExportJSON}
            className="min-h-[44px] rounded-lg bg-emerald-500/10 border border-emerald-500/20 px-4 py-2 text-sm font-medium text-emerald-400 transition-colors hover:bg-emerald-500/20"
          >
            导出 JSON
          </button>
          <button
            onClick={handleExportCSV}
            className="min-h-[44px] rounded-lg bg-amber-500/10 border border-amber-500/20 px-4 py-2 text-sm font-medium text-amber-400 transition-colors hover:bg-amber-500/20"
          >
            导出 CSV
          </button>
        </div>
      </div>

      {error && (
        <div className="rounded-lg bg-red-500/10 border border-red-500/20 px-3 py-1.5 text-xs text-red-400">
          {error}
        </div>
      )}

      <div className="grid grid-cols-1 gap-6 xl:grid-cols-3">
        {/* Timeline / Snapshot List */}
        <div className="xl:col-span-1 space-y-3">
          <h2 className="text-sm font-semibold text-zinc-200">快照历史</h2>
          <div className="max-h-[600px] overflow-y-auto space-y-2">
            {loading && snapshots.length === 0 && (
              <p className="text-sm text-zinc-500">加载中...</p>
            )}
            {!loading && snapshots.length === 0 && (
              <p className="text-sm text-zinc-500">暂无快照数据。点击{'"'}手动快照{'"'}生成第一个快照。</p>
            )}
            {snapshots.map((snap) => (
              <button
                key={snap.tick}
                onClick={() => setSelectedSnapshot(snap)}
                className={`w-full text-left rounded-lg border p-3 transition-colors ${
                  selectedSnapshot?.tick === snap.tick
                    ? "bg-blue-500/10 border-blue-500/30"
                    : "bg-zinc-900/50 border-zinc-800 hover:bg-zinc-800/50"
                }`}
              >
                <div className="flex items-center justify-between">
                  <span className="text-sm font-medium text-zinc-200">Tick #{snap.tick}</span>
                  <span className="text-xs text-zinc-500">{formatTimestamp(snap.timestamp)}</span>
                </div>
                <div className="mt-1 flex gap-3 text-xs text-zinc-400">
                  <span>{snap.active_agents} 存活</span>
                  <span>GDP {snap.gdp.toLocaleString()}</span>
                </div>
              </button>
            ))}
          </div>
        </div>

        {/* Snapshot Detail */}
        <div className="xl:col-span-2 space-y-4">
          {selectedSnapshot ? (
            <>
              {/* Stats Cards */}
              <div className="grid grid-cols-2 sm:grid-cols-3 gap-3">
                <StatCard label="总人口" value={selectedSnapshot.total_population.toString()} />
                <StatCard label="活跃 Agent" value={selectedSnapshot.active_agents.toString()} />
                <StatCard label="GDP" value={selectedSnapshot.gdp.toLocaleString()} />
                <StatCard
                  label="基尼系数"
                  value={selectedSnapshot.gini_coefficient.toFixed(4)}
                />
                <StatCard label="Tick" value={`#${selectedSnapshot.tick}`} />
                <StatCard
                  label="快照时间"
                  value={formatTimestamp(selectedSnapshot.timestamp)}
                />
              </div>

              {/* Skill Distribution */}
              <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4">
                <h3 className="text-sm font-semibold text-zinc-200 mb-3">技能分布 TOP 5</h3>
                {selectedSnapshot.skill_distribution_top5.length === 0 ? (
                  <p className="text-xs text-zinc-500">暂无技能数据</p>
                ) : (
                  <div className="space-y-2">
                    {selectedSnapshot.skill_distribution_top5.map((skill, i) => (
                      <div key={skill.skill_name} className="flex items-center gap-3">
                        <span className="w-5 text-xs text-zinc-500">#{i + 1}</span>
                        <span className="flex-1 text-sm text-zinc-200">{skill.skill_name}</span>
                        <span className="text-xs text-zinc-400">{skill.agent_count} 人</span>
                        <div className="w-24 bg-zinc-800 rounded-full h-2">
                          <div
                            className="bg-blue-500 h-2 rounded-full"
                            style={{
                              width: `${Math.min(100, (skill.agent_count / Math.max(1, selectedSnapshot.active_agents)) * 100)}%`,
                            }}
                          />
                        </div>
                        <span className="text-xs text-zinc-500">
                          Lv {skill.avg_level.toFixed(1)}
                        </span>
                      </div>
                    ))}
                  </div>
                )}
              </div>

              {/* Key Events Timeline */}
              <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4">
                <h3 className="text-sm font-semibold text-zinc-200 mb-3">关键事件</h3>
                {selectedSnapshot.key_events.length === 0 ? (
                  <p className="text-xs text-zinc-500">此快照期间无关键事件</p>
                ) : (
                  <div className="space-y-2">
                    {selectedSnapshot.key_events.map((evt, i) => (
                      <div key={i} className="flex items-start gap-3 text-sm">
                        <span className="shrink-0 mt-0.5 inline-block rounded px-1.5 py-0.5 text-[10px] font-medium bg-zinc-800 text-zinc-300">
                          Tick {evt.tick}
                        </span>
                        <span
                          className={`shrink-0 mt-0.5 inline-block rounded px-1.5 py-0.5 text-[10px] font-medium ${
                            evt.event_type === "agent_died"
                              ? "bg-red-500/10 text-red-400"
                              : evt.event_type === "large_transaction"
                              ? "bg-amber-500/10 text-amber-400"
                              : evt.event_type === "agent_spawned"
                              ? "bg-green-500/10 text-green-400"
                              : "bg-zinc-800 text-zinc-300"
                          }`}
                        >
                          {evt.event_type}
                        </span>
                        <span className="text-zinc-400">{evt.description}</span>
                      </div>
                    ))}
                  </div>
                )}
              </div>
            </>
          ) : (
            <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-8 text-center">
              <p className="text-zinc-500">选择左侧快照查看详细数据</p>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

function StatCard({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-lg border border-zinc-800 bg-zinc-900/50 p-3">
      <p className="text-[10px] text-zinc-500">{label}</p>
      <p className="text-lg font-semibold text-zinc-100 truncate">{value}</p>
    </div>
  );
}
