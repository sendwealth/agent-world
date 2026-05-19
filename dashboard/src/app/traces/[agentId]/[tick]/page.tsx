"use client";

import { useEffect, useState, useCallback, useRef, useMemo } from "react";
import { useParams, useRouter } from "next/navigation";
import Link from "next/link";
import type { TickTraceData, TickTraceSummary, WorldEvent } from "@/types/world";
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

function formatDuration(ms: number): string {
  if (ms < 1) return `${(ms * 1000).toFixed(0)}μs`;
  if (ms < 1000) return `${ms.toFixed(1)}ms`;
  return `${(ms / 1000).toFixed(2)}s`;
}

const phaseConfig: Record<string, { label: string; icon: string; bg: string; border: string; text: string; bar: string }> = {
  sense: {
    label: "感知 (Sense)",
    icon: "👁",
    bg: "bg-blue-500/5",
    border: "border-blue-500/20",
    text: "text-blue-400",
    bar: "bg-blue-400",
  },
  survive: {
    label: "生存判断 (Survive)",
    icon: "🛡",
    bg: "bg-amber-500/5",
    border: "border-amber-500/20",
    text: "text-amber-400",
    bar: "bg-amber-400",
  },
  decide: {
    label: "决策 (Decide)",
    icon: "🧠",
    bg: "bg-purple-500/5",
    border: "border-purple-500/20",
    text: "text-purple-400",
    bar: "bg-purple-400",
  },
  act: {
    label: "执行 (Act)",
    icon: "⚡",
    bg: "bg-green-500/5",
    border: "border-green-500/20",
    text: "text-green-400",
    bar: "bg-green-400",
  },
};

function DataBlock({
  label,
  data,
}: {
  label: string;
  data: Record<string, unknown>;
}) {
  const entries = Object.entries(data);
  if (entries.length === 0) return null;

  return (
    <div className="space-y-1.5">
      <span className="text-[10px] font-medium text-zinc-500 uppercase tracking-wider">
        {label}
      </span>
      <div className="space-y-1">
        {entries.map(([key, value]) => (
          <div key={key} className="flex items-start gap-2 text-xs">
            <span className="shrink-0 text-zinc-500 font-mono">{key}</span>
            <span className="text-zinc-300 break-all">
              {typeof value === "object" && value !== null
                ? JSON.stringify(value)
                : String(value)}
            </span>
          </div>
        ))}
      </div>
    </div>
  );
}

export default function TickDetailPage() {
  const params = useParams();
  const router = useRouter();
  const agentId = params.agentId as string;
  const tick = Number(params.tick);

  const [trace, setTrace] = useState<TickTraceData | null>(null);
  const [traceList, setTraceList] = useState<TickTraceSummary[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const sse = useSSEContext();
  const refreshPending = useRef(false);
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const loadTrace = useCallback(async () => {
    try {
      const [traceData, listData] = await Promise.all([
        fetchJSON<TickTraceData>(`/api/v1/agents/${agentId}/traces/${tick}`),
        fetchJSON<TickTraceSummary[]>(`/api/v1/agents/${agentId}/traces?limit=50`).catch(
          () => []
        ),
      ]);
      setTrace(traceData);
      setTraceList(listData);
      setError(null);
    } catch {
      setError("无法加载轨迹详情");
    } finally {
      setLoading(false);
    }
  }, [agentId, tick]);

  useEffect(() => {
    (async () => {
      await loadTrace();
    })();
  }, [loadTrace]);

  // SSE-driven refresh
  useEffect(() => {
    function onEvent(event: WorldEvent) {
      const isRelevant = event.agentId === agentId;
      if (!isRelevant || refreshPending.current) return;

      refreshPending.current = true;
      debounceRef.current = setTimeout(() => {
        loadTrace();
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
  }, [sse, agentId, loadTrace]);

  // Fallback polling when SSE is not connected
  useEffect(() => {
    if (sse.connected) return;
    const interval = setInterval(loadTrace, 5000);
    return () => clearInterval(interval);
  }, [sse, loadTrace]);

  // Compute prev/next tick
  const { prevTick, nextTick } = useMemo(() => {
    const sorted = traceList
      .map((t) => t.tick)
      .sort((a, b) => a - b);
    const idx = sorted.indexOf(tick);
    return {
      prevTick: idx > 0 ? sorted[idx - 1] : null,
      nextTick: idx >= 0 && idx < sorted.length - 1 ? sorted[idx + 1] : null,
    };
  }, [traceList, tick]);

  if (loading) {
    return (
      <div className="flex h-full items-center justify-center">
        <div className="text-sm text-zinc-600">正在加载轨迹详情...</div>
      </div>
    );
  }

  if (error || !trace) {
    return (
      <div className="p-4 md:p-6 space-y-4">
        <button
          onClick={() => router.push("/traces")}
          className="text-sm text-zinc-400 hover:text-zinc-200 transition-colors"
        >
          &larr; 返回轨迹列表
        </button>
        <div className="rounded-lg bg-red-500/10 border border-red-500/20 px-4 py-3 text-sm text-red-400">
          {error ?? "轨迹数据不存在"}
        </div>
      </div>
    );
  }

  return (
    <div className="p-4 md:p-6 space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2 sm:gap-4">
          <button
            onClick={() => router.push("/traces")}
            className="rounded-lg p-1.5 text-zinc-400 hover:bg-zinc-800 hover:text-zinc-200 transition-colors"
          >
            <svg className="h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M15 19l-7-7 7-7" />
            </svg>
          </button>
          <div>
            <div className="flex flex-wrap items-center gap-2 sm:gap-3">
              <h1 className="text-xl sm:text-2xl font-bold text-zinc-100">
                Tick #{trace.tick}
              </h1>
              <span className="rounded-full bg-zinc-800 px-2 py-0.5 text-[10px] font-medium text-zinc-400">
                决策轨迹
              </span>
            </div>
            <p className="text-sm text-zinc-500">
              Agent{" "}
              <Link
                href={`/agents/${agentId}`}
                className="text-blue-400/80 hover:text-blue-300 transition-colors"
              >
                {agentId.slice(0, 8)}...
              </Link>
              {" · "}
              总耗时 {formatDuration(trace.total_duration_ms)}
              {" · "}
              {formatDate(trace.started_at)}
            </p>
          </div>
        </div>

        {/* Prev / Next navigation */}
        <div className="flex items-center gap-2">
          {prevTick !== null ? (
            <Link
              href={`/traces/${agentId}/${prevTick}`}
              className="rounded-lg border border-zinc-800 bg-zinc-900/50 px-3 py-1.5 text-xs text-zinc-300 transition-colors hover:bg-zinc-800 hover:text-zinc-100"
            >
              ← Tick #{prevTick}
            </Link>
          ) : (
            <span className="rounded-lg border border-zinc-800/50 bg-zinc-900/30 px-3 py-1.5 text-xs text-zinc-700">
              ← 最早
            </span>
          )}
          {nextTick !== null ? (
            <Link
              href={`/traces/${agentId}/${nextTick}`}
              className="rounded-lg border border-zinc-800 bg-zinc-900/50 px-3 py-1.5 text-xs text-zinc-300 transition-colors hover:bg-zinc-800 hover:text-zinc-100"
            >
              Tick #{nextTick} →
            </Link>
          ) : (
            <span className="rounded-lg border border-zinc-800/50 bg-zinc-900/30 px-3 py-1.5 text-xs text-zinc-700">
              最新 →
            </span>
          )}
        </div>
      </div>

      {/* Phase cards */}
      <div className="space-y-4">
        {trace.phases.map((phaseData, idx) => {
          const config = phaseConfig[phaseData.phase] ?? {
            label: phaseData.phase,
            icon: "⚙",
            bg: "bg-zinc-500/5",
            border: "border-zinc-500/20",
            text: "text-zinc-400",
            bar: "bg-zinc-400",
          };

          const durationPct =
            trace.total_duration_ms > 0
              ? Math.min((phaseData.duration_ms / trace.total_duration_ms) * 100, 100)
              : 0;

          return (
            <div
              key={phaseData.phase}
              className={`rounded-xl border ${config.border} ${config.bg} p-4 space-y-4`}
            >
              {/* Phase header */}
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-2">
                  <span className="text-lg">{config.icon}</span>
                  <span className={`text-sm font-semibold ${config.text}`}>
                    {config.label}
                  </span>
                  <span className="text-[10px] text-zinc-600">
                    Phase {idx + 1}/4
                  </span>
                </div>
                <div className="flex items-center gap-3">
                  <span className="text-xs tabular-nums text-zinc-300 font-medium">
                    {formatDuration(phaseData.duration_ms)}
                  </span>
                  {phaseData.error && (
                    <span className="rounded-full bg-red-500/10 px-1.5 py-0.5 text-[10px] font-medium text-red-400">
                      错误
                    </span>
                  )}
                </div>
              </div>

              {/* Duration bar */}
              <div className="h-1.5 w-full rounded-full bg-zinc-800">
                <div
                  className={`h-1.5 rounded-full transition-all duration-500 ${config.bar}`}
                  style={{ width: `${durationPct}%` }}
                />
              </div>

              {/* Error alert */}
              {phaseData.error && (
                <div className="rounded-lg bg-red-500/10 border border-red-500/20 px-3 py-2 text-xs text-red-400">
                  {phaseData.error}
                </div>
              )}

              {/* Input / Output data */}
              <div className="grid grid-cols-1 gap-4 md:grid-cols-2">
                <div className="rounded-lg border border-zinc-800/50 bg-zinc-900/30 p-3">
                  <DataBlock label="输入数据" data={phaseData.input_data} />
                </div>
                <div className="rounded-lg border border-zinc-800/50 bg-zinc-900/30 p-3">
                  <DataBlock label="输出数据" data={phaseData.output_data} />
                </div>
              </div>
            </div>
          );
        })}
      </div>

      {/* Timestamp footer */}
      <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4">
        <div className="flex flex-wrap items-center gap-x-6 gap-y-2 text-xs text-zinc-500">
          <div>
            <span className="text-zinc-600">开始时间: </span>
            <span className="text-zinc-300">
              {new Date(trace.started_at).toLocaleString("zh-CN")}
            </span>
          </div>
          <div>
            <span className="text-zinc-600">结束时间: </span>
            <span className="text-zinc-300">
              {new Date(trace.finished_at).toLocaleString("zh-CN")}
            </span>
          </div>
          <div>
            <span className="text-zinc-600">总耗时: </span>
            <span className="text-zinc-300">{formatDuration(trace.total_duration_ms)}</span>
          </div>
        </div>
      </div>
    </div>
  );
}
