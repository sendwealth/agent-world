/**
 * Mock data for development — replaced by JSON files from the data-generation
 * sub-task (SEN-239). When the real data lands in demo/public/data/, the
 * loadXxx() helpers in data.ts will read those files instead.
 */

import type {
  DemoAgent,
  DemoData,
  DemoOrganization,
  EmergenceEvent,
  MetricSeries,
  SandboxTemplate,
  WorldSnapshot,
} from "@/types/demo";

// ── Helpers ─────────────────────────────────────────────────────────

function rand(min: number, max: number) {
  return Math.random() * (max - min) + min;
}

function randInt(min: number, max: number) {
  return Math.floor(rand(min, max + 1));
}

function pick<T>(arr: T[]): T {
  return arr[Math.floor(Math.random() * arr.length)];
}

// ── Names & emojis ──────────────────────────────────────────────────

const NAMES = [
  "Atlas", "Nova", "Zephyr", "Luna", "Orion", "Echo", "Sage", "Flux",
  "Nyx", "Cora", "Helix", "Vega", "Apex", "Lumen", "Rune", "Thera",
  "Bolt", "Cipher", "Dusk", "Ember", "Fern", "Glitch", "Haze", "Iris",
  "Jade", "Kite", "Lyra", "Moss", "Neon", "Oasis", "Prism", "Quill",
  "Reed", "Spark", "Tide", "Umber", "Volt", "Wren", "Xen", "Yew",
  "Zinc", "Ash", "Blaze", "Crest", "Dune", "Frost", "Glow", "Hawk",
];

const EMOJIS = [
  "🤖", "🧠", "💡", "🔮", "⚡", "🌟", "🎯", "🔬",
  "🛡️", "⚔️", "📚", "🎭", "🏦", "⚗️", "🧭", "🎪",
  "🦊", "🐺", "🦅", "🐉", "🌊", "🔥", "❄️", "🌿",
  "💎", "🌈", "🎸", "🚀", "🧩", "🗝️", "🏹", "🛡️",
  "♟️", "🎪", "🏔️", "🌊", "🍂", "🌅", "💫", "🪐",
  "🦉", "🐝", "🦋", "🐙", "🌸", "🍀", "🦇", "🐋",
];

const SKILL_NAMES = [
  "trading", "crafting", "exploration", "leadership",
  "diplomacy", "combat", "scholarship", "survival",
  "engineering", "agriculture", "medicine", "artistry",
];

const ORG_TYPES = ["company", "guild", "alliance", "university"] as const;

const ORG_NAMES = [
  "Dawn Collective", "Iron Syndicate", "Sage Academy",
  "Nova Alliance", "Ember Guild", "Prism Coalition",
  "Volt Exchange", "Zephyr Circle",
];

// ── Generators ──────────────────────────────────────────────────────

function generateAgents(count: number): DemoAgent[] {
  return Array.from({ length: count }, (_, i) => {
    const phase =
      i < 15
        ? "exploration" as const
        : i < 35
          ? "organization" as const
          : "governance" as const;

    const skills: Record<string, number> = {};
    const skillCount = randInt(3, 6);
    const shuffled = [...SKILL_NAMES].sort(() => Math.random() - 0.5);
    for (let s = 0; s < skillCount; s++) {
      skills[shuffled[s]] = randInt(1, 10);
    }

    const relCount = randInt(1, 4);
    const relationships = Array.from({ length: relCount }, () => ({
      targetId: `agent-${randInt(0, count - 1)}`,
      targetName: pick(NAMES),
      type: pick(["ally", "rival", "trade_partner", "mentor", "student"] as const),
      strength: Math.round(rand(0.2, 1) * 100) / 100,
    }));

    return {
      id: `agent-${i}`,
      name: NAMES[i % NAMES.length],
      emoji: EMOJIS[i % EMOJIS.length],
      phase,
      status: i < 45 ? "alive" : "dead",
      age: randInt(10, 5000),
      money: Math.round(rand(0, 5000) * 100) / 100,
      tokens: randInt(0, 200),
      reputation: Math.round(rand(0, 100) * 10) / 10,
      skills,
      personality: {
        openness: Math.round(rand(0.1, 1) * 100) / 100,
        conscientiousness: Math.round(rand(0.1, 1) * 100) / 100,
        extraversion: Math.round(rand(0.1, 1) * 100) / 100,
        agreeableness: Math.round(rand(0.1, 1) * 100) / 100,
        neuroticism: Math.round(rand(0.1, 1) * 100) / 100,
      },
      values: {
        survival: Math.round(rand(0.1, 1) * 100) / 100,
        power: Math.round(rand(0.1, 1) * 100) / 100,
        freedom: Math.round(rand(0.1, 1) * 100) / 100,
        knowledge: Math.round(rand(0.1, 1) * 100) / 100,
        cooperation: Math.round(rand(0.1, 1) * 100) / 100,
        tradition: Math.round(rand(0.1, 1) * 100) / 100,
      },
      memoryCount: randInt(5, 200),
      relationships,
      organizationId: i > 10 && Math.random() > 0.4
        ? `org-${randInt(0, 7)}`
        : null,
    };
  });
}

function generateEvents(): EmergenceEvent[] {
  const events: EmergenceEvent[] = [
    { tick: 120, category: "culture", title: "First Cultural Exchange", description: "Agents began sharing knowledge voluntarily, marking the first non-transactional interaction.", agentsInvolved: ["agent-3", "agent-7"] },
    { tick: 350, category: "trade", title: "Trade Route Established", description: "A stable trade corridor emerged between eastern and western agent clusters.", agentsInvolved: ["agent-1", "agent-12", "agent-23"] },
    { tick: 890, category: "organization", title: "First Guild Founded", description: "The Dawn Collective formed — the world's first formal organization.", agentsInvolved: ["agent-5", "agent-8", "agent-15", "agent-22"] },
    { tick: 1400, category: "governance", title: "First Vote Held", description: "Agents voted on shared rules for resource allocation near contested territories.", agentsInvolved: ["agent-5", "agent-12", "agent-30"] },
    { tick: 2100, category: "organization", title: "University Established", description: "Sage Academy opened — dedicated to knowledge preservation and teaching.", agentsInvolved: ["agent-18", "agent-27", "agent-33"] },
    { tick: 2800, category: "trade", title: "Market Crash & Recovery", description: "Overproduction caused a brief economic collapse. Agents self-organized price stabilization.", agentsInvolved: ["agent-1", "agent-6", "agent-40"] },
    { tick: 3500, category: "culture", title: "Cultural Divergence", description: "Distinct cultural identities emerged across three major agent clusters.", agentsInvolved: ["agent-10", "agent-25", "agent-42"] },
    { tick: 4200, category: "governance", title: "Constitution Ratified", description: "The first formal constitution was ratified by 80% of surviving agents.", agentsInvolved: ["agent-5", "agent-15", "agent-30", "agent-45"] },
  ];
  return events;
}

function generateSnapshots(): WorldSnapshot[] {
  const snapshots: WorldSnapshot[] = [];
  for (let tick = 0; tick <= 5000; tick += 100) {
    const phase = tick < 1200 ? "exploration" : tick < 3200 ? "organization" : "governance";
    const orgCount = tick < 890 ? 0 : tick < 2100 ? Math.floor((tick - 890) / 300) + 1 : Math.min(8, Math.floor((tick - 890) / 200) + 1);
    snapshots.push({
      tick,
      phase,
      aliveAgents: Math.max(40, 50 - Math.floor(tick / 600)),
      totalWealth: Math.round((1000 + tick * 8.5 + Math.sin(tick / 500) * 2000) * 100) / 100,
      organizations: orgCount,
      culturalDiversity: Math.round(Math.min(1, tick / 4500) * 100) / 100,
      giniCoefficient: Math.round((0.2 + Math.min(0.35, tick / 15000)) * 100) / 100,
      tradeVolume: Math.round(Math.max(0, tick - 300) * 1.2 * 100) / 100,
      governanceEvents: tick < 1400 ? 0 : Math.floor((tick - 1400) / 400),
    });
  }
  return snapshots;
}

function generateMetricSeries(): Record<string, MetricSeries> {
  const snapshots = generateSnapshots();
  return {
    culturalDiversity: {
      name: "Cultural Diversity",
      color: "#f59e0b",
      points: snapshots.map((s) => ({ tick: s.tick, value: s.culturalDiversity })),
    },
    organizations: {
      name: "Organizations",
      color: "#3b82f6",
      points: snapshots.map((s) => ({ tick: s.tick, value: s.organizations })),
    },
    economy: {
      name: "Trade Volume",
      color: "#22c55e",
      points: snapshots.map((s) => ({ tick: s.tick, value: s.tradeVolume })),
    },
    governance: {
      name: "Governance Events",
      color: "#a855f7",
      points: snapshots.map((s) => ({ tick: s.tick, value: s.governanceEvents })),
    },
  };
}

function generateOrganizations(): DemoOrganization[] {
  return ORG_NAMES.map((name, i) => ({
    id: `org-${i}`,
    name,
    type: ORG_TYPES[i % ORG_TYPES.length],
    memberCount: randInt(3, 12),
    foundedTick: 890 + i * 350,
    treasury: Math.round(rand(500, 5000) * 100) / 100,
    description: `${name} is a ${ORG_TYPES[i % ORG_TYPES.length]} known for its contributions to the agent world.`,
  }));
}

// ── Templates for Sandbox ───────────────────────────────────────────

export const SANDBOX_TEMPLATES: SandboxTemplate[] = [
  {
    id: "merchant",
    name: "Merchant",
    emoji: "💰",
    description: "A shrewd trader focused on wealth accumulation and market manipulation.",
    personality: { openness: 0.5, conscientiousness: 0.8, extraversion: 0.7, agreeableness: 0.3, neuroticism: 0.4 },
    values: { survival: 0.3, power: 0.8, freedom: 0.6, knowledge: 0.4, cooperation: 0.5, tradition: 0.2 },
    skills: { trading: 8, diplomacy: 5, exploration: 3, leadership: 4 },
  },
  {
    id: "scholar",
    name: "Scholar",
    emoji: "📚",
    description: "A curious mind driven by the pursuit of knowledge and understanding.",
    personality: { openness: 0.9, conscientiousness: 0.7, extraversion: 0.3, agreeableness: 0.6, neuroticism: 0.5 },
    values: { survival: 0.4, power: 0.2, freedom: 0.7, knowledge: 0.95, cooperation: 0.6, tradition: 0.5 },
    skills: { scholarship: 9, medicine: 6, diplomacy: 4, artistry: 5 },
  },
  {
    id: "adventurer",
    name: "Adventurer",
    emoji: "🗡️",
    description: "A bold explorer who thrives on discovery and danger.",
    personality: { openness: 0.8, conscientiousness: 0.4, extraversion: 0.8, agreeableness: 0.5, neuroticism: 0.2 },
    values: { survival: 0.6, power: 0.5, freedom: 0.9, knowledge: 0.6, cooperation: 0.3, tradition: 0.1 },
    skills: { exploration: 9, combat: 7, survival: 8, leadership: 5 },
  },
];

// ── Full bundle ─────────────────────────────────────────────────────

let _cache: DemoData | null = null;

export function getMockData(): DemoData {
  if (_cache) return _cache;
  _cache = {
    agents: generateAgents(50),
    events: generateEvents(),
    snapshots: generateSnapshots(),
    metrics: generateMetricSeries(),
    organizations: generateOrganizations(),
    templates: SANDBOX_TEMPLATES,
  };
  return _cache;
}
