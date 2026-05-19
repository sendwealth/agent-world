"use client";

import { useEffect, useState, useMemo, useCallback } from "react";
import {
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ResponsiveContainer,
  AreaChart,
  Area,
} from "recharts";
import type { StockData, Organization } from "@/types/world";
import { fetchJSON } from "@/lib/api";
import { useSSEContext } from "@/components/SSEProvider";

// Generate simulated stock data from organizations
function generateStockData(orgs: Organization[]): StockData[] {
  if (orgs.length === 0) return [];

  return orgs
    .filter((o) => o.status === "active")
    .map((org) => {
      const basePrice = Math.max(100, org.treasury / Math.max(org.member_count, 1));
      const history = [];
      for (let i = 20; i >= 0; i--) {
        const variation = (Math.random() - 0.48) * basePrice * 0.1;
        const price = Math.max(10, basePrice + variation - i * (basePrice * 0.005));
        history.push({
          tick: org.last_activity_tick - i * 2,
          price: Math.round(price * 100) / 100,
        });
      }

      const currentPrice = history[history.length - 1]?.price ?? basePrice;
      const previousPrice = history[history.length - 2]?.price ?? currentPrice;
      const change = currentPrice - previousPrice;
      const changePercent = previousPrice > 0 ? (change / previousPrice) * 100 : 0;

      return {
        symbol: org.name.slice(0, 4).toUpperCase(),
        name: org.name,
        price: Math.round(currentPrice * 100) / 100,
        change: Math.round(change * 100) / 100,
        changePercent: Math.round(changePercent * 100) / 100,
        volume: org.member_count * Math.floor(Math.random() * 100 + 50),
        history,
      };
    });
}

const CustomTooltip = ({ active, payload, label }: { active?: boolean; payload?: Array<{ value: number; name: string; color: string }>; label?: number }) => {
  if (!active || !payload) return null;
  return (
    <div className="rounded-lg border border-zinc-700 bg-zinc-800/95 px-3 py-2 text-xs shadow-xl backdrop-blur-sm">
      <p className="text-zinc-400 mb-1">Tick #{label}</p>
      {payload.map((p, i) => (
        <p key={i} style={{ color: p.color }} className="font-medium tabular-nums">
          {p.name}: ${typeof p.value === "number" ? p.value.toFixed(2) : p.value}
        </p>
      ))}
    </div>
  );
};

export default function StocksPage() {
  const [stocks, setStocks] = useState<StockData[]>([]);
  const [loading, setLoading] = useState(true);
  const [selectedStock, setSelectedStock] = useState<string | null>(null);
  const [sortField, setSortField] = useState<"price" | "change" | "volume">("volume");

  const sse = useSSEContext();

  const loadData = useCallback(async () => {
    try {
      const orgsData = await fetchJSON<Organization[]>("/api/v1/orgs");
      setStocks(generateStockData(orgsData));
    } catch {
      // may not have orgs data yet
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    (async () => {
      await loadData();
    })();
    const interval = setInterval(loadData, 15000);
    return () => clearInterval(interval);
  }, [loadData]);

  useEffect(() => {
    function onEvent() {
      loadData();
    }
    const unsubscribe = sse.subscribe(onEvent);
    return unsubscribe;
  }, [sse, loadData]);

  const selected = useMemo(
    () => stocks.find((s) => s.symbol === selectedStock),
    [stocks, selectedStock]
  );

  const sortedStocks = useMemo(() => {
    const sorted = [...stocks].sort((a, b) => {
      if (sortField === "price") return b.price - a.price;
      if (sortField === "change") return b.changePercent - a.changePercent;
      return b.volume - a.volume;
    });
    return sorted;
  }, [stocks, sortField]);

  // Market indices
  const marketCap = stocks.reduce((sum, s) => sum + s.price * s.volume, 0);
  const avgChange = stocks.length > 0 ? stocks.reduce((sum, s) => sum + s.changePercent, 0) / stocks.length : 0;
  const totalVolume = stocks.reduce((sum, s) => sum + s.volume, 0);

  if (loading) {
    return (
      <div className="flex h-full items-center justify-center">
        <div className="text-sm text-zinc-600">正在加载股市数据...</div>
      </div>
    );
  }

  return (
    <div className="p-4 md:p-6 space-y-6">
      {/* Header */}
      <div>
        <h1 className="text-xl md:text-2xl font-bold text-zinc-100">股市走势图</h1>
        <p className="text-sm text-zinc-500">
          {stocks.length > 0
            ? `${stocks.length} 只股票 · 总市值 $${marketCap >= 1000000 ? `${(marketCap / 1000000).toFixed(1)}M` : marketCap.toLocaleString()} · 总成交量 ${totalVolume.toLocaleString()}`
            : "暂无股票数据 (基于组织数据生成)"}
        </p>
      </div>

      {/* Market Indices */}
      <div className="grid grid-cols-1 gap-4 sm:grid-cols-3">
        <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4 space-y-1">
          <p className="text-sm text-zinc-400">市场总值</p>
          <p className="text-2xl font-bold text-zinc-100">${marketCap >= 1000000 ? `${(marketCap / 1000000).toFixed(1)}M` : marketCap.toLocaleString()}</p>
        </div>
        <div className={`rounded-xl border p-4 space-y-1 ${avgChange >= 0 ? "border-green-500/20 bg-green-500/5" : "border-red-500/20 bg-red-500/5"}`}>
          <p className="text-sm text-zinc-400">平均涨跌</p>
          <p className={`text-2xl font-bold ${avgChange >= 0 ? "text-green-400" : "text-red-400"}`}>
            {avgChange >= 0 ? "+" : ""}{avgChange.toFixed(2)}%
          </p>
        </div>
        <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4 space-y-1">
          <p className="text-sm text-zinc-400">活跃股票</p>
          <p className="text-2xl font-bold text-zinc-100">{stocks.length}</p>
        </div>
      </div>

      {/* Stock Chart */}
      {selected && (
        <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4 space-y-3">
          <div className="flex items-center justify-between">
            <div>
              <h2 className="text-sm font-semibold text-zinc-200">{selected.name} ({selected.symbol})</h2>
              <div className="flex items-center gap-2 mt-1">
                <span className="text-lg font-bold text-zinc-100">${selected.price.toFixed(2)}</span>
                <span className={`text-sm font-medium ${selected.change >= 0 ? "text-green-400" : "text-red-400"}`}>
                  {selected.change >= 0 ? "+" : ""}{selected.change.toFixed(2)} ({selected.changePercent >= 0 ? "+" : ""}{selected.changePercent.toFixed(2)}%)
                </span>
              </div>
            </div>
            <button
              onClick={() => setSelectedStock(null)}
              className="text-xs text-zinc-500 hover:text-zinc-300 transition-colors"
            >
              关闭
            </button>
          </div>
          <ResponsiveContainer width="100%" height={280}>
            <AreaChart data={selected.history}>
              <defs>
                <linearGradient id="stockGradient" x1="0" y1="0" x2="0" y2="1">
                  <stop offset="5%" stopColor={selected.change >= 0 ? "#22c55e" : "#ef4444"} stopOpacity={0.3} />
                  <stop offset="95%" stopColor={selected.change >= 0 ? "#22c55e" : "#ef4444"} stopOpacity={0} />
                </linearGradient>
              </defs>
              <CartesianGrid strokeDasharray="3 3" stroke="#27272a" />
              <XAxis dataKey="tick" stroke="#52525b" tick={{ fontSize: 10 }} tickFormatter={(v: number) => `#${v}`} />
              <YAxis stroke="#52525b" tick={{ fontSize: 10 }} tickFormatter={(v: number) => `$${v.toFixed(0)}`} domain={["auto", "auto"]} />
              <Tooltip content={<CustomTooltip />} />
              <Area
                type="monotone"
                dataKey="price"
                stroke={selected.change >= 0 ? "#22c55e" : "#ef4444"}
                fill="url(#stockGradient)"
                strokeWidth={2}
                name="价格"
              />
            </AreaChart>
          </ResponsiveContainer>
        </div>
      )}

      {/* Sort Controls */}
      <div className="flex items-center gap-2">
        <span className="text-xs text-zinc-500">排序:</span>
        {(["volume", "price", "change"] as const).map((f) => {
          const labels = { volume: "成交量", price: "价格", change: "涨跌幅" };
          return (
            <button
              key={f}
              onClick={() => setSortField(f)}
              className={`rounded-lg px-3 py-1 text-xs font-medium transition-colors ${
                sortField === f
                  ? "bg-blue-500/15 text-blue-400 border border-blue-500/30"
                  : "bg-zinc-800/50 text-zinc-400 border border-zinc-800 hover:bg-zinc-800"
              }`}
            >
              {labels[f]}
            </button>
          );
        })}
      </div>

      {/* Stock List */}
      {stocks.length === 0 ? (
        <div className="flex h-48 items-center justify-center text-sm text-zinc-600">
          暂无股票数据 — 需要先创建组织
        </div>
      ) : (
        <div className="rounded-xl border border-zinc-800 bg-zinc-900/50">
          <div className="overflow-x-auto">
            <table className="w-full">
              <thead>
                <tr className="border-b border-zinc-800">
                  <th className="px-4 py-3 text-left text-xs font-semibold text-zinc-400">代码</th>
                  <th className="px-4 py-3 text-left text-xs font-semibold text-zinc-400">名称</th>
                  <th className="px-4 py-3 text-right text-xs font-semibold text-zinc-400">价格</th>
                  <th className="px-4 py-3 text-right text-xs font-semibold text-zinc-400">涨跌</th>
                  <th className="px-4 py-3 text-right text-xs font-semibold text-zinc-400">涨跌幅</th>
                  <th className="px-4 py-3 text-right text-xs font-semibold text-zinc-400">成交量</th>
                  <th className="px-4 py-3 text-left text-xs font-semibold text-zinc-400">走势</th>
                </tr>
              </thead>
              <tbody>
                {sortedStocks.map((stock) => (
                  <tr
                    key={stock.symbol}
                    className="border-b border-zinc-800/50 last:border-0 cursor-pointer hover:bg-zinc-800/30 transition-colors"
                    onClick={() => setSelectedStock(stock.symbol)}
                  >
                    <td className="px-4 py-3 text-sm font-medium text-blue-400">{stock.symbol}</td>
                    <td className="px-4 py-3 text-sm text-zinc-300">{stock.name}</td>
                    <td className="px-4 py-3 text-right text-sm text-zinc-200 tabular-nums">${stock.price.toFixed(2)}</td>
                    <td className={`px-4 py-3 text-right text-sm tabular-nums ${stock.change >= 0 ? "text-green-400" : "text-red-400"}`}>
                      {stock.change >= 0 ? "+" : ""}{stock.change.toFixed(2)}
                    </td>
                    <td className={`px-4 py-3 text-right text-sm tabular-nums ${stock.changePercent >= 0 ? "text-green-400" : "text-red-400"}`}>
                      {stock.changePercent >= 0 ? "+" : ""}{stock.changePercent.toFixed(2)}%
                    </td>
                    <td className="px-4 py-3 text-right text-sm text-zinc-400 tabular-nums">{stock.volume.toLocaleString()}</td>
                    <td className="px-4 py-3 w-32">
                      <svg viewBox="0 0 100 30" className="w-full h-6">
                        <polyline
                          fill="none"
                          stroke={stock.change >= 0 ? "#22c55e" : "#ef4444"}
                          strokeWidth="1.5"
                          points={stock.history
                            .slice(-10)
                            .map((h, i, arr) => {
                              const min = Math.min(...arr.map((a) => a.price));
                              const max = Math.max(...arr.map((a) => a.price));
                              const range = max - min || 1;
                              const x = (i / (arr.length - 1)) * 100;
                              const y = 28 - ((h.price - min) / range) * 26;
                              return `${x},${y}`;
                            })
                            .join(" ")}
                        />
                      </svg>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      )}
    </div>
  );
}
