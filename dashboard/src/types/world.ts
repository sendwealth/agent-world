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

// Knowledge Marketplace types

export type KnowledgeCategory =
  | "strategy"
  | "tactics"
  | "survival"
  | "economy"
  | "social"
  | "technical"
  | "general";

export type ListingStatus = "active" | "inactive" | "delisted";

export type MarketplaceSort =
  | "newest"
  | "oldest"
  | "price_asc"
  | "price_desc"
  | "rating_desc"
  | "purchases_desc";

export interface KnowledgeListing {
  id: string;
  title: string;
  description: string;
  category: KnowledgeCategory;
  content_hash: string;
  price: number;
  currency: string;
  publisher_id: string;
  status: ListingStatus;
  purchase_count: number;
  average_rating: number;
  rating_count: number;
  tags: string[];
  created_tick: number;
}

export interface ListingPurchase {
  id: string;
  listing_id: string;
  buyer_id: string;
  seller_id: string;
  price: number;
  currency: string;
  tick: number;
}

export interface ListingRating {
  id: string;
  listing_id: string;
  rater_id: string;
  score: number;
  review: string | null;
  tick: number;
}
