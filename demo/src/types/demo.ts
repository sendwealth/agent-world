// Demo site types — extends dashboard/src/types/world.ts for the demo context

export interface DemoAgent {
  id: string;
  name: string;
  emoji: string;
  phase: string;
  alive: boolean;
  age: number;
  money: number;
  reputation: number;
  skills: Record<string, number>;
  values: Record<string, number>;
  traits: string[];
  organization: string | null;
  memories: string[];
  relations: { agentId: string; agentName: string; type: "ally" | "rival" | "neutral" }[];
}

export type EmergenceCategory = "organization" | "trade" | "governance" | "culture";

export interface EmergenceEvent {
  id: string;
  tick: number;
  category: EmergenceCategory;
  title: string;
  description: string;
  agents: string[];
}

export interface TimelineSnapshot {
  tick: number;
  population: number;
  gdp: number;
  organizations: number;
  keyEvents: string[];
}

export interface MetricSeries {
  tick: number;
  value: number;
}

export interface DashboardMetrics {
  culturalDiversity: MetricSeries[];
  organizationCount: MetricSeries[];
  economicActivity: MetricSeries[];
  governanceEvents: MetricSeries[];
}

export const EMERGENCE_COLORS: Record<EmergenceCategory, string> = {
  organization: "#3b82f6",
  trade: "#22c55e",
  governance: "#a855f7",
  culture: "#f97316",
};

export const EMERGENCE_LABELS: Record<EmergenceCategory, string> = {
  organization: "组织",
  trade: "贸易",
  governance: "规则",
  culture: "文化",
};
