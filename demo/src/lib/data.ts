/**
 * Data loading utilities.
 * Loads real JSON data from public/data/ and transforms to UI-friendly types.
 */

import type {
  DemoAgent,
  DemoOrganization,
  EmergenceCategory,
  EmergenceEvent,
  MetricSeries,
  RawAgent,
  RawEmergenceMetrics,
  RawOrganization,
  RawTimelineEvent,
  RawWorldSnapshot,
  SandboxTemplate,
  WorldSnapshot,
} from "@/types/demo";

// ── Phase classification ────────────────────────────────────────────

function classifyPhase(tick: number): "exploration" | "organization" | "governance" {
  if (tick < 1200) return "exploration";
  if (tick < 3200) return "organization";
  return "governance";
}

function mapEventType(type: string): EmergenceCategory {
  switch (type) {
    case "economic":
    case "trade":
      return "economic";
    case "organization":
      return "organization";
    case "governance":
      return "governance";
    case "cultural":
    case "culture":
      return "culture";
    default:
      return "milestone";
  }
}

// ── Generic fetcher ─────────────────────────────────────────────────

async function fetchJSON<T>(filename: string): Promise<T> {
  const res = await fetch(`/data/${filename}`);
  if (!res.ok) throw new Error(`Failed to load ${filename}: ${res.status}`);
  return (await res.json()) as T;
}

// ── Transformations ─────────────────────────────────────────────────

function normalizeAgent(raw: RawAgent, agentOrgMap: Map<string, string>): DemoAgent {
  const orgId = agentOrgMap.get(raw.id) ?? null;

  return {
    id: raw.id,
    name: raw.name,
    phase: classifyPhase(raw.death_tick ?? 5000),
    status: raw.alive ? "alive" : "dead",
    age: raw.death_tick ?? 5000,
    money: raw.money,
    tokens: raw.tokens,
    reputation: raw.reputation,
    skills: raw.skills,
    personality: raw.personality_traits,
    values: raw.values,
    memoryCount: raw.memories.length,
    memories: raw.memories,
    relationships: raw.relationships,
    organizationId: orgId,
  };
}

function normalizeEvent(raw: RawTimelineEvent): EmergenceEvent {
  return {
    id: raw.id,
    tick: raw.tick,
    category: mapEventType(raw.type),
    title: raw.title,
    description: raw.description,
    agentsInvolved: raw.involved_agents.map((a) => a.name),
    agentsDetail: raw.involved_agents,
    orgsDetail: raw.involved_orgs,
  };
}

function normalizeSnapshot(raw: RawWorldSnapshot): WorldSnapshot {
  return {
    tick: raw.tick,
    phase: classifyPhase(raw.tick),
    aliveAgents: raw.active_agents,
    totalPopulation: raw.total_population,
    gdp: raw.gdp,
    giniCoefficient: raw.gini_coefficient,
    skillDistribution: raw.skill_distribution_top5,
    keyEvents: raw.key_events,
  };
}

// ── Cache layer ─────────────────────────────────────────────────────

let agentsCache: DemoAgent[] | null = null;
let eventsCache: EmergenceEvent[] | null = null;
let snapshotsCache: WorldSnapshot[] | null = null;
let metricsCache: Record<string, MetricSeries> | null = null;
let orgsCache: DemoOrganization[] | null = null;

// ── Public API ──────────────────────────────────────────────────────

export async function loadAgents(): Promise<DemoAgent[]> {
  if (agentsCache) return agentsCache;

  const [rawAgents, orgs] = await Promise.all([
    fetchJSON<RawAgent[]>("agents.json"),
    loadOrganizations(),
  ]);

  // Build agent→org map from org member lists
  const agentOrgMap = new Map<string, string>();
  for (const org of orgs) {
    for (const member of org.members) {
      agentOrgMap.set(member.agent_id, org.id);
    }
  }

  agentsCache = rawAgents.map((a) => normalizeAgent(a, agentOrgMap));
  return agentsCache;
}

export async function loadEvents(): Promise<EmergenceEvent[]> {
  if (eventsCache) return eventsCache;
  const raw = await fetchJSON<RawTimelineEvent[]>("timeline-events.json");
  eventsCache = raw.map(normalizeEvent);
  return eventsCache;
}

export async function loadSnapshots(): Promise<WorldSnapshot[]> {
  if (snapshotsCache) return snapshotsCache;
  const raw = await fetchJSON<RawWorldSnapshot[]>("world-snapshots.json");
  snapshotsCache = raw.map(normalizeSnapshot);
  return snapshotsCache;
}

export async function loadMetrics(): Promise<Record<string, MetricSeries>> {
  if (metricsCache) return metricsCache;
  const raw = await fetchJSON<RawEmergenceMetrics>("emergence-metrics.json");

  metricsCache = {
    culturalDiversity: {
      name: "Cultural Diversity",
      color: "#f59e0b",
      points: raw.cultural_diversity,
    },
    organizations: {
      name: "Organizations",
      color: "#3b82f6",
      points: raw.organization_count,
    },
    economy: {
      name: "Trade Volume",
      color: "#22c55e",
      points: raw.economic_activity.map((p) => ({
        tick: p.tick,
        value: p.trade_volume,
      })),
    },
    governance: {
      name: "Governance Events",
      color: "#a855f7",
      points: raw.governance_events.map((p) => ({
        tick: p.tick,
        value: p.proposals + p.votes_cast,
      })),
    },
  };
  return metricsCache;
}

export async function loadOrganizations(): Promise<DemoOrganization[]> {
  if (orgsCache) return orgsCache;
  const raw = await fetchJSON<RawOrganization[]>("organizations.json");
  orgsCache = raw.map((o) => ({
    id: o.id,
    name: o.name,
    type: o.type,
    status: o.status,
    memberCount: o.member_count,
    members: o.members,
    foundedTick: o.created_tick,
    treasury: o.treasury,
    debts: o.debts,
    lastActivityTick: o.last_activity_tick,
  }));
  return orgsCache;
}

export function loadTemplates(): SandboxTemplate[] {
  return [
    {
      id: "merchant",
      name: "Merchant",
      emoji: "💰",
      description: "A shrewd trader focused on wealth accumulation and market manipulation.",
      personality: { openness: 0.5, conscientiousness: 0.8, extraversion: 0.7, agreeableness: 0.3, neuroticism: 0.4 },
      values: ["财富", "权力", "自由", "合作", "创新"],
      skills: { trading: 8, diplomacy: 5, exploration: 3, leadership: 4 },
    },
    {
      id: "scholar",
      name: "Scholar",
      emoji: "📚",
      description: "A curious mind driven by the pursuit of knowledge and understanding.",
      personality: { openness: 0.9, conscientiousness: 0.7, extraversion: 0.3, agreeableness: 0.6, neuroticism: 0.5 },
      values: ["智慧", "自由", "合作", "传统", "平等"],
      skills: { researching: 9, medicine: 6, diplomacy: 4, enchanting: 5 },
    },
    {
      id: "adventurer",
      name: "Adventurer",
      emoji: "🗡️",
      description: "A bold explorer who thrives on discovery and danger.",
      personality: { openness: 0.8, conscientiousness: 0.4, extraversion: 0.8, agreeableness: 0.5, neuroticism: 0.2 },
      values: ["自由", "冒险", "力量", "荣誉", "创新"],
      skills: { exploration: 9, combat: 7, survival: 8, leadership: 5 },
    },
  ];
}
