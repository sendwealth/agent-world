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
  | "tax"
  | "tax_collected"
  | "treasury_distributed"
  | "leadership_election_started"
  | "leadership_changed"
  | "treaty_proposed"
  | "treaty_signed"
  | "treaty_broken"
  | "relation_changed"
  | "coordination_task_created"
  | "coordination_task_agent_joined"
  | "coordination_task_agent_submitted"
  | "coordination_task_completed"
  | "coordination_task_cancelled"
  | "coordination_task_expired";

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

// Coordination (team) task types

export type CoordinationTaskStatus =
  | "open"
  | "in_progress"
  | "all_submitted"
  | "completed"
  | "expired"
  | "cancelled";

export interface CoordinationContribution {
  agent_id: string;
  content: string;
  submitted_tick: number;
}

export interface CoordinationTask {
  id: string;
  title: string;
  description: string;
  status: CoordinationTaskStatus;
  reward_pool: number;
  currency: string;
  escrow_held: boolean;
  coordinator_id: string;
  max_agents: number;
  participants: string[];
  contributions: Record<string, CoordinationContribution>;
  reward_overrides: Record<string, number>;
  org_id: string | null;
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

// Governance types — mirrors Rust API response shapes

export interface OrgMetrics {
  org_id: string;
  // Election metrics
  election_count: number;
  avg_participation_rate: number;
  avg_candidate_count: number;
  avg_term_length_ticks: number;
  // Tax metrics
  total_tax_collected: number;
  tax_per_member: number;
  tax_collection_count: number;
  treasury_balance: number;
  // Diplomacy metrics
  treaties_signed: number;
  treaties_broken: number;
  active_relations_count: number;
  // Organization health
  member_count: number;
  governance_stability_score: number; // 0.0-1.0
}

export interface WorldGovernanceSummary {
  total_orgs: number;
  avg_stability: number;
  total_tax_collected: number;
  total_treaties: number;
  election_activity_rate: number;
}

export interface GovernanceTimelineEvent {
  event_type: string;
  org_id: string;
  tick: number;
  summary: string;
}

// Human Participation types

export type OracleType = "guidance" | "warning" | "blessing" | "curse";

export type OracleStatus = "pending" | "delivered" | "acknowledged" | "expired";

export interface Oracle {
  id: string;
  human_id: string;
  oracle_type: OracleType;
  target_agent_id: string;
  content: string;
  status: OracleStatus;
  agent_response: string | null;
  created_tick: number;
  delivered_tick: number | null;
}

export type BountyStatus =
  | "open"
  | "in_progress"
  | "completed"
  | "expired"
  | "cancelled";

export interface Bounty {
  id: string;
  human_id: string;
  title: string;
  description: string;
  reward: number;
  target_agent_id: string | null;
  status: BountyStatus;
  claimant_agent_id: string | null;
  result: string | null;
  expires_tick: number | null;
  created_tick: number;
}

export interface HumanPortfolio {
  human_id: string;
  total_assets: number;
  total_invested: number;
  total_pnl: number;
  holdings: HumanHolding[];
  history: { tick: number; value: number }[];
}

export interface HumanHolding {
  agent_id: string;
  agent_name: string;
  invested: number;
  current_value: number;
  pnl: number;
  pnl_percent: number;
}

export interface HumanInfluenceEntry {
  human_id: string;
  display_name: string;
  total_influence: number;
  oracle_count: number;
  bounty_count: number;
  agents_affected: number;
  economic_impact: number;
  political_impact: number;
  cultural_impact: number;
}

export type HumanInterventionType =
  | "direct_control"
  | "guidance"
  | "observation"
  | "voting";

export interface HumanInterventionEvent {
  id: string;
  human_id: string;
  intervention_type: HumanInterventionType;
  target_agent_id: string | null;
  description: string;
  tick: number;
  impact_score: number;
}

export interface ClaimedAgent {
  agent_id: string;
  agent_name: string;
  alive: boolean;
  tokens: number;
  money: number;
  reputation: number;
  skills: Record<string, number>;
  age: number;
}

// Provider types

export type ProviderProtocol =
  | "openai_compatible"
  | "anthropic"
  | "ollama"
  | "google"
  | "azure";

export type ConnectionStatus =
  | "online"
  | "offline"
  | "untested";

export interface Provider {
  id: string;
  display_name: string;
  protocol: ProviderProtocol;
  base_url: string;
  api_key?: string;
  api_version?: string;
  models?: string[];
  status?: ConnectionStatus;
  is_default?: boolean;
  created_at?: string;
  updated_at?: string;
}

export interface ConnectionTestResult {
  success: boolean;
  latency_ms: number;
  error?: string;
  sample?: string;
}

export interface DiscoverModelsResult {
  models: string[];
}

// Agent model assignment types

export interface AgentModelAssignment {
  agent_id: string;
  provider_id: string;
  model_id: string;
}

export interface SetAgentModelRequest {
  provider_id: string;
  model_id: string;
}

/** Agent record from GET /api/v1/agents (world engine). */
export interface AgentRecord {
  id: string;
  name: string;
  phase: string;
  tokens: number;
  money: number;
  alive: boolean;
  ticks_survived: number;
  personality: string;
  parent_ids: string[];
  generation: number;
  skills: Record<string, number>;
}

export type NotificationType =
  | "agent_death"
  | "leadership_changed"
  | "treaty_signed"
  | "treaty_broken"
  | "oracle_delivered"
  | "bounty_claimed";

export interface AgentNotification {
  id: string;
  type: NotificationType;
  title: string;
  description: string;
  tick: number;
  timestamp: number;
  read: boolean;
  agent_id?: string;
  agent_name?: string;
}
