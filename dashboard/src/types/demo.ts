// Demo site types — extends dashboard/src/types/world.ts for the demo context

// ── Agents ──────────────────────────────────────────────────────────

export interface PersonalityTraits {
  openness: number;
  conscientiousness: number;
  extraversion: number;
  agreeableness: number;
  neuroticism: number;
}

export interface Relationship {
  target_id: string;
  target_name: string;
  type: string;
  strength: number;
}

/** Raw agent as stored in agents.json */
export interface RawAgent {
  id: string;
  name: string;
  phase: string;
  money: number;
  tokens: number;
  reputation: number;
  personality_traits: PersonalityTraits;
  values: string[];
  skills: Record<string, number>;
  relationships: Relationship[];
  memories: string[];
  birth_tick: number;
  death_tick: number | null;
  alive: boolean;
}

/** Normalized agent used by UI components */
export interface DemoAgent {
  id: string;
  name: string;
  phase: "exploration" | "organization" | "governance";
  status: "alive" | "dead";
  age: number;
  money: number;
  tokens: number;
  reputation: number;
  skills: Record<string, number>;
  personality: PersonalityTraits;
  values: string[];
  memoryCount: number;
  memories: string[];
  relationships: Relationship[];
  organizationId: string | null;
}

// ── Timeline / Events ───────────────────────────────────────────────

export type EmergenceCategory =
  | "organization"
  | "economic"
  | "governance"
  | "culture"
  | "milestone";

export interface InvolvedAgent {
  id: string;
  name: string;
}

export interface InvolvedOrg {
  id: string;
  name: string;
}

/** Raw timeline event from timeline-events.json */
export interface RawTimelineEvent {
  id: string;
  tick: number;
  type: string;
  title: string;
  description: string;
  involved_agents: InvolvedAgent[];
  involved_orgs: InvolvedOrg[];
}

/** Normalized emergence event used by UI */
export interface EmergenceEvent {
  id: string;
  tick: number;
  category: EmergenceCategory;
  title: string;
  description: string;
  agentsInvolved: string[];
  agentsDetail: InvolvedAgent[];
  orgsDetail: InvolvedOrg[];
}

// ── World Snapshots ─────────────────────────────────────────────────

export interface SkillDistribution {
  skill_name: string;
  agent_count: number;
  avg_level: number;
}

export interface SnapshotKeyEvent {
  tick: number;
  event_type: string;
  agent_id: string | null;
  description: string;
}

export interface RawWorldSnapshot {
  tick: number;
  timestamp: number;
  total_population: number;
  active_agents: number;
  gdp: number;
  gini_coefficient: number;
  skill_distribution_top5: SkillDistribution[];
  key_events: SnapshotKeyEvent[];
}

/** Normalized world snapshot used by UI */
export interface WorldSnapshot {
  tick: number;
  phase: "exploration" | "organization" | "governance";
  aliveAgents: number;
  totalPopulation: number;
  gdp: number;
  giniCoefficient: number;
  skillDistribution: SkillDistribution[];
  keyEvents: SnapshotKeyEvent[];
}

// ── Emergence Metrics ───────────────────────────────────────────────

export interface MetricPoint {
  tick: number;
  value: number;
}

export interface EconomicMetricPoint {
  tick: number;
  trade_volume: number;
  gdp: number;
}

export interface GovernanceMetricPoint {
  tick: number;
  proposals: number;
  votes_cast: number;
}

export interface RawEmergenceMetrics {
  cultural_diversity: MetricPoint[];
  organization_count: MetricPoint[];
  economic_activity: EconomicMetricPoint[];
  governance_events: GovernanceMetricPoint[];
}

export interface MetricSeries {
  name: string;
  color: string;
  points: MetricPoint[];
}

// ── Organizations ───────────────────────────────────────────────────

export interface OrgMember {
  agent_id: string;
  agent_name: string;
  role: string;
  share: number;
  joined_tick: number;
}

export interface RawOrganization {
  id: string;
  name: string;
  type: "company" | "guild" | "alliance" | "university";
  status: string;
  treasury: number;
  debts: number;
  member_count: number;
  members: OrgMember[];
  created_tick: number;
  last_activity_tick: number;
}

export interface DemoOrganization {
  id: string;
  name: string;
  type: "company" | "guild" | "alliance" | "university";
  status: string;
  memberCount: number;
  members: OrgMember[];
  foundedTick: number;
  treasury: number;
  debts: number;
  lastActivityTick: number;
}

// ── Interaction Network ─────────────────────────────────────────────

export interface NetworkNode {
  id: string;
  name: string;
  group: number;
}

export interface NetworkEdge {
  source: string;
  target: string;
  weight: number;
  type: string;
}

export interface InteractionNetwork {
  nodes: NetworkNode[];
  edges: NetworkEdge[];
}

// ── Sandbox ─────────────────────────────────────────────────────────

export interface SandboxTemplate {
  id: string;
  name: string;
  emoji: string;
  description: string;
  personality: PersonalityTraits;
  values: string[];
  skills: Record<string, number>;
}

// ── Constants ───────────────────────────────────────────────────────

export const EMERGENCE_COLORS: Record<EmergenceCategory, string> = {
  organization: "#3b82f6",
  economic: "#22c55e",
  governance: "#a855f7",
  culture: "#f59e0b",
  milestone: "#06b6d4",
};

export const EMERGENCE_LABELS: Record<EmergenceCategory, string> = {
  organization: "Organization",
  economic: "Economic",
  governance: "Governance",
  culture: "Culture",
  milestone: "Milestone",
};
