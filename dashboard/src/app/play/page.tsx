"use client";

import { useCallback, useEffect, useMemo, useState } from "react";
import type {
  ActionReceipt,
  HumanAgentStatus,
  HumanLeaderboardEntry,
  HumanPlayStats,
  IncarnateResponse,
  QueuedAction,
  WorldEvent,
} from "@/types/world";
import { fetchJSON, postJSON } from "@/lib/api";
import { useSSEContext } from "@/components/SSEProvider";

// ── Action catalog ────────────────────────────────────────────────────────

interface ActionDef {
  verb: string;
  label: string;
  icon: string;
  cost: number;
  income: number;
  hint: string;
}

const ACTIONS: ActionDef[] = [
  { verb: "rest", label: "休息", icon: "😴", cost: 0, income: 5, hint: "恢复 +5 tokens，安全选项" },
  { verb: "explore", label: "探索", icon: "🧭", cost: 3, income: 2, hint: "花费 3 tokens，发现周围资源" },
  { verb: "gather", label: "采集", icon: "🌾", cost: 0, income: 0, hint: "无消耗，获得 +10 money" },
  { verb: "communicate", label: "交流", icon: "💬", cost: 10, income: 0, hint: "与其他 agent 交互" },
  { verb: "trade", label: "交易", icon: "🤝", cost: 10, income: 0, hint: "购买/出售物资" },
  { verb: "build", label: "建造", icon: "🏗️", cost: 20, income: 5, hint: "建造建筑（消耗大）" },
  { verb: "socialize", label: "社交", icon: "🫂", cost: 5, income: 0, hint: "提升社交关系" },
  { verb: "move", label: "移动", icon: "🚶", cost: 12, income: 0, hint: "在世界中移动" },
];

// ── Helpers ───────────────────────────────────────────────────────────────

const STORAGE_KEY = "multica.play.agent_id";

function loadStoredAgentId(): string | null {
  if (typeof window === "undefined") return null;
  return window.localStorage.getItem(STORAGE_KEY);
}

function storeAgentId(id: string) {
  if (typeof window === "undefined") return;
  window.localStorage.setItem(STORAGE_KEY, id);
}

function clearStoredAgentId() {
  if (typeof window === "undefined") return;
  window.localStorage.removeItem(STORAGE_KEY);
}

// ── Component ─────────────────────────────────────────────────────────────

export default function PlayPage() {
  const [agentId, setAgentId] = useState<string | null>(null);
  const [status, setStatus] = useState<HumanAgentStatus | null>(null);
  const [leaderboard, setLeaderboard] = useState<HumanLeaderboardEntry[]>([]);
  const [stats, setStats] = useState<HumanPlayStats | null>(null);
  const [queue, setQueue] = useState<QueuedAction[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [incarnating, setIncarnating] = useState(false);
  const [name, setName] = useState("");
  const [avatar, setAvatar] = useState("🧑‍🚀");
  const [lastReceipt, setLastReceipt] = useState<ActionReceipt | null>(null);
  const [feed, setFeed] = useState<WorldEvent[]>([]);

  const sse = useSSEContext();

  // Restore agent_id from localStorage on mount
  useEffect(() => {
    const stored = loadStoredAgentId();
    if (stored) setAgentId(stored);
    setLoading(false);
  }, []);

  // Poll status every 2s when incarnated
  const refreshStatus = useCallback(async () => {
    if (!agentId) return;
    try {
      const [s, q] = await Promise.all([
        fetchJSON<HumanAgentStatus>(`/api/v1/play/${agentId}/status`),
        fetchJSON<{ agent_id: string; pending: QueuedAction[] }>(
          `/api/v1/play/${agentId}/queue`,
        ),
      ]);
      setStatus(s);
      setQueue(q.pending);
      if (!s.alive) {
        // Agent died — clear stored id but keep status visible
        clearStoredAgentId();
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }, [agentId]);

  useEffect(() => {
    if (!agentId) return;
    refreshStatus();
    const interval = setInterval(refreshStatus, 2000);
    return () => clearInterval(interval);
  }, [agentId, refreshStatus]);

  // Refresh leaderboard + stats
  useEffect(() => {
    async function load() {
      try {
        const [lb, s] = await Promise.all([
          fetchJSON<HumanLeaderboardEntry[]>("/api/v1/play/leaderboard"),
          fetchJSON<HumanPlayStats>("/api/v1/play/stats"),
        ]);
        setLeaderboard(lb);
        setStats(s);
      } catch {
        // non-critical
      }
    }
    load();
    const interval = setInterval(load, 5000);
    return () => clearInterval(interval);
  }, []);

  // Subscribe to SSE feed for this agent
  useEffect(() => {
    if (!agentId) return;
    const unsub = sse.subscribe((evt) => {
      // Only track events relevant to this agent or global tick events
      const relevant =
        evt.agentId === agentId ||
        evt.type === "tick_advanced" ||
        evt.type === "agent_spawned" ||
        evt.type === "agent_died";
      if (!relevant) return;
      setFeed((prev) => [evt, ...prev].slice(0, 50));
    });
    return unsub;
  }, [agentId, sse]);

  // ── Actions ───────────────────────────────────────────────────────────

  const handleIncarnate = useCallback(async () => {
    if (!name.trim()) {
      setError("请输入角色名字");
      return;
    }
    setIncarnating(true);
    setError(null);
    try {
      const resp = await postJSON<IncarnateResponse>("/api/v1/play/incarnate", {
        name: name.trim(),
        avatar,
      });
      setAgentId(resp.agent_id);
      storeAgentId(resp.agent_id);
      setStatus({
        agent_id: resp.agent_id,
        human_id: resp.human_id,
        name: resp.name,
        alive: true,
        tokens: resp.tokens,
        money: resp.money,
        phase: "adult",
        ticks_survived: 0,
        last_action_tick: resp.spawned_tick,
        pending_actions: 0,
      });
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setIncarnating(false);
    }
  }, [name, avatar]);

  const handleSubmitAction = useCallback(
    async (action: string, params: Record<string, unknown> = {}) => {
      if (!agentId) return;
      setError(null);
      try {
        const receipt = await postJSON<ActionReceipt>(
          `/api/v1/play/${agentId}/action`,
          { action, params },
        );
        setLastReceipt(receipt);
        // Refresh status right away to update pending count
        refreshStatus();
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err));
      }
    },
    [agentId, refreshStatus],
  );

  const handleRespawn = useCallback(() => {
    setAgentId(null);
    setStatus(null);
    setQueue([]);
    setFeed([]);
    clearStoredAgentId();
  }, []);

  // ── Render ────────────────────────────────────────────────────────────

  const alive = status?.alive ?? false;
  const tokens = status?.tokens ?? 0;
  const money = status?.money ?? 0;

  const feedByTick = useMemo(() => {
    const seen = new Set<string>();
    return feed.filter((e) => {
      const key = `${e.tick}-${e.type}-${e.agentId ?? ""}`;
      if (seen.has(key)) return false;
      seen.add(key);
      return true;
    });
  }, [feed]);

  if (loading) {
    return (
      <div className="flex items-center justify-center py-20 text-zinc-500">
        正在唤醒世界…
      </div>
    );
  }

  // ── Not incarnated: show incarnation form ────────────────────────────
  if (!agentId || !status) {
    return (
      <div className="mx-auto max-w-md space-y-6 py-10">
        <header className="text-center">
          <h1 className="bg-gradient-to-r from-emerald-400 via-teal-300 to-cyan-400 bg-clip-text text-3xl font-bold text-transparent">
            化身为 Agent
          </h1>
          <p className="mt-2 text-sm text-zinc-400">
            加入 AI agent 社会。持币、交易、交流、求生。
          </p>
        </header>

        <div className="rounded-2xl border border-zinc-800 bg-zinc-900/60 p-6 space-y-4">
          <div>
            <label className="mb-1 block text-xs font-medium text-zinc-500">
              角色名字
            </label>
            <input
              type="text"
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="例如：Neo"
              maxLength={24}
              className="w-full rounded-lg border border-zinc-700 bg-zinc-950 px-3 py-2 text-zinc-100 outline-none focus:border-emerald-500"
            />
          </div>
          <div>
            <label className="mb-1 block text-xs font-medium text-zinc-500">
              头像
            </label>
            <div className="flex flex-wrap gap-2">
              {["🧑‍🚀", "🧙", "🥷", "🦊", "🐲", "🤖", "👑", "🌸"].map((a) => (
                <button
                  key={a}
                  type="button"
                  onClick={() => setAvatar(a)}
                  className={`h-10 w-10 rounded-lg text-xl transition-colors ${
                    avatar === a
                      ? "bg-emerald-500/20 ring-2 ring-emerald-500"
                      : "bg-zinc-800 hover:bg-zinc-700"
                  }`}
                >
                  {a}
                </button>
              ))}
            </div>
          </div>

          {error && (
            <p className="rounded-lg bg-red-500/10 px-3 py-2 text-sm text-red-400">
              {error}
            </p>
          )}

          <button
            type="button"
            onClick={handleIncarnate}
            disabled={incarnating || !name.trim()}
            className="w-full rounded-lg bg-emerald-500 py-2.5 font-semibold text-zinc-950 transition-colors hover:bg-emerald-400 disabled:cursor-not-allowed disabled:opacity-50"
          >
            {incarnating ? "正在化身…" : "开始化身"}
          </button>
          <p className="text-center text-xs text-zinc-600">
            初始资源：200 tokens · 100 money · 新手保护期 10 ticks
          </p>
        </div>

        {stats && (
          <div className="grid grid-cols-3 gap-2 text-center text-xs">
            <Stat label="总化身数" value={stats.total_incarnations} />
            <Stat label="存活中" value={stats.alive} />
            <Stat label="已死亡" value={stats.dead} />
          </div>
        )}
      </div>
    );
  }

  // ── Incarnated: show play UI ─────────────────────────────────────────
  return (
    <div className="mx-auto max-w-5xl space-y-6 pb-12">
      {/* Player Card */}
      <div className="rounded-2xl border border-zinc-800 bg-zinc-900/60 p-5">
        <div className="flex items-start justify-between gap-4">
          <div className="flex items-center gap-3">
            <span className="text-4xl">{avatar}</span>
            <div>
              <h1 className="text-xl font-bold text-zinc-100">{status.name}</h1>
              <p className="font-mono text-xs text-zinc-500">
                {status.agent_id.slice(0, 8)}
              </p>
            </div>
          </div>
          <div className="flex items-center gap-2">
            <Badge tone={alive ? "alive" : "dead"}>
              {alive ? "存活" : "已死亡"}
            </Badge>
            <Badge tone="phase">{status.phase}</Badge>
          </div>
        </div>

        <div className="mt-5 grid grid-cols-2 gap-3 sm:grid-cols-4">
          <Stat label="Tokens" value={tokens} accent="text-amber-400" />
          <Stat label="Money" value={money} accent="text-emerald-400" />
          <Stat label="存活 Ticks" value={status.ticks_survived} accent="text-cyan-400" />
          <Stat label="待执行" value={status.pending_actions} accent="text-purple-400" />
        </div>

        {!alive && (
          <div className="mt-4 rounded-lg border border-red-500/30 bg-red-500/10 px-4 py-3 text-sm text-red-300">
            你的 agent 已经死亡。你可以重新化身继续游戏。
            <button
              type="button"
              onClick={handleRespawn}
              className="ml-3 rounded bg-red-500/20 px-2 py-0.5 text-xs text-red-200 hover:bg-red-500/30"
            >
              重新化身
            </button>
          </div>
        )}
      </div>

      {error && (
        <p className="rounded-lg bg-red-500/10 px-3 py-2 text-sm text-red-400">
          {error}
        </p>
      )}

      {/* Action Panel */}
      <section className="rounded-2xl border border-zinc-800 bg-zinc-900/60 p-5">
        <h2 className="mb-3 text-sm font-semibold text-zinc-300">提交动作</h2>
        <div className="grid grid-cols-2 gap-2 sm:grid-cols-4">
          {ACTIONS.map((a) => (
            <button
              key={a.verb}
              type="button"
              disabled={!alive}
              title={a.hint}
              onClick={() => handleSubmitAction(a.verb)}
              className="flex flex-col items-start rounded-lg border border-zinc-700 bg-zinc-950/50 p-3 text-left transition-colors hover:border-emerald-500/60 hover:bg-zinc-900 disabled:cursor-not-allowed disabled:opacity-40"
            >
              <span className="text-xl">{a.icon}</span>
              <span className="mt-1 text-sm font-medium text-zinc-200">
                {a.label}
              </span>
              <span className="text-[10px] text-zinc-500">
                {a.cost > 0 ? `-${a.cost}🪙` : "免费"}
                {a.income > 0 ? ` · +${a.income}🪙` : ""}
              </span>
            </button>
          ))}
        </div>
        {lastReceipt && (
          <p className="mt-3 text-xs text-zinc-500">
            已入队：{lastReceipt.action} @ tick {lastReceipt.enqueued_tick}
          </p>
        )}
      </section>

      {/* Pending Queue */}
      {queue.length > 0 && (
        <section className="rounded-2xl border border-zinc-800 bg-zinc-900/60 p-5">
          <h2 className="mb-3 text-sm font-semibold text-zinc-300">
            待执行动作 ({queue.length})
          </h2>
          <ul className="space-y-1.5">
            {queue.map((a) => (
              <li
                key={a.id}
                className="flex items-center justify-between rounded bg-zinc-950/40 px-3 py-1.5 text-xs"
              >
                <span className="font-medium text-zinc-200">{a.action}</span>
                <span className="text-zinc-500">tick {a.enqueued_tick}</span>
              </li>
            ))}
          </ul>
        </section>
      )}

      {/* Event Feed */}
      <section className="rounded-2xl border border-zinc-800 bg-zinc-900/60 p-5">
        <div className="mb-3 flex items-center justify-between">
          <h2 className="text-sm font-semibold text-zinc-300">实时事件流</h2>
          <span
            className={`flex items-center gap-1 text-xs ${
              sse.connected ? "text-emerald-400" : "text-zinc-500"
            }`}
          >
            <span
              className={`h-1.5 w-1.5 rounded-full ${
                sse.connected ? "bg-emerald-400" : "bg-zinc-600"
              }`}
            />
            {sse.connected ? "已连接" : "断开"}
          </span>
        </div>
        {feedByTick.length === 0 ? (
          <p className="py-6 text-center text-xs text-zinc-600">
            等待事件…
          </p>
        ) : (
          <ul className="max-h-72 space-y-1.5 overflow-y-auto pr-1">
            {feedByTick.map((e, i) => (
              <li
                key={`${e.tick}-${e.type}-${i}`}
                className="flex items-start gap-2 rounded bg-zinc-950/40 px-3 py-1.5 text-xs"
              >
                <span className="shrink-0 font-mono text-zinc-600">
                  t{e.tick}
                </span>
                <span className="shrink-0 text-zinc-400">{e.type}</span>
                <span className="text-zinc-300">
                  {e.agentName ?? e.agentId?.slice(0, 8) ?? "—"}
                  {e.description ? ` · ${e.description}` : ""}
                </span>
              </li>
            ))}
          </ul>
        )}
      </section>

      {/* Leaderboard */}
      {leaderboard.length > 0 && (
        <section className="rounded-2xl border border-zinc-800 bg-zinc-900/60 p-5">
          <h2 className="mb-3 text-sm font-semibold text-zinc-300">
            玩家排行
          </h2>
          <table className="w-full text-sm">
            <thead>
              <tr className="text-left text-xs text-zinc-500">
                <th className="pb-2">#</th>
                <th className="pb-2">角色</th>
                <th className="pb-2 text-right">Tokens</th>
                <th className="pb-2 text-right">存活</th>
                <th className="pb-2 text-right">状态</th>
              </tr>
            </thead>
            <tbody>
              {leaderboard.slice(0, 10).map((e) => (
                <tr
                  key={e.agent_id}
                  className={`border-t border-zinc-800 ${
                    e.agent_id === agentId ? "bg-emerald-500/5" : ""
                  }`}
                >
                  <td className="py-1.5 text-zinc-500">{e.rank}</td>
                  <td className="py-1.5 text-zinc-200">{e.name}</td>
                  <td className="py-1.5 text-right font-mono text-amber-400">
                    {e.tokens}
                  </td>
                  <td className="py-1.5 text-right font-mono text-cyan-400">
                    {e.ticks_survived}
                  </td>
                  <td className="py-1.5 text-right">
                    {e.alive ? (
                      <span className="text-emerald-400">存活</span>
                    ) : (
                      <span className="text-zinc-600">死亡</span>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </section>
      )}
    </div>
  );
}

// ── Small UI helpers ──────────────────────────────────────────────────────

function Stat({
  label,
  value,
  accent,
}: {
  label: string;
  value: number | string;
  accent?: string;
}) {
  return (
    <div className="rounded-lg border border-zinc-800 bg-zinc-950/40 px-3 py-2 text-center">
      <div className={`font-mono text-lg font-semibold ${accent ?? "text-zinc-100"}`}>
        {value}
      </div>
      <div className="text-[10px] uppercase tracking-wider text-zinc-500">
        {label}
      </div>
    </div>
  );
}

function Badge({
  tone,
  children,
}: {
  tone: "alive" | "dead" | "phase";
  children: React.ReactNode;
}) {
  const cls =
    tone === "alive"
      ? "bg-emerald-500/15 text-emerald-400"
      : tone === "dead"
        ? "bg-red-500/15 text-red-400"
        : "bg-zinc-700/40 text-zinc-300";
  return (
    <span
      className={`rounded-full px-2.5 py-0.5 text-xs font-medium ${cls}`}
    >
      {children}
    </span>
  );
}
