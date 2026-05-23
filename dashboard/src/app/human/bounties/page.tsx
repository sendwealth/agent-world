"use client";

import { useEffect, useRef, useState, useCallback } from "react";
import type { Bounty, BountyStatus, Agent } from "@/types/world";
import { fetchJSON, postJSON } from "@/lib/api";

const HUMAN_ID = "default-human";

const STATUS_CONFIG: Record<BountyStatus, { label: string; color: string }> = {
  open: { label: "开放中", color: "bg-green-500/10 text-green-400" },
  in_progress: { label: "进行中", color: "bg-blue-500/10 text-blue-400" },
  completed: { label: "已完成", color: "bg-zinc-500/10 text-zinc-400" },
  expired: { label: "已过期", color: "bg-zinc-800 text-zinc-500" },
  cancelled: { label: "已取消", color: "bg-zinc-800 text-zinc-500" },
};

const STATUS_FILTERS: { value: BountyStatus | "all"; label: string }[] = [
  { value: "all", label: "全部" },
  { value: "open", label: "开放中" },
  { value: "in_progress", label: "进行中" },
  { value: "completed", label: "已完成" },
  { value: "expired", label: "已过期" },
  { value: "cancelled", label: "已取消" },
];

export default function BountiesPage() {
  const [bounties, setBounties] = useState<Bounty[]>([]);
  const [agents, setAgents] = useState<Agent[]>([]);
  const [loading, setLoading] = useState(true);
  const [statusFilter, setStatusFilter] = useState<BountyStatus | "all">("all");
  const [showCreate, setShowCreate] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Create form
  const [formTitle, setFormTitle] = useState("");
  const [formDesc, setFormDesc] = useState("");
  const [formReward, setFormReward] = useState("100");
  const [formTarget, setFormTarget] = useState("");
  const [formExpires, setFormExpires] = useState("");
  const [submitting, setSubmitting] = useState(false);

  const loadRef = useRef<() => void>(() => {});

  useEffect(() => {
    let cancelled = false;
    let loadingDone = false;

    async function load() {
      try {
        const [bountiesData, agentsData] = await Promise.all([
          fetchJSON<Bounty[]>("/api/v1/human/bounties").catch(() => []),
          fetchJSON<Agent[]>("/api/v1/agents").catch(() => []),
        ]);
        if (!cancelled) {
          setBounties(bountiesData);
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

  const filteredBounties =
    statusFilter === "all"
      ? bounties
      : bounties.filter((b) => b.status === statusFilter);

  const stats = {
    total: bounties.length,
    open: bounties.filter((b) => b.status === "open").length,
    in_progress: bounties.filter((b) => b.status === "in_progress").length,
    completed: bounties.filter((b) => b.status === "completed").length,
    totalReward: bounties.reduce((s, b) => s + b.reward, 0),
  };

  const handleCreate = useCallback(
    async (e: React.FormEvent) => {
      e.preventDefault();
      setError(null);

      if (!formTitle.trim()) {
        setError("请输入悬赏标题");
        return;
      }
      const reward = Number(formReward);
      if (isNaN(reward) || reward <= 0) {
        setError("奖励金额必须大于 0");
        return;
      }

      setSubmitting(true);
      try {
        await postJSON<Bounty>("/api/v1/human/bounties", {
          human_id: HUMAN_ID,
          title: formTitle.trim(),
          description: formDesc.trim(),
          reward,
          target_agent_id: formTarget.trim() || null,
          expires_tick: formExpires ? Number(formExpires) : null,
        });
        setShowCreate(false);
        setFormTitle("");
        setFormDesc("");
        setFormReward("100");
        setFormTarget("");
        setFormExpires("");
        await loadData();
      } catch (err) {
        setError(err instanceof Error ? err.message : "发布失败");
      } finally {
        setSubmitting(false);
      }
    },
    [formTitle, formDesc, formReward, formTarget, formExpires, loadData]
  );

  if (loading) {
    return (
      <div className="flex h-full items-center justify-center">
        <div className="text-sm text-zinc-600">正在加载悬赏市场...</div>
      </div>
    );
  }

  return (
    <div className="p-4 md:p-6 space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-xl md:text-2xl font-bold text-zinc-100">悬赏市场</h1>
          <p className="text-sm text-zinc-500">
            {stats.total} 个悬赏 · 总奖金 {stats.totalReward} tokens
          </p>
        </div>
        <button
          onClick={() => setShowCreate(true)}
          className="rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-500 transition-colors"
        >
          + 发布悬赏
        </button>
      </div>

      {/* Stats */}
      <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
        {[
          { label: "开放中", value: stats.open, color: "text-green-400" },
          { label: "进行中", value: stats.in_progress, color: "text-blue-400" },
          { label: "已完成", value: stats.completed, color: "text-zinc-400" },
          { label: "总奖金", value: stats.totalReward, color: "text-amber-400" },
        ].map((s) => (
          <div key={s.label} className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-3">
            <p className="text-xs text-zinc-500">{s.label}</p>
            <p className={`text-lg font-bold ${s.color}`}>{s.value}</p>
          </div>
        ))}
      </div>

      {/* Filters */}
      <div className="flex flex-wrap gap-1.5">
        {STATUS_FILTERS.map((f) => (
          <button
            key={f.value}
            onClick={() => setStatusFilter(f.value)}
            className={`rounded-lg px-3 py-1.5 text-xs font-medium transition-colors ${
              statusFilter === f.value
                ? "bg-blue-500/15 text-blue-400 border border-blue-500/30"
                : "bg-zinc-800/50 text-zinc-400 border border-zinc-800 hover:bg-zinc-800"
            }`}
          >
            {f.label}
          </button>
        ))}
      </div>

      {/* Bounty List */}
      {filteredBounties.length === 0 ? (
        <div className="flex h-48 items-center justify-center text-sm text-zinc-600 rounded-xl border border-zinc-800 bg-zinc-900/50">
          暂无悬赏
        </div>
      ) : (
        <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
          {filteredBounties.map((bounty) => {
            const cfg = STATUS_CONFIG[bounty.status];
            return (
              <div
                key={bounty.id}
                className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4 space-y-3"
              >
                <div className="flex items-start justify-between gap-2">
                  <h3 className="text-sm font-medium text-zinc-200 line-clamp-1">
                    {bounty.title}
                  </h3>
                  <span className={`shrink-0 text-[10px] font-medium px-2 py-0.5 rounded-full ${cfg.color}`}>
                    {cfg.label}
                  </span>
                </div>

                {bounty.description && (
                  <p className="text-xs text-zinc-500 line-clamp-2">{bounty.description}</p>
                )}

                <div className="flex items-center justify-between">
                  <span className="text-sm font-medium text-amber-400">{bounty.reward} tokens</span>
                  <span className="text-[10px] text-zinc-600">T{bounty.created_tick}</span>
                </div>

                {bounty.claimant_agent_id && (
                  <div className="text-[10px] text-zinc-500">
                    认领者: <span className="font-mono">{bounty.claimant_agent_id.slice(0, 8)}...</span>
                  </div>
                )}

                {bounty.result && (
                  <div className="rounded-lg bg-zinc-800/50 px-3 py-2 text-xs text-zinc-400">
                    {bounty.result}
                  </div>
                )}
              </div>
            );
          })}
        </div>
      )}

      {/* Create Dialog */}
      {showCreate && (
        <div
          className="fixed inset-0 z-50 flex items-center justify-center bg-black/60"
          onClick={() => setShowCreate(false)}
        >
          <div
            role="dialog"
            aria-modal="true"
            className="w-full max-w-lg rounded-xl border border-zinc-800 bg-zinc-900 p-6 shadow-xl"
            onClick={(e) => e.stopPropagation()}
          >
            <h2 className="text-lg font-bold text-zinc-100 mb-4">发布悬赏</h2>
            <form onSubmit={handleCreate} className="space-y-4">
              <div>
                <label className="block text-xs font-medium text-zinc-400 mb-1">
                  标题 <span className="text-red-400">*</span>
                </label>
                <input
                  type="text"
                  value={formTitle}
                  onChange={(e) => setFormTitle(e.target.value)}
                  placeholder="悬赏标题"
                  className="w-full rounded-lg border border-zinc-800 bg-zinc-950 px-3 py-2 text-sm text-zinc-200 placeholder-zinc-600 outline-none focus:border-zinc-700"
                />
              </div>
              <div>
                <label className="block text-xs font-medium text-zinc-400 mb-1">描述</label>
                <textarea
                  value={formDesc}
                  onChange={(e) => setFormDesc(e.target.value)}
                  placeholder="悬赏描述"
                  rows={3}
                  className="w-full rounded-lg border border-zinc-800 bg-zinc-950 px-3 py-2 text-sm text-zinc-200 placeholder-zinc-600 outline-none focus:border-zinc-700 resize-none"
                />
              </div>
              <div className="grid grid-cols-2 gap-4">
                <div>
                  <label className="block text-xs font-medium text-zinc-400 mb-1">
                    奖励 (Token) <span className="text-red-400">*</span>
                  </label>
                  <input
                    type="number"
                    min={1}
                    value={formReward}
                    onChange={(e) => setFormReward(e.target.value)}
                    className="w-full rounded-lg border border-zinc-800 bg-zinc-950 px-3 py-2 text-sm text-zinc-200 outline-none focus:border-zinc-700"
                  />
                </div>
                <div>
                  <label className="block text-xs font-medium text-zinc-400 mb-1">过期 Tick</label>
                  <input
                    type="number"
                    min={1}
                    value={formExpires}
                    onChange={(e) => setFormExpires(e.target.value)}
                    placeholder="可选"
                    className="w-full rounded-lg border border-zinc-800 bg-zinc-950 px-3 py-2 text-sm text-zinc-200 placeholder-zinc-600 outline-none focus:border-zinc-700"
                  />
                </div>
              </div>
              <div>
                <label className="block text-xs font-medium text-zinc-400 mb-1">目标 Agent ID</label>
                <select
                  value={formTarget}
                  onChange={(e) => setFormTarget(e.target.value)}
                  className="w-full rounded-lg border border-zinc-800 bg-zinc-950 px-3 py-2 text-sm text-zinc-200 outline-none focus:border-zinc-700"
                >
                  <option value="">不限</option>
                  {agents.filter((a) => a.alive).map((a) => (
                    <option key={a.id} value={a.id}>{a.name}</option>
                  ))}
                </select>
              </div>

              {error && (
                <div className="rounded-lg bg-red-500/10 border border-red-500/20 px-3 py-1.5 text-xs text-red-400">
                  {error}
                </div>
              )}

              <div className="flex justify-end gap-3 pt-2">
                <button
                  type="button"
                  onClick={() => setShowCreate(false)}
                  className="rounded-lg px-4 py-2 text-sm text-zinc-400 hover:text-zinc-200"
                >
                  取消
                </button>
                <button
                  type="submit"
                  disabled={submitting}
                  className="rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-500 disabled:opacity-50"
                >
                  {submitting ? "发布中..." : "发布悬赏"}
                </button>
              </div>
            </form>
          </div>
        </div>
      )}
    </div>
  );
}
