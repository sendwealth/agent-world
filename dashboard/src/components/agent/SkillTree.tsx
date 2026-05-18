"use client";

import { useMemo } from "react";

// Skill category mapping
const skillCategories: Record<string, { category: string; icon: string; order: number }> = {
  // Combat / survival
  combat: { category: "战斗", icon: "⚔️", order: 0 },
  defense: { category: "战斗", icon: "⚔️", order: 0 },
  attack: { category: "战斗", icon: "⚔️", order: 0 },
  // Economy
  trading: { category: "经济", icon: "💰", order: 1 },
  negotiation: { category: "经济", icon: "💰", order: 1 },
  bartering: { category: "经济", icon: "💰", order: 1 },
  commerce: { category: "经济", icon: "💰", order: 1 },
  investment: { category: "经济", icon: "💰", order: 1 },
  // Crafting / production
  crafting: { category: "制造", icon: "🔨", order: 2 },
  building: { category: "制造", icon: "🔨", order: 2 },
  engineering: { category: "制造", icon: "🔨", order: 2 },
  mining: { category: "制造", icon: "🔨", order: 2 },
  farming: { category: "制造", icon: "🔨", order: 2 },
  // Social
  communication: { category: "社交", icon: "💬", order: 3 },
  leadership: { category: "社交", icon: "💬", order: 3 },
  persuasion: { category: "社交", icon: "💬", order: 3 },
  charisma: { category: "社交", icon: "💬", order: 3 },
  teaching: { category: "社交", icon: "💬", order: 3 },
  // Knowledge
  research: { category: "知识", icon: "📚", order: 4 },
  science: { category: "知识", icon: "📚", order: 4 },
  medicine: { category: "知识", icon: "📚", order: 4 },
  alchemy: { category: "知识", icon: "📚", order: 4 },
  magic: { category: "知识", icon: "📚", order: 4 },
  // Exploration
  exploration: { category: "探索", icon: "🧭", order: 5 },
  navigation: { category: "探索", icon: "🧭", order: 5 },
  scouting: { category: "探索", icon: "🧭", order: 5 },
  survival: { category: "探索", icon: "🧭", order: 5 },
};

function getCategory(skillName: string) {
  const lower = skillName.toLowerCase();
  for (const [key, val] of Object.entries(skillCategories)) {
    if (lower.includes(key)) return val;
  }
  return { category: "其他", icon: "✨", order: 99 };
}

function levelColor(level: number): string {
  if (level >= 9) return "from-amber-400 to-yellow-300";
  if (level >= 7) return "from-purple-400 to-violet-400";
  if (level >= 5) return "from-blue-400 to-cyan-400";
  if (level >= 3) return "from-green-400 to-emerald-400";
  return "from-zinc-500 to-zinc-400";
}

function levelGlow(level: number): string {
  if (level >= 9) return "shadow-amber-400/30";
  if (level >= 7) return "shadow-purple-400/30";
  if (level >= 5) return "shadow-blue-400/20";
  return "";
}

function levelBadge(level: number): string {
  if (level >= 9) return "text-amber-300 bg-amber-500/20 border-amber-500/30";
  if (level >= 7) return "text-purple-300 bg-purple-500/20 border-purple-500/30";
  if (level >= 5) return "text-blue-300 bg-blue-500/20 border-blue-500/30";
  if (level >= 3) return "text-green-300 bg-green-500/20 border-green-500/30";
  return "text-zinc-400 bg-zinc-700/50 border-zinc-600/30";
}

interface SkillTreeProps {
  skills: Record<string, number>;
}

export default function SkillTree({ skills }: SkillTreeProps) {
  const grouped = useMemo(() => {
    const entries = Object.entries(skills).sort(([, a], [, b]) => b - a);
    const groups: Record<string, { icon: string; order: number; skills: [string, number][] }> = {};

    for (const [name, level] of entries) {
      const cat = getCategory(name);
      if (!groups[cat.category]) {
        groups[cat.category] = { icon: cat.icon, order: cat.order, skills: [] };
      }
      groups[cat.category].skills.push([name, level]);
    }

    return Object.entries(groups).sort(([, a], [, b]) => a.order - b.order);
  }, [skills]);

  const totalSkills = Object.keys(skills).length;
  const avgLevel =
    totalSkills > 0
      ? Object.values(skills).reduce((a, b) => a + b, 0) / totalSkills
      : 0;
  const masteredCount = Object.values(skills).filter((l) => l >= 8).length;

  if (totalSkills === 0) {
    return (
      <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4">
        <h2 className="text-sm font-semibold text-zinc-200">技能树</h2>
        <p className="mt-2 text-sm text-zinc-600">暂无技能数据</p>
      </div>
    );
  }

  return (
    <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4 space-y-4">
      <div className="flex items-center justify-between">
        <h2 className="text-sm font-semibold text-zinc-200">技能树</h2>
        <div className="flex items-center gap-3 text-xs text-zinc-500">
          <span>{totalSkills} 技能</span>
          <span>·</span>
          <span>平均 Lv.{avgLevel.toFixed(1)}</span>
          <span>·</span>
          <span className="text-amber-400">{masteredCount} 精通</span>
        </div>
      </div>

      {/* Skill overview radar-like grid */}
      <div className="flex items-center justify-center gap-1 py-2">
        {Object.entries(skills)
          .sort(([, a], [, b]) => b - a)
          .slice(0, 20)
          .map(([name, level]) => (
            <div
              key={name}
              className={`group relative flex items-end justify-center ${levelGlow(level)}`}
              style={{ height: "48px" }}
            >
              <div
                className={`w-3 rounded-t bg-gradient-to-t ${levelColor(level)} transition-all duration-500`}
                style={{ height: `${Math.max((level / 10) * 40, 4)}px` }}
                title={`${name}: Lv.${level}`}
              />
              {/* Tooltip */}
              <div className="pointer-events-none absolute bottom-full mb-1 hidden whitespace-nowrap rounded bg-zinc-800 px-2 py-1 text-[10px] text-zinc-300 group-hover:block">
                {name} Lv.{level}
              </div>
            </div>
          ))}
      </div>

      {/* Grouped skill cards */}
      <div className="space-y-3">
        {grouped.map(([category, { icon, skills: catSkills }]) => (
          <div key={category}>
            <div className="mb-1.5 flex items-center gap-1.5">
              <span className="text-xs">{icon}</span>
              <span className="text-[10px] font-medium uppercase tracking-wider text-zinc-500">
                {category}
              </span>
              <span className="text-[10px] text-zinc-700">({catSkills.length})</span>
            </div>
            <div className="flex flex-wrap gap-1.5">
              {catSkills.map(([name, level]) => (
                <div
                  key={name}
                  className={`flex items-center gap-1.5 rounded-md border px-2 py-1 transition-colors ${levelBadge(level)}`}
                >
                  <div
                    className={`h-1.5 w-1.5 rounded-full bg-gradient-to-r ${levelColor(level)}`}
                  />
                  <span className="text-xs text-zinc-300">{name}</span>
                  <span className="text-[10px] font-medium tabular-nums opacity-70">
                    {level}
                  </span>
                </div>
              ))}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
