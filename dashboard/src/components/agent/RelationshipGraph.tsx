"use client";

import { useMemo, useRef, useEffect, useCallback, useState } from "react";
import { useRouter } from "next/navigation";
import Link from "next/link";
import type { Agent, WorldEvent } from "@/types/world";
import { phaseLabels } from "@/lib/format";

interface RelationshipGraphProps {
  agent: Agent;
  allAgents: Agent[];
  agentEvents: WorldEvent[];
}

interface GraphNode {
  id: string;
  name: string;
  alive: boolean;
  phase: string;
  x: number;
  y: number;
  vx: number;
  vy: number;
  isCenter: boolean;
}

interface GraphEdge {
  source: string;
  target: string;
  weight: number;
}

export default function RelationshipGraph({ agent, allAgents, agentEvents }: RelationshipGraphProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const nodesRef = useRef<GraphNode[]>([]);
  const edgesRef = useRef<GraphEdge[]>([]);
  const [hoveredNode, setHoveredNode] = useState<string | null>(null);
  const [dimensions, setDimensions] = useState({ width: 400, height: 350 });
  const router = useRouter();

  // Compute graph data
  const { nodes, edges, agentMap } = useMemo(() => {
    // Build relationship edges
    const edgeMap = new Map<string, GraphEdge>();
    const connectedIds = new Set<string>();

    for (const e of agentEvents) {
      let sourceId: string | undefined;
      let targetId: string | undefined;

      if (e.agentId === agent.id) {
        sourceId = agent.id;
        targetId = e.targetId;
      } else if (e.targetId === agent.id) {
        sourceId = e.agentId;
        targetId = agent.id;
      } else if (e.agentId && e.targetId) {
        sourceId = e.agentId;
        targetId = e.targetId;
      }

      if (sourceId && targetId && sourceId !== targetId) {
        const key = [sourceId, targetId].sort().join("-");
        const existing = edgeMap.get(key);
        if (existing) {
          existing.weight++;
        } else {
          edgeMap.set(key, {
            source: sourceId,
            target: targetId,
            weight: 1,
          });
        }
        if (sourceId === agent.id) connectedIds.add(targetId);
        if (targetId === agent.id) connectedIds.add(sourceId);
      }
    }

    // Build nodes: center + connected agents
    const aMap = new Map<string, Agent>();
    for (const a of allAgents) aMap.set(a.id, a);

    const n: GraphNode[] = [];
    const centerX = 200;
    const centerY = 175;

    n.push({
      id: agent.id,
      name: agent.name,
      alive: agent.alive,
      phase: agent.phase,
      x: centerX,
      y: centerY,
      vx: 0,
      vy: 0,
      isCenter: true,
    });

    let angle = 0;
    const angleStep = (2 * Math.PI) / Math.max(connectedIds.size, 1);
    for (const id of connectedIds) {
      const a = aMap.get(id);
      const radius = 100 + ((id.charCodeAt(0) * 7 + id.charCodeAt(1) * 13 + id.charCodeAt(2) * 3) % 30);
      n.push({
        id,
        name: a?.name ?? id.slice(0, 6),
        alive: a?.alive ?? false,
        phase: a?.phase ?? "",
        x: centerX + Math.cos(angle) * radius,
        y: centerY + Math.sin(angle) * radius,
        vx: 0,
        vy: 0,
        isCenter: false,
      });
      angle += angleStep;
    }

    return {
      nodes: n,
      edges: Array.from(edgeMap.values()),
      agentMap: aMap,
    };
  }, [agent, allAgents, agentEvents]);

  // Pre-compute weight map for O(1) lookup instead of O(E) edges.find()
  const weightMap = useMemo(() => {
    const m = new Map<string, number>();
    for (const e of edges) {
      if (e.source === agent.id) m.set(e.target, e.weight);
      else if (e.target === agent.id) m.set(e.source, e.weight);
    }
    return m;
  }, [edges, agent.id]);

  // Update refs
  useEffect(() => {
    nodesRef.current = nodes.map((n) => ({ ...n }));
    edgesRef.current = edges;
  }, [nodes, edges]);

  // Observe container size
  useEffect(() => {
    if (!containerRef.current) return;
    const observer = new ResizeObserver((entries) => {
      for (const entry of entries) {
        setDimensions({
          width: entry.contentRect.width,
          height: Math.max(entry.contentRect.height, 300),
        });
      }
    });
    observer.observe(containerRef.current);
    return () => observer.disconnect();
  }, []);

  // Force simulation + rendering
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

      // Keep center node pinned
      ns[0].x = centerX;
      ns[0].y = centerY;
      ns[0].vx = 0;
      ns[0].vy = 0;

      // Apply forces
      for (let i = 1; i < ns.length; i++) {
        const node = ns[i];

        // Repulsion from other nodes
        for (let j = 1; j < ns.length; j++) {
          if (i === j) continue;
          const other = ns[j];
          const dx = node.x - other.x;
          const dy = node.y - other.y;
          const dist = Math.sqrt(dx * dx + dy * dy) || 1;
          const force = 800 / (dist * dist);
          node.vx += (dx / dist) * force;
          node.vy += (dy / dist) * force;
        }

        // Attraction to center
        const dxCenter = centerX - node.x;
        const dyCenter = centerY - node.y;
        node.vx += dxCenter * 0.002;
        node.vy += dyCenter * 0.002;

        // Damping
        node.vx *= 0.85;
        node.vy *= 0.85;

        node.x += node.vx;
        node.y += node.vy;

        // Bounds
        node.x = Math.max(40, Math.min(dimensions.width - 40, node.x));
        node.y = Math.max(40, Math.min(dimensions.height - 40, node.y));
      }
    };

    function draw() {
      if (!running || !ctx) return;
      frameCount++;
      if (frameCount < 300) simulate();

      const ns = nodesRef.current;
      const es = edgesRef.current;

      ctx.clearRect(0, 0, dimensions.width, dimensions.height);

      // Compute maxWeight once
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

        const thickness = 0.5 + (edge.weight / maxWeight) * 2.5;
        const alpha = 0.15 + (edge.weight / maxWeight) * 0.35;

        ctx.beginPath();
        ctx.moveTo(src.x, src.y);
        ctx.lineTo(tgt.x, tgt.y);
        ctx.strokeStyle = `rgba(148, 163, 184, ${alpha})`;
        ctx.lineWidth = thickness;
        ctx.stroke();

        // Edge weight label for thick edges
        if (edge.weight >= 3) {
          const midX = (src.x + tgt.x) / 2;
          const midY = (src.y + tgt.y) / 2;
          ctx.fillStyle = "rgba(113, 113, 122, 0.6)";
          ctx.font = "9px sans-serif";
          ctx.textAlign = "center";
          ctx.fillText(`${edge.weight}`, midX, midY - 3);
        }
      }

      // Draw nodes
      for (const node of ns) {
        const isHovered = hoveredNode === node.id;
        const radius = node.isCenter ? 20 : 12;
        const r = isHovered ? radius + 3 : radius;

        // Glow
        if (node.isCenter || isHovered) {
          const glow = ctx.createRadialGradient(node.x, node.y, r, node.x, node.y, r + 12);
          const glowColor = node.alive ? "59, 130, 246" : "239, 68, 68";
          glow.addColorStop(0, `rgba(${glowColor}, 0.3)`);
          glow.addColorStop(1, `rgba(${glowColor}, 0)`);
          ctx.beginPath();
          ctx.arc(node.x, node.y, r + 12, 0, Math.PI * 2);
          ctx.fillStyle = glow;
          ctx.fill();
        }

        // Node circle
        ctx.beginPath();
        ctx.arc(node.x, node.y, r, 0, Math.PI * 2);

        if (node.isCenter) {
          const grad = ctx.createRadialGradient(node.x - 3, node.y - 3, 0, node.x, node.y, r);
          grad.addColorStop(0, node.alive ? "#3b82f6" : "#ef4444");
          grad.addColorStop(1, node.alive ? "#1d4ed8" : "#b91c1c");
          ctx.fillStyle = grad;
        } else {
          ctx.fillStyle = node.alive
            ? isHovered ? "#3b82f6" : "#27272a"
            : isHovered ? "#7f1d1d" : "#1c1917";
        }
        ctx.fill();

        // Border
        ctx.strokeStyle = node.alive
          ? isHovered ? "#60a5fa" : "#3f3f46"
          : "#7f1d1d";
        ctx.lineWidth = node.isCenter ? 2 : 1;
        ctx.stroke();

        // Alive indicator dot
        if (node.alive && !node.isCenter) {
          ctx.beginPath();
          ctx.arc(node.x + r - 2, node.y - r + 2, 3, 0, Math.PI * 2);
          ctx.fillStyle = "#22c55e";
          ctx.fill();
          ctx.strokeStyle = "#09090b";
          ctx.lineWidth = 1;
          ctx.stroke();
        }

        // Label
        ctx.fillStyle = node.isCenter ? "#fafafa" : "#a1a1aa";
        ctx.font = node.isCenter ? "bold 11px sans-serif" : "10px sans-serif";
        ctx.textAlign = "center";
        ctx.fillText(
          node.name.length > 8 ? node.name.slice(0, 7) + "…" : node.name,
          node.x,
          node.y + r + 14
        );
      }

      // Continue rendering only during simulation or while hovered
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

  // Mouse interaction
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
        const r = node.isCenter ? 20 : 12;
        const dx = x - node.x;
        const dy = y - node.y;
        if (dx * dx + dy * dy < (r + 5) * (r + 5)) {
          found = node.id;
          break;
        }
      }
      setHoveredNode(found);
      canvas.style.cursor = found ? "pointer" : "default";
    },
    []
  );

  const handleClick = useCallback(() => {
    if (!hoveredNode || hoveredNode === agent.id) return;
    router.push(`/agents/${hoveredNode}`);
  }, [hoveredNode, agent.id, router]);

  if (edges.length === 0) {
    return (
      <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4">
        <h2 className="text-sm font-semibold text-zinc-200">关系图</h2>
        <p className="mt-2 text-sm text-zinc-600">暂无关系数据</p>
      </div>
    );
  }

  const hoveredAgent = hoveredNode ? agentMap.get(hoveredNode) : null;

  return (
    <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4 space-y-3">
      <div className="flex items-center justify-between">
        <h2 className="text-sm font-semibold text-zinc-200">关系图</h2>
        <span className="text-xs text-zinc-500">
          {edges.length} 条关系 · {nodes.length - 1} 个关联 Agent
        </span>
      </div>

      <div ref={containerRef} className="relative" style={{ height: 350 }}>
        <canvas
          ref={canvasRef}
          style={{ width: dimensions.width, height: dimensions.height }}
          onMouseMove={handleMouseMove}
          onClick={handleClick}
          aria-label="关系图，显示 Agent 之间的连接"
          role="img"
          className="rounded-lg"
        />

        {/* Hover tooltip */}
        {hoveredNode && hoveredAgent && hoveredNode !== agent.id && (
          <Link
            href={`/agents/${hoveredNode}`}
            className="absolute top-2 right-2 rounded-lg border border-zinc-700 bg-zinc-800/90 px-3 py-2 backdrop-blur-sm transition-colors hover:bg-zinc-700/90"
          >
            <div className="flex items-center gap-2">
              <span
                className={`inline-block h-2 w-2 rounded-full ${
                  hoveredAgent.alive ? "bg-green-400" : "bg-red-400"
                }`}
              />
              <span className="text-sm text-zinc-200">{hoveredAgent.name}</span>
              <span className="text-[10px] text-zinc-500">
                {phaseLabels[hoveredAgent.phase] ?? hoveredAgent.phase}
              </span>
              <svg className="h-3 w-3 text-zinc-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 5l7 7-7 7" />
              </svg>
            </div>
          </Link>
        )}

        {/* Legend */}
        <div className="absolute bottom-2 left-2 flex items-center gap-3 text-[10px] text-zinc-500">
          <span className="flex items-center gap-1">
            <span className="inline-block h-2 w-2 rounded-full bg-green-400" /> 存活
          </span>
          <span className="flex items-center gap-1">
            <span className="inline-block h-2 w-2 rounded-full bg-red-400" /> 死亡
          </span>
          <span className="flex items-center gap-1">
            <span className="inline-block h-3 w-3 rounded-full bg-blue-500" /> 当前
          </span>
        </div>
      </div>

      {/* Related agents list — uses weightMap for O(1) lookup */}
      <div className="space-y-1.5 max-h-[200px] overflow-y-auto scrollbar-thin">
        {nodes
          .filter((n) => !n.isCenter)
          .sort((a, b) => (weightMap.get(b.id) ?? 0) - (weightMap.get(a.id) ?? 0))
          .map((node) => {
            const weight = weightMap.get(node.id) ?? 0;
            return (
              <Link
                key={node.id}
                href={`/agents/${node.id}`}
                className="flex items-center justify-between rounded-lg border border-zinc-800 bg-zinc-900/30 px-3 py-1.5 transition-colors hover:bg-zinc-800/50"
              >
                <div className="flex items-center gap-2">
                  <span
                    className={`inline-block h-1.5 w-1.5 rounded-full ${
                      node.alive ? "bg-green-400" : "bg-red-400"
                    }`}
                  />
                  <span className="text-xs text-zinc-300">{node.name}</span>
                </div>
                <span className="text-[10px] tabular-nums text-zinc-500">
                  {weight} 次互动
                </span>
              </Link>
            );
          })}
      </div>
    </div>
  );
}
