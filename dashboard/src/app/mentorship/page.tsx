"use client";

import { useEffect, useState, useCallback } from "react";
import { fetchJSON } from "@/lib/api";
import { useSSEContext } from "@/components/SSEProvider";
import type { MentorshipStats, MentorshipSession, WorldEvent } from "@/types/world";

const STATUS_LABELS: Record<string, { label: string; color: string }> = {
  active: { label: "活跃", color: "text-green-400" },
  completed: { label: "已完成", color: "text-blue-400" },
  dropped: { label: "已退出", color: "text-red-400" },
};

export default function MentorshipPage() {
  const [stats, setStats] = useState<MentorshipStats | null>(null);
  const [sessions, setSessions] = useState<MentorshipSession[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [tab, setTab] = useState<"stats" | "sessions">("stats");
  const [agentFilter, setAgentFilter] = useState("");
  const [filterType, setFilterType] = useState<"mentor" | "apprentice">("mentor");

  const sse = useSSEContext();

  const loadData = useCallback(async () => {
    try {
      const statsData = await fetchJSON<MentorshipStats>("/api/v1/mentorship/stats").catch(() => null);
      setStats(statsData);
      setError(null);
    } catch {
      setError("无法加载导师制数据");
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
      if (event.type === "skill_level_up") {
        loadData();
      }
    }
    const unsubscribe = sse.subscribe(onEvent);
    return unsubscribe;
  }, [sse, loadData]);

  const loadSessions = useCallback(async () => {
    if (!agentFilter.trim()) return;
    try {
      const endpoint =
        filterType === "mentor"
          ? `/api/v1/mentorship/mentor/${agentFilter.trim()}`
          : `/api/v1/mentorship/apprentice/${agentFilter.trim()}`;
      const data = await fetchJSON<MentorshipSession[]>(endpoint).catch(() => []);
      setSessions(data);
    } catch {
      setSessions([]);
    }
  }, [agentFilter, filterType]);

  useEffect(() => {
    /* eslint-disable react-hooks/set-state-in-effect */
    if (agentFilter.trim()) {
      loadSessions();
    }
  }, [agentFilter, filterType, loadSessions]);

  if (loading) {
    return (
      <div className="flex h-full items-center justify-center">
        <div className="text-sm text-zinc-600">正在加载导师制数据...</div>
      </div>
    );
  }

  return (
    <div className="p-4 md:p-6 space-y-6">
      <div>
        <h1 className="text-xl md:text-2xl font-bold text-zinc-100">导师制</h1>
        <p className="text-sm text-zinc-500">
          {stats
            ? `${stats.total_sessions} 个会话 · ${stats.active_sessions} 个活跃 · ${stats.completed_sessions} 个已完成`
            : "Agent 间的技能传授关系"}
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
          <p className="text-sm text-zinc-400">总会话数</p>
          <p className="text-2xl font-bold text-blue-400">{stats?.total_sessions ?? 0}</p>
        </div>
        <div className="rounded-xl border border-green-500/20 bg-green-500/5 p-4 space-y-1">
          <p className="text-sm text-zinc-400">活跃会话</p>
          <p className="text-2xl font-bold text-green-400">{stats?.active_sessions ?? 0}</p>
        </div>
        <div className="rounded-xl border border-purple-500/20 bg-purple-500/5 p-4 space-y-1">
          <p className="text-sm text-zinc-400">平均传授率</p>
          <p className="text-2xl font-bold text-purple-400">{stats?.avg_skill_transfer_rate?.toFixed(2) ?? "—"}</p>
        </div>
        <div className="rounded-xl border border-amber-500/20 bg-amber-500/5 p-4 space-y-1">
          <p className="text-sm text-zinc-400">已完成</p>
          <p className="text-2xl font-bold text-amber-400">{stats?.completed_sessions ?? 0}</p>
        </div>
      </div>

      {/* Tab Switcher */}
      <div className="flex items-center gap-2">
        {(["stats", "sessions"] as const).map((t) => (
          <button
            key={t}
            onClick={() => setTab(t)}
            className={`rounded-lg px-3 py-1 text-xs font-medium transition-colors ${
              tab === t
                ? "bg-blue-500/15 text-blue-400 border border-blue-500/30"
                : "bg-zinc-800/50 text-zinc-400 border border-zinc-800 hover:bg-zinc-800"
            }`}
          >
            {t === "stats" ? "概览" : "查询会话"}
          </button>
        ))}
      </div>

      {/* Stats Tab - Popular Skills */}
      {tab === "stats" && stats && (
        <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4 space-y-3">
          <h2 className="text-sm font-semibold text-zinc-200">热门传授技能</h2>
          {(stats.popular_skills ?? []).length === 0 ? (
            <p className="text-sm text-zinc-600 h-32 flex items-center justify-center">暂无数据</p>
          ) : (
            <div className="space-y-2">
              {(stats.popular_skills ?? []).map((skill, i) => (
                <div key={i} className="flex items-center justify-between rounded-lg border border-zinc-800 bg-zinc-800/30 px-3 py-2">
                  <span className="text-sm text-zinc-200">{skill.skill}</span>
                  <div className="flex items-center gap-2">
                    <span className="text-xs text-zinc-500">{skill.count} 个会话</span>
                    <div className="w-24 h-1.5 rounded-full bg-zinc-800 overflow-hidden">
                      <div
                        className="h-full rounded-full bg-blue-400"
                        style={{ width: `${Math.min(100, (skill.count / Math.max(...(stats.popular_skills?.map((s) => s.count) ?? [1]))) * 100)}%` }}
                      />
                    </div>
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>
      )}

      {/* Sessions Tab */}
      {tab === "sessions" && (
        <div className="space-y-4">
          <div className="flex items-center gap-2">
            <div className="flex items-center gap-1">
              {(["mentor", "apprentice"] as const).map((ft) => (
                <button
                  key={ft}
                  onClick={() => setFilterType(ft)}
                  className={`rounded-lg px-3 py-1 text-xs font-medium transition-colors ${
                    filterType === ft
                      ? "bg-blue-500/15 text-blue-400 border border-blue-500/30"
                      : "bg-zinc-800/50 text-zinc-400 border border-zinc-800 hover:bg-zinc-800"
                  }`}
                >
                  {ft === "mentor" ? "作为导师" : "作为学徒"}
                </button>
              ))}
            </div>
            <input
              type="text"
              value={agentFilter}
              onChange={(e) => setAgentFilter(e.target.value)}
              placeholder="输入 Agent ID..."
              className="flex-1 rounded-lg border border-zinc-800 bg-zinc-900/50 px-3 py-2 text-sm text-zinc-200 placeholder:text-zinc-600 focus:border-blue-500/50 focus:outline-none"
            />
            <button
              onClick={loadSessions}
              disabled={!agentFilter.trim()}
              className="rounded-lg bg-blue-500/15 px-4 py-2 text-sm font-medium text-blue-400 border border-blue-500/30 hover:bg-blue-500/20 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
            >
              查询
            </button>
          </div>

          {sessions.length === 0 ? (
            <div className="flex h-48 items-center justify-center text-sm text-zinc-600">
              {agentFilter.trim() ? "未找到会话" : "输入 Agent ID 开始查询"}
            </div>
          ) : (
            <div className="rounded-xl border border-zinc-800 bg-zinc-900/50">
              <div className="overflow-x-auto">
                <table className="w-full">
                  <thead>
                    <tr className="border-b border-zinc-800">
                      <th className="px-4 py-3 text-left text-xs font-semibold text-zinc-400">ID</th>
                      <th className="px-4 py-3 text-left text-xs font-semibold text-zinc-400">导师</th>
                      <th className="px-4 py-3 text-left text-xs font-semibold text-zinc-400">学徒</th>
                      <th className="px-4 py-3 text-left text-xs font-semibold text-zinc-400">技能</th>
                      <th className="px-4 py-3 text-right text-xs font-semibold text-zinc-400">技能等级</th>
                      <th className="px-4 py-3 text-left text-xs font-semibold text-zinc-400">状态</th>
                    </tr>
                  </thead>
                  <tbody>
                    {sessions.map((session) => {
                      const statusInfo = STATUS_LABELS[session.status] ?? { label: session.status, color: "text-zinc-400" };
                      return (
                        <tr key={session.id} className="border-b border-zinc-800/50 last:border-0">
                          <td className="px-4 py-3 text-sm font-mono text-zinc-300">{session.id.slice(0, 8)}</td>
                          <td className="px-4 py-3 text-sm font-mono text-zinc-300">{session.mentor_id.slice(0, 8)}</td>
                          <td className="px-4 py-3 text-sm font-mono text-zinc-300">{session.apprentice_id.slice(0, 8)}</td>
                          <td className="px-4 py-3 text-sm text-zinc-200">{session.skill_name}</td>
                          <td className="px-4 py-3 text-right text-sm text-zinc-400">{session.mentor_skill_level}</td>
                          <td className={`px-4 py-3 text-sm font-medium ${statusInfo.color}`}>{statusInfo.label}</td>
                        </tr>
                      );
                    })}
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
