// World state types for the agent-world simulation

export interface WorldStats {
  agentCount: number;
  aliveCount: number;
  deadCount: number;
  totalMoney: number;
  totalTokens: number;
  tick: number;
  taskCount: number;
}

export interface Agent {
  id: string;
  name: string;
  phase: string;
  money: number;
  tokens: number;
  reputation?: number;
  skills: Record<string, number>;
  alive: boolean;
  age?: number; // in ticks
  ticks_survived?: number;
  personality?: string;
  parent_ids?: string[];
  generation?: number;
  createdAt?: string;
  created_at?: string; // API returns snake_case
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
  | "tick_advanced"
  | "agent_spawned"
  | "agent_dying"
  | "agent_died"
  | "agent_rescued"
  | "transaction_completed"
  | "balance_changed"
  | "phase_changed"
  | "rule_violated"
  | "snapshot_taken"
  | "escrow_created"
  | "escrow_claimed"
  | "escrow_released"
  | "escrow_refunded"
  | "escrow_frozen"
  | "task_created"
  | "task_claimed"
  | "task_started"
  | "task_submitted"
  | "task_reviewed"
  | "task_completed"
  | "task_expired"
  | "reward_distributed"
  | "reputation_changed"
  | "skill_level_up"
  | "skill_mutated"
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
  | "coordination_task_expired"
  | "soft_rule_proposed"
  | "soft_rule_activated"
  | "soft_rule_expired"
  | "soft_rule_repealed"
  | "investment_product_created"
  | "investment_purchased"
  | "investment_sold"
  | "investment_dividend"
  | "feed_post_created"
  | "feed_post_liked"
  | "feed_comment_created"
  | "feed_comment_liked";

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

// Stock types — mirrors Rust `StockResponse` from `api_stocks.rs`

/** Raw shape returned by `GET /api/v1/stocks` and `GET /api/v1/stocks/:id`. */
export interface StockResponse {
  id: string;
  org_id: string;
  ticker: string;
  total_shares: number;
  price: number;
  status: "pre_ipo" | "listed" | "delisted";
  listed_tick: number;
}

/**
 * View-model used by the stocks page. Mirrors `StockResponse` for listed
 * stocks; `change`, `changePercent`, `volume`, and `history` are derived
 * client-side because the backend does not yet expose per-tick price history
 * or trade volume summaries.
 */
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

// Tool Marketplace types

export type ToolCategory =
  | "computation"
  | "communication"
  | "analysis"
  | "storage"
  | "automation"
  | "defense"
  | "production"
  | "utility";

export type ToolListingMode = "sale" | "rent" | "both";

export type ToolListingStatus = "active" | "inactive" | "delisted";

export type RentalStatus = "active" | "expired" | "cancelled";

export interface ToolListing {
  id: string;
  name: string;
  description: string;
  category: ToolCategory;
  owner_id: string;
  purchase_price: number;
  rental_price_per_tick: number;
  currency: string;
  listing_mode: ToolListingMode;
  status: ToolListingStatus;
  total_purchases: number;
  total_rentals: number;
  rating_sum: number;
  rating_count: number;
  tags: string[];
  created_tick: number;
}

export interface ToolRentalRecord {
  id: string;
  tool_id: string;
  renter_id: string;
  owner_id: string;
  price_per_tick: number;
  currency: string;
  start_tick: number;
  end_tick: number;
  status: RentalStatus;
}

export interface ToolPurchaseRecord {
  id: string;
  tool_id: string;
  buyer_id: string;
  seller_id: string;
  price: number;
  currency: string;
  tick: number;
}

export interface ToolRating {
  id: string;
  tool_id: string;
  rater_id: string;
  score: number;
  review: string | null;
  tick: number;
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
  // Legislation metrics
  rules_proposed: number;
  rules_activated: number;
  rules_expired: number;
  rules_repealed: number;
  legislation_success_rate: number; // 0.0-1.0
}

export interface WorldGovernanceSummary {
  total_orgs: number;
  avg_stability: number;
  total_tax_collected: number;
  total_treaties: number;
  election_activity_rate: number;
  // Legislation summary
  total_rules_proposed: number;
  total_rules_activated: number;
  avg_legislation_success_rate: number;
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
  reputation?: number;
  skills: Record<string, number>;
  age?: number;
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
  | "bounty_claimed"
  | "low_token_warning"
  | "agent_help_request"
  | "agent_diary";

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

// Agent Diary types

export type DiaryMood =
  | "happy"
  | "anxious"
  | "fearful"
  | "calm"
  | "hopeful"
  | "angry"
  | "sad"
  | "neutral"
  | "excited"
  | "confused";

export interface DiaryEntry {
  agent_id: string;
  tick: number;
  phase: string;
  mood: string;
  summary: string;
  key_events: string[];
  decisions: string[];
  reflection: string;
  created_at: string;
}

// Chat timeline message types (unified conversation view)

export type ChatMessageRole = "oracle" | "agent_response" | "diary";

export interface ChatMessage {
  id: string;
  role: ChatMessageRole;
  content: string;
  tick: number;
  timestamp: string;
  /** For oracle messages */
  oracle_type?: OracleType;
  /** For agent_response messages */
  oracle_id?: string;
  /** For diary messages */
  mood?: string;
  phase?: string;
  /** Urgency flag (low token, death risk) */
  urgent?: boolean;
}

// Plugin types — mirrors Rust API response shapes

export type PluginState =
  | "registered"
  | "active"
  | "disabled"
  | "error"
  | "unloaded";

export interface PluginInfo {
  id: string;
  name: string;
  version: string;
  description: string;
  author: string;
  priority: number;
  state: PluginState;
  permissions: string[];
  hooks: string[];
}

export interface PluginListResponse {
  plugins: PluginInfo[];
  total: number;
  active: number;
}

export interface PluginActionResponse {
  id: string;
  action: string;
  success: boolean;
  message: string;
}

export interface RegisterPluginRequest {
  id: string;
  name: string;
  version: string;
  description: string;
  author: string;
  priority?: number;
  permissions?: string[];
}

export interface SandboxListResponse {
  plugins: Record<string, unknown>[];
  total: number;
  active: number;
}

// ── Legislation Cycle types ──

export type LegislationCyclePhase = "election" | "proposal" | "voting" | "enactment" | "completed";
export type RuleType = "soft" | "hard" | "constitutional";
export type RuleConditionOperator = "gt" | "lt" | "eq" | "gte" | "lte" | "ne";

export interface RuleCondition {
  field: string;
  operator: RuleConditionOperator;
  value: number;
}

export interface RuleEffect {
  target: string;
  action: string;
  params: Record<string, unknown>;
}

export interface CandidateRule {
  id: string;
  proposer_id: string;
  title: string;
  description: string;
  rule_type: RuleType;
  conditions: RuleCondition[];
  effects: RuleEffect[];
  vote_count: number;
  enacted: boolean;
  created_tick: number;
  expires_tick: number | null;
}

export interface LegislationCycleRecord {
  cycle_id: string;
  org_id: string;
  phase: LegislationCyclePhase;
  leader_id: string | null;
  candidates: CandidateRule[];
  enacted_rules: CandidateRule[];
  created_tick: number;
  completed_tick: number | null;
  vote_deadline_tick: number | null;
}

export interface CycleResponse {
  cycle: LegislationCycleRecord | null;
}

export interface StartCycleResultResponse {
  cycle_id: string;
  message: string;
}

export interface EnactedRulesResponse {
  rules: CandidateRule[];
}

export interface CandidateRulesResponse {
  rules: CandidateRule[];
}

export interface FullCycleResultResponse {
  cycle_id: string;
  enacted: CandidateRule[];
  failed: CandidateRule[];
  message: string;
}

// ── Federation / Diplomacy types ──

export type RelationStatus = "neutral" | "friendly" | "hostile" | "allied" | "at_war";
export type CrossWorldTreatyType = "trade" | "non_aggression" | "alliance" | "cultural" | "technology" | "migration";
export type CrossWorldTreatyStatus = "proposed" | "active" | "rejected" | "broken" | "expired";

export interface FederatedWorld {
  id: string;
  name: string;
  endpoint: string;
  registered_tick: number;
  relation_status?: RelationStatus;
}

export interface CrossWorldTreaty {
  id: string;
  proposer_world_id: string;
  target_world_id: string;
  treaty_type: CrossWorldTreatyType;
  status: CrossWorldTreatyStatus;
  duration_ticks: number;
  terms: string;
  proposed_tick: number;
  accepted_tick: number | null;
  expires_tick: number | null;
}

export interface FederationSummary {
  total_worlds: number;
  active_treaties: number;
  pending_treaties: number;
  active_sanctions: number;
  wars: number;
}

// ── Investment types ──

export type InvestmentProductStatus = "open" | "closed" | "frozen";
export type InvestmentProductType = "bond" | "fund" | "derivative" | "fixed_deposit";

export interface InvestmentProduct {
  id: string;
  name: string;
  product_type: InvestmentProductType;
  manager_id: string;
  total_shares: number;
  available_shares: number;
  price_per_share: number;
  currency: string;
  min_investment: number;
  performance_score: number;
  status: InvestmentProductStatus;
  return_rate: number;
  created_tick: number;
  closed_tick: number | null;
}

export interface InvestmentPortfolio {
  investor_id: string;
  holdings: PortfolioHolding[];
  total_invested: number;
  total_value: number;
  total_pnl: number;
}

export interface PortfolioHolding {
  product_id: string;
  product_name: string;
  shares: number;
  avg_buy_price: number;
  current_value: number;
  pnl: number;
}

export interface InvestmentTransaction {
  id: string;
  investor_id: string;
  product_id: string;
  transaction_type: "buy" | "sell" | "dividend";
  shares: number;
  price: number;
  total: number;
  tick: number;
}

export interface InvestmentLeaderboardEntry {
  investor_id: string;
  total_value: number;
  total_pnl: number;
  pnl_percent: number;
  rank: number;
}

// ── Escrow types ──

export type EscrowStatus = "open" | "claimed" | "completed" | "refunded" | "disputed" | "resolved";

export interface EscrowRecord {
  id: string;
  publisher: string;
  claimant: string | null;
  reward: number;
  deposit: number;
  currency: string;
  status: EscrowStatus;
  created_tick: number;
  expires_at: number | null;
  resolved_at: number | null;
  dispute_reason: string | null;
  award_to_claimant: boolean | null;
}

// ── Trust Network types ──

export type TrustInteractionType = "trade" | "cooperation" | "betrayal" | "gift" | "deception" | "defense";

export interface TrustRelationship {
  from_agent: string;
  to_agent: string;
  score: number;
  interaction_count: number;
  last_interaction_tick: number;
}

export interface TrustStats {
  total_relationships: number;
  avg_trust_score: number;
  allies_count: number;
  enemies_count: number;
  top_allies: TrustRelationship[];
  top_enemies: TrustRelationship[];
}

// ── Mentorship types ──

export type MentorshipStatus = "active" | "completed" | "dropped";

export interface MentorshipSession {
  id: string;
  mentor_id: string;
  apprentice_id: string;
  skill_name: string;
  mentor_skill_level: number;
  status: MentorshipStatus;
  established_tick: number;
  completed_tick: number | null;
}

export interface MentorshipStats {
  total_sessions: number;
  active_sessions: number;
  completed_sessions: number;
  avg_skill_transfer_rate: number;
  popular_skills: { skill: string; count: number }[];
}

// ── Inheritance types ──

export interface Beneficiary {
  agent_id: string;
  share: number;
}

export interface Will {
  testator_id: string;
  beneficiaries: Beneficiary[];
  created_tick: number;
  executed: boolean;
  executed_tick: number | null;
}

export interface InheritanceStats {
  total_wills: number;
  executed_wills: number;
  pending_wills: number;
  total_inherited: number;
  avg_beneficiaries: number;
}

// ── Building types ──

export type BuildingType = "warehouse" | "market" | "workshop" | "defense_tower" | "housing";
export type OwnerType = "personal" | "organization";

export interface Building {
  id: string;
  building_type: BuildingType;
  x: number;
  y: number;
  owner_id: string;
  owner_type: OwnerType;
  health: number;
  max_health: number;
  level: number;
  created_tick: number;
}

// ── Export types ──

export type ExportFormat = "json" | "csv" | "graphml" | "dot" | "gexf";

export interface ExportTypeInfo {
  key: string;
  label: string;
  description: string;
}

// ── Network Graph types (mirrors GET /api/v2/export/network) ──

export interface NetworkNode {
  id: string;
  label: string;
  phase: string;
  alive: boolean;
  tokens: number;
  generation?: number;
  skills?: Record<string, number>;
  organization?: string;
}

export interface NetworkEdge {
  source: string;
  target: string;
  weight: number;
  edge_type: string;
  interaction_count?: number;
}

export interface NetworkGraph {
  node_count: number;
  edge_count: number;
  nodes: NetworkNode[];
  edges: NetworkEdge[];
}

// ── Phase 5.5: Human-as-Agent types ───────────────────────────────────────

export interface IncarnateResponse {
  agent_id: string;
  human_id: string;
  name: string;
  tokens: number;
  money: number;
  spawned_tick: number;
}

export interface HumanAgentStatus {
  agent_id: string;
  human_id: string;
  name: string;
  alive: boolean;
  tokens: number;
  money: number;
  phase: string;
  ticks_survived: number;
  last_action_tick: number;
  pending_actions: number;
}

export interface QueuedAction {
  id: string;
  agent_id: string;
  action: string;
  params: Record<string, unknown>;
  enqueued_tick: number;
  applied: boolean;
}

export interface ActionReceipt {
  queued_id: string;
  agent_id: string;
  action: string;
  enqueued_tick: number;
}

export interface HumanLeaderboardEntry {
  rank: number;
  agent_id: string;
  name: string;
  human_id: string;
  tokens: number;
  ticks_survived: number;
  alive: boolean;
}

export interface HumanPlayStats {
  total_incarnations: number;
  alive: number;
  dead: number;
  pending_actions: number;
}

