"use client";

import { useEffect, useState, useCallback, useMemo } from "react";
import {
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ResponsiveContainer,
  AreaChart,
  Area,
} from "recharts";
import type { HumanPortfolio, Agent } from "@/types/world";
import { fetchJSON, postJSON } from "@/lib/api";

const HUMAN_ID = "default-human";

export default function PortfolioPage() {
  const [portfolio, setPortfolio] = useState<HumanPortfolio | null>(null);
  const [agents, setAgents] = useState<Agent[]>([]);
  const [loading, setLoading] = useState(true);
  const [showInvest, setShowInvest] = useState(false);
  const [investAgent, setInvestAgent] = useState("");
  const [investAmount, setInvestAmount] = useState("100");
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    let loadingDone = false;

    async function load() {
      try {
        const [portfolioData, agentsData] = await Promise.all([
          fetchJSON<HumanPortfolio>(`/api/v1/human/portfolio/${HUMAN_ID}`).catch(() => null),
          fetchJSON<Agent[]>("/api/v1/agents").catch(() => []),
        ]);
        if (!cancelled) {
          setPortfolio(portfolioData);
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

    load();
    const interval = setInterval(load, 10000);
    return () => {
      cancelled = true;
      clearInterval(interval);
    };
  }, []);

  const aliveAgents = useMemo(() => agents.filter((a) => a.alive), [agents]);

  const sortedHoldings = useMemo(() => {
    if (!portfolio) return [];
    return [...portfolio.holdings].sort((a, b) => b.current_value - a.current_value);
  }, [portfolio]);

  const handleInvest = useCallback(
    async (e: React.FormEvent) => {
      e.preventDefault();
      setError(null);

      if (!investAgent) {
        setError("请选择投资目标");
        return;
      }
      const amount = Number(investAmount);
      if (isNaN(amount) || amount <= 0) {
        setError("投资金额必须大于 0");
        return;
      }

      setSubmitting(true);
      try {
        const result = await postJSON<HumanPortfolio>("/api/v1/human/portfolio/invest", {
          human_id: HUMAN_ID,
          agent_id: investAgent,
          amount,
        });
        setPortfolio(result);
        setShowInvest(false);
        setInvestAgent("");
        setInvestAmount("100");
      } catch (err) {
        setError(err instanceof Error ? err.message : "投资失败");
      } finally {
        setSubmitting(false);
      }
    },
    [investAgent, investAmount]
  );

  if (loading) {
    return (
      <div className="flex h-full items-center justify-center">
        <div className="text-sm text-zinc-600">正在加载投资组合...</div>
      </div>
    );
  }

  const pnlPercent = portfolio && portfolio.total_invested > 0
    ? ((portfolio.total_pnl / portfolio.total_invested) * 100).toFixed(2)
    : "0.00";

  return (
    <div className="p-4 md:p-6 space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-xl md:text-2xl font-bold text-zinc-100">投资组合</h1>
          <p className="text-sm text-zinc-500">
            {portfolio ? `${portfolio.holdings.length} 个持仓` : "暂无投资"}
          </p>
        </div>
        <button
          onClick={() => setShowInvest(true)}
          className="rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-500 transition-colors"
        >
          + 投资
        </button>
      </div>

      {/* Stats */}
      {portfolio && (
        <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
          <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4">
            <p className="text-xs text-zinc-500">总资产</p>
            <p className="text-xl font-bold text-zinc-100">{portfolio.total_assets}</p>
          </div>
          <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4">
            <p className="text-xs text-zinc-500">总投入</p>
            <p className="text-xl font-bold text-zinc-300">{portfolio.total_invested}</p>
          </div>
          <div className={`rounded-xl border p-4 ${
            portfolio.total_pnl >= 0
              ? "border-green-500/20 bg-green-500/5"
              : "border-red-500/20 bg-red-500/5"
          }`}>
            <p className="text-xs text-zinc-500">总盈亏</p>
            <p className={`text-xl font-bold ${portfolio.total_pnl >= 0 ? "text-green-400" : "text-red-400"}`}>
              {portfolio.total_pnl >= 0 ? "+" : ""}{portfolio.total_pnl}
            </p>
          </div>
          <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4">
            <p className="text-xs text-zinc-500">收益率</p>
            <p className={`text-xl font-bold ${Number(pnlPercent) >= 0 ? "text-green-400" : "text-red-400"}`}>
              {Number(pnlPercent) >= 0 ? "+" : ""}{pnlPercent}%
            </p>
          </div>
        </div>
      )}

      {/* Asset Trend Chart */}
      {portfolio && portfolio.history.length > 1 && (
        <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4 space-y-3">
          <h2 className="text-sm font-semibold text-zinc-300">资产趋势</h2>
          <ResponsiveContainer width="100%" height={240}>
            <AreaChart data={portfolio.history}>
              <defs>
                <linearGradient id="portfolioGradient" x1="0" y1="0" x2="0" y2="1">
                  <stop offset="5%" stopColor="#3b82f6" stopOpacity={0.3} />
                  <stop offset="95%" stopColor="#3b82f6" stopOpacity={0} />
                </linearGradient>
              </defs>
              <CartesianGrid strokeDasharray="3 3" stroke="#27272a" />
              <XAxis dataKey="tick" stroke="#52525b" tick={{ fontSize: 10 }} tickFormatter={(v: number) => `#${v}`} />
              <YAxis stroke="#52525b" tick={{ fontSize: 10 }} />
              <Tooltip
                content={({ active, payload, label }) => {
                  if (!active || !payload) return null;
                  return (
                    <div className="rounded-lg border border-zinc-700 bg-zinc-800/95 px-3 py-2 text-xs shadow-xl backdrop-blur-sm">
                      <p className="text-zinc-400">Tick #{label}</p>
                      <p className="text-blue-400 font-medium">价值: {payload[0]?.value}</p>
                    </div>
                  );
                }}
              />
              <Area
                type="monotone"
                dataKey="value"
                stroke="#3b82f6"
                fill="url(#portfolioGradient)"
                strokeWidth={2}
                name="价值"
              />
            </AreaChart>
          </ResponsiveContainer>
        </div>
      )}

      {/* Holdings List */}
      {portfolio && sortedHoldings.length > 0 && (
        <div className="space-y-3">
          <h2 className="text-sm font-semibold text-zinc-300">持仓列表</h2>
          <div className="rounded-xl border border-zinc-800 bg-zinc-900/50">
            <div className="overflow-x-auto">
              <table className="w-full">
                <thead>
                  <tr className="border-b border-zinc-800">
                    <th className="px-4 py-3 text-left text-xs font-semibold text-zinc-400">Agent</th>
                    <th className="px-4 py-3 text-right text-xs font-semibold text-zinc-400">投入</th>
                    <th className="px-4 py-3 text-right text-xs font-semibold text-zinc-400">当前价值</th>
                    <th className="px-4 py-3 text-right text-xs font-semibold text-zinc-400">盈亏</th>
                    <th className="px-4 py-3 text-right text-xs font-semibold text-zinc-400">收益率</th>
                  </tr>
                </thead>
                <tbody>
                  {sortedHoldings.map((h) => (
                    <tr key={h.agent_id} className="border-b border-zinc-800/50 last:border-0">
                      <td className="px-4 py-3 text-sm text-zinc-200">{h.agent_name}</td>
                      <td className="px-4 py-3 text-right text-sm text-zinc-300 tabular-nums">{h.invested}</td>
                      <td className="px-4 py-3 text-right text-sm text-zinc-300 tabular-nums">{h.current_value}</td>
                      <td className={`px-4 py-3 text-right text-sm tabular-nums ${h.pnl >= 0 ? "text-green-400" : "text-red-400"}`}>
                        {h.pnl >= 0 ? "+" : ""}{h.pnl}
                      </td>
                      <td className={`px-4 py-3 text-right text-sm tabular-nums ${h.pnl_percent >= 0 ? "text-green-400" : "text-red-400"}`}>
                        {h.pnl_percent >= 0 ? "+" : ""}{h.pnl_percent.toFixed(1)}%
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </div>
        </div>
      )}

      {/* Empty state */}
      {(!portfolio || portfolio.holdings.length === 0) && (
        <div className="flex h-48 items-center justify-center text-sm text-zinc-600 rounded-xl border border-zinc-800 bg-zinc-900/50">
          暂无投资 — 点击&ldquo;投资&rdquo;按钮开始
        </div>
      )}

      {/* Invest Dialog */}
      {showInvest && (
        <div
          className="fixed inset-0 z-50 flex items-center justify-center bg-black/60"
          onClick={() => setShowInvest(false)}
        >
          <div
            role="dialog"
            aria-modal="true"
            className="w-full max-w-md rounded-xl border border-zinc-800 bg-zinc-900 p-6 shadow-xl"
            onClick={(e) => e.stopPropagation()}
          >
            <h2 className="text-lg font-bold text-zinc-100 mb-4">投资 Agent</h2>
            <form onSubmit={handleInvest} className="space-y-4">
              <div>
                <label className="block text-xs font-medium text-zinc-400 mb-1">
                  目标 Agent <span className="text-red-400">*</span>
                </label>
                <select
                  value={investAgent}
                  onChange={(e) => setInvestAgent(e.target.value)}
                  className="w-full rounded-lg border border-zinc-800 bg-zinc-950 px-3 py-2 text-sm text-zinc-200 outline-none focus:border-zinc-700"
                >
                  <option value="">选择 Agent</option>
                  {aliveAgents.map((a) => (
                    <option key={a.id} value={a.id}>{a.name} (Token: {a.tokens})</option>
                  ))}
                </select>
              </div>
              <div>
                <label className="block text-xs font-medium text-zinc-400 mb-1">
                  投资金额 <span className="text-red-400">*</span>
                </label>
                <input
                  type="number"
                  min={1}
                  value={investAmount}
                  onChange={(e) => setInvestAmount(e.target.value)}
                  className="w-full rounded-lg border border-zinc-800 bg-zinc-950 px-3 py-2 text-sm text-zinc-200 outline-none focus:border-zinc-700"
                />
              </div>
              {error && (
                <div className="rounded-lg bg-red-500/10 border border-red-500/20 px-3 py-1.5 text-xs text-red-400">
                  {error}
                </div>
              )}
              <div className="flex justify-end gap-3 pt-2">
                <button
                  type="button"
                  onClick={() => setShowInvest(false)}
                  className="rounded-lg px-4 py-2 text-sm text-zinc-400 hover:text-zinc-200"
                >
                  取消
                </button>
                <button
                  type="submit"
                  disabled={submitting}
                  className="rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-500 disabled:opacity-50"
                >
                  {submitting ? "投资中..." : "确认投资"}
                </button>
              </div>
            </form>
          </div>
        </div>
      )}
    </div>
  );
}
