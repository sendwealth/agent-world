"use client";

import { useEffect, useState, useMemo, useRef, useCallback } from "react";
import Link from "next/link";
import type { Organization } from "@/types/world";
import { fetchJSON } from "@/lib/api";
import { useSSEContext } from "@/components/SSEProvider";
import type { WorldEvent } from "@/types/world";

const orgTypeLabels: Record<string, string> = {
  company: "公司",
  guild: "公会",
  alliance: "联盟",
  university: "大学",
};

const orgTypeColors: Record<string, string> = {
  company: "bg-blue-500/10 text-blue-400 border-blue-500/20",
  guild: "bg-purple-500/10 text-purple-400 border-purple-500/20",
  alliance: "bg-emerald-500/10 text-emerald-400 border-emerald-500/20",
  university: "bg-amber-500/10 text-amber-400 border-amber-500/20",
};

const statusLabels: Record<string, string> = {
  active: "活跃",
  inactive: "不活跃",
  dissolved: "已解散",
};

const statusColors: Record<string, string> = {
  active: "bg-green-500/10 text-green-400",
  inactive: "bg-zinc-500/10 text-zinc-400",
  dissolved: "bg-red-500/10 text-red-400",
};

// Force-directed graph using Canvas (same pattern as RelationshipGraph.tsx)
interface GraphNode {
  id: string;
  name: string;
  type: string;
  memberCount: number;
  treasury: number;
  x: number;
  y: number;
  vx: number;
  vy: number;
}

interface GraphEdge {
  source: string;
  target: string;
  weight: number;
}

function OrganizationGraph({ orgs }: { orgs: Organization[] }) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const nodesRef = useRef<GraphNode[]>([]);
  const edgesRef = useRef<GraphEdge[]>([]);
  const [hoveredNode, setHoveredNode] = useState<string | null>(null);
  const [dimensions, setDimensions] = useState({ width: 600, height: 450 });

  const { nodes, edges } = useMemo(() => {
    if (orgs.length === 0) return { nodes: [], edges: [] };

    const centerX = dimensions.width / 2;
    const centerY = dimensions.height / 2;

    const ns: GraphNode[] = orgs.map((org, i) => {
      const angle = (2 * Math.PI * i) / orgs.length;
      const radius = 120 + ((i * 37 + org.member_count * 13) % 80);
      return {
        id: org.id,
        name: org.name,
        type: org.type,
        memberCount: org.member_count,
        treasury: org.treasury,
        x: centerX + Math.cos(angle) * radius,
        y: centerY + Math.sin(angle) * radius,
        vx: 0,
        vy: 0,
      };
    });

    // Build edges: orgs sharing members
    const es: GraphEdge[] = [];
    const memberOrgs = new Map<string, string[]>();
    for (const org of orgs) {
      for (const m of org.members) {
        const list = memberOrgs.get(m.agent_id) ?? [];
        list.push(org.id);
        memberOrgs.set(m.agent_id, list);
      }
    }
    const edgeMap = new Map<string, GraphEdge>();
    for (const [, orgIds] of memberOrgs) {
      for (let i = 0; i < orgIds.length; i++) {
        for (let j = i + 1; j < orgIds.length; j++) {
          const key = [orgIds[i], orgIds[j]].sort().join("-");
          const existing = edgeMap.get(key);
          if (existing) {
            existing.weight++;
          } else {
            edgeMap.set(key, { source: orgIds[i], target: orgIds[j], weight: 1 });
          }
        }
      }
    }
    es.push(...edgeMap.values());

    return { nodes: ns, edges: es };
  }, [orgs, dimensions]);

  useEffect(() => {
    nodesRef.current = nodes.map((n) => ({ ...n }));
    edgesRef.current = edges;
  }, [nodes, edges]);

  useEffect(() => {
    if (!containerRef.current) return;
    const observer = new ResizeObserver((entries) => {
      for (const entry of entries) {
        setDimensions({
          width: entry.contentRect.width,
          height: Math.max(entry.contentRect.height, 400),
        });
      }
    });
    observer.observe(containerRef.current);
    return () => observer.disconnect();
  }, []);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    const dpr = window.devicePixelRatio || 1;
    canvas.width = dimensions.width * dpr;
    canvas.height = dimensions.height * dpr;
    ctx.scale(dpr, dpr);

    let frameCount = 0;
    let running = true;

    const simulate = () => {
      const ns = nodesRef.current;
      if (ns.length === 0) return;

      const centerX = dimensions.width / 2;
      const centerY = dimensions.height / 2;

      for (let i = 0; i < ns.length; i++) {
        const node = ns[i];

        // Repulsion from other nodes
        for (let j = 0; j < ns.length; j++) {
          if (i === j) continue;
          const other = ns[j];
          const dx = node.x - other.x;
          const dy = node.y - other.y;
          const dist = Math.sqrt(dx * dx + dy * dy) || 1;
          const force = 2000 / (dist * dist);
          node.vx += (dx / dist) * force;
          node.vy += (dy / dist) * force;
        }

        // Attraction to center
        const dxCenter = centerX - node.x;
        const dyCenter = centerY - node.y;
        node.vx += dxCenter * 0.003;
        node.vy += dyCenter * 0.003;

        // Damping
        node.vx *= 0.85;
        node.vy *= 0.85;

        node.x += node.vx;
        node.y += node.vy;

        // Bounds
        node.x = Math.max(50, Math.min(dimensions.width - 50, node.x));
        node.y = Math.max(50, Math.min(dimensions.height - 50, node.y));
      }
    };

    function draw() {
      if (!running || !ctx) return;
      frameCount++;
      if (frameCount < 300) simulate();

      const ns = nodesRef.current;
      const es = edgesRef.current;

      ctx.clearRect(0, 0, dimensions.width, dimensions.height);

      let maxWeight = 1;
      for (const e of es) {
        if (e.weight > maxWeight) maxWeight = e.weight;
      }

      // Draw edges
      const nodeMap = new Map(ns.map((n) => [n.id, n]));
      for (const edge of es) {
        const src = nodeMap.get(edge.source);
        const tgt = nodeMap.get(edge.target);
        if (!src || !tgt) continue;

        const thickness = 1 + (edge.weight / maxWeight) * 3;
        const alpha = 0.2 + (edge.weight / maxWeight) * 0.4;

        ctx.beginPath();
        ctx.moveTo(src.x, src.y);
        ctx.lineTo(tgt.x, tgt.y);
        ctx.strokeStyle = `rgba(96, 165, 250, ${alpha})`;
        ctx.lineWidth = thickness;
        ctx.stroke();
      }

      // Draw nodes
      const typeColors: Record<string, string> = {
        company: "#3b82f6",
        guild: "#a855f7",
        alliance: "#10b981",
        university: "#f59e0b",
      };

      for (const node of ns) {
        const isHovered = hoveredNode === node.id;
        const radius = Math.max(12, Math.min(24, 10 + node.memberCount * 2));
        const r = isHovered ? radius + 4 : radius;
        const color = typeColors[node.type] ?? "#6b7280";

        // Glow
        if (isHovered) {
          const glow = ctx.createRadialGradient(node.x, node.y, r, node.x, node.y, r + 16);
          glow.addColorStop(0, color.replace(")", ", 0.4)").replace("rgb", "rgba").replace("#", ""));
          glow.addColorStop(1, "rgba(0,0,0,0)");
          ctx.beginPath();
          ctx.arc(node.x, node.y, r + 16, 0, Math.PI * 2);
          ctx.fillStyle = `rgba(59, 130, 246, 0.2)`;
          ctx.fill();
        }

        // Node circle
        ctx.beginPath();
        ctx.arc(node.x, node.y, r, 0, Math.PI * 2);
        ctx.fillStyle = isHovered ? color : "#18181b";
        ctx.fill();
        ctx.strokeStyle = color;
        ctx.lineWidth = isHovered ? 3 : 2;
        ctx.stroke();

        // Label
        ctx.fillStyle = isHovered ? "#fafafa" : "#a1a1aa";
        ctx.font = isHovered ? "bold 12px sans-serif" : "11px sans-serif";
        ctx.textAlign = "center";
        ctx.fillText(
          node.name.length > 10 ? node.name.slice(0, 9) + "\u2026" : node.name,
          node.x,
          node.y + r + 16
        );
      }

      if (frameCount < 300 || hoveredNode !== null) {
        rafId = requestAnimationFrame(draw);
      }
    }

    let rafId = requestAnimationFrame(draw);
    return () => {
      running = false;
      cancelAnimationFrame(rafId);
    };
  }, [dimensions, hoveredNode]);

  const handleMouseMove = useCallback(
    (e: React.MouseEvent<HTMLCanvasElement>) => {
      const canvas = canvasRef.current;
      if (!canvas) return;
      const rect = canvas.getBoundingClientRect();
      const x = e.clientX - rect.left;
      const y = e.clientY - rect.top;

      const ns = nodesRef.current;
      let found: string | null = null;
      for (const node of ns) {
        const r = Math.max(12, Math.min(24, 10 + node.memberCount * 2));
        const dx = x - node.x;
        const dy = y - node.y;
        if (dx * dx + dy * dy < (r + 8) * (r + 8)) {
          found = node.id;
          break;
        }
      }
      setHoveredNode(found);
      canvas.style.cursor = found ? "pointer" : "default";
    },
    []
  );

  if (orgs.length === 0) {
    return (
      <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-6">
        <p className="text-center text-sm text-zinc-600">暂无组织数据</p>
      </div>
    );
  }

  return (
    <div ref={containerRef} className="relative" style={{ height: 450 }}>
      <canvas
        ref={canvasRef}
        style={{ width: dimensions.width, height: dimensions.height }}
        onMouseMove={handleMouseMove}
        aria-label="组织关系力导向图"
        role="img"
        className="rounded-lg"
      />
      {/* Legend */}
      <div className="absolute bottom-2 left-2 flex flex-wrap items-center gap-3 text-[10px] text-zinc-500">
        {Object.entries(orgTypeLabels).map(([key, label]) => {
          const colors: Record<string, string> = {
            company: "bg-blue-500",
            guild: "bg-purple-500",
            alliance: "bg-emerald-500",
            university: "bg-amber-500",
          };
          return (
            <span key={key} className="flex items-center gap-1">
              <span className={`inline-block h-2.5 w-2.5 rounded-full ${colors[key]}`} />
              {label}
            </span>
          );
        })}
      </div>
    </div>
  );
}

export default function OrganizationsPage() {
  const [orgs, setOrgs] = useState<Organization[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [typeFilter, setTypeFilter] = useState<string>("all");

  const sse = useSSEContext();

  const loadOrgs = useCallback(async () => {
    try {
      const data = await fetchJSON<Organization[]>("/api/v1/orgs");
      setOrgs(data);
      setError(null);
    } catch {
      setError("无法加载组织数据");
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    (async () => {
      await loadOrgs();
    })();
    const interval = setInterval(loadOrgs, 10000);
    return () => clearInterval(interval);
  }, [loadOrgs]);

  // SSE-driven refresh
  useEffect(() => {
    function onEvent(event: WorldEvent) {
      const t = event.type as string;
      if (
        t.startsWith("org_") ||
        t === "trade" ||
        t === "investment"
      ) {
        loadOrgs();
      }
    }
    const unsubscribe = sse.subscribe(onEvent);
    return unsubscribe;
  }, [sse, loadOrgs]);

  const filtered = useMemo(() => {
    if (typeFilter === "all") return orgs;
    return orgs.filter((o) => o.type === typeFilter);
  }, [orgs, typeFilter]);

  const activeCount = orgs.filter((o) => o.status === "active").length;
  const totalMembers = orgs.reduce((sum, o) => sum + o.member_count, 0);
  const totalTreasury = orgs.reduce((sum, o) => sum + o.treasury, 0);

  const filterButtons = [
    { value: "all", label: "全部", count: orgs.length },
    { value: "company", label: "公司", count: orgs.filter((o) => o.type === "company").length },
    { value: "guild", label: "公会", count: orgs.filter((o) => o.type === "guild").length },
    { value: "alliance", label: "联盟", count: orgs.filter((o) => o.type === "alliance").length },
    { value: "university", label: "大学", count: orgs.filter((o) => o.type === "university").length },
  ];

  return (
    <div className="p-4 md:p-6 space-y-6">
      {/* Header */}
      <div>
        <h1 className="text-xl md:text-2xl font-bold text-zinc-100">组织关系图</h1>
        <p className="text-sm text-zinc-500">
          {loading
            ? "正在加载..."
            : `${orgs.length} 个组织 · ${activeCount} 活跃 · ${totalMembers} 成员 · 总资金 $${totalTreasury.toLocaleString()}`}
        </p>
      </div>

      {error && (
        <div className="rounded-lg bg-red-500/10 border border-red-500/20 px-3 py-1.5 text-xs text-red-400">
          {error}
        </div>
      )}

      {/* Force Graph */}
      {!loading && (
        <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4 space-y-3">
          <div className="flex items-center justify-between">
            <h2 className="text-sm font-semibold text-zinc-200">组织关系网络</h2>
            <span className="text-xs text-zinc-500">
              {filtered.length} 个组织 · {edgesForOrgs(filtered)} 条关联
            </span>
          </div>
          <OrganizationGraph orgs={filtered} />
        </div>
      )}

      {/* Filters */}
      <div className="flex flex-wrap items-center gap-1.5">
        {filterButtons.map((btn) => (
          <button
            key={btn.value}
            onClick={() => setTypeFilter(btn.value)}
            className={`rounded-lg px-3 py-1.5 text-xs font-medium transition-colors ${
              typeFilter === btn.value
                ? "bg-blue-500/15 text-blue-400 border border-blue-500/30"
                : "bg-zinc-800/50 text-zinc-400 border border-zinc-800 hover:bg-zinc-800 hover:text-zinc-300"
            }`}
          >
            {btn.label} ({btn.count})
          </button>
        ))}
      </div>

      {/* Organization Cards */}
      {loading ? (
        <div className="flex h-48 items-center justify-center text-sm text-zinc-600">
          正在加载组织数据...
        </div>
      ) : filtered.length === 0 ? (
        <div className="flex h-48 items-center justify-center text-sm text-zinc-600">
          暂无组织数据
        </div>
      ) : (
        <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 xl:grid-cols-3">
          {filtered.map((org) => (
            <Link
              key={org.id}
              href={`/organizations/${org.id}`}
              className="block rounded-xl border border-zinc-800 bg-zinc-900/50 p-4 transition-colors hover:bg-zinc-800/50 space-y-3"
            >
              <div className="flex items-center justify-between">
                <span className="text-sm font-medium text-zinc-200">{org.name}</span>
                <span className={`rounded-full border px-2 py-0.5 text-[10px] font-medium ${orgTypeColors[org.type] ?? ""}`}>
                  {orgTypeLabels[org.type] ?? org.type}
                </span>
              </div>
              <div className="flex items-center gap-3 text-xs text-zinc-400">
                <span className={`rounded-full px-2 py-0.5 ${statusColors[org.status] ?? ""}`}>
                  {statusLabels[org.status] ?? org.status}
                </span>
                <span>{org.member_count} 成员</span>
                <span>资金 ${org.treasury.toLocaleString()}</span>
              </div>
              <div className="flex flex-wrap gap-1">
                {org.members.slice(0, 5).map((m) => (
                  <span key={m.agent_id} className="rounded bg-zinc-800 px-1.5 py-0.5 text-[10px] text-zinc-400">
                    {m.agent_name}
                  </span>
                ))}
                {org.members.length > 5 && (
                  <span className="text-[10px] text-zinc-600">+{org.members.length - 5}</span>
                )}
              </div>
            </Link>
          ))}
        </div>
      )}
    </div>
  );
}

function edgesForOrgs(orgs: Organization[]): number {
  const memberOrgs = new Map<string, string[]>();
  for (const org of orgs) {
    for (const m of org.members) {
      const list = memberOrgs.get(m.agent_id) ?? [];
      list.push(org.id);
      memberOrgs.set(m.agent_id, list);
    }
  }
  let count = 0;
  for (const [, orgIds] of memberOrgs) {
    for (let i = 0; i < orgIds.length; i++) {
      for (let j = i + 1; j < orgIds.length; j++) {
        count++;
      }
    }
  }
  return count;
}
