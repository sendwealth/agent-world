"use client";

import { useEffect, useState, useMemo, useCallback, useRef } from "react";
import { useRouter } from "next/navigation";
import type { Agent, TickTraceSummary, WorldEvent } from "@/types/world";
import { fetchJSON } from "@/lib/api";
import { useSSEContext } from "@/components/SSEProvider";

function formatDate(iso: string): string {
  const d = new Date(iso);
  const now = new Date();
  const diff = now.getTime() - d.getTime();
  const minutes = Math.floor(diff / 60_000);
  if (minutes < 1) return "刚刚";
  if (minutes < 60) return `${minutes} 分钟前`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours} 小时前`;
  const days = Math.floor(hours / 24);
  if (days < 30) return `${days} 天前`;
  return d.toLocaleDateString("zh-CN");
}

const survivalModeColors: Record<string, { dot: string; text: string; bg: string }> = {
  normal: { dot: "bg-green-400", text: "text-green-400", bg: "bg-green-500/10" },
  urgent: { dot: "bg-amber-400", text: "text-amber-400", bg: "bg-amber-500/10" },
  panic: { dot: "bg-red-400", text: "text-red-400", bg: "bg-red-500/10" },
};

function formatDuration(ms: number): string {
  if (ms < 1) return `${(ms * 1000).toFixed(0)}μs`;
  if (ms < 1000) return `${ms.toFixed(1)}ms`;
  return `${(ms / 1000).toFixed(2)}s`;
}

export default function TracesPage() {
  const router = useRouter();
  const [agents, setAgents] = useState<Agent[]>([]);
  const [selectedAgentId, setSelectedAgentId] = useState<string | null>(null);
  const [traces, setTraces] = useState<TickTraceSummary[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [tracesLoading, setTracesLoading] = useState(false);

  const sse = useSSEContext();
  const refreshPending = useRef(false);
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Load agents list
  useEffect(() => {
    let cancelled = false;

    async function load() {
      try {
        const data = await fetchJSON<Agent[]>("/api/v1/agents");
        if (!cancelled) {
          setAgents(data);
          setError(null);
        }
      } catch {
        if (!cancelled) {
          setError("无法连接到世界引擎");
        }
      } finally {
        if (!cancelled) setLoading(false);
      }
    }

    load();
    const interval = setInterval(load, 10000);
    return () => {
      cancelled = true;
      clearInterval(interval);
    };
  }, []);

  // Load traces when agent is selected
  const loadTraces = useCallback(async (agentId: string) => {
    setTracesLoading(true);
    try {
      const data = await fetchJSON<TickTraceSummary[]>(
        `/api/v1/agents/${agentId}/traces?limit=50`
      );
      setTraces(data);
      setError(null);
    } catch {
      setError("无法加载决策轨迹");
      setTraces([]);
    } finally {
      setTracesLoading(false);
    }
  }, []);

  useEffect(() => {
    if (selectedAgentId) {
      loadTraces(selectedAgentId);
    } else {
      setTraces([]);
    }
  }, [selectedAgentId, loadTraces]);

  // SSE-driven refresh
  useEffect(() => {
    if (!selectedAgentId) return;

    function onEvent(event: WorldEvent) {
      const isRelevant =
        event.agentId === selectedAgentId || event.targetId === selectedAgentId;
      if (!isRelevant || refreshPending.current) return;

      refreshPending.current = true;
      debounceRef.current = setTimeout(() => {
        loadTraces(selectedAgentId);
        refreshPending.current = false;
      }, 500);
    }

    const unsubscribe = sse.subscribe(onEvent);
    return () => {
      unsubscribe();
      if (debounceRef.current) {
        clearTimeout(debounceRef.current);
        debounceRef.current = null;
      }
      refreshPending.current = false;
    };
  }, [sse, selectedAgentId, loadTraces]);

  // Compute stats
  const stats = useMemo(() => {
    if (traces.length === 0) return null;
    const latestTick = Math.max(...traces.map((t) => t.tick));
    const avgDuration = traces.reduce((sum, t) => sum + t.duration_ms, 0) / traces.length;
    const errorCount = traces.filter((t) => t.error !== null).length;
    return { total: traces.length, latestTick, avgDuration, errorCount };
  }, [traces]);

  const selectedAgent = useMemo(
    () => agents.find((a) => a.id === selectedAgentId) ?? null,
    [agents, selectedAgentId]
  );

  if (loading) {
    return (
      <div className="flex h-full items-center justify-center">
        <div className="text-sm text-zinc-600">正在加载 Agent 数据...</div>
      </div>
    );
  }

  if (error && agents.length === 0) {
    return (
      <div className="p-4 md:p-6 space-y-4">
        <div className="rounded-lg bg-red-500/10 border border-red-500/20 px-4 py-3 text-sm text-red-400">
          {error}
        </div>
      </div>
    );
  }

  return (
    <div className="p-4 md:p-6 space-y-6">
      {/* Header */}
      <div>
        <h1 className="text-2xl font-bold text-zinc-100">决策轨迹</h1>
        <p className="text-sm text-zinc-500">
          查看 Agent 每个 Tick 的决策过程和执行阶段
        </p>
      </div>

      {/* Agent Selector */}
      <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4 space-y-4">
        <h2 className="text-sm font-semibold text-zinc-200">选择 Agent</h2>
        <div className="relative">
          <select
            value={selectedAgentId ?? ""}
            onChange={(e) => setSelectedAgentId(e.target.value || null)}
            className="w-full appearance-none rounded-lg border border-zinc-800 bg-zinc-900/50 px-3 py-2.5 text-sm text-zinc-200 outline-none transition-colors focus:border-blue-500/40 sm:w-80"
          >
            <option value="">-- 选择一个 Agent --</option>
            {agents.map((agent) => (
              <option key={agent.id} value={agent.id}>
                {agent.name} {!agent.alive ? "(已死亡)" : ""} · {agent.age} Tick
              </option>
            ))}
          </select>
          <svg
            className="pointer-events-none absolute right-3 top-3 h-4 w-4 text-zinc-500"
            fill="none"
            stroke="currentColor"
            viewBox="0 0 24 24"
          >
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 9l-7 7-7-7" />
          </svg>
        </div>

        {selectedAgent && (
          <div className="flex flex-wrap items-center gap-3 text-xs text-zinc-400">
            <span className="font-medium text-zinc-200">{selectedAgent.name}</span>
            <span
              className={`inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-[10px] font-medium ${
                selectedAgent.alive
                  ? "bg-green-500/10 text-green-400"
                  : "bg-red-500/10 text-red-400"
              }`}
            >
              <span
                className={`inline-block h-1.5 w-1.5 rounded-full ${
                  selectedAgent.alive ? "bg-green-400" : "bg-red-400"
                }`}
              />
              {selectedAgent.alive ? "存活" : "死亡"}
            </span>
            <span>{selectedAgent.tokens.toLocaleString()} Token</span>
            <span>信誉 {selectedAgent.reputation.toFixed(1)}</span>
          </div>
        )}
      </div>

      {/* Stats */}
      {stats && (
        <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
          <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-3 space-y-1">
            <span className="text-[10px] font-medium text-zinc-500">追踪记录</span>
            <p className="text-lg font-bold tabular-nums text-zinc-100">{stats.total}</p>
          </div>
          <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-3 space-y-1">
            <span className="text-[10px] font-medium text-zinc-500">最新 Tick</span>
            <p className="text-lg font-bold tabular-nums text-zinc-100">#{stats.latestTick}</p>
          </div>
          <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-3 space-y-1">
            <span className="text-[10px] font-medium text-zinc-500">平均耗时</span>
            <p className="text-lg font-bold tabular-nums text-zinc-100">
              {formatDuration(stats.avgDuration)}
            </p>
          </div>
          <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-3 space-y-1">
            <span className="text-[10px] font-medium text-zinc-500">错误数</span>
            <p className={`text-lg font-bold tabular-nums ${stats.errorCount > 0 ? "text-red-400" : "text-zinc-100"}`}>
              {stats.errorCount}
            </p>
          </div>
        </div>
      )}

      {/* Timeline */}
      {selectedAgentId && (
        <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4 space-y-4">
          <div className="flex items-center justify-between">
            <h2 className="text-sm font-semibold text-zinc-200">Tick 轨迹时间线</h2>
            {tracesLoading && (
              <span className="text-xs text-zinc-600">加载中...</span>
            )}
          </div>

          {!tracesLoading && traces.length === 0 ? (
            <p className="text-sm text-zinc-600 py-8 text-center">
              暂无决策轨迹数据
            </p>
          ) : (
            <div className="max-h-[600px] overflow-y-auto scrollbar-thin space-y-0">
              {traces
                .sort((a, b) => b.tick - a.tick)
                .map((trace, idx) => {
                  const modeStyle = survivalModeColors[trace.survival_mode] ?? survivalModeColors.normal;
                  const hasError = trace.error !== null;

                  return (
                    <button
                      key={trace.tick}
                      onClick={() => router.push(`/traces/${selectedAgentId}/${trace.tick}`)}
                      className="relative flex w-full gap-3 pb-4 text-left transition-colors group"
                    >
                      {/* Timeline line */}
                      {idx < traces.length - 1 && (
                        <div className="absolute left-[5px] top-3 h-full w-px bg-zinc-800" />
                      )}
                      {/* Dot */}
                      <div
                        className={`relative mt-1.5 h-2.5 w-2.5 shrink-0 rounded-full ${
                          hasError ? "bg-red-400" : modeStyle.dot
                        }`}
                      />
                      {/* Content */}
                      <div className="min-w-0 flex-1 space-y-0.5">
                        <div className="flex flex-wrap items-center gap-2">
                          <span className="text-xs font-medium text-zinc-200 group-hover:text-blue-400 transition-colors">
                            Tick #{trace.tick}
                          </span>
                          <span className={`rounded-full px-1.5 py-0.5 text-[10px] font-medium ${modeStyle.bg} ${modeStyle.text}`}>
                            {trace.survival_mode}
                          </span>
                          <span className="rounded-full bg-zinc-800 px-1.5 py-0.5 text-[10px] font-medium text-zinc-400">
                            {trace.action}
                          </span>
                          {hasError && (
                            <span className="rounded-full bg-red-500/10 px-1.5 py-0.5 text-[10px] font-medium text-red-400">
                              错误
                            </span>
                          )}
                        </div>
                        <div className="flex flex-wrap items-center gap-x-3 text-[10px] text-zinc-600">
                          <span>耗时 {formatDuration(trace.duration_ms)}</span>
                          <span>Token 比率 {trace.token_ratio.toFixed(2)}</span>
                          <span>{formatDate(trace.started_at)}</span>
                        </div>
                        {hasError && (
                          <p className="text-[10px] text-red-400/80 truncate">{trace.error}</p>
                        )}
                      </div>
                      {/* Arrow */}
                      <svg
                        className="mt-2 h-4 w-4 shrink-0 text-zinc-700 group-hover:text-zinc-400 transition-colors"
                        fill="none"
                        stroke="currentColor"
                        viewBox="0 0 24 24"
                      >
                        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 5l7 7-7 7" />
                      </svg>
                    </button>
                  );
                })}
            </div>
          )}
        </div>
      )}

      {/* Empty state when no agent selected */}
      {!selectedAgentId && (
        <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-12 text-center">
          <p className="text-3xl mb-3">🔍</p>
          <p className="text-sm text-zinc-400">选择一个 Agent 以查看决策轨迹</p>
          <p className="text-xs text-zinc-600 mt-1">
            决策轨迹展示了 Agent 每个 Tick 中 Sense → Survive → Decide → Act 的完整过程
          </p>
        </div>
      )}
    </div>
  );
}
