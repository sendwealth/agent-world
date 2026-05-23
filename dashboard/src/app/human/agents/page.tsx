"use client";

import { useEffect, useRef, useState, useCallback } from "react";
import type { Agent, ClaimedAgent } from "@/types/world";
import { fetchJSON, postJSON } from "@/lib/api";

const HUMAN_ID = "default-human";

export default function HumanAgentsPage() {
  const [agents, setAgents] = useState<Agent[]>([]);
  const [claimed, setClaimed] = useState<ClaimedAgent[]>([]);
  const [loading, setLoading] = useState(true);
  const [claiming, setClaiming] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const loadRef = useRef<() => void>(() => {});

  useEffect(() => {
    let cancelled = false;
    let loadingDone = false;

    async function load() {
      try {
        const [agentsData, claimedData] = await Promise.all([
          fetchJSON<Agent[]>("/api/v1/agents"),
          fetchJSON<ClaimedAgent[]>(`/api/v1/human/agents?human_id=${HUMAN_ID}`).catch(() => []),
        ]);
        if (!cancelled) {
          setAgents(agentsData);
          setClaimed(claimedData);
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

  const handleClaim = useCallback(async (agentId: string) => {
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
  }, [loadData]);

  const claimedIds = new Set(claimed.map((c) => c.agent_id));
  const aliveAgents = agents.filter((a) => a.alive);

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
        <h1 className="text-xl md:text-2xl font-bold text-zinc-100">我的 Agent</h1>
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
              const tokenRatio = agent.tokens / Math.max(agent.age || 1, 1);
              const tokenStatus =
                tokenRatio < 0.5 ? "critical" : tokenRatio < 1 ? "low" : "normal";
              const tokenColors = {
                critical: "text-red-400 bg-red-500/10",
                low: "text-amber-400 bg-amber-500/10",
                normal: "text-green-400 bg-green-500/10",
              };

              return (
                <div
                  key={agent.agent_id}
                  className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4 space-y-3"
                >
                  <div className="flex items-center justify-between">
                    <h3 className="text-sm font-medium text-zinc-200">
                      {agent.agent_name}
                    </h3>
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

                  <div className="grid grid-cols-2 gap-2 text-xs">
                    <div>
                      <span className="text-zinc-500">Token</span>
                      <p className="font-medium text-zinc-200">{agent.tokens}</p>
                    </div>
                    <div>
                      <span className="text-zinc-500">金钱</span>
                      <p className="font-medium text-zinc-200">{agent.money}</p>
                    </div>
                    <div>
                      <span className="text-zinc-500">信誉</span>
                      <p className="font-medium text-zinc-200">{agent.reputation.toFixed(1)}</p>
                    </div>
                    <div>
                      <span className="text-zinc-500">年龄</span>
                      <p className="font-medium text-zinc-200">{agent.age} ticks</p>
                    </div>
                  </div>

                  <div>
                    <span className="text-[10px] text-zinc-500">Token 状态</span>
                    <div className="mt-1">
                      <span className={`text-[10px] font-medium px-2 py-0.5 rounded-full ${tokenColors[tokenStatus]}`}>
                        {tokenStatus === "critical" ? "危急" : tokenStatus === "low" ? "低" : "正常"}
                      </span>
                    </div>
                  </div>

                  {Object.keys(agent.skills).length > 0 && (
                    <div>
                      <span className="text-[10px] text-zinc-500">技能</span>
                      <div className="flex flex-wrap gap-1 mt-1">
                        {Object.entries(agent.skills).map(([skill, level]) => (
                          <span
                            key={skill}
                            className="rounded bg-zinc-800 px-1.5 py-0.5 text-[10px] text-zinc-400"
                          >
                            {skill} Lv.{level}
                          </span>
                        ))}
                      </div>
                    </div>
                  )}
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
                    <th className="px-4 py-3 text-left text-xs font-semibold text-zinc-400">名称</th>
                    <th className="px-4 py-3 text-right text-xs font-semibold text-zinc-400">Token</th>
                    <th className="px-4 py-3 text-right text-xs font-semibold text-zinc-400">金钱</th>
                    <th className="px-4 py-3 text-right text-xs font-semibold text-zinc-400">年龄</th>
                    <th className="px-4 py-3 text-right text-xs font-semibold text-zinc-400">操作</th>
                  </tr>
                </thead>
                <tbody>
                  {aliveAgents.map((agent) => (
                    <tr key={agent.id} className="border-b border-zinc-800/50 last:border-0">
                      <td className="px-4 py-3 text-sm text-zinc-200">{agent.name}</td>
                      <td className="px-4 py-3 text-right text-sm text-zinc-300 tabular-nums">{agent.tokens}</td>
                      <td className="px-4 py-3 text-right text-sm text-zinc-300 tabular-nums">{agent.money}</td>
                      <td className="px-4 py-3 text-right text-sm text-zinc-400">{agent.age}</td>
                      <td className="px-4 py-3 text-right">
                        {claimedIds.has(agent.id) ? (
                          <span className="text-xs text-green-400">已认领</span>
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
