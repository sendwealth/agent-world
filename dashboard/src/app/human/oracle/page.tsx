"use client";

import { useEffect, useRef, useState, useCallback, useMemo } from "react";
import type { Oracle, OracleType, Agent } from "@/types/world";
import { fetchJSON, postJSON } from "@/lib/api";

const HUMAN_ID = "default-human";

const ORACLE_TYPES: { value: OracleType; label: string; icon: string; color: string }[] = [
  { value: "guidance", label: "指引", icon: "💡", color: "bg-blue-500/10 text-blue-400 border-blue-500/30" },
  { value: "warning", label: "警告", icon: "⚠️", color: "bg-amber-500/10 text-amber-400 border-amber-500/30" },
  { value: "blessing", label: "祈福", icon: "✨", color: "bg-green-500/10 text-green-400 border-green-500/30" },
  { value: "curse", label: "诅咒", icon: "🔥", color: "bg-red-500/10 text-red-400 border-red-500/30" },
];

export default function OraclePage() {
  const [oracles, setOracles] = useState<Oracle[]>([]);
  const [agents, setAgents] = useState<Agent[]>([]);
  const [loading, setLoading] = useState(true);
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Form state
  const [oracleType, setOracleType] = useState<OracleType>("guidance");
  const [targetAgentId, setTargetAgentId] = useState("");
  const [content, setContent] = useState("");
  const [agentSearch, setAgentSearch] = useState("");

  const loadRef = useRef<() => void>(() => {});

  useEffect(() => {
    let cancelled = false;
    let loadingDone = false;

    async function load() {
      try {
        const [oraclesData, agentsData] = await Promise.all([
          fetchJSON<Oracle[]>("/api/v1/human/oracles").catch(() => []),
          fetchJSON<Agent[]>("/api/v1/agents").catch(() => []),
        ]);
        if (!cancelled) {
          setOracles(oraclesData);
          setAgents(agentsData);
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

  const aliveAgents = useMemo(
    () => agents.filter((a) => a.alive),
    [agents]
  );

  const filteredAgents = useMemo(() => {
    if (!agentSearch.trim()) return aliveAgents.slice(0, 20);
    const q = agentSearch.toLowerCase();
    return aliveAgents.filter(
      (a) =>
        a.name.toLowerCase().includes(q) || a.id.toLowerCase().includes(q)
    ).slice(0, 20);
  }, [aliveAgents, agentSearch]);

  const handleSubmit = useCallback(
    async (e: React.FormEvent) => {
      e.preventDefault();
      setError(null);

      if (!targetAgentId.trim()) {
        setError("请选择目标 Agent");
        return;
      }
      if (!content.trim()) {
        setError("请输入神谕内容");
        return;
      }
      if (content.length > 500) {
        setError("内容不能超过 500 字");
        return;
      }

      setSubmitting(true);
      try {
        await postJSON<Oracle>("/api/v1/human/oracles", {
          human_id: HUMAN_ID,
          oracle_type: oracleType,
          target_agent_id: targetAgentId.trim(),
          content: content.trim(),
        });
        setContent("");
        setTargetAgentId("");
        setAgentSearch("");
        await loadData();
      } catch (err) {
        setError(err instanceof Error ? err.message : "发送失败");
      } finally {
        setSubmitting(false);
      }
    },
    [oracleType, targetAgentId, content, loadData]
  );

  if (loading) {
    return (
      <div className="flex h-full items-center justify-center">
        <div className="text-sm text-zinc-600">正在加载...</div>
      </div>
    );
  }

  return (
    <div className="p-4 md:p-6 space-y-6">
      <div>
        <h1 className="text-xl md:text-2xl font-bold text-zinc-100">神谕编辑器</h1>
        <p className="text-sm text-zinc-500">
          向 Agent 发送神谕指引其行为 · 已发送 {oracles.length} 条神谕
        </p>
      </div>

      {/* Oracle Form */}
      <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4 space-y-4">
        <h2 className="text-sm font-semibold text-zinc-200">编写神谕</h2>

        <form onSubmit={handleSubmit} className="space-y-4">
          {/* Oracle Type */}
          <div>
            <label className="block text-xs font-medium text-zinc-400 mb-2">
              神谕类型
            </label>
            <div className="flex flex-wrap gap-2">
              {ORACLE_TYPES.map((t) => (
                <button
                  key={t.value}
                  type="button"
                  onClick={() => setOracleType(t.value)}
                  className={`flex items-center gap-1.5 rounded-lg border px-3 py-2 text-sm font-medium transition-colors ${
                    oracleType === t.value
                      ? t.color
                      : "border-zinc-800 bg-zinc-900 text-zinc-400 hover:bg-zinc-800"
                  }`}
                >
                  <span>{t.icon}</span>
                  {t.label}
                </button>
              ))}
            </div>
          </div>

          {/* Target Agent */}
          <div>
            <label className="block text-xs font-medium text-zinc-400 mb-1">
              目标 Agent <span className="text-red-400">*</span>
            </label>
            <input
              type="text"
              value={agentSearch}
              onChange={(e) => {
                setAgentSearch(e.target.value);
                if (targetAgentId) setTargetAgentId("");
              }}
              placeholder="搜索 Agent 名称或 ID..."
              className="w-full rounded-lg border border-zinc-800 bg-zinc-950 px-3 py-2 text-sm text-zinc-200 placeholder-zinc-600 outline-none focus:border-zinc-700 focus:ring-1 focus:ring-zinc-700"
            />
            {agentSearch && !targetAgentId && (
              <div className="mt-1 max-h-40 overflow-y-auto rounded-lg border border-zinc-800 bg-zinc-900">
                {filteredAgents.length === 0 ? (
                  <div className="px-3 py-2 text-xs text-zinc-600">未找到匹配的 Agent</div>
                ) : (
                  filteredAgents.map((a) => (
                    <button
                      key={a.id}
                      type="button"
                      onClick={() => {
                        setTargetAgentId(a.id);
                        setAgentSearch(a.name);
                      }}
                      className="w-full text-left px-3 py-2 text-sm text-zinc-300 hover:bg-zinc-800/50 transition-colors flex items-center justify-between"
                    >
                      <span>{a.name}</span>
                      <span className="text-[10px] text-zinc-600 font-mono">{a.id.slice(0, 8)}...</span>
                    </button>
                  ))
                )}
              </div>
            )}
            {targetAgentId && (
              <div className="mt-1 flex items-center gap-2">
                <span className="text-xs text-green-400">已选择</span>
                <span className="text-xs text-zinc-400">{agentSearch}</span>
                <button
                  type="button"
                  onClick={() => { setTargetAgentId(""); setAgentSearch(""); }}
                  className="text-xs text-zinc-500 hover:text-zinc-300"
                >
                  清除
                </button>
              </div>
            )}
          </div>

          {/* Content */}
          <div>
            <label className="block text-xs font-medium text-zinc-400 mb-1">
              内容 <span className="text-red-400">*</span>
            </label>
            <textarea
              value={content}
              onChange={(e) => setContent(e.target.value)}
              placeholder="输入神谕内容..."
              rows={4}
              maxLength={500}
              className="w-full rounded-lg border border-zinc-800 bg-zinc-950 px-3 py-2 text-sm text-zinc-200 placeholder-zinc-600 outline-none focus:border-zinc-700 focus:ring-1 focus:ring-zinc-700 resize-none"
            />
            <div className="flex justify-end mt-1">
              <span className={`text-[10px] ${content.length > 450 ? "text-amber-400" : "text-zinc-600"}`}>
                {content.length}/500
              </span>
            </div>
          </div>

          {error && (
            <div className="rounded-lg bg-red-500/10 border border-red-500/20 px-3 py-1.5 text-xs text-red-400">
              {error}
            </div>
          )}

          <button
            type="submit"
            disabled={submitting || !targetAgentId || !content.trim()}
            className="rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-500 disabled:opacity-50 transition-colors"
          >
            {submitting ? "发送中..." : "发送神谕"}
          </button>
        </form>
      </div>

      {/* Oracle History */}
      <div className="space-y-3">
        <h2 className="text-sm font-semibold text-zinc-300">神谕历史</h2>
        {oracles.length === 0 ? (
          <div className="flex h-32 items-center justify-center text-sm text-zinc-600 rounded-xl border border-zinc-800 bg-zinc-900/50">
            暂无神谕记录
          </div>
        ) : (
          <div className="space-y-2">
            {oracles.map((oracle) => {
              const typeConfig = ORACLE_TYPES.find((t) => t.value === oracle.oracle_type);
              return (
                <div
                  key={oracle.id}
                  className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4 space-y-2"
                >
                  <div className="flex items-center justify-between">
                    <div className="flex items-center gap-2">
                      <span>{typeConfig?.icon}</span>
                      <span className="text-xs font-medium text-zinc-300">
                        {typeConfig?.label ?? oracle.oracle_type}
                      </span>
                    </div>
                    <div className="flex items-center gap-2">
                      <span className="text-[10px] text-zinc-500">T{oracle.created_tick}</span>
                      <span className={`text-[10px] font-medium px-2 py-0.5 rounded-full ${
                        oracle.status === "delivered"
                          ? "bg-green-500/10 text-green-400"
                          : oracle.status === "acknowledged"
                          ? "bg-blue-500/10 text-blue-400"
                          : "bg-zinc-800 text-zinc-400"
                      }`}>
                        {oracle.status}
                      </span>
                    </div>
                  </div>
                  <p className="text-sm text-zinc-300">{oracle.content}</p>
                  <div className="text-[10px] text-zinc-600">
                    目标: <span className="font-mono">{oracle.target_agent_id.slice(0, 8)}...</span>
                  </div>
                  {oracle.agent_response && (
                    <div className="mt-2 rounded-lg bg-zinc-800/50 px-3 py-2">
                      <span className="text-[10px] text-zinc-500">Agent 回应:</span>
                      <p className="text-xs text-zinc-400 mt-0.5">{oracle.agent_response}</p>
                    </div>
                  )}
                </div>
              );
            })}
          </div>
        )}
      </div>
    </div>
  );
}
