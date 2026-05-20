"use client";

import type { DemoAgent } from "@/types/demo";

interface AgentDetailProps {
  agent: DemoAgent;
  onClose: () => void;
}

const PHASE_LABELS: Record<string, string> = {
  survive: "生存",
  explore: "探索",
  build: "建设",
  trade: "贸易",
  govern: "治理",
  create: "创造",
  transcend: "超越",
};

const VALUE_LABELS: Record<string, string> = {
  freedom: "自由",
  equality: "平等",
  power: "权力",
  knowledge: "知识",
  wealth: "财富",
  harmony: "和谐",
  justice: "公正",
  creativity: "创造力",
};

function RadarChart({ values }: { values: Record<string, number> }) {
  const entries = Object.entries(values);
  const count = entries.length;
  const cx = 100;
  const cy = 100;
  const r = 80;

  const getPoint = (index: number, value: number): [number, number] => {
    const angle = (Math.PI * 2 * index) / count - Math.PI / 2;
    const normalized = value / 100;
    return [
      cx + r * normalized * Math.cos(angle),
      cy + r * normalized * Math.sin(angle),
    ];
  };

  const webLines = Array.from({ length: count }, (_, i) => {
    const angle = (Math.PI * 2 * i) / count - Math.PI / 2;
    return (
      <line
        key={`web-${i}`}
        x1={cx}
        y1={cy}
        x2={cx + r * Math.cos(angle)}
        y2={cy + r * Math.sin(angle)}
        stroke="#27272a"
        strokeWidth={1}
      />
    );
  });

  const dataPoints = entries.map(([key, val], i) => {
    const [x, y] = getPoint(i, val);
    const labelAngle = (Math.PI * 2 * i) / count - Math.PI / 2;
    const labelR = r + 20;
    const lx = cx + labelR * Math.cos(labelAngle);
    const ly = cy + labelR * Math.sin(labelAngle);
    return { key, val, x, y, lx, ly };
  });

  const polygonPoints = dataPoints.map((p) => `${p.x},${p.y}`).join(" ");

  return (
    <svg viewBox="0 0 200 200" className="mx-auto h-48 w-48">
      {/* Concentric rings */}
      {[0.25, 0.5, 0.75, 1].map((scale) => (
        <circle
          key={scale}
          cx={cx}
          cy={cy}
          r={r * scale}
          fill="none"
          stroke="#27272a"
          strokeWidth={0.5}
        />
      ))}
      {webLines}
      {/* Data polygon */}
      <polygon
        points={polygonPoints}
        fill="rgba(59,130,246,0.15)"
        stroke="#3b82f6"
        strokeWidth={1.5}
      />
      {/* Data points and labels */}
      {dataPoints.map((p) => (
        <g key={p.key}>
          <circle cx={p.x} cy={p.y} r={3} fill="#3b82f6" />
          <text
            x={p.lx}
            y={p.ly}
            textAnchor="middle"
            dominantBaseline="middle"
            className="fill-zinc-400 text-[8px]"
          >
            {VALUE_LABELS[p.key] ?? p.key}
          </text>
        </g>
      ))}
    </svg>
  );
}

export function AgentDetail({ agent, onClose }: AgentDetailProps) {
  const topSkills = Object.entries(agent.skills)
    .sort(([, a], [, b]) => b - a)
    .slice(0, 5);

  return (
    <div className="rounded-xl border border-zinc-800 bg-zinc-900/80 p-6">
      <div className="flex items-start justify-between">
        <div className="flex items-center gap-4">
          <div className="flex h-16 w-16 items-center justify-center rounded-xl bg-zinc-800 text-3xl">
            {agent.emoji}
          </div>
          <div>
            <h2 className="text-xl font-bold text-white">{agent.name}</h2>
            <div className="mt-1 flex items-center gap-2 text-sm text-zinc-400">
              <span>{PHASE_LABELS[agent.phase] ?? agent.phase}</span>
              <span className="text-zinc-600">·</span>
              <span>{agent.age} Tick</span>
              {!agent.alive && (
                <>
                  <span className="text-zinc-600">·</span>
                  <span className="text-red-400">已消亡</span>
                </>
              )}
            </div>
          </div>
        </div>
        <button
          onClick={onClose}
          className="flex h-8 w-8 items-center justify-center rounded-md text-zinc-400 hover:bg-zinc-800 hover:text-white"
          aria-label="Close detail"
        >
          ✕
        </button>
      </div>

      {/* Organization */}
      {agent.organization && (
        <div className="mt-4 rounded-lg bg-blue-900/20 px-3 py-2 text-sm text-blue-300">
          所属组织: {agent.organization}
        </div>
      )}

      {/* Traits */}
      <div className="mt-4 flex flex-wrap gap-2">
        {agent.traits.map((t) => (
          <span
            key={t}
            className="rounded-full bg-zinc-800 px-3 py-1 text-xs text-zinc-300"
          >
            {t}
          </span>
        ))}
      </div>

      {/* Values radar */}
      <div className="mt-6">
        <h3 className="mb-3 text-sm font-semibold text-zinc-300">价值观</h3>
        <RadarChart values={agent.values} />
      </div>

      {/* Skills */}
      <div className="mt-6">
        <h3 className="mb-3 text-sm font-semibold text-zinc-300">技能</h3>
        <div className="space-y-2">
          {topSkills.map(([skill, level]) => (
            <div key={skill} className="flex items-center gap-3">
              <span className="w-20 text-xs text-zinc-400">{skill}</span>
              <div className="flex-1">
                <div className="h-1.5 overflow-hidden rounded-full bg-zinc-800">
                  <div
                    className="h-full rounded-full bg-blue-500 transition-all"
                    style={{ width: `${level}%` }}
                  />
                </div>
              </div>
              <span className="w-8 text-right text-xs text-zinc-500">{level}</span>
            </div>
          ))}
        </div>
      </div>

      {/* Relations */}
      {agent.relations.length > 0 && (
        <div className="mt-6">
          <h3 className="mb-3 text-sm font-semibold text-zinc-300">关系网络</h3>
          <div className="space-y-2">
            {agent.relations.map((rel) => (
              <div
                key={rel.agentId}
                className="flex items-center justify-between rounded-lg bg-zinc-800/50 px-3 py-2 text-sm"
              >
                <span className="text-zinc-200">{rel.agentName}</span>
                <span
                  className={`text-xs ${
                    rel.type === "ally"
                      ? "text-green-400"
                      : rel.type === "rival"
                        ? "text-red-400"
                        : "text-zinc-500"
                  }`}
                >
                  {rel.type === "ally" ? "盟友" : rel.type === "rival" ? "对手" : "中立"}
                </span>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* Memories */}
      {agent.memories.length > 0 && (
        <div className="mt-6">
          <h3 className="mb-3 text-sm font-semibold text-zinc-300">记忆</h3>
          <div className="space-y-2">
            {agent.memories.map((mem, i) => (
              <div key={i} className="rounded-lg bg-zinc-800/50 px-3 py-2 text-sm text-zinc-400">
                {mem}
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
