"use client";

import { useEffect, useState, useMemo, useCallback } from "react";
import {
  LineChart,
  Line,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ResponsiveContainer,
  AreaChart,
  Area,
  BarChart,
  Bar,
} from "recharts";
import type { WorldSnapshotData, SkillCount } from "@/types/world";
import { fetchJSON } from "@/lib/api";
import { useSSEContext } from "@/components/SSEProvider";
import type { WorldEvent, WorldStats } from "@/types/world";

interface SnapshotData {
  tick: number;
  gdp: number;
  gini: number;
  population: number;
  activeAgents: number;
  topSkills: SkillCount[];
}

const CustomTooltip = ({ active, payload, label }: { active?: boolean; payload?: Array<{ value: number; name: string; color: string }>; label?: number }) => {
  if (!active || !payload) return null;
  return (
    <div className="rounded-lg border border-zinc-700 bg-zinc-800/95 px-3 py-2 text-xs shadow-xl backdrop-blur-sm">
      <p className="text-zinc-400 mb-1">Tick #{label}</p>
      {payload.map((p, i) => (
        <p key={i} style={{ color: p.color }} className="font-medium tabular-nums">
          {p.name}: {typeof p.value === "number" ? p.value.toLocaleString(undefined, { maximumFractionDigits: 2 }) : p.value}
        </p>
      ))}
    </div>
  );
};

export default function EconomyPage() {
  const [snapshots, setSnapshots] = useState<SnapshotData[]>([]);
  const [stats, setStats] = useState<WorldStats | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const sse = useSSEContext();

  const loadData = useCallback(async () => {
    try {
      const [snapshotData, statsData] = await Promise.all([
        fetchJSON<WorldSnapshotData[]>("/api/v1/snapshots?limit=50"),
        fetchJSON<WorldStats>("/api/v1/world/stats").catch(() => null),
      ]);
      const mapped = snapshotData.map((s) => ({
        tick: s.tick,
        gdp: s.gdp,
        gini: s.gini_coefficient,
        population: s.total_population,
        activeAgents: s.active_agents,
        topSkills: s.skill_distribution_top5,
      }));
      setSnapshots(mapped);
      setStats(statsData);
      setError(null);
    } catch {
      setError("无法加载经济数据");
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    (async () => {
      await loadData();
    })();
    const interval = setInterval(loadData, 10000);
    return () => clearInterval(interval);
  }, [loadData]);

  // SSE-driven refresh
  useEffect(() => {
    function onEvent(event: WorldEvent) {
      if (event.type === "tax_collected" || event.type === "treasury_distributed" || event.type === "investment_purchased" || event.type === "transaction_completed") {
        loadData();
      }
    }
    const unsubscribe = sse.subscribe(onEvent);
    return unsubscribe;
  }, [sse, loadData]);

  // Chart data
  const gdpData = useMemo(
    () => snapshots.map((s) => ({ tick: s.tick, gdp: s.gdp })),
    [snapshots]
  );

  const giniData = useMemo(
    () => snapshots.map((s) => ({ tick: s.tick, gini: s.gini })),
    [snapshots]
  );

  const populationData = useMemo(
    () => snapshots.map((s) => ({ tick: s.tick, total: s.population, active: s.activeAgents })),
    [snapshots]
  );

  // Latest snapshot for skill distribution
  const latestSkills = useMemo(() => {
    if (snapshots.length === 0) return [];
    return snapshots[snapshots.length - 1].topSkills;
  }, [snapshots]);

  const skillChartData = useMemo(
    () => latestSkills.map((s) => ({ name: s.skill_name, count: s.agent_count, avgLevel: s.avg_level })),
    [latestSkills]
  );

  if (loading) {
    return (
      <div className="flex h-full items-center justify-center">
        <div className="text-sm text-zinc-600">正在加载经济数据...</div>
      </div>
    );
  }

  const latestGdp = snapshots.length > 0 ? snapshots[snapshots.length - 1].gdp : 0;
  const latestGini = snapshots.length > 0 ? snapshots[snapshots.length - 1].gini : 0;
  const totalMoney = stats?.totalMoney ?? 0;

  return (
    <div className="p-4 md:p-6 space-y-6">
      {/* Header */}
      <div>
        <h1 className="text-xl md:text-2xl font-bold text-zinc-100">经济指标面板</h1>
        <p className="text-sm text-zinc-500">
          {stats
            ? `Tick #${stats.tick} · GDP $${latestGdp.toLocaleString()} · 总货币 $${totalMoney.toLocaleString()}`
            : "加载中..."}
        </p>
      </div>

      {error && (
        <div className="rounded-lg bg-red-500/10 border border-red-500/20 px-3 py-1.5 text-xs text-red-400">
          {error}
        </div>
      )}

      {/* Key Indicators */}
      <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 xl:grid-cols-4">
        <div className="rounded-xl border border-green-500/20 bg-green-500/5 p-4 space-y-1">
          <p className="text-sm text-zinc-400">GDP</p>
          <p className="text-2xl font-bold text-green-400">${latestGdp.toLocaleString()}</p>
          <p className="text-xs text-zinc-500">总产出代理</p>
        </div>
        <div className="rounded-xl border border-amber-500/20 bg-amber-500/5 p-4 space-y-1">
          <p className="text-sm text-zinc-400">任务数</p>
          <p className="text-2xl font-bold text-amber-400">{stats?.taskCount ?? "—"}</p>
          <p className="text-xs text-zinc-500">活跃任务</p>
        </div>
        <div className="rounded-xl border border-purple-500/20 bg-purple-500/5 p-4 space-y-1">
          <p className="text-sm text-zinc-400">基尼系数</p>
          <p className="text-2xl font-bold text-purple-400">{latestGini.toFixed(3)}</p>
          <p className="text-xs text-zinc-500">{latestGini < 0.3 ? "较平等" : latestGini < 0.5 ? "中等" : "较不平等"}</p>
        </div>
        <div className="rounded-xl border border-blue-500/20 bg-blue-500/5 p-4 space-y-1">
          <p className="text-sm text-zinc-400">总货币量</p>
          <p className="text-2xl font-bold text-blue-400">${totalMoney.toLocaleString()}</p>
          <p className="text-xs text-zinc-500">流通中</p>
        </div>
      </div>

      {/* Charts */}
      <div className="grid grid-cols-1 gap-6 xl:grid-cols-2">
        {/* GDP Trend */}
        <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4 space-y-3">
          <h2 className="text-sm font-semibold text-zinc-200">GDP 走势</h2>
          {gdpData.length === 0 ? (
            <p className="text-sm text-zinc-600 h-48 flex items-center justify-center">暂无数据</p>
          ) : (
            <ResponsiveContainer width="100%" height={240}>
              <AreaChart data={gdpData}>
                <defs>
                  <linearGradient id="gdpGradient" x1="0" y1="0" x2="0" y2="1">
                    <stop offset="5%" stopColor="#22c55e" stopOpacity={0.3} />
                    <stop offset="95%" stopColor="#22c55e" stopOpacity={0} />
                  </linearGradient>
                </defs>
                <CartesianGrid strokeDasharray="3 3" stroke="#27272a" />
                <XAxis dataKey="tick" stroke="#52525b" tick={{ fontSize: 10 }} tickFormatter={(v: number) => `#${v}`} />
                <YAxis stroke="#52525b" tick={{ fontSize: 10 }} tickFormatter={(v: number) => v >= 1000000 ? `${(v / 1000000).toFixed(1)}M` : v >= 1000 ? `${(v / 1000).toFixed(0)}K` : `${v}`} />
                <Tooltip content={<CustomTooltip />} />
                <Area type="monotone" dataKey="gdp" stroke="#22c55e" fill="url(#gdpGradient)" strokeWidth={2} name="GDP" />
              </AreaChart>
            </ResponsiveContainer>
          )}
        </div>

        {/* Gini Coefficient Trend */}
        <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4 space-y-3">
          <h2 className="text-sm font-semibold text-zinc-200">基尼系数走势</h2>
          {giniData.length === 0 ? (
            <p className="text-sm text-zinc-600 h-48 flex items-center justify-center">暂无数据</p>
          ) : (
            <ResponsiveContainer width="100%" height={240}>
              <LineChart data={giniData}>
                <CartesianGrid strokeDasharray="3 3" stroke="#27272a" />
                <XAxis dataKey="tick" stroke="#52525b" tick={{ fontSize: 10 }} tickFormatter={(v: number) => `#${v}`} />
                <YAxis stroke="#52525b" tick={{ fontSize: 10 }} domain={[0, 1]} tickFormatter={(v: number) => v.toFixed(1)} />
                <Tooltip content={<CustomTooltip />} />
                <Line type="monotone" dataKey="gini" stroke="#a855f7" strokeWidth={2} dot={false} name="基尼系数" />
              </LineChart>
            </ResponsiveContainer>
          )}
        </div>

        {/* Population Trend */}
        <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4 space-y-3">
          <h2 className="text-sm font-semibold text-zinc-200">人口趋势</h2>
          {populationData.length === 0 ? (
            <p className="text-sm text-zinc-600 h-48 flex items-center justify-center">暂无数据</p>
          ) : (
            <ResponsiveContainer width="100%" height={240}>
              <AreaChart data={populationData}>
                <defs>
                  <linearGradient id="popGradient" x1="0" y1="0" x2="0" y2="1">
                    <stop offset="5%" stopColor="#3b82f6" stopOpacity={0.3} />
                    <stop offset="95%" stopColor="#3b82f6" stopOpacity={0} />
                  </linearGradient>
                </defs>
                <CartesianGrid strokeDasharray="3 3" stroke="#27272a" />
                <XAxis dataKey="tick" stroke="#52525b" tick={{ fontSize: 10 }} tickFormatter={(v: number) => `#${v}`} />
                <YAxis stroke="#52525b" tick={{ fontSize: 10 }} />
                <Tooltip content={<CustomTooltip />} />
                <Area type="monotone" dataKey="total" stroke="#3b82f6" fill="url(#popGradient)" strokeWidth={2} name="总人口" />
                <Line type="monotone" dataKey="active" stroke="#22c55e" strokeWidth={2} dot={false} name="活跃" />
              </AreaChart>
            </ResponsiveContainer>
          )}
        </div>

        {/* Skill Distribution */}
        <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4 space-y-3">
          <h2 className="text-sm font-semibold text-zinc-200">技能分布 Top 5</h2>
          {skillChartData.length === 0 ? (
            <p className="text-sm text-zinc-600 h-48 flex items-center justify-center">暂无数据</p>
          ) : (
            <ResponsiveContainer width="100%" height={240}>
              <BarChart data={skillChartData} layout="vertical">
                <CartesianGrid strokeDasharray="3 3" stroke="#27272a" horizontal={false} />
                <XAxis type="number" stroke="#52525b" tick={{ fontSize: 10 }} />
                <YAxis type="category" dataKey="name" stroke="#52525b" tick={{ fontSize: 10 }} width={80} />
                <Tooltip content={<CustomTooltip />} />
                <Bar dataKey="count" fill="#3b82f6" name="Agent 数量" radius={[0, 4, 4, 0]} />
              </BarChart>
            </ResponsiveContainer>
          )}
        </div>
      </div>
    </div>
  );
}
