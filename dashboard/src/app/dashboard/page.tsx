"use client";

import { useEffect, useState, useMemo } from "react";
import Link from "next/link";
import { useRouter } from "next/navigation";
import type { Agent, NetworkGraph, NetworkEdge, WorldEvent } from "@/types/world";
import { fetchJSON, postJSON } from "@/lib/api";
import { useWorldState } from "@/hooks/useWorldState";
import { useSSEContext } from "@/components/SSEProvider";
import { EVENT_TYPE_CONFIG } from "@/lib/event-types";
import { phaseLabels } from "@/lib/format";

// ─── Helpers ────────────────────────────────────────────────

function formatNumber(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
  return n.toLocaleString("en-US");
}

function topSkill(skills: Record<string, number>): { name: string; level: number } | null {
  const entries = Object.entries(skills);
  if (entries.length === 0) return null;
  const sorted = entries.sort((a, b) => b[1] - a[1]);
  return { name: sorted[0][0], level: sorted[0][1] };
}

const EVENT_BG_MAP: Record<string, string> = {
  "text-green-400": "bg-green-500/10",
  "text-red-400": "bg-red-500/10",
  "text-blue-400": "bg-blue-500/10",
  "text-amber-400": "bg-amber-500/10",
  "text-yellow-400": "bg-yellow-500/10",
  "text-purple-400": "bg-purple-500/10",
  "text-cyan-400": "bg-cyan-500/10",
  "text-emerald-400": "bg-emerald-500/10",
  "text-orange-400": "bg-orange-500/10",
  "text-indigo-400": "bg-indigo-500/10",
  "text-violet-400": "bg-violet-500/10",
  "text-teal-400": "bg-teal-500/10",
  "text-sky-400": "bg-sky-500/10",
  "text-zinc-400": "bg-zinc-500/10",
};

function eventTypeColor(type: string): { bg: string; text: string } {
  const cfg = EVENT_TYPE_CONFIG[type];
  if (cfg) {
    const bg = EVENT_BG_MAP[cfg.color] ?? "bg-zinc-500/10";
    return { bg, text: cfg.color };
  }
  return { bg: "bg-zinc-500/10", text: "text-zinc-400" };
}

function eventTypeLabel(type: string): string {
  return EVENT_TYPE_CONFIG[type]?.label ?? type;
}

// ─── Left Sidebar: Agent List ───────────────────────────────

function AgentSidebar({ agents }: { agents: Agent[] }) {
  const alive = useMemo(() => agents.filter((a) => a.alive), [agents]);

  return (
    <aside className="w-full lg:w-[260px] shrink-0 border-b lg:border-b-0 lg:border-r border-border bg-surface">
      <div className="p-3 border-b border-border">
        <h2 className="text-xs font-semibold text-muted tracking-wider uppercase">
          Agent 列表
        </h2>
        <p className="text-[10px] text-muted mt-0.5">{alive.length} 存活</p>
      </div>
      <div className="overflow-y-auto max-h-[calc(100vh-160px)] scrollbar-thin">
        {alive.length === 0 ? (
          <div className="p-4 text-xs text-muted text-center">暂无存活 Agent</div>
        ) : (
          <div className="flex flex-col">
            {alive.map((agent) => {
              const skill = topSkill(agent.skills);
              return (
                <Link
                  key={agent.id}
                  href={`/agents/${agent.id}`}
                  className="flex items-center gap-3 px-3 py-2.5 border-b border-border/50 transition-colors hover:bg-card"
                >
                  {/* Avatar circle */}
                  <div className="w-8 h-8 rounded-full bg-accent/15 text-accent flex items-center justify-center text-xs font-bold shrink-0">
                    {agent.name.charAt(0).toUpperCase()}
                  </div>
                  <div className="min-w-0 flex-1">
                    <div className="flex items-center gap-1.5">
                      <span className="text-sm font-medium text-fg truncate">
                        {agent.name}
                      </span>
                      <span className="inline-flex items-center rounded-[var(--radius-sm)] px-1.5 py-0 text-[9px] font-medium bg-accent/10 text-accent shrink-0">
                        {phaseLabels[agent.phase] ?? agent.phase}
                      </span>
                    </div>
                    <div className="text-[11px] text-muted truncate mt-0.5">
                      {skill
                        ? `${skill.name} Lv.${skill.level}`
                        : "无技能"}
                    </div>
                  </div>
                </Link>
              );
            })}
          </div>
        )}
      </div>
    </aside>
  );
}

// ─── Main: Stat Cards ───────────────────────────────────────

function StatCards({
  aliveCount,
  totalTokens,
  deadCount,
}: {
  aliveCount: number;
  totalTokens: number;
  deadCount: number;
}) {
  const cards = [
    {
      label: "存活 AGENT",
      value: formatNumber(aliveCount),
      sub: aliveCount > 0 ? "● 运行中" : "—",
      color: "text-success",
      subColor: "text-success",
    },
    {
      label: "世界 GDP (TOKEN)",
      value: formatNumber(totalTokens),
      sub: "总量",
      color: "text-accent",
      subColor: "text-muted",
    },
    {
      label: "通胀率",
      value: "N/A",
      sub: "待接入",
      color: "text-warning",
      subColor: "text-muted",
    },
    {
      label: "总死亡",
      value: formatNumber(deadCount),
      sub: deadCount > 0 ? "累计" : "—",
      color: "text-danger",
      subColor: "text-muted",
    },
  ];

  return (
    <div className="grid grid-cols-2 lg:grid-cols-4 gap-3">
      {cards.map((card) => (
        <div
          key={card.label}
          className="rounded-[var(--radius)] border border-border bg-card p-4"
        >
          <div className="text-[10px] font-mono tracking-wider text-muted uppercase mb-1">
            {card.label}
          </div>
          <div className={`text-2xl font-bold tabular-nums ${card.color}`}>
            {card.value}
          </div>
          <div className={`text-[10px] mt-1 ${card.subColor}`}>{card.sub}</div>
        </div>
      ))}
    </div>
  );
}

// ─── Main: World Graph (SVG Ring with real interaction edges) ───

const WORLD_GRAPH_EDGE_CAP = 60;

/** Deduplicate edges so that an undirected {A,B} pair renders once,
 *  accumulating weight across both directions and edge types. */
function dedupeEdges(edges: NetworkEdge[]): { source: string; target: string; weight: number }[] {
  const map = new Map<string, { source: string; target: string; weight: number }>();
  for (const edge of edges) {
    const [lo, hi] = edge.source < edge.target
      ? [edge.source, edge.target]
      : [edge.target, edge.source];
    const key = `${lo}|${hi}`;
    const existing = map.get(key);
    if (existing) {
      existing.weight += edge.weight;
    } else {
      map.set(key, { source: lo, target: hi, weight: edge.weight });
    }
  }
  return Array.from(map.values());
}

function WorldGraph({ agents }: { agents: Agent[] }) {
  const alive = useMemo(() => agents.filter((a) => a.alive), [agents]);
  const [graph, setGraph] = useState<NetworkGraph | null>(null);

  // Poll /api/v2/export/network for real interaction edges (trust, trade, message).
  useEffect(() => {
    let cancelled = false;

    async function load() {
      try {
        const data = await fetchJSON<NetworkGraph>(
          "/api/v2/export/network?format=json&edge_types=trust,trade,message",
        );
        if (!cancelled) setGraph(data);
      } catch {
        // Backend may not be running; silently retry on next interval.
      }
    }

    load();
    const interval = setInterval(load, 10000);
    return () => {
      cancelled = true;
      clearInterval(interval);
    };
  }, []);

  // Ring layout parameters
  const size = 340;
  const cx = size / 2;
  const cy = size / 2;
  const radius = 120;

  const nodes = useMemo(() => {
    if (alive.length === 0) return [];
    return alive.map((agent, i) => {
      const angle = (2 * Math.PI * i) / alive.length - Math.PI / 2;
      return {
        id: agent.id,
        name: agent.name,
        x: cx + radius * Math.cos(angle),
        y: cy + radius * Math.sin(angle),
        initial: agent.name.charAt(0).toUpperCase(),
        href: `/agents/${agent.id}`,
      };
    });
  }, [alive, cx, cy, radius]);

  // Build real edges from the network graph response.
  // Keep only edges between alive agents that are present in the ring layout,
  // deduplicate pairs, then cap to the heaviest WORLD_GRAPH_EDGE_CAP for perf.
  const renderEdges = useMemo(() => {
    if (!graph || nodes.length === 0) return [];
    const nodeIds = new Set(nodes.map((n) => n.id));
    const relevant = dedupeEdges(graph.edges).filter(
      (e) => nodeIds.has(e.source) && nodeIds.has(e.target),
    );
    relevant.sort((a, b) => b.weight - a.weight);
    return relevant.slice(0, WORLD_GRAPH_EDGE_CAP);
  }, [graph, nodes]);

  const nodeMap = useMemo(
    () => new Map(nodes.map((n) => [n.id, n])),
    [nodes],
  );

  return (
    <section className="rounded-[var(--radius)] border border-border bg-card p-4">
      <div className="flex items-center justify-between mb-3">
        <h3 className="text-sm font-semibold text-fg">世界图谱</h3>
        <span className="text-[10px] text-muted font-mono">
          节点 = Agent · 连线 = 真实交互
        </span>
      </div>
      <div className="flex items-center justify-center">
        {nodes.length === 0 ? (
          <div className="h-[200px] flex items-center justify-center text-sm text-muted">
            暂无 Agent 数据
          </div>
        ) : (
          <svg
            width={size}
            height={size}
            viewBox={`0 0 ${size} ${size}`}
            className="max-w-full"
          >
            {/* Background ring */}
            <circle
              cx={cx}
              cy={cy}
              r={radius}
              fill="none"
              stroke="var(--border)"
              strokeWidth="1"
              strokeDasharray="4 4"
              opacity={0.5}
            />

            {/* Real interaction edges from /api/v2/export/network */}
            {renderEdges.map((edge) => {
              const src = nodeMap.get(edge.source);
              const tgt = nodeMap.get(edge.target);
              if (!src || !tgt) return null;
              return (
                <line
                  key={`edge-${edge.source}-${edge.target}`}
                  x1={src.x}
                  y1={src.y}
                  x2={tgt.x}
                  y2={tgt.y}
                  stroke="var(--accent)"
                  strokeWidth="0.5"
                  opacity={0.2}
                />
              );
            })}

            {/* Agent nodes */}
            {nodes.map((node) => (
              <a key={node.id} href={node.href} style={{ cursor: "pointer" }}>
                <circle
                  cx={node.x}
                  cy={node.y}
                  r={14}
                  fill="var(--accent)"
                  fillOpacity={0.12}
                  stroke="var(--accent)"
                  strokeWidth={1}
                  opacity={0.8}
                />
                <text
                  x={node.x}
                  y={node.y}
                  textAnchor="middle"
                  dominantBaseline="central"
                  fill="var(--accent)"
                  fontSize="11"
                  fontWeight="bold"
                  fontFamily="var(--font-geist-sans), system-ui, sans-serif"
                >
                  {node.initial}
                </text>
              </a>
            ))}
          </svg>
        )}
      </div>
    </section>
  );
}

// ─── Main: Economy Chart (SVG mock) ─────────────────────────

function EconomyChart({ events }: { events: WorldEvent[] }) {
  // Derive economic activity from balance_changed events per tick.
  // balance_changed events now carry { tick, agent_name, old_balance,
  // new_balance }. Use the top-level tick, falling back to data.tick.
  const chartData = useMemo(() => {
    const tickMap = new Map<number, number>();
    for (const e of events) {
      if (e.type !== "balance_changed") continue;
      const tick = e.tick ?? (e.data?.tick as number | undefined) ?? 0;
      tickMap.set(tick, (tickMap.get(tick) ?? 0) + 1);
    }
    const sortedTicks = [...tickMap.entries()].sort((a, b) => a[0] - b[0]);
    if (sortedTicks.length === 0) return [];
    const last30 = sortedTicks.slice(-30);
    return last30.map(([tick, value]) => ({ tick, value }));
  }, [events]);

  const width = 600;
  const height = 160;
  const padding = { top: 10, right: 10, bottom: 20, left: 40 };
  const chartW = width - padding.left - padding.right;
  const chartH = height - padding.top - padding.bottom;

  const maxVal = chartData.length > 0 ? Math.max(...chartData.map((d) => d.value)) : 1;
  const minVal = chartData.length > 0 ? Math.min(...chartData.map((d) => d.value)) : 0;
  const range = maxVal - minVal || 1;

  const points = chartData.map((d, i) => ({
    x: padding.left + (i / Math.max(chartData.length - 1, 1)) * chartW,
    y: padding.top + chartH - ((d.value - minVal) / range) * chartH,
  }));

  // Guard path computation — empty data means no SVG
  const linePath = points.length > 0
    ? points.map((p, i) => (i === 0 ? `M ${p.x} ${p.y}` : `L ${p.x} ${p.y}`)).join(" ")
    : "";
  const areaPath = points.length > 0
    ? linePath + ` L ${points[points.length - 1].x} ${padding.top + chartH} L ${points[0].x} ${padding.top + chartH} Z`
    : "";

  return (
    <section className="rounded-[var(--radius)] border border-border bg-card p-4">
      <div className="flex items-center justify-between mb-3">
        <h3 className="text-sm font-semibold text-fg">经济活动</h3>
        <span className="text-[10px] text-muted font-mono">
          {chartData.length > 0 ? `近 ${chartData.length} Tick` : "等待数据"}
        </span>
      </div>
      {chartData.length < 2 ? (
        <div className="h-[160px] flex items-center justify-center text-sm text-muted">
          等待交易数据...
        </div>
      ) : (
      <svg
        width="100%"
        viewBox={`0 0 ${width} ${height}`}
        className="overflow-visible"
        preserveAspectRatio="none"
      >
        {/* Grid lines */}
        {[0, 0.25, 0.5, 0.75, 1].map((frac) => (
          <line
            key={frac}
            x1={padding.left}
            y1={padding.top + chartH * (1 - frac)}
            x2={padding.left + chartW}
            y2={padding.top + chartH * (1 - frac)}
            stroke="var(--border)"
            strokeWidth="0.5"
            opacity={0.5}
          />
        ))}

        {/* Area fill */}
        <path d={areaPath} fill="var(--accent)" fillOpacity={0.06} />

        {/* Line */}
        <path
          d={linePath}
          fill="none"
          stroke="var(--accent)"
          strokeWidth={1.5}
          strokeLinejoin="round"
        />

        {/* End dot */}
        {points.length > 0 && (
          <circle
            cx={points[points.length - 1].x}
            cy={points[points.length - 1].y}
            r={3}
            fill="var(--accent)"
          />
        )}
      </svg>
      )}
    </section>
  );
}

// ─── Main: Event Stream ─────────────────────────────────────

function EventStream({ events, connected }: { events: WorldEvent[]; connected: boolean }) {
  // Filter out tick_advanced events to reduce noise; keep 50 most recent
  const displayEvents = useMemo(() => {
    return events
      .filter((e) => e.type !== "tick_advanced")
      .slice(0, 50);
  }, [events]);

  return (
    <section className="rounded-[var(--radius)] border border-border bg-card">
      <div className="flex items-center justify-between p-4 pb-3 border-b border-border/50">
        <h3 id="event-stream-heading" className="text-sm font-semibold text-fg">事件流</h3>
        <span className={`inline-flex items-center gap-1.5 px-2 py-0.5 rounded-full ${connected ? "bg-success/10 text-success" : "bg-warning/10 text-warning"} text-[10px] font-mono`}>
          <span className="relative flex h-1.5 w-1.5">
            {connected && <span className="absolute inline-flex h-full w-full animate-ping rounded-full bg-success opacity-75" />}
            <span className={`relative inline-flex h-1.5 w-1.5 rounded-full ${connected ? "bg-success" : "bg-warning"}`} />
          </span>
          {connected ? "实时" : "断开"}
        </span>
      </div>
      <div
        className="overflow-y-auto max-h-[320px] scrollbar-thin"
        role="log"
        aria-live="polite"
        aria-labelledby="event-stream-heading"
      >
        {displayEvents.length === 0 ? (
          <div className="p-4 text-xs text-muted text-center">
            等待事件...
          </div>
        ) : (
          <div className="flex flex-col">
            {displayEvents.map((event) => {
              const { bg, text } = eventTypeColor(event.type);
              return (
                <div
                  key={event.id}
                  className="flex items-start gap-2.5 px-4 py-2 border-b border-border/30 hover:bg-card/50 transition-colors"
                >
                  {/* Tick number */}
                  <span className="text-[10px] font-mono text-muted tabular-nums shrink-0 pt-0.5 w-10">
                    #{event.tick}
                  </span>
                  {/* Event type badge */}
                  <span
                    className={`inline-flex items-center rounded-full px-1.5 py-0.5 text-[9px] font-medium shrink-0 ${bg} ${text}`}
                  >
                    {eventTypeLabel(event.type)}
                  </span>
                  {/* Description */}
                  <span className="text-xs text-fg2 leading-relaxed break-all">
                    {event.description}
                  </span>
                </div>
              );
            })}
          </div>
        )}
      </div>
    </section>
  );
}

// ─── Right Sidebar ──────────────────────────────────────────

function RightSidebar({ stats }: { stats: { tick: number; totalTokens: number; totalMoney: number } | null }) {
  const router = useRouter();
  const [actionMsg, setActionMsg] = useState<string | null>(null);

  const handleSpawn = async () => {
    const name = window.prompt("新 Agent 名称:");
    if (!name?.trim()) return;
    try {
      await postJSON("/api/v1/agents", { name: name.trim() });
      setActionMsg(`Agent ${name} 已创建`);
    } catch (e) {
      setActionMsg(`创建失败: ${e instanceof Error ? e.message : "未知错误"}`);
    }
  };

  // Derive tick from tick_advanced events only (consistent with Topbar)
  const { events } = useSSEContext();
  let tick: number | null | undefined;
  for (const e of events) {
    if (e.type === "tick_advanced" && e.tick > 0) { tick = e.tick; break; }
  }
  if (tick == null) tick = stats?.tick;

  return (
    <aside className="w-full lg:w-[320px] shrink-0 space-y-4">
      {/* 世界规则 */}
      <Panel title="世界规则">
        <div className="flex flex-wrap gap-1.5">
          {[
            { code: "R001", label: "Token 消耗" },
            { code: "R002", label: "死亡判定" },
            { code: "R003", label: "繁殖冷却" },
            { code: "R004", label: "技能上限" },
            { code: "R005", label: "税收规则" },
            { code: "R006", label: "信誉衰减" },
            { code: "R007", label: "交易手续费" },
          ].map((rule) => (
            <span
              key={rule.code}
              className="inline-flex items-center gap-1 rounded-full px-2.5 py-1 text-[10px] font-mono bg-surface border border-border text-fg2"
            >
              <span className="text-accent">{rule.code}</span>
              {rule.label}
            </span>
          ))}
        </div>
      </Panel>

      {/* 人类操作 */}
      <Panel title="人类操作">
        {actionMsg && (
          <p className="text-xs text-zinc-400 bg-zinc-800/50 rounded-lg px-3 py-2 mb-2">{actionMsg}</p>
        )}
        <div className="grid grid-cols-2 gap-2">
          <ActionBtn icon="+" label="Spawn 新 Agent" onClick={handleSpawn} />
          <ActionBtn icon="$" label="发布任务悬赏" onClick={() => router.push("/marketplace")} />
          <ActionBtn icon="I" label="投资 Agent" onClick={() => router.push("/agents")} />
        </div>
        <div className="mt-2">
          <ActionBtn icon="?" label="查看墓碑" onClick={() => router.push("/agents")} />
        </div>
      </Panel>

      {/* 世界快照 */}
      <Panel title="世界快照">
        <div className="space-y-2">
          <KVRow
            label="世界日"
            value={tick != null ? `第 ${Math.floor(tick / 24) + 1} 天` : "—"}
          />
          <KVRow
            label="运行时长"
            value={tick != null ? `${Math.floor(tick * 0.5)}s` : "—"}
          />
          <KVRow
            label="Token 总量"
            value={stats ? formatNumber(stats.totalTokens) : "—"}
          />
          <KVRow
            label="Money 总量"
            value={stats ? formatNumber(stats.totalMoney) : "—"}
          />
        </div>
      </Panel>
    </aside>
  );
}

// ─── Shared Components ──────────────────────────────────────

function Panel({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div className="rounded-[var(--radius)] border border-border bg-card p-4">
      <h3 className="text-sm font-semibold text-fg mb-3">{title}</h3>
      {children}
    </div>
  );
}

function ActionBtn({ icon, label, onClick, disabled }: { icon: string; label: string; onClick?: () => void; disabled?: boolean }) {
  return (
    <button
      onClick={onClick}
      disabled={disabled}
      className={`flex items-center gap-2 w-full px-3 py-2 rounded-[var(--radius-sm)] border border-border bg-surface text-sm text-fg2 transition-colors ${disabled ? "opacity-40 cursor-not-allowed" : "hover:bg-accent/10 hover:border-accent/30 hover:text-accent"}`}
      title={disabled ? "功能开发中" : label}
    >
      <span className="w-6 h-6 rounded-md bg-accent/10 text-accent flex items-center justify-center text-xs font-bold shrink-0">
        {icon}
      </span>
      <span className="text-xs truncate">{label}</span>
    </button>
  );
}

function KVRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-center justify-between">
      <span className="text-xs text-muted">{label}</span>
      <span className="text-xs font-mono text-fg tabular-nums">{value}</span>
    </div>
  );
}

// ─── Page ───────────────────────────────────────────────────

export default function DashboardPage() {
  const { stats, events, error, connected } = useWorldState();
  const [agents, setAgents] = useState<Agent[]>([]);

  // Fetch agents periodically
  useEffect(() => {
    let cancelled = false;

    async function load() {
      try {
        const data = await fetchJSON<Agent[]>("/api/v1/agents");
        if (!cancelled) setAgents(data);
      } catch {
        // Silently ignore
      }
    }

    load();
    const interval = setInterval(load, 5000);
    return () => {
      cancelled = true;
      clearInterval(interval);
    };
  }, []);

  const aliveCount = stats?.aliveCount ?? agents.filter((a) => a.alive).length;
  const deadCount = stats?.deadCount ?? agents.filter((a) => !a.alive).length;
  const totalTokens = stats?.totalTokens ?? 0;

  return (
    <div className="flex flex-col lg:flex-row h-full min-h-0">
      {/* Left Sidebar - Agent List */}
      <AgentSidebar agents={agents} />

      {/* Main Content */}
      <div className="flex-1 min-w-0 overflow-y-auto scrollbar-thin">
        <div className="p-4 md:p-6 space-y-4">
          {/* Stat Cards */}
          <StatCards
            aliveCount={aliveCount}
            totalTokens={totalTokens}
            deadCount={deadCount}
          />

          {/* Error Banner */}
          {error && (
            <div className="rounded-[var(--radius)] border border-danger/30 bg-danger/10 p-3 text-xs text-danger">
              ⚠ {error}
            </div>
          )}

          {/* World Graph */}
          <WorldGraph agents={agents} />

          {/* Economy Chart */}
          <EconomyChart events={events} />

          {/* Event Stream */}
          <EventStream events={events} connected={connected} />
        </div>
      </div>

      {/* Right Sidebar */}
      <div className="lg:border-l border-border bg-surface p-4 lg:p-0 lg:overflow-y-auto lg:scrollbar-thin">
        <div className="lg:p-4">
          <RightSidebar stats={stats} />
        </div>
      </div>
    </div>
  );
}
