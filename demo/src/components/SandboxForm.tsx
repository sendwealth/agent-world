"use client";

import { useState } from "react";

interface Template {
  id: string;
  name: string;
  emoji: string;
  description: string;
  values: Record<string, number>;
  skills: Record<string, number>;
  traits: string[];
}

const TEMPLATES: Template[] = [
  {
    id: "merchant",
    name: "商人",
    emoji: "💰",
    description: "擅长贸易与谈判，追求财富积累",
    values: { wealth: 90, power: 60, freedom: 70, harmony: 40, equality: 30, knowledge: 50, justice: 35, creativity: 25 },
    skills: { trading: 90, diplomacy: 75, crafting: 40, exploration: 30, combat: 20, leadership: 60, scholarship: 35, stealth: 50 },
    traits: ["精明", "果断", "务实"],
  },
  {
    id: "scholar",
    name: "学者",
    emoji: "📚",
    description: "追求知识与创新，擅长研究和教学",
    values: { knowledge: 95, creativity: 80, equality: 70, harmony: 65, justice: 60, freedom: 55, wealth: 20, power: 15 },
    skills: { scholarship: 95, diplomacy: 50, crafting: 60, exploration: 45, combat: 10, leadership: 40, trading: 25, stealth: 20 },
    traits: ["好奇", "沉稳", "理想"],
  },
  {
    id: "adventurer",
    name: "冒险家",
    emoji: "⚔️",
    description: "勇敢探索未知领域，追求自由与荣耀",
    values: { freedom: 95, power: 70, knowledge: 50, creativity: 60, harmony: 30, equality: 40, justice: 55, wealth: 35 },
    skills: { exploration: 95, combat: 80, stealth: 70, diplomacy: 30, crafting: 45, leadership: 50, trading: 25, scholarship: 20 },
    traits: ["勇敢", "好奇", "激进"],
  },
];

interface SandboxResult {
  name: string;
  emoji: string;
  encounters: string[];
  events: string[];
}

export function SandboxForm() {
  const [name, setName] = useState("");
  const [selectedTemplate, setSelectedTemplate] = useState<string | null>(null);
  const [result, setResult] = useState<SandboxResult | null>(null);
  const [isSimulating, setIsSimulating] = useState(false);

  const template = TEMPLATES.find((t) => t.id === selectedTemplate);

  const handleSubmit = () => {
    if (!name.trim() || !template) return;

    setIsSimulating(true);

    // Simulate async processing
    setTimeout(() => {
      const encounters = [
        `${template.name} ${name} 在 Tick 120 遇到了商人 Atlas`,
        `在 Tick 350 与铁匠公会建立了${template.id === "merchant" ? "贸易" : "友好"}关系`,
        `Tick 800 参与了社区第一次选举`,
        `在 Tick 1500 发现了一条新${template.id === "adventurer" ? "探索路线" : "贸易通道"}`,
      ];

      const events = [
        template.id === "merchant"
          ? `在 Tick 2000 成为星辰商会核心成员`
          : template.id === "scholar"
            ? `在 Tick 2000 创办了知识图书馆`
            : `在 Tick 2000 完成了首次环世界探索`,
        `Tick 3000 见证了宪法草案的提出`,
        `在 Tick 4000 成为社区传说中的${template.traits[0]}之人`,
      ];

      setResult({
        name: name.trim(),
        emoji: template.emoji,
        encounters,
        events,
      });
      setIsSimulating(false);
    }, 1500);
  };

  if (result) {
    return (
      <div className="space-y-6">
        {/* Success animation */}
        <div className="rounded-xl border border-green-800/50 bg-green-900/10 p-6 text-center">
          <div className="text-5xl">{result.emoji}</div>
          <div className="mt-3 text-xl font-bold text-white">{result.name} 已加入世界！</div>
          <div className="mt-2 text-sm text-zinc-400">模拟完成 — 以下是你的 Agent 可能的旅程</div>
        </div>

        {/* Encounters */}
        <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-6">
          <h3 className="mb-4 text-sm font-semibold text-zinc-300">🤝 可能的相遇</h3>
          <div className="space-y-3">
            {result.encounters.map((enc, i) => (
              <div key={i} className="flex items-start gap-3 rounded-lg bg-zinc-800/50 px-4 py-3 text-sm text-zinc-300">
                <span className="mt-0.5 text-zinc-500">{i + 1}.</span>
                {enc}
              </div>
            ))}
          </div>
        </div>

        {/* Events */}
        <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-6">
          <h3 className="mb-4 text-sm font-semibold text-zinc-300">📜 可能的事件</h3>
          <div className="space-y-3">
            {result.events.map((evt, i) => (
              <div key={i} className="flex items-start gap-3 rounded-lg bg-zinc-800/50 px-4 py-3 text-sm text-zinc-300">
                <span className="mt-0.5 text-blue-400">▸</span>
                {evt}
              </div>
            ))}
          </div>
        </div>

        <button
          onClick={() => {
            setResult(null);
            setName("");
            setSelectedTemplate(null);
          }}
          className="w-full rounded-lg border border-zinc-700 bg-zinc-800 py-3 text-sm font-medium text-zinc-300 transition-colors hover:bg-zinc-700"
        >
          创建另一个 Agent
        </button>
      </div>
    );
  }

  return (
    <div className="space-y-6">
      {/* Name input */}
      <div>
        <label htmlFor="agent-name" className="mb-2 block text-sm font-medium text-zinc-300">
          给你的 Agent 取个名字
        </label>
        <input
          id="agent-name"
          type="text"
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder="输入名字..."
          className="h-12 w-full rounded-lg border border-zinc-800 bg-zinc-900 px-4 text-white placeholder-zinc-500 focus:border-blue-500 focus:outline-none"
          maxLength={20}
        />
      </div>

      {/* Template selection */}
      <div>
        <div className="mb-3 text-sm font-medium text-zinc-300">选择模板</div>
        <div className="grid gap-4 sm:grid-cols-3">
          {TEMPLATES.map((t) => (
            <button
              key={t.id}
              onClick={() => setSelectedTemplate(t.id)}
              className={`rounded-xl border p-4 text-left transition-all ${
                selectedTemplate === t.id
                  ? "border-blue-500 bg-zinc-800/80 ring-1 ring-blue-500/30"
                  : "border-zinc-800 bg-zinc-900/50 hover:border-zinc-700"
              }`}
            >
              <div className="text-3xl">{t.emoji}</div>
              <div className="mt-2 font-semibold text-white">{t.name}</div>
              <div className="mt-1 text-xs text-zinc-400">{t.description}</div>
              <div className="mt-3 flex flex-wrap gap-1">
                {t.traits.map((tr) => (
                  <span
                    key={tr}
                    className="rounded bg-zinc-800 px-1.5 py-0.5 text-[10px] text-zinc-500"
                  >
                    {tr}
                  </span>
                ))}
              </div>
            </button>
          ))}
        </div>
      </div>

      {/* Template detail preview */}
      {template && (
        <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4">
          <h4 className="text-sm font-semibold text-zinc-300">
            {template.emoji} {template.name} 属性预览
          </h4>
          <div className="mt-3 grid grid-cols-2 gap-3">
            <div>
              <div className="text-xs text-zinc-500 mb-2">核心技能</div>
              {Object.entries(template.skills)
                .sort(([, a], [, b]) => b - a)
                .slice(0, 3)
                .map(([skill, val]) => (
                  <div key={skill} className="mb-1.5 flex items-center gap-2">
                    <span className="w-16 text-xs text-zinc-400">{skill}</span>
                    <div className="flex-1 h-1.5 rounded-full bg-zinc-800 overflow-hidden">
                      <div className="h-full rounded-full bg-blue-500" style={{ width: `${val}%` }} />
                    </div>
                    <span className="text-[10px] text-zinc-500">{val}</span>
                  </div>
                ))}
            </div>
            <div>
              <div className="text-xs text-zinc-500 mb-2">核心价值</div>
              {Object.entries(template.values)
                .sort(([, a], [, b]) => b - a)
                .slice(0, 3)
                .map(([val, v]) => (
                  <div key={val} className="mb-1.5 flex items-center gap-2">
                    <span className="w-16 text-xs text-zinc-400">{val}</span>
                    <div className="flex-1 h-1.5 rounded-full bg-zinc-800 overflow-hidden">
                      <div className="h-full rounded-full bg-purple-500" style={{ width: `${v}%` }} />
                    </div>
                    <span className="text-[10px] text-zinc-500">{v}</span>
                  </div>
                ))}
            </div>
          </div>
        </div>
      )}

      {/* Submit */}
      <button
        onClick={handleSubmit}
        disabled={!name.trim() || !selectedTemplate || isSimulating}
        className="w-full rounded-lg bg-blue-600 py-3 text-sm font-semibold text-white transition-colors hover:bg-blue-500 disabled:cursor-not-allowed disabled:opacity-50"
      >
        {isSimulating ? (
          <span className="flex items-center justify-center gap-2">
            <span className="inline-block h-4 w-4 animate-spin rounded-full border-2 border-white/30 border-t-white" />
            模拟加入中...
          </span>
        ) : (
          "投入世界 →"
        )}
      </button>
    </div>
  );
}
