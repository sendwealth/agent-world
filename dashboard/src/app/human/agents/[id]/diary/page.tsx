"use client";

import { useEffect, useState, useMemo, useCallback, useRef } from "react";
import { useParams, useRouter } from "next/navigation";
import type { Agent, DiaryEntry } from "@/types/world";
import { fetchJSON } from "@/lib/api";
import { formatDate } from "@/lib/format";

// ---------------------------------------------------------------------------
// Mood config: icon + colors
// ---------------------------------------------------------------------------

const moodConfig: Record<string, { icon: string; color: string; bg: string; dot: string }> = {
  happy:    { icon: "😊", color: "text-green-400",  bg: "bg-green-500/10",  dot: "bg-green-400" },
  hopeful:  { icon: "🌅", color: "text-emerald-400",bg: "bg-emerald-500/10", dot: "bg-emerald-400" },
  calm:     { icon: "😌", color: "text-blue-400",   bg: "bg-blue-500/10",   dot: "bg-blue-400" },
  excited:  { icon: "🤩", color: "text-purple-400", bg: "bg-purple-500/10", dot: "bg-purple-400" },
  anxious:  { icon: "😰", color: "text-yellow-400", bg: "bg-yellow-500/10", dot: "bg-yellow-400" },
  fearful:  { icon: "😱", color: "text-red-400",    bg: "bg-red-500/10",    dot: "bg-red-400" },
  angry:    { icon: "😠", color: "text-red-500",    bg: "bg-red-600/10",    dot: "bg-red-500" },
  sad:      { icon: "😢", color: "text-indigo-400", bg: "bg-indigo-500/10", dot: "bg-indigo-400" },
  confused: { icon: "😕", color: "text-zinc-400",   bg: "bg-zinc-500/10",   dot: "bg-zinc-400" },
  neutral:  { icon: "😐", color: "text-zinc-400",   bg: "bg-zinc-800/50",   dot: "bg-zinc-500" },
};

function getMoodStyle(mood: string) {
  return moodConfig[mood] ?? moodConfig.neutral;
}

// ---------------------------------------------------------------------------
// View mode
// ---------------------------------------------------------------------------

type ViewMode = "agent" | "system";

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export default function DiaryPage() {
  const params = useParams();
  const router = useRouter();
  const agentId = params.id as string;

  // Data state
  const [agent, setAgent] = useState<Agent | null>(null);
  const [entries, setEntries] = useState<DiaryEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  // UI state
  const [viewMode, setViewMode] = useState<ViewMode>("agent");
  const [searchQuery, setSearchQuery] = useState("");
  const [searchInput, setSearchInput] = useState("");
  const [moodFilter, setMoodFilter] = useState<string>("all");
  const [daysFilter, setDaysFilter] = useState<number>(7);
  const [expandedTick, setExpandedTick] = useState<number | null>(null);

  const searchTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  // ---- Load agent info ----
  useEffect(() => {
    let cancelled = false;
    async function load() {
      try {
        const data = await fetchJSON<Agent>(`/api/v1/agents/${agentId}`);
        if (!cancelled) setAgent(data);
      } catch {
        if (!cancelled) setError("无法加载 Agent 信息");
      }
    }
    load();
    return () => { cancelled = true; };
  }, [agentId]);

  // ---- Load diary entries ----
  const loadEntries = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const data = await fetchJSON<DiaryEntry[]>(
        `/api/v1/agents/${agentId}/diary?days=${daysFilter}`
      );
      setEntries(data);
    } catch {
      setError("无法加载日记数据，请确认世界引擎已启用日记功能");
      setEntries([]);
    } finally {
      setLoading(false);
    }
  }, [agentId, daysFilter]);

  // ---- Keyword search ----
  const performSearch = useCallback(async (q: string) => {
    if (!q.trim()) {
      loadEntries();
      return;
    }
    setLoading(true);
    setError(null);
    try {
      const data = await fetchJSON<DiaryEntry[]>(
        `/api/v1/agents/${agentId}/diary/search?q=${encodeURIComponent(q)}&limit=50`
      );
      setEntries(data);
    } catch {
      setError("搜索失败");
      setEntries([]);
    } finally {
      setLoading(false);
    }
  }, [agentId, loadEntries]);

  // Initial load + reload on daysFilter change
  useEffect(() => {
    if (!searchQuery) {
      // eslint-disable-next-line react-hooks/set-state-in-effect
      loadEntries();
    }
  }, [loadEntries, searchQuery]);

  // Debounced search
  const handleSearchInput = (val: string) => {
    setSearchInput(val);
    if (searchTimer.current) clearTimeout(searchTimer.current);
    searchTimer.current = setTimeout(() => {
      setSearchQuery(val);
      performSearch(val);
    }, 400);
  };

  // ---- Filtered entries (client-side mood filter) ----
  const filteredEntries = useMemo(() => {
    let result = [...entries];
    if (moodFilter !== "all") {
      result = result.filter((e) => e.mood === moodFilter);
    }
    // Sort newest-first for display
    return result.sort((a, b) => b.tick - a.tick);
  }, [entries, moodFilter]);

  // ---- Mood distribution for filter bar ----
  const moodCounts = useMemo(() => {
    const counts: Record<string, number> = {};
    for (const e of entries) {
      counts[e.mood] = (counts[e.mood] || 0) + 1;
    }
    return counts;
  }, [entries]);

  // ---- Render ----
  return (
    <div className="p-4 md:p-6 space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between flex-wrap gap-3">
        <div className="flex items-center gap-2 sm:gap-4">
          <button
            onClick={() => router.push("/human/agents")}
            className="rounded-lg p-1.5 text-zinc-400 hover:bg-zinc-800 hover:text-zinc-200 transition-colors"
          >
            <svg className="h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M15 19l-7-7 7-7" />
            </svg>
          </button>
          <div>
            <h1 className="text-xl sm:text-2xl font-bold text-zinc-100">
              {agent ? `${agent.name} 的日记` : "Agent 日记"}
            </h1>
            {agent && (
              <p className="text-sm text-zinc-500">
                {agent.alive ? "存活" : "已死亡"} · {(agent.age ?? agent.ticks_survived ?? 0)} Tick · {entries.length} 篇日记
              </p>
            )}
          </div>
        </div>

        {/* View mode toggle */}
        <div className="flex rounded-lg border border-zinc-800 bg-zinc-900/50 p-0.5">
          <button
            onClick={() => setViewMode("agent")}
            className={`rounded-md px-3 py-1.5 text-xs font-medium transition-colors ${
              viewMode === "agent"
                ? "bg-blue-600 text-white"
                : "text-zinc-400 hover:text-zinc-200"
            }`}
          >
            📖 Agent 视角
          </button>
          <button
            onClick={() => setViewMode("system")}
            className={`rounded-md px-3 py-1.5 text-xs font-medium transition-colors ${
              viewMode === "system"
                ? "bg-blue-600 text-white"
                : "text-zinc-400 hover:text-zinc-200"
            }`}
          >
            🔍 系统视角
          </button>
        </div>
      </div>

      {/* Filters */}
      <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4 space-y-4">
        {/* Search + Days */}
        <div className="flex flex-wrap gap-3">
          <div className="relative flex-1 min-w-[200px]">
            <svg
              className="absolute left-3 top-2.5 h-4 w-4 text-zinc-500"
              fill="none" stroke="currentColor" viewBox="0 0 24 24"
            >
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
            </svg>
            <input
              type="text"
              value={searchInput}
              onChange={(e) => handleSearchInput(e.target.value)}
              placeholder="搜索日记内容..."
              className="w-full rounded-lg border border-zinc-800 bg-zinc-900/50 pl-9 pr-3 py-2 text-sm text-zinc-200 placeholder:text-zinc-600 outline-none transition-colors focus:border-blue-500/40"
            />
            {searchInput && (
              <button
                onClick={() => { setSearchInput(""); setSearchQuery(""); loadEntries(); }}
                className="absolute right-2 top-2 text-zinc-500 hover:text-zinc-300"
              >
                <svg className="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
                </svg>
              </button>
            )}
          </div>
          <select
            value={daysFilter}
            onChange={(e) => setDaysFilter(Number(e.target.value))}
            className="appearance-none rounded-lg border border-zinc-800 bg-zinc-900/50 px-3 py-2 text-sm text-zinc-200 outline-none focus:border-blue-500/40"
          >
            <option value={1}>最近 1 天</option>
            <option value={3}>最近 3 天</option>
            <option value={7}>最近 7 天</option>
            <option value={30}>最近 30 天</option>
            <option value={365}>全部</option>
          </select>
        </div>

        {/* Mood filter chips */}
        <div className="flex flex-wrap gap-1.5">
          <button
            onClick={() => setMoodFilter("all")}
            className={`rounded-full px-2.5 py-1 text-[11px] font-medium transition-colors ${
              moodFilter === "all"
                ? "bg-zinc-700 text-zinc-100"
                : "bg-zinc-800/50 text-zinc-500 hover:text-zinc-300"
            }`}
          >
            全部 ({entries.length})
          </button>
          {Object.entries(moodCounts)
            .sort(([, a], [, b]) => b - a)
            .map(([mood, count]) => {
              const style = getMoodStyle(mood);
              return (
                <button
                  key={mood}
                  onClick={() => setMoodFilter(moodFilter === mood ? "all" : mood)}
                  className={`inline-flex items-center gap-1 rounded-full px-2.5 py-1 text-[11px] font-medium transition-colors ${
                    moodFilter === mood
                      ? `${style.bg} ${style.color}`
                      : "bg-zinc-800/50 text-zinc-500 hover:text-zinc-300"
                  }`}
                >
                  <span>{style.icon}</span>
                  {mood} ({count})
                </button>
              );
            })}
        </div>
      </div>

      {/* Error */}
      {error && (
        <div className="rounded-lg bg-red-500/10 border border-red-500/20 px-4 py-3 text-sm text-red-400">
          {error}
        </div>
      )}

      {/* Timeline */}
      <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4 space-y-4">
        <div className="flex items-center justify-between">
          <h2 className="text-sm font-semibold text-zinc-200">
            {viewMode === "agent" ? "日记时间线" : "系统日志"}
          </h2>
          {loading && <span className="text-xs text-zinc-600">加载中...</span>}
        </div>

        {!loading && filteredEntries.length === 0 ? (
          <div className="py-12 text-center">
            <p className="text-3xl mb-3">📝</p>
            <p className="text-sm text-zinc-400">
              {searchQuery ? "没有找到匹配的日记" : "暂无日记数据"}
            </p>
            <p className="text-xs text-zinc-600 mt-1">
              {searchQuery
                ? "尝试不同的关键词"
                : "日记由 Agent Runtime 在每个 Tick 结束后自动生成"}
            </p>
          </div>
        ) : (
          <div className="max-h-[700px] overflow-y-auto scrollbar-thin space-y-0">
            {filteredEntries.map((entry, idx) => {
              const mood = getMoodStyle(entry.mood);
              const isExpanded = expandedTick === entry.tick;

              return (
                <div key={`${entry.agent_id}-${entry.tick}`} className="relative flex gap-3 pb-4">
                  {/* Timeline line */}
                  {idx < filteredEntries.length - 1 && (
                    <div className="absolute left-[7px] top-6 h-full w-px bg-zinc-800" />
                  )}
                  {/* Dot */}
                  <div className={`relative mt-2 h-3.5 w-3.5 shrink-0 rounded-full ${mood.dot}`} />

                  {/* Content */}
                  <div className="min-w-0 flex-1">
                    {/* Agent view */}
                    {viewMode === "agent" ? (
                      <div className="space-y-2">
                        {/* Tick header */}
                        <div className="flex flex-wrap items-center gap-2">
                          <span className="text-xs font-medium text-zinc-200">Tick #{entry.tick}</span>
                          <span className={`inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-[10px] font-medium ${mood.bg} ${mood.color}`}>
                            {mood.icon} {entry.mood}
                          </span>
                          {entry.phase && (
                            <span className="rounded-full bg-zinc-800 px-2 py-0.5 text-[10px] font-medium text-zinc-500">
                              {entry.phase}
                            </span>
                          )}
                          <span className="text-[10px] text-zinc-600">
                            {entry.created_at ? formatDate(entry.created_at) : ""}
                          </span>
                          <button
                            onClick={() => setExpandedTick(isExpanded ? null : entry.tick)}
                            className="ml-auto text-[10px] text-zinc-600 hover:text-zinc-300 transition-colors"
                          >
                            {isExpanded ? "收起" : "展开详情"}
                          </button>
                        </div>

                        {/* Summary (first-person narrative) */}
                        <p className="text-sm text-zinc-300 leading-relaxed">
                          {entry.summary}
                        </p>

                        {/* Expanded detail */}
                        {isExpanded && (
                          <div className="ml-2 space-y-3 border-l-2 border-zinc-800 pl-4 pt-1 pb-1">
                            {/* Key events */}
                            {entry.key_events.length > 0 && (
                              <div>
                                <span className="text-[10px] font-medium text-zinc-500 uppercase tracking-wider">
                                  关键事件
                                </span>
                                <div className="mt-1 flex flex-wrap gap-1">
                                  {entry.key_events.map((ev, i) => (
                                    <span
                                      key={i}
                                      className="rounded bg-zinc-800 px-2 py-0.5 text-[11px] text-zinc-400"
                                    >
                                      {ev}
                                    </span>
                                  ))}
                                </div>
                              </div>
                            )}

                            {/* Decisions */}
                            {entry.decisions.length > 0 && (
                              <div>
                                <span className="text-[10px] font-medium text-zinc-500 uppercase tracking-wider">
                                  决策
                                </span>
                                <ul className="mt-1 space-y-0.5">
                                  {entry.decisions.map((d, i) => (
                                    <li key={i} className="text-xs text-zinc-400 flex items-start gap-1.5">
                                      <span className="mt-1 h-1 w-1 shrink-0 rounded-full bg-blue-400" />
                                      {d}
                                    </li>
                                  ))}
                                </ul>
                              </div>
                            )}

                            {/* Reflection */}
                            {entry.reflection && (
                              <div>
                                <span className="text-[10px] font-medium text-zinc-500 uppercase tracking-wider">
                                  反思
                                </span>
                                <p className="mt-1 text-xs text-zinc-400 italic leading-relaxed">
                                  &ldquo;{entry.reflection}&rdquo;
                                </p>
                              </div>
                            )}
                          </div>
                        )}
                      </div>
                    ) : (
                      /* System view (TraceStore mode) */
                      <div className="space-y-1.5">
                        <div className="flex flex-wrap items-center gap-2">
                          <span className="text-xs font-mono font-medium text-zinc-300">
                            tick={entry.tick}
                          </span>
                          <span className={`rounded px-1.5 py-0.5 text-[10px] font-mono ${mood.bg} ${mood.color}`}>
                            mood={entry.mood}
                          </span>
                          <span className="rounded bg-zinc-800 px-1.5 py-0.5 text-[10px] font-mono text-zinc-500">
                            phase={entry.phase}
                          </span>
                          <span className="text-[10px] font-mono text-zinc-600">
                            {entry.created_at}
                          </span>
                        </div>
                        <pre className="text-xs text-zinc-400 leading-relaxed whitespace-pre-wrap break-words font-mono bg-zinc-900 rounded-lg p-2 border border-zinc-800">
                          {JSON.stringify(entry, null, 2)}
                        </pre>
                      </div>
                    )}
                  </div>
                </div>
              );
            })}
          </div>
        )}
      </div>
    </div>
  );
}
