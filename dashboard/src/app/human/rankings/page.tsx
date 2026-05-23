"use client";

import { useEffect, useState, useMemo } from "react";
import type { HumanInfluenceEntry } from "@/types/world";
import { fetchJSON } from "@/lib/api";

const SORT_OPTIONS = [
  { value: "total_influence", label: "综合影响力" },
  { value: "economic_impact", label: "经济影响" },
  { value: "political_impact", label: "政治影响" },
  { value: "cultural_impact", label: "文化传播" },
] as const;

export default function RankingsPage() {
  const [rankings, setRankings] = useState<HumanInfluenceEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [sortBy, setSortBy] = useState<string>("total_influence");

  useEffect(() => {
    let cancelled = false;
    let loadingDone = false;

    async function load() {
      try {
        const data = await fetchJSON<HumanInfluenceEntry[]>(
          `/api/v1/human/rankings?sort_by=${sortBy}&limit=100`
        );
        if (!cancelled) {
          setRankings(data);
        }
      } catch {
        // API may not be available
      } finally {
        if (!cancelled && !loadingDone) {
          loadingDone = true;
          setLoading(false);
        }
      }
    }

    load();
    const interval = setInterval(load, 15000);
    return () => {
      cancelled = true;
      clearInterval(interval);
    };
  }, [sortBy]);

  const top10 = useMemo(() => rankings.slice(0, 10), [rankings]);
  const maxInfluence = useMemo(
    () => Math.max(...top10.map((r) => r.total_influence), 1),
    [top10]
  );

  if (loading) {
    return (
      <div className="flex h-full items-center justify-center">
        <div className="text-sm text-zinc-600">正在加载排行榜...</div>
      </div>
    );
  }

  return (
    <div className="p-4 md:p-6 space-y-6">
      <div>
        <h1 className="text-xl md:text-2xl font-bold text-zinc-100">人类影响力排行</h1>
        <p className="text-sm text-zinc-500">
          {rankings.length} 位参与者 · 按{SORT_OPTIONS.find((s) => s.value === sortBy)?.label ?? "综合影响力"}排序
        </p>
      </div>

      {/* Sort Controls */}
      <div className="flex flex-wrap gap-1.5">
        {SORT_OPTIONS.map((opt) => (
          <button
            key={opt.value}
            onClick={() => setSortBy(opt.value)}
            className={`rounded-lg px-3 py-1.5 text-xs font-medium transition-colors ${
              sortBy === opt.value
                ? "bg-blue-500/15 text-blue-400 border border-blue-500/30"
                : "bg-zinc-800/50 text-zinc-400 border border-zinc-800 hover:bg-zinc-800"
            }`}
          >
            {opt.label}
          </button>
        ))}
      </div>

      {/* Top 10 Chart */}
      {top10.length > 0 && (
        <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4 space-y-3">
          <h2 className="text-sm font-semibold text-zinc-300">Top 10</h2>
          <div className="space-y-2">
            {top10.map((entry, idx) => {
              const width = Math.max(
                (entry.total_influence / maxInfluence) * 100,
                5
              );
              const medalColor =
                idx === 0
                  ? "text-amber-400"
                  : idx === 1
                  ? "text-zinc-300"
                  : idx === 2
                  ? "text-amber-600"
                  : "text-zinc-500";
              return (
                <div key={entry.human_id} className="flex items-center gap-3">
                  <span className={`text-sm font-bold w-6 text-right ${medalColor}`}>
                    {idx + 1}
                  </span>
                  <span className="text-sm text-zinc-300 w-28 truncate">
                    {entry.display_name}
                  </span>
                  <div className="flex-1 h-6 bg-zinc-800/50 rounded-lg overflow-hidden">
                    <div
                      className="h-full bg-blue-500/30 rounded-lg flex items-center px-2"
                      style={{ width: `${width}%` }}
                    >
                      <span className="text-[10px] text-blue-300 font-medium">
                        {entry.total_influence}
                      </span>
                    </div>
                  </div>
                </div>
              );
            })}
          </div>
        </div>
      )}

      {/* Full Rankings Table */}
      {rankings.length > 0 ? (
        <div className="rounded-xl border border-zinc-800 bg-zinc-900/50">
          <div className="overflow-x-auto">
            <table className="w-full">
              <thead>
                <tr className="border-b border-zinc-800">
                  <th className="px-4 py-3 text-left text-xs font-semibold text-zinc-400">#</th>
                  <th className="px-4 py-3 text-left text-xs font-semibold text-zinc-400">参与者</th>
                  <th className="px-4 py-3 text-right text-xs font-semibold text-zinc-400">影响力</th>
                  <th className="px-4 py-3 text-right text-xs font-semibold text-zinc-400">神谕</th>
                  <th className="px-4 py-3 text-right text-xs font-semibold text-zinc-400">悬赏</th>
                  <th className="px-4 py-3 text-right text-xs font-semibold text-zinc-400">经济</th>
                  <th className="px-4 py-3 text-right text-xs font-semibold text-zinc-400">政治</th>
                  <th className="px-4 py-3 text-right text-xs font-semibold text-zinc-400">文化</th>
                </tr>
              </thead>
              <tbody>
                {rankings.map((entry, idx) => (
                  <tr
                    key={entry.human_id}
                    className="border-b border-zinc-800/50 last:border-0 hover:bg-zinc-800/20 transition-colors"
                  >
                    <td className="px-4 py-3 text-sm text-zinc-400">{idx + 1}</td>
                    <td className="px-4 py-3 text-sm text-zinc-200 font-medium">
                      {entry.display_name}
                    </td>
                    <td className="px-4 py-3 text-right text-sm text-blue-400 font-medium tabular-nums">
                      {entry.total_influence}
                    </td>
                    <td className="px-4 py-3 text-right text-sm text-zinc-300 tabular-nums">
                      {entry.oracle_count}
                    </td>
                    <td className="px-4 py-3 text-right text-sm text-zinc-300 tabular-nums">
                      {entry.bounty_count}
                    </td>
                    <td className="px-4 py-3 text-right text-sm text-amber-400 tabular-nums">
                      {entry.economic_impact}
                    </td>
                    <td className="px-4 py-3 text-right text-sm text-purple-400 tabular-nums">
                      {entry.political_impact}
                    </td>
                    <td className="px-4 py-3 text-right text-sm text-green-400 tabular-nums">
                      {entry.cultural_impact}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      ) : (
        <div className="flex h-48 items-center justify-center text-sm text-zinc-600 rounded-xl border border-zinc-800 bg-zinc-900/50">
          暂无排行数据 — 开始发送神谕或发布悬赏
        </div>
      )}
    </div>
  );
}
