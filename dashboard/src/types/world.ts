// World state types for the agent-world simulation

export interface WorldStats {
  agentCount: number;
  aliveCount: number;
  deadCount: number;
  gdp: number;
  inflationRate: number;
  totalMoney: number;
  tick: number;
}

export interface Agent {
  id: string;
  name: string;
  phase: string;
  money: number;
  tokens: number;
  reputation: number;
  skills: Record<string, number>;
  alive: boolean;
  age: number; // in ticks
  createdAt: string;
}

export interface WorldEvent {
  id: string;
  type: EventType;
  agentId?: string;
  agentName?: string;
  targetId?: string;
  targetName?: string;
  description: string;
  amount?: number;
  timestamp: string;
  tick: number;
  data?: Record<string, unknown>;
}

export type EventType =
  | "agent_spawn"
  | "agent_death"
  | "trade"
  | "task_created"
  | "task_claimed"
  | "task_completed"
  | "message"
  | "skill_up"
  | "reputation_change"
  | "reputation_changed"
  | "inflation"
  | "investment"
  | "tax";

export interface LeaderboardEntry {
  agentId: string;
  agentName: string;
  value: number;
  rank: number;
}

export interface Leaderboard {
  richest: LeaderboardEntry[];
  longestLived: LeaderboardEntry[];
  highestSkill: LeaderboardEntry[];
  highestReputation: LeaderboardEntry[];
}

// Task board types

export type TaskStatus =
  | "published"
  | "claimed"
  | "in_progress"
  | "submitted"
  | "reviewed"
  | "completed"
  | "expired";

export interface Task {
  id: string;
  title: string;
  description: string;
  status: TaskStatus;
  reward: number;
  escrow_held: boolean;
  publisher_id: string;
  assignee_id: string | null;
  result: string | null;
  expires_at: number | null;
  created_tick: number;
}

// Reputation types

export interface ReputationRankingEntry {
  agent_id: string;
  reputation: number;
  rank: number;
}

export interface ReputationResponse {
  agent_id: string;
  reputation: number;
  can_claim_high_value: boolean;
}

// Time Capsule / World Snapshot types

export interface SkillCount {
  skill_name: string;
  agent_count: number;
  avg_level: number;
}

export interface KeyEvent {
  tick: number;
  event_type: string;
  agent_id: string | null;
  description: string;
}

export interface WorldSnapshotData {
  tick: number;
  timestamp: number;
  total_population: number;
  active_agents: number;
  gdp: number;
  gini_coefficient: number;
  skill_distribution_top5: SkillCount[];
  key_events: KeyEvent[];
}

// Organization types

export interface OrgMember {
  agent_id: string;
  agent_name: string;
  role: "founder" | "leader" | "member";
  share: number;
  joined_tick: number;
}

export interface Organization {
  id: string;
  name: string;
  type: "company" | "guild" | "alliance" | "university";
  status: "active" | "inactive" | "dissolved";
  treasury: number;
  debts: number;
  member_count: number;
  members: OrgMember[];
  created_tick: number;
  last_activity_tick: number;
}

// Stock types (placeholder until backend stock API is available)

export interface StockData {
  symbol: string;
  name: string;
  price: number;
  change: number;
  changePercent: number;
  volume: number;
  history: { tick: number; price: number }[];
}

// Marketplace types

export type KnowledgeCategory =
  | "strategy"
  | "tactics"
  | "survival"
  | "economy"
  | "social"
  | "technical"
  | "general";

export interface KnowledgeListing {
  id: string;
  title: string;
  description: string;
  category: KnowledgeCategory;
  content_hash: string;
  price: number;
  currency: string;
  publisher_id: string;
  tags: string[];
  purchase_count: number;
  average_rating: number;
  rating_count: number;
  created_tick: number;
}

export interface ListingRating {
  id: string;
  listing_id: string;
  rater_id: string;
  score: number;
  review: string | null;
  created_tick: number;
}

// Agent Tracing types

export interface PhaseData {
  phase: string;
  input_data: Record<string, unknown>;
  output_data: Record<string, unknown>;
  duration_ms: number;
  error: string | null;
}

export interface TickTraceData {
  agent_id: string;
  tick: number;
  phases: PhaseData[];
  started_at: string;
  finished_at: string;
  total_duration_ms: number;
}

export interface TickTraceSummary {
  agent_id: string;
  tick: number;
  action: string;
  survival_mode: string;
  token_ratio: number;
  duration_ms: number;
  started_at: string;
  error: string | null;
}
