"use client";

import { useEffect, useState, useMemo, useRef, useCallback } from "react";
import Link from "next/link";
import type { Agent } from "@/types/world";
import { fetchJSON } from "@/lib/api";
import { useSSEContext } from "@/components/SSEProvider";
import type { WorldEvent } from "@/types/world";
import {
  BarChart,
  Bar,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ResponsiveContainer,
  RadialBarChart,
  RadialBar,
  PolarAngleAxis,
} from "recharts";

// Skill category mapping
const skillCategories: Record<string, { category: string; color: string; order: number }> = {
  combat: { category: "战斗", color: "#ef4444", order: 0 },
  defense: { category: "战斗", color: "#ef4444", order: 0 },
  attack: { category: "战斗", color: "#ef4444", order: 0 },
  trading: { category: "经济", color: "#f59e0b", order: 1 },
  negotiation: { category: "经济", color: "#f59e0b", order: 1 },
  bartering: { category: "经济", color: "#f59e0b", order: 1 },
  commerce: { category: "经济", color: "#f59e0b", order: 1 },
  investment: { category: "经济", color: "#f59e0b", order: 1 },
  crafting: { category: "制造", color: "#8b5cf6", order: 2 },
  building: { category: "制造", color: "#8b5cf6", order: 2 },
  engineering: { category: "制造", color: "#8b5cf6", order: 2 },
  mining: { category: "制造", color: "#8b5cf6", order: 2 },
  farming: { category: "制造", color: "#8b5cf6", order: 2 },
  communication: { category: "社交", color: "#3b82f6", order: 3 },
  leadership: { category: "社交", color: "#3b82f6", order: 3 },
  persuasion: { category: "社交", color: "#3b82f6", order: 3 },
  charisma: { category: "社交", color: "#3b82f6", order: 3 },
  teaching: { category: "社交", color: "#3b82f6", order: 3 },
  research: { category: "知识", color: "#10b981", order: 4 },
  science: { category: "知识", color: "#10b981", order: 4 },
  medicine: { category: "知识", color: "#10b981", order: 4 },
  alchemy: { category: "知识", color: "#10b981", order: 4 },
  magic: { category: "知识", color: "#10b981", order: 4 },
  exploration: { category: "探索", color: "#06b6d4", order: 5 },
  navigation: { category: "探索", color: "#06b6d4", order: 5 },
  scouting: { category: "探索", color: "#06b6d4", order: 5 },
  survival: { category: "探索", color: "#06b6d4", order: 5 },
};

function getCategory(skillName: string) {
  const lower = skillName.toLowerCase();
  for (const [key, val] of Object.entries(skillCategories)) {
    if (lower.includes(key)) return val;
  }
  return { category: "其他", color: "#71717a", order: 99 };
}

// Canvas-based Evolution Tree visualization
function EvolutionTree({ agents }: { agents: Agent[] }) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const [dimensions, setDimensions] = useState({ width: 600, height: 500 });
  const [hoveredNode, setHoveredNode] = useState<string | null>(null);

  const treeData = useMemo(() => {
    // Group agents by their dominant skill category
    const categories = new Map<string, { agents: Agent[]; color: string }>();

    for (const agent of agents) {
      if (!agent.alive) continue;
      const skills = Object.entries(agent.skills);
      if (skills.length === 0) continue;

      const topSkill = skills.sort(([, a], [, b]) => b - a)[0];
      const cat = getCategory(topSkill[0]);

      if (!categories.has(cat.category)) {
        categories.set(cat.category, { agents: [], color: cat.color });
      }
      categories.get(cat.category)!.agents.push(agent);
    }

    return Array.from(categories.entries()).map(([category, data]) => ({
      category,
      color: data.color,
      agents: data.agents.sort((a, b) => {
        const aMax = Math.max(...Object.values(a.skills), 0);
        const bMax = Math.max(...Object.values(b.skills), 0);
        return bMax - aMax;
      }),
    }));
  }, [agents]);

  useEffect(() => {
    if (!containerRef.current) return;
    const observer = new ResizeObserver((entries) => {
      for (const entry of entries) {
        setDimensions({
          width: entry.contentRect.width,
          height: Math.max(entry.contentRect.height, 450),
        });
      }
    });
    observer.observe(containerRef.current);
    return () => observer.disconnect();
  }, []);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas || treeData.length === 0) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    const dpr = window.devicePixelRatio || 1;
    canvas.width = dimensions.width * dpr;
    canvas.height = dimensions.height * dpr;
    ctx.scale(dpr, dpr);

    ctx.clearRect(0, 0, dimensions.width, dimensions.height);

    const centerX = dimensions.width / 2;
    const centerY = 50;

    // Draw root
    ctx.beginPath();
    ctx.arc(centerX, centerY, 16, 0, Math.PI * 2);
    ctx.fillStyle = "#fafafa";
    ctx.fill();
    ctx.strokeStyle = "#52525b";
    ctx.lineWidth = 2;
    ctx.stroke();

    ctx.fillStyle = "#09090b";
    ctx.font = "bold 10px sans-serif";
    ctx.textAlign = "center";
    ctx.fillText("Agent", centerX, centerY + 4);

    // Draw category branches
    const branchCount = treeData.length;
    const angleStep = (Math.PI * 1.5) / Math.max(branchCount - 1, 1);
    const startAngle = Math.PI * 0.75;

    for (let i = 0; i < branchCount; i++) {
      const branch = treeData[i];
      const angle = startAngle + angleStep * i;
      const branchLength = 100;
      const branchX = centerX + Math.cos(angle) * branchLength;
      const branchY = centerY + Math.sin(angle) * branchLength;

      // Draw branch line
      ctx.beginPath();
      ctx.moveTo(centerX, centerY + 16);
      ctx.lineTo(branchX, branchY);
      ctx.strokeStyle = branch.color;
      ctx.lineWidth = 2;
      ctx.stroke();

      // Draw category node
      const isHovered = hoveredNode === branch.category;
      const nodeRadius = isHovered ? 20 : 16;
      ctx.beginPath();
      ctx.arc(branchX, branchY, nodeRadius, 0, Math.PI * 2);
      ctx.fillStyle = isHovered ? branch.color : "#18181b";
      ctx.fill();
      ctx.strokeStyle = branch.color;
      ctx.lineWidth = isHovered ? 3 : 2;
      ctx.stroke();

      // Category label
      ctx.fillStyle = isHovered ? "#fafafa" : "#a1a1aa";
      ctx.font = isHovered ? "bold 11px sans-serif" : "10px sans-serif";
      ctx.textAlign = "center";
      ctx.fillText(branch.category, branchX, branchY + nodeRadius + 14);
      ctx.fillStyle = "#71717a";
      ctx.font = "9px sans-serif";
      ctx.fillText(`${branch.agents.length}`, branchX, branchY + 4);

      // Draw leaf nodes (agents)
      const leafCount = Math.min(branch.agents.length, 5);
      const leafAngleStep = (Math.PI * 0.8) / Math.max(leafCount - 1, 1);
      const leafStartAngle = angle - (Math.PI * 0.4);

      for (let j = 0; j < leafCount; j++) {
        const leafAngle = leafStartAngle + leafAngleStep * j;
        const leafLength = 70;
        const leafX = branchX + Math.cos(leafAngle) * leafLength;
        const leafY = branchY + Math.sin(leafAngle) * leafLength;

        // Draw leaf line
        ctx.beginPath();
        ctx.moveTo(branchX, branchY);
        ctx.lineTo(leafX, leafY);
        ctx.strokeStyle = `${branch.color}80`;
        ctx.lineWidth = 1;
        ctx.stroke();

        const agent = branch.agents[j];
        const maxLevel = Math.max(...Object.values(agent.skills), 0);
        const leafRadius = 4 + maxLevel * 0.5;

        // Draw leaf node
        ctx.beginPath();
        ctx.arc(leafX, leafY, leafRadius, 0, Math.PI * 2);
        ctx.fillStyle = branch.color;
        ctx.globalAlpha = 0.6 + (maxLevel / 10) * 0.4;
        ctx.fill();
        ctx.globalAlpha = 1;

        // Agent label
        if (leafCount <= 3 || isHovered) {
          ctx.fillStyle = "#71717a";
          ctx.font = "8px sans-serif";
          ctx.textAlign = "center";
          ctx.fillText(
            agent.name.length > 6 ? agent.name.slice(0, 5) + "\u2026" : agent.name,
            leafX,
            leafY + leafRadius + 10
          );
        }
      }

      if (branch.agents.length > 5) {
        const moreAngle = startAngle + angleStep * i + Math.PI * 0.4;
        const moreX = branchX + Math.cos(moreAngle + Math.PI * 0.2) * 70;
        const moreY = branchY + Math.sin(moreAngle + Math.PI * 0.2) * 70;
        ctx.fillStyle = "#52525b";
        ctx.font = "9px sans-serif";
        ctx.textAlign = "center";
        ctx.fillText(`+${branch.agents.length - 5}`, moreX, moreY + 4);
      }
    }
  }, [dimensions, treeData, hoveredNode]);

  const handleMouseMove = useCallback(
    (e: React.MouseEvent<HTMLCanvasElement>) => {
      const canvas = canvasRef.current;
      if (!canvas) return;
      const rect = canvas.getBoundingClientRect();
      const x = e.clientX - rect.left;
      const y = e.clientY - rect.top;

      const centerX = dimensions.width / 2;
      const centerY = 50;
      const branchCount = treeData.length;
      const angleStep = (Math.PI * 1.5) / Math.max(branchCount - 1, 1);
      const startAngle = Math.PI * 0.75;

      let found: string | null = null;
      for (let i = 0; i < branchCount; i++) {
        const angle = startAngle + angleStep * i;
        const branchX = centerX + Math.cos(angle) * 100;
        const branchY = centerY + Math.sin(angle) * 100;
        const dx = x - branchX;
        const dy = y - branchY;
        if (dx * dx + dy * dy < 24 * 24) {
          found = treeData[i].category;
          break;
        }
      }
      setHoveredNode(found);
      canvas.style.cursor = found ? "pointer" : "default";
    },
    [dimensions, treeData]
  );

  if (treeData.length === 0) {
    return (
      <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-6">
        <p className="text-center text-sm text-zinc-600">暂无进化数据</p>
      </div>
    );
  }

  return (
    <div ref={containerRef} className="relative" style={{ height: 500 }}>
      <canvas
        ref={canvasRef}
        style={{ width: dimensions.width, height: dimensions.height }}
        onMouseMove={handleMouseMove}
        aria-label="Agent 技能进化树"
        role="img"
        className="rounded-lg"
      />
    </div>
  );
}

const CustomTooltip = ({ active, payload, label }: { active?: boolean; payload?: Array<{ value: number; name: string; color: string }>; label?: string }) => {
  if (!active || !payload) return null;
  return (
    <div className="rounded-lg border border-zinc-700 bg-zinc-800/95 px-3 py-2 text-xs shadow-xl backdrop-blur-sm">
      <p className="text-zinc-400 mb-1">{label}</p>
      {payload.map((p, i) => (
        <p key={i} style={{ color: p.color }} className="font-medium tabular-nums">
          {p.name}: {typeof p.value === "number" ? p.value.toFixed(1) : p.value}
        </p>
      ))}
    </div>
  );
};

export default function EvolutionPage() {
  const [agents, setAgents] = useState<Agent[]>([]);
  const [loading, setLoading] = useState(true);
  const [viewMode, setViewMode] = useState<"tree" | "skills" | "agents">("tree");

  const sse = useSSEContext();

  const loadData = useCallback(async () => {
    try {
      const agentsData = await fetchJSON<Agent[]>("/api/v1/agents");
      setAgents(agentsData);
    } catch {
      // silently fail
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

  useEffect(() => {
    function onEvent(event: WorldEvent) {
      if (event.type === "skill_up" || event.type === "agent_spawn" || event.type === "agent_death") {
        loadData();
      }
    }
    const unsubscribe = sse.subscribe(onEvent);
    return unsubscribe;
  }, [sse, loadData]);

  // Aggregate skill stats
  const skillStats = useMemo(() => {
    const stats = new Map<string, { count: number; totalLevel: number; maxLevel: number; category: string; color: string }>();

    for (const agent of agents) {
      if (!agent.alive) continue;
      for (const [name, level] of Object.entries(agent.skills)) {
        const existing = stats.get(name);
        const cat = getCategory(name);
        if (existing) {
          existing.count++;
          existing.totalLevel += level;
          existing.maxLevel = Math.max(existing.maxLevel, level);
        } else {
          stats.set(name, { count: 1, totalLevel: level, maxLevel: level, category: cat.category, color: cat.color });
        }
      }
    }

    return Array.from(stats.entries())
      .map(([name, data]) => ({
        name,
        count: data.count,
        avgLevel: data.totalLevel / data.count,
        maxLevel: data.maxLevel,
        category: data.category,
        color: data.color,
      }))
      .sort((a, b) => b.count - a.count);
  }, [agents]);

  // Top agents by skill count / level
  const topAgents = useMemo(() => {
    return agents
      .filter((a) => a.alive)
      .map((a) => ({
        ...a,
        skillCount: Object.keys(a.skills).length,
        avgLevel: Object.values(a.skills).length > 0
          ? Object.values(a.skills).reduce((s, v) => s + v, 0) / Object.values(a.skills).length
          : 0,
        maxLevel: Math.max(...Object.values(a.skills), 0),
      }))
      .sort((a, b) => b.maxLevel - a.maxLevel || b.skillCount - a.skillCount)
      .slice(0, 20);
  }, [agents]);

  // Category summary for radial chart
  const categorySummary = useMemo(() => {
    const cats = new Map<string, { total: number; count: number }>();
    for (const skill of skillStats) {
      const existing = cats.get(skill.category);
      if (existing) {
        existing.total += skill.avgLevel;
        existing.count++;
      } else {
        cats.set(skill.category, { total: skill.avgLevel, count: 1 });
      }
    }
    return Array.from(cats.entries()).map(([name, data]) => ({
      name,
      value: data.count > 0 ? data.total / data.count : 0,
      fill: getCategory(name).color,
    }));
  }, [skillStats]);

  if (loading) {
    return (
      <div className="flex h-full items-center justify-center">
        <div className="text-sm text-zinc-600">正在加载进化数据...</div>
      </div>
    );
  }

  const aliveAgents = agents.filter((a) => a.alive);
  const totalSkills = skillStats.length;
  const masteredSkills = skillStats.filter((s) => s.maxLevel >= 8).length;

  return (
    <div className="p-4 md:p-6 space-y-6">
      {/* Header */}
      <div>
        <h1 className="text-xl md:text-2xl font-bold text-zinc-100">进化树可视化</h1>
        <p className="text-sm text-zinc-500">
          {aliveAgents.length} 存活 Agent · {totalSkills} 种技能 · {masteredSkills} 已精通
        </p>
      </div>

      {/* Stats */}
      <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 xl:grid-cols-4">
        <div className="rounded-xl border border-emerald-500/20 bg-emerald-500/5 p-4 space-y-1">
          <p className="text-sm text-zinc-400">存活 Agent</p>
          <p className="text-2xl font-bold text-emerald-400">{aliveAgents.length}</p>
        </div>
        <div className="rounded-xl border border-blue-500/20 bg-blue-500/5 p-4 space-y-1">
          <p className="text-sm text-zinc-400">技能种类</p>
          <p className="text-2xl font-bold text-blue-400">{totalSkills}</p>
        </div>
        <div className="rounded-xl border border-purple-500/20 bg-purple-500/5 p-4 space-y-1">
          <p className="text-sm text-zinc-400">精通技能</p>
          <p className="text-2xl font-bold text-purple-400">{masteredSkills}</p>
          <p className="text-xs text-zinc-500">Lv.8+</p>
        </div>
        <div className="rounded-xl border border-amber-500/20 bg-amber-500/5 p-4 space-y-1">
          <p className="text-sm text-zinc-400">技能类别</p>
          <p className="text-2xl font-bold text-amber-400">{categorySummary.length}</p>
        </div>
      </div>

      {/* View Mode Tabs */}
      <div className="flex items-center gap-1 border-b border-zinc-800">
        {(["tree", "skills", "agents"] as const).map((mode) => {
          const labels = { tree: "进化树", skills: "技能分布", agents: "Agent 排行" };
          return (
            <button
              key={mode}
              onClick={() => setViewMode(mode)}
              className={`px-4 py-2.5 text-sm font-medium transition-colors border-b-2 ${
                viewMode === mode
                  ? "text-blue-400 border-blue-400"
                  : "text-zinc-400 border-transparent hover:text-zinc-200"
              }`}
            >
              {labels[mode]}
            </button>
          );
        })}
      </div>

      {/* Tree View */}
      {viewMode === "tree" && (
        <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4 space-y-3">
          <h2 className="text-sm font-semibold text-zinc-200">Agent 技能进化路径</h2>
          <EvolutionTree agents={agents} />
        </div>
      )}

      {/* Skills Distribution View */}
      {viewMode === "skills" && (
        <div className="grid grid-cols-1 gap-6 xl:grid-cols-2">
          {/* Skill frequency chart */}
          <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4 space-y-3">
            <h2 className="text-sm font-semibold text-zinc-200">技能普及度 Top 15</h2>
            <ResponsiveContainer width="100%" height={300}>
              <BarChart data={skillStats.slice(0, 15)} layout="vertical">
                <CartesianGrid strokeDasharray="3 3" stroke="#27272a" horizontal={false} />
                <XAxis type="number" stroke="#52525b" tick={{ fontSize: 10 }} />
                <YAxis type="category" dataKey="name" stroke="#52525b" tick={{ fontSize: 10 }} width={80} />
                <Tooltip content={<CustomTooltip />} />
                <Bar dataKey="count" name="掌握人数" radius={[0, 4, 4, 0]}>
                  {skillStats.slice(0, 15).map((entry, index) => (
                    <rect key={index} fill={entry.color} />
                  ))}
                </Bar>
              </BarChart>
            </ResponsiveContainer>
          </div>

          {/* Category radial chart */}
          <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4 space-y-3">
            <h2 className="text-sm font-semibold text-zinc-200">技能类别平均等级</h2>
            <ResponsiveContainer width="100%" height={300}>
              <RadialBarChart cx="50%" cy="50%" innerRadius="20%" outerRadius="90%" data={categorySummary.map((c) => ({ ...c, fill: c.fill }))}>
                <PolarAngleAxis type="number" domain={[0, 10]} tick={false} />
                <RadialBar background dataKey="value" />
                <Tooltip content={<CustomTooltip />} />
              </RadialBarChart>
            </ResponsiveContainer>
            <div className="flex flex-wrap items-center justify-center gap-3">
              {categorySummary.map((cat) => (
                <span key={cat.name} className="flex items-center gap-1 text-[10px] text-zinc-400">
                  <span className="inline-block h-2.5 w-2.5 rounded-full" style={{ backgroundColor: cat.fill }} />
                  {cat.name} ({cat.value.toFixed(1)})
                </span>
              ))}
            </div>
          </div>

          {/* Skill level distribution */}
          <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4 space-y-3 xl:col-span-2">
            <h2 className="text-sm font-semibold text-zinc-200">全部技能详情</h2>
            <div className="flex flex-wrap gap-2">
              {skillStats.map((skill) => (
                <div
                  key={skill.name}
                  className="rounded-lg border border-zinc-800 bg-zinc-900/30 px-3 py-1.5 space-y-0.5"
                >
                  <div className="flex items-center gap-2">
                    <span className="inline-block h-2 w-2 rounded-full" style={{ backgroundColor: skill.color }} />
                    <span className="text-xs text-zinc-200">{skill.name}</span>
                  </div>
                  <div className="flex items-center gap-2 text-[10px] text-zinc-500">
                    <span>{skill.count} 人</span>
                    <span>平均 Lv.{skill.avgLevel.toFixed(1)}</span>
                    <span className="text-amber-400">最高 Lv.{skill.maxLevel}</span>
                  </div>
                </div>
              ))}
            </div>
          </div>
        </div>
      )}

      {/* Agent Rankings View */}
      {viewMode === "agents" && (
        <div className="space-y-4">
          <div className="rounded-xl border border-zinc-800 bg-zinc-900/50">
            <div className="overflow-x-auto">
              <table className="w-full">
                <thead>
                  <tr className="border-b border-zinc-800">
                    <th className="px-4 py-3 text-left text-xs font-semibold text-zinc-400">排名</th>
                    <th className="px-4 py-3 text-left text-xs font-semibold text-zinc-400">名称</th>
                    <th className="px-4 py-3 text-right text-xs font-semibold text-zinc-400">技能数</th>
                    <th className="px-4 py-3 text-right text-xs font-semibold text-zinc-400">最高等级</th>
                    <th className="px-4 py-3 text-right text-xs font-semibold text-zinc-400">平均等级</th>
                    <th className="px-4 py-3 text-left text-xs font-semibold text-zinc-400">核心技能</th>
                  </tr>
                </thead>
                <tbody>
                  {topAgents.map((agent, idx) => {
                    const topSkills = Object.entries(agent.skills)
                      .sort(([, a], [, b]) => b - a)
                      .slice(0, 3);
                    return (
                      <tr key={agent.id} className="border-b border-zinc-800/50 last:border-0">
                        <td className="px-4 py-3">
                          <span className={`text-sm font-bold ${
                            idx === 0 ? "text-amber-400" : idx === 1 ? "text-zinc-300" : idx === 2 ? "text-amber-700" : "text-zinc-500"
                          }`}>
                            #{idx + 1}
                          </span>
                        </td>
                        <td className="px-4 py-3">
                          <Link href={`/agents/${agent.id}`} className="text-sm font-medium text-zinc-200 hover:text-blue-400 transition-colors">
                            {agent.name}
                          </Link>
                        </td>
                        <td className="px-4 py-3 text-right text-sm text-zinc-300 tabular-nums">{agent.skillCount}</td>
                        <td className="px-4 py-3 text-right">
                          <span className={`text-sm font-medium tabular-nums ${
                            agent.maxLevel >= 8 ? "text-purple-400" : agent.maxLevel >= 5 ? "text-blue-400" : "text-zinc-300"
                          }`}>
                            {agent.maxLevel}
                          </span>
                        </td>
                        <td className="px-4 py-3 text-right text-sm text-zinc-400 tabular-nums">{agent.avgLevel.toFixed(1)}</td>
                        <td className="px-4 py-3">
                          <div className="flex flex-wrap gap-1">
                            {topSkills.map(([name, level]) => (
                              <span key={name} className={`rounded px-1.5 py-0.5 text-[10px] ${
                                level >= 8 ? "bg-purple-500/10 text-purple-400" :
                                level >= 5 ? "bg-blue-500/10 text-blue-400" :
                                "bg-zinc-800 text-zinc-400"
                              }`}>
                                {name} {level}
                              </span>
                            ))}
                          </div>
                        </td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
