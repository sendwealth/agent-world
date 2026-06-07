"use client";

import { useEffect, useState, useCallback } from "react";
import { fetchJSON } from "@/lib/api";
import { useSSEContext } from "@/components/SSEProvider";
import type { Building, WorldEvent } from "@/types/world";

const BUILDING_TYPE_LABELS: Record<string, { label: string; icon: string }> = {
  warehouse: { label: "仓库", icon: "📦" },
  market: { label: "市场", icon: "🏪" },
  workshop: { label: "工坊", icon: "🔨" },
  defense_tower: { label: "防御塔", icon: "🏰" },
  housing: { label: "住宅", icon: "🏠" },
};

const OWNER_TYPE_LABELS: Record<string, string> = {
  personal: "个人",
  organization: "组织",
};

export default function BuildingsPage() {
  const [buildings, setBuildings] = useState<Building[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [typeFilter, setTypeFilter] = useState<string>("all");

  const sse = useSSEContext();

  const loadData = useCallback(async () => {
    try {
      const data = await fetchJSON<Building[]>("/api/v1/map/buildings").catch(() => []);
      setBuildings(data);
      setError(null);
    } catch {
      setError("无法加载建筑数据");
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
      if (event.type === "transaction_completed" || event.type === "balance_changed") {
        loadData();
      }
    }
    const unsubscribe = sse.subscribe(onEvent);
    return unsubscribe;
  }, [sse, loadData]);

  const filtered = typeFilter === "all" ? buildings : buildings.filter((b) => b.building_type === typeFilter);

  const totalBuildings = buildings.length;
  const avgHealth = buildings.length > 0
    ? buildings.reduce((s, b) => s + b.health / b.max_health, 0) / buildings.length
    : 0;
  const personalCount = buildings.filter((b) => b.owner_type === "personal").length;
  const orgCount = buildings.filter((b) => b.owner_type === "organization").length;

  // Group buildings by type for overview
  const buildingsByType = buildings.reduce<Record<string, number>>((acc, b) => {
    acc[b.building_type] = (acc[b.building_type] ?? 0) + 1;
    return acc;
  }, {});

  if (loading) {
    return (
      <div className="flex h-full items-center justify-center">
        <div className="text-sm text-zinc-600">正在加载建筑数据...</div>
      </div>
    );
  }

  return (
    <div className="p-4 md:p-6 space-y-6">
      <div>
        <h1 className="text-xl md:text-2xl font-bold text-zinc-100">世界建筑</h1>
        <p className="text-sm text-zinc-500">
          {totalBuildings} 座建筑 · 个人 {personalCount} · 组织 {orgCount} · 平均耐久 {avgHealth.toFixed(0)}%
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
          <p className="text-sm text-zinc-400">总建筑数</p>
          <p className="text-2xl font-bold text-blue-400">{totalBuildings}</p>
        </div>
        <div className="rounded-xl border border-green-500/20 bg-green-500/5 p-4 space-y-1">
          <p className="text-sm text-zinc-400">个人建筑</p>
          <p className="text-2xl font-bold text-green-400">{personalCount}</p>
        </div>
        <div className="rounded-xl border border-purple-500/20 bg-purple-500/5 p-4 space-y-1">
          <p className="text-sm text-zinc-400">组织建筑</p>
          <p className="text-2xl font-bold text-purple-400">{orgCount}</p>
        </div>
        <div className="rounded-xl border border-amber-500/20 bg-amber-500/5 p-4 space-y-1">
          <p className="text-sm text-zinc-400">平均耐久</p>
          <p className="text-2xl font-bold text-amber-400">{avgHealth.toFixed(0)}%</p>
        </div>
      </div>

      {/* Building Type Summary */}
      <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4 space-y-3">
        <h2 className="text-sm font-semibold text-zinc-200">建筑类型分布</h2>
        {Object.keys(buildingsByType).length === 0 ? (
          <p className="text-sm text-zinc-600 h-24 flex items-center justify-center">暂无建筑</p>
        ) : (
          <div className="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-5 gap-3">
            {Object.entries(buildingsByType).map(([type, count]) => {
              const info = BUILDING_TYPE_LABELS[type] ?? { label: type, icon: "?" };
              return (
                <div key={type} className="rounded-lg border border-zinc-800 bg-zinc-800/30 p-3 text-center space-y-1">
                  <span className="text-lg">{info.icon}</span>
                  <p className="text-xs text-zinc-400">{info.label}</p>
                  <p className="text-sm font-bold text-zinc-200">{count}</p>
                </div>
              );
            })}
          </div>
        )}
      </div>

      {/* Type Filter */}
      <div className="flex items-center gap-2 flex-wrap">
        <span className="text-xs text-zinc-500">类型:</span>
        {["all", ...Object.keys(BUILDING_TYPE_LABELS)].map((t) => (
          <button
            key={t}
            onClick={() => setTypeFilter(t)}
            className={`rounded-lg px-3 py-1 text-xs font-medium transition-colors ${
              typeFilter === t
                ? "bg-blue-500/15 text-blue-400 border border-blue-500/30"
                : "bg-zinc-800/50 text-zinc-400 border border-zinc-800 hover:bg-zinc-800"
            }`}
          >
            {t === "all" ? "全部" : BUILDING_TYPE_LABELS[t]?.label ?? t}
          </button>
        ))}
      </div>

      {/* Building List */}
      {filtered.length === 0 ? (
        <div className="flex h-48 items-center justify-center text-sm text-zinc-600">
          暂无建筑数据
        </div>
      ) : (
        <div className="rounded-xl border border-zinc-800 bg-zinc-900/50">
          <div className="overflow-x-auto">
            <table className="w-full">
              <thead>
                <tr className="border-b border-zinc-800">
                  <th className="px-4 py-3 text-left text-xs font-semibold text-zinc-400">ID</th>
                  <th className="px-4 py-3 text-left text-xs font-semibold text-zinc-400">类型</th>
                  <th className="px-4 py-3 text-left text-xs font-semibold text-zinc-400">坐标</th>
                  <th className="px-4 py-3 text-left text-xs font-semibold text-zinc-400">所有者</th>
                  <th className="px-4 py-3 text-left text-xs font-semibold text-zinc-400">所有权</th>
                  <th className="px-4 py-3 text-left text-xs font-semibold text-zinc-400">耐久</th>
                  <th className="px-4 py-3 text-right text-xs font-semibold text-zinc-400">等级</th>
                </tr>
              </thead>
              <tbody>
                {filtered.map((building) => {
                  const typeInfo = BUILDING_TYPE_LABELS[building.building_type] ?? { label: building.building_type, icon: "?" };
                  const healthPercent = building.max_health > 0 ? building.health / building.max_health : 0;
                  const healthColor = healthPercent > 0.6 ? "text-green-400" : healthPercent > 0.3 ? "text-amber-400" : "text-red-400";
                  return (
                    <tr key={building.id} className="border-b border-zinc-800/50 last:border-0 hover:bg-zinc-800/30 transition-colors">
                      <td className="px-4 py-3 text-sm font-mono text-zinc-300">{building.id.slice(0, 8)}</td>
                      <td className="px-4 py-3 text-sm text-zinc-200">
                        {typeInfo.icon} {typeInfo.label}
                      </td>
                      <td className="px-4 py-3 text-sm text-zinc-400 font-mono">({building.x}, {building.y})</td>
                      <td className="px-4 py-3 text-sm font-mono text-zinc-300">{building.owner_id.slice(0, 8)}</td>
                      <td className="px-4 py-3 text-sm text-zinc-400">{OWNER_TYPE_LABELS[building.owner_type] ?? building.owner_type}</td>
                      <td className="px-4 py-3">
                        <div className="flex items-center gap-2">
                          <div className="w-20 h-1.5 rounded-full bg-zinc-800 overflow-hidden">
                            <div
                              className={`h-full rounded-full ${healthPercent > 0.6 ? "bg-green-400" : healthPercent > 0.3 ? "bg-amber-400" : "bg-red-400"}`}
                              style={{ width: `${healthPercent * 100}%` }}
                            />
                          </div>
                          <span className={`text-xs ${healthColor}`}>{building.health}/{building.max_health}</span>
                        </div>
                      </td>
                      <td className="px-4 py-3 text-right text-sm text-zinc-400">Lv.{building.level}</td>
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
