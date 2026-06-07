"use client";

import { useEffect, useState, useCallback } from "react";
import { fetchJSON } from "@/lib/api";
import { useSSEContext } from "@/components/SSEProvider";
import type {
  LegislationCycleRecord,
  CandidateRule,
  WorldEvent,
} from "@/types/world";

const PHASE_LABELS: Record<string, { label: string; color: string }> = {
  election: { label: "选举", color: "text-blue-400" },
  proposal: { label: "提案", color: "text-amber-400" },
  voting: { label: "投票", color: "text-green-400" },
  enactment: { label: "颁布", color: "text-purple-400" },
  completed: { label: "已完成", color: "text-zinc-400" },
};

const RULE_TYPE_LABELS: Record<string, string> = {
  soft: "软规则",
  hard: "硬规则",
  constitutional: "宪法",
};

export default function LegislationPage() {
  const [activeCycles, setActiveCycles] = useState<LegislationCycleRecord[]>([]);
  const [completedCycles, setCompletedCycles] = useState<LegislationCycleRecord[]>([]);
  const [selectedCycle, setSelectedCycle] = useState<LegislationCycleRecord | null>(null);
  const [candidateRules, setCandidateRules] = useState<CandidateRule[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [tab, setTab] = useState<"active" | "completed">("active");

  const sse = useSSEContext();

  const loadData = useCallback(async () => {
    try {
      const [active, completed] = await Promise.all([
        fetchJSON<LegislationCycleRecord[]>("/api/v1/legislation/cycles/active").catch(() => []),
        fetchJSON<LegislationCycleRecord[]>("/api/v1/legislation/cycles/completed").catch(() => []),
      ]);
      setActiveCycles(active);
      setCompletedCycles(completed);
      setError(null);
    } catch {
      setError("无法加载立法数据");
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
        event.type === "soft_rule_proposed" ||
        event.type === "soft_rule_activated" ||
        event.type === "soft_rule_expired" ||
        event.type === "soft_rule_repealed" ||
        event.type === "leadership_election_started" ||
        event.type === "leadership_changed"
      ) {
        loadData();
      }
    }
    const unsubscribe = sse.subscribe(onEvent);
    return unsubscribe;
  }, [sse, loadData]);

  const selectCycle = useCallback(async (cycle: LegislationCycleRecord) => {
    setSelectedCycle(cycle);
    try {
      const data = await fetchJSON<{ rules: CandidateRule[] }>(
        `/api/v1/legislation/cycles/${cycle.org_id}/rules`
      ).catch(() => ({ rules: [] }));
      setCandidateRules(data.rules ?? []);
    } catch {
      setCandidateRules([]);
    }
  }, []);

  const allCycles = tab === "active" ? activeCycles : completedCycles;
  const totalRules = [...activeCycles, ...completedCycles].reduce(
    (sum, c) => sum + (c.enacted_rules?.length ?? 0) + (c.candidates?.length ?? 0),
    0
  );

  if (loading) {
    return (
      <div className="flex h-full items-center justify-center">
        <div className="text-sm text-zinc-600">正在加载立法数据...</div>
      </div>
    );
  }

  return (
    <div className="p-4 md:p-6 space-y-6">
      <div>
        <h1 className="text-xl md:text-2xl font-bold text-zinc-100">立法周期</h1>
        <p className="text-sm text-zinc-500">
          {activeCycles.length} 个活跃周期 · {completedCycles.length} 个已完成 · 共 {totalRules} 条规则
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
          <p className="text-sm text-zinc-400">活跃周期</p>
          <p className="text-2xl font-bold text-blue-400">{activeCycles.length}</p>
        </div>
        <div className="rounded-xl border border-green-500/20 bg-green-500/5 p-4 space-y-1">
          <p className="text-sm text-zinc-400">已完成周期</p>
          <p className="text-2xl font-bold text-green-400">{completedCycles.length}</p>
        </div>
        <div className="rounded-xl border border-purple-500/20 bg-purple-500/5 p-4 space-y-1">
          <p className="text-sm text-zinc-400">已颁布规则</p>
          <p className="text-2xl font-bold text-purple-400">
            {completedCycles.reduce((s, c) => s + (c.enacted_rules?.length ?? 0), 0)}
          </p>
        </div>
        <div className="rounded-xl border border-amber-500/20 bg-amber-500/5 p-4 space-y-1">
          <p className="text-sm text-zinc-400">候选规则</p>
          <p className="text-2xl font-bold text-amber-400">
            {activeCycles.reduce((s, c) => s + (c.candidates?.length ?? 0), 0)}
          </p>
        </div>
      </div>

      {/* Tab Switcher */}
      <div className="flex items-center gap-2">
        {(["active", "completed"] as const).map((t) => (
          <button
            key={t}
            onClick={() => setTab(t)}
            className={`rounded-lg px-3 py-1 text-xs font-medium transition-colors ${
              tab === t
                ? "bg-blue-500/15 text-blue-400 border border-blue-500/30"
                : "bg-zinc-800/50 text-zinc-400 border border-zinc-800 hover:bg-zinc-800"
            }`}
          >
            {t === "active" ? "活跃" : "已完成"}
          </button>
        ))}
      </div>

      {/* Cycle List */}
      {allCycles.length === 0 ? (
        <div className="flex h-48 items-center justify-center text-sm text-zinc-600">
          暂无{tab === "active" ? "活跃" : "已完成"}立法周期
        </div>
      ) : (
        <div className="rounded-xl border border-zinc-800 bg-zinc-900/50">
          <div className="overflow-x-auto">
            <table className="w-full">
              <thead>
                <tr className="border-b border-zinc-800">
                  <th className="px-4 py-3 text-left text-xs font-semibold text-zinc-400">周期 ID</th>
                  <th className="px-4 py-3 text-left text-xs font-semibold text-zinc-400">组织</th>
                  <th className="px-4 py-3 text-left text-xs font-semibold text-zinc-400">阶段</th>
                  <th className="px-4 py-3 text-left text-xs font-semibold text-zinc-400">领导者</th>
                  <th className="px-4 py-3 text-right text-xs font-semibold text-zinc-400">候选规则</th>
                  <th className="px-4 py-3 text-right text-xs font-semibold text-zinc-400">已颁布</th>
                  <th className="px-4 py-3 text-right text-xs font-semibold text-zinc-400">创建 Tick</th>
                </tr>
              </thead>
              <tbody>
                {allCycles.map((cycle) => {
                  const phase = PHASE_LABELS[cycle.phase] ?? { label: cycle.phase, color: "text-zinc-400" };
                  return (
                    <tr
                      key={cycle.cycle_id}
                      className="border-b border-zinc-800/50 last:border-0 cursor-pointer hover:bg-zinc-800/30 transition-colors"
                      onClick={() => selectCycle(cycle)}
                    >
                      <td className="px-4 py-3 text-sm font-mono text-zinc-300">{cycle.cycle_id.slice(0, 8)}</td>
                      <td className="px-4 py-3 text-sm text-zinc-300">{cycle.org_id.slice(0, 8)}</td>
                      <td className={`px-4 py-3 text-sm font-medium ${phase.color}`}>{phase.label}</td>
                      <td className="px-4 py-3 text-sm text-zinc-300">{cycle.leader_id?.slice(0, 8) ?? "—"}</td>
                      <td className="px-4 py-3 text-right text-sm text-zinc-400">{cycle.candidates?.length ?? 0}</td>
                      <td className="px-4 py-3 text-right text-sm text-zinc-400">{cycle.enacted_rules?.length ?? 0}</td>
                      <td className="px-4 py-3 text-right text-sm text-zinc-400">#{cycle.created_tick}</td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        </div>
      )}

      {/* Selected Cycle Detail */}
      {selectedCycle && (
        <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4 space-y-4">
          <div className="flex items-center justify-between">
            <h2 className="text-sm font-semibold text-zinc-200">
              周期详情 — {selectedCycle.cycle_id.slice(0, 8)}
            </h2>
            <button
              onClick={() => setSelectedCycle(null)}
              className="text-xs text-zinc-500 hover:text-zinc-300 transition-colors"
            >
              关闭
            </button>
          </div>

          <div className="grid grid-cols-2 gap-3 text-xs">
            <div>
              <span className="text-zinc-500">组织:</span>{" "}
              <span className="text-zinc-300">{selectedCycle.org_id}</span>
            </div>
            <div>
              <span className="text-zinc-500">阶段:</span>{" "}
              <span className={PHASE_LABELS[selectedCycle.phase]?.color ?? "text-zinc-300"}>
                {PHASE_LABELS[selectedCycle.phase]?.label ?? selectedCycle.phase}
              </span>
            </div>
            <div>
              <span className="text-zinc-500">领导者:</span>{" "}
              <span className="text-zinc-300">{selectedCycle.leader_id ?? "未选出"}</span>
            </div>
            <div>
              <span className="text-zinc-500">创建:</span>{" "}
              <span className="text-zinc-300">Tick #{selectedCycle.created_tick}</span>
            </div>
          </div>

          {/* Candidate Rules */}
          {candidateRules.length > 0 && (
            <div className="space-y-2">
              <h3 className="text-xs font-semibold text-zinc-400">候选规则</h3>
              {candidateRules.map((rule) => (
                <div
                  key={rule.id}
                  className="rounded-lg border border-zinc-800 bg-zinc-800/30 p-3 space-y-1"
                >
                  <div className="flex items-center justify-between">
                    <span className="text-sm font-medium text-zinc-200">{rule.title}</span>
                    <span className="text-xs text-zinc-500">
                      {RULE_TYPE_LABELS[rule.rule_type] ?? rule.rule_type}
                    </span>
                  </div>
                  <p className="text-xs text-zinc-400">{rule.description}</p>
                  <div className="flex items-center gap-3 text-xs text-zinc-500">
                    <span>提案者: {rule.proposer_id.slice(0, 8)}</span>
                    <span>票数: {rule.vote_count}</span>
                    <span>状态: {rule.enacted ? "已颁布" : "待投票"}</span>
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
