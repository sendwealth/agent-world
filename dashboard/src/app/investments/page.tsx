"use client";

import { useEffect, useState, useMemo, useCallback } from "react";
import { fetchJSON } from "@/lib/api";
import { useSSEContext } from "@/components/SSEProvider";
import type {
  InvestmentProduct,
  InvestmentLeaderboardEntry,
  InvestmentTransaction,
  WorldEvent,
} from "@/types/world";

const STATUS_LABELS: Record<string, { label: string; color: string }> = {
  open: { label: "开放", color: "text-green-400" },
  closed: { label: "已关闭", color: "text-zinc-400" },
  frozen: { label: "冻结", color: "text-blue-400" },
};

const TYPE_LABELS: Record<string, string> = {
  bond: "债券",
  fund: "基金",
  derivative: "衍生品",
  fixed_deposit: "定存",
};

export default function InvestmentsPage() {
  const [products, setProducts] = useState<InvestmentProduct[]>([]);
  const [leaderboard, setLeaderboard] = useState<InvestmentLeaderboardEntry[]>([]);
  const [transactions, setTransactions] = useState<InvestmentTransaction[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [tab, setTab] = useState<"products" | "leaderboard" | "transactions">("products");
  const [statusFilter, setStatusFilter] = useState<string>("all");

  const sse = useSSEContext();

  const loadData = useCallback(async () => {
    try {
      const [productsData, leaderboardData, transactionsData] = await Promise.all([
        fetchJSON<InvestmentProduct[]>("/api/v1/investments/products").catch(() => []),
        fetchJSON<InvestmentLeaderboardEntry[]>("/api/v1/investments/leaderboard").catch(() => []),
        fetchJSON<InvestmentTransaction[]>("/api/v1/investments/transactions").catch(() => []),
      ]);
      setProducts(productsData);
      setLeaderboard(leaderboardData);
      setTransactions(transactionsData);
      setError(null);
    } catch {
      setError("无法加载投资数据");
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    loadData();
    const interval = setInterval(loadData, 10000);
    return () => clearInterval(interval);
  }, [loadData]);

  useEffect(() => {
    function onEvent(event: WorldEvent) {
      if (
        event.type === "investment_product_created" ||
        event.type === "investment_purchased" ||
        event.type === "investment_sold" ||
        event.type === "investment_dividend"
      ) {
        loadData();
      }
    }
    const unsubscribe = sse.subscribe(onEvent);
    return unsubscribe;
  }, [sse, loadData]);

  const filteredProducts = useMemo(
    () => products.filter((p) => statusFilter === "all" || p.status === statusFilter),
    [products, statusFilter]
  );

  const totalValue = products.reduce((s, p) => s + p.total_shares * p.price_per_share, 0);
  const avgReturn = products.length > 0
    ? products.reduce((s, p) => s + p.return_rate, 0) / products.length
    : 0;
  const openProducts = products.filter((p) => p.status === "open").length;

  if (loading) {
    return (
      <div className="flex h-full items-center justify-center">
        <div className="text-sm text-zinc-600">正在加载投资数据...</div>
      </div>
    );
  }

  return (
    <div className="p-4 md:p-6 space-y-6">
      <div>
        <h1 className="text-xl md:text-2xl font-bold text-zinc-100">投资市场</h1>
        <p className="text-sm text-zinc-500">
          {products.length} 个产品 · 总市值 ${totalValue.toLocaleString()} · 平均回报率 {avgReturn.toFixed(2)}%
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
          <p className="text-sm text-zinc-400">总市值</p>
          <p className="text-2xl font-bold text-blue-400">${totalValue.toLocaleString()}</p>
        </div>
        <div className="rounded-xl border border-green-500/20 bg-green-500/5 p-4 space-y-1">
          <p className="text-sm text-zinc-400">开放产品</p>
          <p className="text-2xl font-bold text-green-400">{openProducts}</p>
        </div>
        <div className="rounded-xl border border-amber-500/20 bg-amber-500/5 p-4 space-y-1">
          <p className="text-sm text-zinc-400">平均回报率</p>
          <p className="text-2xl font-bold text-amber-400">{avgReturn.toFixed(2)}%</p>
        </div>
        <div className="rounded-xl border border-purple-500/20 bg-purple-500/5 p-4 space-y-1">
          <p className="text-sm text-zinc-400">交易记录</p>
          <p className="text-2xl font-bold text-purple-400">{transactions.length}</p>
        </div>
      </div>

      {/* Tab Switcher */}
      <div className="flex items-center gap-2">
        {(["products", "leaderboard", "transactions"] as const).map((t) => (
          <button
            key={t}
            onClick={() => setTab(t)}
            className={`rounded-lg px-3 py-1 text-xs font-medium transition-colors ${
              tab === t
                ? "bg-blue-500/15 text-blue-400 border border-blue-500/30"
                : "bg-zinc-800/50 text-zinc-400 border border-zinc-800 hover:bg-zinc-800"
            }`}
          >
            {t === "products" ? "投资产品" : t === "leaderboard" ? "排行榜" : "交易记录"}
          </button>
        ))}
      </div>

      {/* Products Tab */}
      {tab === "products" && (
        <>
          <div className="flex items-center gap-2">
            <span className="text-xs text-zinc-500">状态:</span>
            {["all", "open", "closed", "frozen"].map((s) => (
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

          {filteredProducts.length === 0 ? (
            <div className="flex h-48 items-center justify-center text-sm text-zinc-600">
              暂无投资产品
            </div>
          ) : (
            <div className="grid grid-cols-1 gap-4 md:grid-cols-2 xl:grid-cols-3">
              {filteredProducts.map((product) => {
                const statusInfo = STATUS_LABELS[product.status] ?? { label: product.status, color: "text-zinc-400" };
                return (
                  <div
                    key={product.id}
                    className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4 space-y-3"
                  >
                    <div className="flex items-center justify-between">
                      <h3 className="text-sm font-semibold text-zinc-200">{product.name}</h3>
                      <span className={`text-xs font-medium ${statusInfo.color}`}>{statusInfo.label}</span>
                    </div>
                    <div className="grid grid-cols-2 gap-2 text-xs">
                      <div>
                        <span className="text-zinc-500">类型:</span>{" "}
                        <span className="text-zinc-300">{TYPE_LABELS[product.product_type] ?? product.product_type}</span>
                      </div>
                      <div>
                        <span className="text-zinc-500">价格:</span>{" "}
                        <span className="text-zinc-300">${product.price_per_share}</span>
                      </div>
                      <div>
                        <span className="text-zinc-500">总份额:</span>{" "}
                        <span className="text-zinc-300">{product.total_shares}</span>
                      </div>
                      <div>
                        <span className="text-zinc-500">可用:</span>{" "}
                        <span className="text-zinc-300">{product.available_shares}</span>
                      </div>
                      <div>
                        <span className="text-zinc-500">回报率:</span>{" "}
                        <span className="text-green-400">{product.return_rate.toFixed(2)}%</span>
                      </div>
                      <div>
                        <span className="text-zinc-500">绩效:</span>{" "}
                        <span className="text-zinc-300">{product.performance_score.toFixed(1)}</span>
                      </div>
                    </div>
                    <div className="text-xs text-zinc-500">
                      管理者: {product.manager_id.slice(0, 8)} · 创建于 Tick #{product.created_tick}
                    </div>
                  </div>
                );
              })}
            </div>
          )}
        </>
      )}

      {/* Leaderboard Tab */}
      {tab === "leaderboard" && (
        <>
          {leaderboard.length === 0 ? (
            <div className="flex h-48 items-center justify-center text-sm text-zinc-600">
              暂无排行榜数据
            </div>
          ) : (
            <div className="rounded-xl border border-zinc-800 bg-zinc-900/50">
              <div className="overflow-x-auto">
                <table className="w-full">
                  <thead>
                    <tr className="border-b border-zinc-800">
                      <th className="px-4 py-3 text-left text-xs font-semibold text-zinc-400">排名</th>
                      <th className="px-4 py-3 text-left text-xs font-semibold text-zinc-400">投资者</th>
                      <th className="px-4 py-3 text-right text-xs font-semibold text-zinc-400">总价值</th>
                      <th className="px-4 py-3 text-right text-xs font-semibold text-zinc-400">盈亏</th>
                      <th className="px-4 py-3 text-right text-xs font-semibold text-zinc-400">回报率</th>
                    </tr>
                  </thead>
                  <tbody>
                    {leaderboard.map((entry) => (
                      <tr key={entry.investor_id} className="border-b border-zinc-800/50 last:border-0">
                        <td className="px-4 py-3 text-sm text-zinc-400">#{entry.rank}</td>
                        <td className="px-4 py-3 text-sm font-mono text-zinc-300">{entry.investor_id.slice(0, 8)}</td>
                        <td className="px-4 py-3 text-right text-sm text-zinc-200">${entry.total_value.toLocaleString()}</td>
                        <td className={`px-4 py-3 text-right text-sm tabular-nums ${entry.total_pnl >= 0 ? "text-green-400" : "text-red-400"}`}>
                          {entry.total_pnl >= 0 ? "+" : ""}{entry.total_pnl.toLocaleString()}
                        </td>
                        <td className={`px-4 py-3 text-right text-sm tabular-nums ${entry.pnl_percent >= 0 ? "text-green-400" : "text-red-400"}`}>
                          {entry.pnl_percent >= 0 ? "+" : ""}{entry.pnl_percent.toFixed(2)}%
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </div>
          )}
        </>
      )}

      {/* Transactions Tab */}
      {tab === "transactions" && (
        <>
          {transactions.length === 0 ? (
            <div className="flex h-48 items-center justify-center text-sm text-zinc-600">
              暂无交易记录
            </div>
          ) : (
            <div className="rounded-xl border border-zinc-800 bg-zinc-900/50">
              <div className="overflow-x-auto">
                <table className="w-full">
                  <thead>
                    <tr className="border-b border-zinc-800">
                      <th className="px-4 py-3 text-left text-xs font-semibold text-zinc-400">ID</th>
                      <th className="px-4 py-3 text-left text-xs font-semibold text-zinc-400">类型</th>
                      <th className="px-4 py-3 text-left text-xs font-semibold text-zinc-400">投资者</th>
                      <th className="px-4 py-3 text-right text-xs font-semibold text-zinc-400">份额</th>
                      <th className="px-4 py-3 text-right text-xs font-semibold text-zinc-400">单价</th>
                      <th className="px-4 py-3 text-right text-xs font-semibold text-zinc-400">总额</th>
                      <th className="px-4 py-3 text-right text-xs font-semibold text-zinc-400">Tick</th>
                    </tr>
                  </thead>
                  <tbody>
                    {transactions.map((tx) => (
                      <tr key={tx.id} className="border-b border-zinc-800/50 last:border-0">
                        <td className="px-4 py-3 text-sm font-mono text-zinc-300">{tx.id.slice(0, 8)}</td>
                        <td className={`px-4 py-3 text-sm font-medium ${
                          tx.transaction_type === "buy" ? "text-green-400" :
                          tx.transaction_type === "sell" ? "text-red-400" : "text-amber-400"
                        }`}>
                          {tx.transaction_type === "buy" ? "买入" : tx.transaction_type === "sell" ? "卖出" : "分红"}
                        </td>
                        <td className="px-4 py-3 text-sm font-mono text-zinc-300">{tx.investor_id.slice(0, 8)}</td>
                        <td className="px-4 py-3 text-right text-sm text-zinc-400">{tx.shares}</td>
                        <td className="px-4 py-3 text-right text-sm text-zinc-400">${tx.price}</td>
                        <td className="px-4 py-3 text-right text-sm text-zinc-200">${tx.total.toLocaleString()}</td>
                        <td className="px-4 py-3 text-right text-sm text-zinc-400">#{tx.tick}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </div>
          )}
        </>
      )}
    </div>
  );
}
