"use client";

import { useEffect, useRef, useState, useCallback, useMemo } from "react";
import Link from "next/link";
import type { Agent, ClaimedAgent, Oracle, Bounty, DiaryEntry } from "@/types/world";
import { fetchJSON, postJSON } from "@/lib/api";

const HUMAN_ID = "default-human";

const MOOD_DISPLAY: Record<string, { icon: string; color: string; bg: string }> = {
  happy: { icon: "😊", color: "text-green-400", bg: "bg-green-500/10" },
  content: { icon: "😌", color: "text-green-300", bg: "bg-green-500/10" },
  neutral: { icon: "😐", color: "text-zinc-400", bg: "bg-zinc-800" },
  anxious: { icon: "😟", color: "text-amber-400", bg: "bg-amber-500/10" },
  sad: { icon: "😢", color: "text-blue-400", bg: "bg-blue-500/10" },
  angry: { icon: "😠", color: "text-red-400", bg: "bg-red-500/10" },
  fearful: { icon: "😨", color: "text-red-300", bg: "bg-red-500/10" },
  hopeful: { icon: "🤞", color: "text-purple-400", bg: "bg-purple-500/10" },
  desperate: { icon: "😰", color: "text-red-500", bg: "bg-red-500/15" },
};

function getMood(mood: string) {
  return MOOD_DISPLAY[mood.toLowerCase()] ?? { icon: "❓", color: "text-zinc-400", bg: "bg-zinc-800" };
}

export default function HumanAgentsPage() {
  const [agents, setAgents] = useState<Agent[]>([]);
  const [claimed, setClaimed] = useState<ClaimedAgent[]>([]);
  const [oracles, setOracles] = useState<Oracle[]>([]);
  const [bounties, setBounties] = useState<Bounty[]>([]);
  const [diaryMap, setDiaryMap] = useState<Record<string, DiaryEntry>>({});
  const [loading, setLoading] = useState(true);
  const [claiming, setClaiming] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const loadRef = useRef<() => void>(() => {});

  useEffect(() => {
    let cancelled = false;
    let loadingDone = false;

    async function load() {
      try {
        const [agentsData, claimedData, oraclesData, bountiesData] =
          await Promise.all([
            fetchJSON<Agent[]>("/api/v1/agents"),
            fetchJSON<ClaimedAgent[]>(
              `/api/v1/human/agents?human_id=${HUMAN_ID}`
            ).catch(() => []),
            fetchJSON<Oracle[]>("/api/v1/human/oracles").catch(() => []),
            fetchJSON<Bounty[]>("/api/v1/human/bounties").catch(() => []),
          ]);

        // Load last diary for each claimed agent
        const newDiaryMap: Record<string, DiaryEntry> = {};
        await Promise.all(
          (claimedData as ClaimedAgent[]).map(async (agent) => {
            try {
              const entries = await fetchJSON<DiaryEntry[]>(
                `/api/v1/agents/${agent.agent_id}/diary?days=7`
              );
              if (entries.length > 0) {
                newDiaryMap[agent.agent_id] = entries[entries.length - 1];
              }
            } catch {
              // diary not available for this agent
            }
          })
        );

        if (!cancelled) {
          setAgents(agentsData);
          setClaimed(claimedData);
          setOracles(oraclesData);
          setBounties(bountiesData);
          setDiaryMap(newDiaryMap);
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

    loadRef.current = load;
    load();
    const interval = setInterval(load, 10000);
    return () => {
      cancelled = true;
      clearInterval(interval);
    };
  }, []);

  const loadData = useCallback(() => loadRef.current(), []);

  const handleClaim = useCallback(
    async (agentId: string) => {
      setClaiming(agentId);
      setError(null);
      try {
        await postJSON<ClaimedAgent>("/api/v1/human/agents/claim", {
          human_id: HUMAN_ID,
          agent_id: agentId,
        });
        await loadData();
      } catch (err) {
        setError(err instanceof Error ? err.message : "认领失败");
      } finally {
        setClaiming(null);
      }
    },
    [loadData]
  );

  const claimedIds = useMemo(
    () => new Set(claimed.map((c) => c.agent_id)),
    [claimed]
  );
  const aliveAgents = useMemo(
    () => agents.filter((a) => a.alive),
    [agents]
  );

  // Oracle stats per agent
  const oracleStats = useMemo(() => {
    const stats: Record<string, { pending: number; responded: number }> = {};
    for (const o of oracles) {
      if (!stats[o.target_agent_id]) {
        stats[o.target_agent_id] = { pending: 0, responded: 0 };
      }
      if (o.status === "pending" || o.status === "delivered") {
        stats[o.target_agent_id].pending++;
      } else if (o.status === "acknowledged") {
        stats[o.target_agent_id].responded++;
      }
    }
    return stats;
  }, [oracles]);

  // Bounty stats per agent
  const bountyStats = useMemo(() => {
    const stats: Record<string, number> = {};
    for (const b of bounties) {
      if (b.status === "open" || b.status === "in_progress") {
        const key = b.target_agent_id ?? b.claimant_agent_id;
        if (key) {
          stats[key] = (stats[key] ?? 0) + 1;
        }
      }
    }
    return stats;
  }, [bounties]);

  if (loading) {
    return (
      <div className="flex h-full items-center justify-center">
        <div className="text-sm text-zinc-600">正在加载 Agent 数据...</div>
      </div>
    );
  }

  return (
    <div className="p-4 md:p-6 space-y-6">
      <div>
        <h1 className="text-xl md:text-2xl font-bold text-zinc-100">
          我的 Agent
        </h1>
        <p className="text-sm text-zinc-500">
          已认领 {claimed.length} 个 Agent · 共 {aliveAgents.length} 个存活
        </p>
      </div>

      {error && (
        <div className="rounded-lg bg-red-500/10 border border-red-500/20 px-3 py-1.5 text-xs text-red-400">
          {error}
        </div>
      )}

      {/* Claimed Agents */}
      {claimed.length > 0 && (
        <div className="space-y-3">
          <h2 className="text-sm font-semibold text-zinc-300">已认领的 Agent</h2>
          <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
            {claimed.map((agent) => {
              const tokenRatio =
                agent.tokens / Math.max(agent.age ?? 1, 1);
              const tokenStatus =
                tokenRatio < 0.5
                  ? "critical"
                  : tokenRatio < 1
                  ? "low"
                  : "normal";
              const tokenColors = {
                critical: "text-red-400 bg-red-500/10",
                low: "text-amber-400 bg-amber-500/10",
                normal: "text-green-400 bg-green-500/10",
              };
              const lastDiary = diaryMap[agent.agent_id];
              const mood = lastDiary ? getMood(lastDiary.mood) : null;
              const oStats = oracleStats[agent.agent_id] ?? {
                pending: 0,
                responded: 0,
              };
              const activeBounties = bountyStats[agent.agent_id] ?? 0;
              const isUrgent =
                tokenStatus === "critical" ||
                (lastDiary &&
                  (lastDiary.mood.toLowerCase() === "desperate" ||
                    lastDiary.mood.toLowerCase() === "fearful"));

              return (
                <div
                  key={agent.agent_id}
                  className={`rounded-xl border p-4 space-y-3 transition-colors ${
                    isUrgent
                      ? "border-red-500/30 bg-red-500/5"
                      : "border-zinc-800 bg-zinc-900/50"
                  }`}
                >
                  {/* Header row with mood */}
                  <div className="flex items-center justify-between">
                    <div className="flex items-center gap-2">
                      {mood && (
                        <span
                          className={`text-base ${mood.color}`}
                          title={`情绪: ${lastDiary?.mood ?? "未知"}`}
                        >
                          {mood.icon}
                        </span>
                      )}
                      <h3 className="text-sm font-medium text-zinc-200">
                        {agent.agent_name}
                      </h3>
                    </div>
                    <div className="flex items-center gap-1.5">
                      {isUrgent && (
                        <span className="animate-pulse text-red-400 text-[10px]">
                          ⚠
                        </span>
                      )}
                      <span
                        className={`text-[10px] font-medium px-2 py-0.5 rounded-full ${
                          agent.alive
                            ? "bg-green-500/10 text-green-400"
                            : "bg-zinc-800 text-zinc-500"
                        }`}
                      >
                        {agent.alive ? "存活" : "死亡"}
                      </span>
                    </div>
                  </div>

                  {/* Stats grid */}
                  <div className="grid grid-cols-2 gap-2 text-xs">
                    <div>
                      <span className="text-zinc-500">Token</span>
                      <p className="font-medium text-zinc-200">
                        {agent.tokens}
                      </p>
                    </div>
                    <div>
                      <span className="text-zinc-500">金钱</span>
                      <p className="font-medium text-zinc-200">
                        {agent.money}
                      </p>
                    </div>
                    <div>
                      <span className="text-zinc-500">信誉</span>
                      <p className="font-medium text-zinc-200">
                        {(agent.reputation ?? 0).toFixed(1)}
                      </p>
                    </div>
                    <div>
                      <span className="text-zinc-500">年龄</span>
                      <p className="font-medium text-zinc-200">
                        {(agent.age ?? 0)} ticks
                      </p>
                    </div>
                  </div>

                  {/* Token status */}
                  <div>
                    <span className="text-[10px] text-zinc-500">
                      Token 状态
                    </span>
                    <div className="mt-1">
                      <span
                        className={`text-[10px] font-medium px-2 py-0.5 rounded-full ${tokenColors[tokenStatus]}`}
                      >
                        {tokenStatus === "critical"
                          ? "危急"
                          : tokenStatus === "low"
                          ? "低"
                          : "正常"}
                      </span>
                    </div>
                  </div>

                  {/* Last diary excerpt */}
                  {lastDiary && (
                    <div className="rounded-lg bg-zinc-800/50 px-3 py-2">
                      <div className="flex items-center gap-1.5 mb-1">
                        <span className={`text-xs ${mood?.color ?? "text-zinc-400"}`}>
                          {mood?.icon ?? "📝"}
                        </span>
                        <span className="text-[10px] text-zinc-500">
                          最近日记 · T{lastDiary.tick}
                        </span>
                        {lastDiary.phase && (
                          <span className="rounded bg-zinc-700 px-1 py-0.5 text-[10px] text-zinc-400">
                            {lastDiary.phase}
                          </span>
                        )}
                      </div>
                      <p className="text-xs text-zinc-400 line-clamp-2">
                        {lastDiary.summary}
                      </p>
                    </div>
                  )}

                  {/* Oracle status */}
                  <div className="flex items-center gap-3 text-[10px]">
                    <span className="text-green-400">
                      {oStats.pending} 待回应
                    </span>
                    <span className="text-blue-400">
                      {oStats.responded} 已回应
                    </span>
                    {activeBounties > 0 && (
                      <span className="text-amber-400">
                        {activeBounties} 悬赏
                      </span>
                    )}
                  </div>

                  {/* Skills */}
                  {Object.keys(agent.skills).length > 0 && (
                    <div>
                      <span className="text-[10px] text-zinc-500">技能</span>
                      <div className="flex flex-wrap gap-1 mt-1">
                        {Object.entries(agent.skills).map(
                          ([skill, level]) => (
                            <span
                              key={skill}
                              className="rounded bg-zinc-800 px-1.5 py-0.5 text-[10px] text-zinc-400"
                            >
                              {skill} Lv.{level}
                            </span>
                          )
                        )}
                      </div>
                    </div>
                  )}

                  {/* Chat link */}
                  <Link
                    href={`/human/agents/${agent.agent_id}/chat`}
                    className="flex items-center justify-center gap-1.5 rounded-lg border border-zinc-800 bg-zinc-900 px-3 py-2 text-xs font-medium text-zinc-300 hover:bg-zinc-800 hover:text-zinc-100 transition-colors"
                  >
                    <svg
                      className="h-3.5 w-3.5"
                      fill="none"
                      stroke="currentColor"
                      viewBox="0 0 24 24"
                    >
                      <path
                        strokeLinecap="round"
                        strokeLinejoin="round"
                        strokeWidth={2}
                        d="M8 12h.01M12 12h.01M16 12h.01M21 12c0 4.418-4.03 8-9 8a9.863 9.863 0 01-4.255-.949L3 20l1.395-3.72C3.512 15.042 3 13.574 3 12c0-4.418 4.03-8 9-8s9 3.582 9 8z"
                      />
                    </svg>
                    对话
                  </Link>
                </div>
              );
            })}
          </div>
        </div>
      )}

      {/* Available Agents */}
      <div className="space-y-3">
        <h2 className="text-sm font-semibold text-zinc-300">可认领的 Agent</h2>
        {aliveAgents.length === 0 ? (
          <div className="flex h-32 items-center justify-center text-sm text-zinc-600">
            暂无可认领的 Agent
          </div>
        ) : (
          <div className="rounded-xl border border-zinc-800 bg-zinc-900/50">
            <div className="overflow-x-auto">
              <table className="w-full">
                <thead>
                  <tr className="border-b border-zinc-800">
                    <th className="px-4 py-3 text-left text-xs font-semibold text-zinc-400">
                      名称
                    </th>
                    <th className="px-4 py-3 text-right text-xs font-semibold text-zinc-400">
                      Token
                    </th>
                    <th className="px-4 py-3 text-right text-xs font-semibold text-zinc-400">
                      金钱
                    </th>
                    <th className="px-4 py-3 text-right text-xs font-semibold text-zinc-400">
                      年龄
                    </th>
                    <th className="px-4 py-3 text-right text-xs font-semibold text-zinc-400">
                      操作
                    </th>
                  </tr>
                </thead>
                <tbody>
                  {aliveAgents.map((agent) => (
                    <tr
                      key={agent.id}
                      className="border-b border-zinc-800/50 last:border-0"
                    >
                      <td className="px-4 py-3 text-sm text-zinc-200">
                        {agent.name}
                      </td>
                      <td className="px-4 py-3 text-right text-sm text-zinc-300 tabular-nums">
                        {agent.tokens}
                      </td>
                      <td className="px-4 py-3 text-right text-sm text-zinc-300 tabular-nums">
                        {agent.money}
                      </td>
                      <td className="px-4 py-3 text-right text-sm text-zinc-400">
                        {(agent.age ?? 0)}
                      </td>
                      <td className="px-4 py-3 text-right">
                        {claimedIds.has(agent.id) ? (
                          <span className="text-xs text-green-400">
                            已认领
                          </span>
                        ) : (
                          <button
                            onClick={() => handleClaim(agent.id)}
                            disabled={claiming === agent.id}
                            className="rounded-lg bg-blue-600 px-3 py-1 text-xs font-medium text-white hover:bg-blue-500 disabled:opacity-50 transition-colors"
                          >
                            {claiming === agent.id ? "认领中..." : "认领"}
                          </button>
                        )}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
