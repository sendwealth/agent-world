/** Core data types for the Agent World Demo site. */

// ── Agents ──────────────────────────────────────────────────────────

export interface DemoAgent {
  id: string;
  name: string;
  emoji: string;
  phase: "exploration" | "organization" | "governance";
  status: "alive" | "dead";
  age: number;
  money: number;
  tokens: number;
  reputation: number;
  skills: Record<string, number>;
  personality: PersonalityTraits;
  values: ValueProfile;
  memoryCount: number;
  relationships: Relationship[];
  organizationId: string | null;
}

export interface PersonalityTraits {
  openness: number;
  conscientiousness: number;
  extraversion: number;
  agreeableness: number;
  neuroticism: number;
}

export interface ValueProfile {
  survival: number;
  power: number;
  freedom: number;
  knowledge: number;
  cooperation: number;
  tradition: number;
}

export interface Relationship {
  targetId: string;
  targetName: string;
  type: "ally" | "rival" | "trade_partner" | "mentor" | "student";
  strength: number;
}

// ── Timeline / Events ───────────────────────────────────────────────

export type EmergenceCategory =
  | "organization"
  | "trade"
  | "governance"
  | "culture";

export interface EmergenceEvent {
  tick: number;
  category: EmergenceCategory;
  title: string;
  description: string;
  agentsInvolved: string[];
}

export interface WorldSnapshot {
  tick: number;
  phase: "exploration" | "organization" | "governance";
  aliveAgents: number;
  totalWealth: number;
  organizations: number;
  culturalDiversity: number;
  giniCoefficient: number;
  tradeVolume: number;
  governanceEvents: number;
}

// ── Metrics ─────────────────────────────────────────────────────────

export interface MetricPoint {
  tick: number;
  value: number;
}

export interface MetricSeries {
  name: string;
  color: string;
  points: MetricPoint[];
}

// ── Organizations ───────────────────────────────────────────────────

export interface DemoOrganization {
  id: string;
  name: string;
  type: "company" | "guild" | "alliance" | "university";
  memberCount: number;
  foundedTick: number;
  treasury: number;
  description: string;
}

// ── Sandbox ─────────────────────────────────────────────────────────

export interface SandboxTemplate {
  id: string;
  name: string;
  emoji: string;
  description: string;
  personality: PersonalityTraits;
  values: ValueProfile;
  skills: Record<string, number>;
}

// ── Data bundle (what the JSON files produce) ───────────────────────

export interface DemoData {
  agents: DemoAgent[];
  events: EmergenceEvent[];
  snapshots: WorldSnapshot[];
  metrics: Record<string, MetricSeries>;
  organizations: DemoOrganization[];
  templates: SandboxTemplate[];
}
