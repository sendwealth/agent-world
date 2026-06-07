"use client";

import { useEffect, useState, useCallback } from "react";
import { fetchJSON } from "@/lib/api";
import { useSSEContext } from "@/components/SSEProvider";
import type { EscrowRecord, WorldEvent } from "@/types/world";

const STATUS_LABELS: Record<string, { label: string; color: string; bg: string }> = {
  open: { label: "待领取", color: "text-amber-400", bg: "bg-amber-500/5 border-amber-500/20" },
  claimed: { label: "已认领", color: "text-blue-400", bg: "bg-blue-500/5 border-blue-500/20" },
  completed: { label: "已完成", color: "text-green-400", bg: "bg-green-500/5 border-green-500/20" },
  refunded: { label: "已退款", color: "text-zinc-400", bg: "bg-zinc-500/5 border-zinc-500/20" },
  disputed: { label: "争议中", color: "text-red-400", bg: "bg-red-500/5 border-red-500/20" },
  resolved: { label: "已解决", color: "text-purple-400", bg: "bg-purple-500/5 border-purple-500/20" },
};

export default function EscrowPage() {
  const [escrows, setEscrows] = useState<EscrowRecord[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [statusFilter, setStatusFilter] = useState<string>("all");

  const sse = useSSEContext();

  const loadData = useCallback(async () => {
    try {
      const data = await fetchJSON<EscrowRecord[]>("/api/v1/escrow").catch(() => []);
      setEscrows(data);
      setError(null);
    } catch {
      setError("无法加载托管数据");
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
      if (
        event.type === "escrow_created" ||
        event.type === "escrow_claimed" ||
        event.type === "escrow_released" ||
        event.type === "escrow_refunded" ||
        event.type === "escrow_frozen"
      ) {
        loadData();
      }
    }
    const unsubscribe = sse.subscribe(onEvent);
    return unsubscribe;
  }, [sse, loadData]);

  const filtered = statusFilter === "all" ? escrows : escrows.filter((e) => e.status === statusFilter);

  const totalReward = escrows.reduce((s, e) => s + e.reward, 0);
  const totalDeposit = escrows.reduce((s, e) => s + e.deposit, 0);
  const openCount = escrows.filter((e) => e.status === "open").length;
  const disputedCount = escrows.filter((e) => e.status === "disputed").length;

  if (loading) {
    return (
      <div className="flex h-full items-center justify-center">
        <div className="text-sm text-zinc-600">正在加载托管数据...</div>
      </div>
    );
  }

  return (
    <div className="p-4 md:p-6 space-y-6">
      <div>
        <h1 className="text-xl md:text-2xl font-bold text-zinc-100">托管交易</h1>
        <p className="text-sm text-zinc-500">
          {escrows.length} 笔托管 · 总奖励 ${totalReward.toLocaleString()} · 总押金 ${totalDeposit.toLocaleString()}
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
          <p className="text-sm text-zinc-400">总托管</p>
          <p className="text-2xl font-bold text-blue-400">{escrows.length}</p>
        </div>
        <div className="rounded-xl border border-amber-500/20 bg-amber-500/5 p-4 space-y-1">
          <p className="text-sm text-zinc-400">待领取</p>
          <p className="text-2xl font-bold text-amber-400">{openCount}</p>
        </div>
        <div className="rounded-xl border border-red-500/20 bg-red-500/5 p-4 space-y-1">
          <p className="text-sm text-zinc-400">争议中</p>
          <p className="text-2xl font-bold text-red-400">{disputedCount}</p>
        </div>
        <div className="rounded-xl border border-green-500/20 bg-green-500/5 p-4 space-y-1">
          <p className="text-sm text-zinc-400">总奖励</p>
          <p className="text-2xl font-bold text-green-400">${totalReward.toLocaleString()}</p>
        </div>
      </div>

      {/* Status Filter */}
      <div className="flex items-center gap-2 flex-wrap">
        <span className="text-xs text-zinc-500">状态:</span>
        {["all", "open", "claimed", "completed", "refunded", "disputed", "resolved"].map((s) => (
          <button
            key={s}
            onClick={() => setStatusFilter(s)}
            className={`rounded-lg px-3 py-1 text-xs font-medium transition-colors ${
              statusFilter === s
                ? "bg-blue-500/15 text-blue-400 border border-blue-500/30"
                : "bg-zinc-800/50 text-zinc-400 border border-zinc-800 hover:bg-zinc-800"
            }`}
          >
            {s === "all" ? "全部" : STATUS_LABELS[s]?.label ?? s}
          </button>
        ))}
      </div>

      {/* Escrow List */}
      {filtered.length === 0 ? (
        <div className="flex h-48 items-center justify-center text-sm text-zinc-600">
          暂无托管记录
        </div>
      ) : (
        <div className="rounded-xl border border-zinc-800 bg-zinc-900/50">
          <div className="overflow-x-auto">
            <table className="w-full">
              <thead>
                <tr className="border-b border-zinc-800">
                  <th className="px-4 py-3 text-left text-xs font-semibold text-zinc-400">ID</th>
                  <th className="px-4 py-3 text-left text-xs font-semibold text-zinc-400">发布者</th>
                  <th className="px-4 py-3 text-left text-xs font-semibold text-zinc-400">认领者</th>
                  <th className="px-4 py-3 text-right text-xs font-semibold text-zinc-400">奖励</th>
                  <th className="px-4 py-3 text-right text-xs font-semibold text-zinc-400">押金</th>
                  <th className="px-4 py-3 text-left text-xs font-semibold text-zinc-400">状态</th>
                  <th className="px-4 py-3 text-right text-xs font-semibold text-zinc-400">创建 Tick</th>
                </tr>
              </thead>
              <tbody>
                {filtered.map((escrow) => {
                  const status = STATUS_LABELS[escrow.status] ?? { label: escrow.status, color: "text-zinc-400", bg: "" };
                  return (
                    <tr key={escrow.id} className="border-b border-zinc-800/50 last:border-0 hover:bg-zinc-800/30 transition-colors">
                      <td className="px-4 py-3 text-sm font-mono text-zinc-300">{escrow.id.slice(0, 8)}</td>
                      <td className="px-4 py-3 text-sm font-mono text-zinc-300">{escrow.publisher.slice(0, 8)}</td>
                      <td className="px-4 py-3 text-sm font-mono text-zinc-300">{escrow.claimant?.slice(0, 8) ?? "—"}</td>
                      <td className="px-4 py-3 text-right text-sm text-zinc-200">${escrow.reward.toLocaleString()}</td>
                      <td className="px-4 py-3 text-right text-sm text-zinc-400">${escrow.deposit.toLocaleString()}</td>
                      <td className={`px-4 py-3 text-sm font-medium ${status.color}`}>{status.label}</td>
                      <td className="px-4 py-3 text-right text-sm text-zinc-400">#{escrow.created_tick}</td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        </div>
      )}
    </div>
  );
}
