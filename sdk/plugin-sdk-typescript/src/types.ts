/**
 * Agent World Plugin SDK — Type Definitions
 *
 * All core types matching the Plugin Interface Specification v1.0.0-draft.
 * These types are used for WASM ABI communication, plugin metadata,
 * world context snapshots, action results, and error handling.
 */

// ─── Core Type Aliases ───────────────────────────────────────────────────

/** Unique identifier for a plugin, namespaced by author (e.g. `"agentworld/code-reviewer"`). */
export type PluginId = string;

/** Semantic version string (MAJOR.MINOR.PATCH). */
export type SemVer = string;

/** Identifier for a built-in or registered skill. */
export type SkillId = string;

/** Unique identifier for an agent in the simulation. */
export type AgentId = string;

// ─── Metadata ────────────────────────────────────────────────────────────

/** Plugin metadata returned from `init()`. */
export interface PluginInfo {
  /** Unique plugin identifier (e.g. `"agentworld/code-reviewer"`). */
  id: PluginId;
  /** Human-readable name. */
  name: string;
  /** Plugin version (semver). */
  version: SemVer;
  /** One-line description. */
  description: string;
  /** Author name or organization. */
  author: string;
  /** Minimum compatible engine API version. */
  min_engine_version: SemVer;
  /** List of skill IDs this plugin depends on. */
  required_skills: SkillId[];
  /** Optional configuration schema (JSON Schema draft-07). */
  config_schema?: string;
  /** Tags for marketplace discovery. */
  tags: string[];
}

// ─── Context Types ───────────────────────────────────────────────────────

/** Snapshot of a single agent's public state. */
export interface AgentSnapshot {
  /** Unique agent identifier. */
  id: AgentId;
  /** Agent display name. */
  name: string;
  /** Current simulation phase. */
  phase: string;
  /** Agent's money balance. */
  money: number;
  /** Agent's token balance. */
  tokens: number;
  /** Agent's reputation score (0.0–1.0). */
  reputation: number;
  /** Skill levels keyed by skill ID. */
  skills: Record<string, number>;
  /** Whether the agent is alive. */
  alive: boolean;
  /** Agent's age in ticks. */
  age: number;
}

/** Read-only snapshot of the world state, provided to the plugin. */
export interface WorldContext {
  /** Current simulation tick. */
  tick: number;
  /** Agent executing the skill (if applicable). */
  agent?: AgentSnapshot;
  /** All visible agents (depends on permissions). */
  visible_agents: AgentSnapshot[];
  /** World-level global key-value state. */
  globals: Record<string, string>;
  /** Events emitted since last tick. */
  recent_events: string[];
}

/** The action context passed to `execute()`. */
export interface ActionContext {
  /** World context (read-only snapshot). */
  world: WorldContext;
  /** Skill-specific input parameters (from the agent's decision). */
  params: Record<string, string>;
  /** Plugin-specific configuration. */
  config: Record<string, string>;
}

// ─── Response Types ──────────────────────────────────────────────────────

/** Kinds of state mutations a plugin can request. */
export enum MutationKind {
  /** Add tokens to an agent's balance. */
  CreditTokens = "credit_tokens",
  /** Subtract tokens from an agent's balance. */
  DebitTokens = "debit_tokens",
  /** Add money to an agent. */
  CreditMoney = "credit_money",
  /** Subtract money from an agent. */
  DebitMoney = "debit_money",
  /** Update a skill level. */
  SetSkill = "set_skill",
  /** Modify reputation. */
  AdjustReputation = "adjust_reputation",
  /** Set a world-level key-value pair. */
  SetGlobal = "set_global",
  /** Emit a custom event. */
  EmitEvent = "emit_event",
}

/** A state mutation requested by the plugin. */
export interface StateMutation {
  /** The kind of mutation. */
  kind: MutationKind;
  /** Target agent ID (if applicable). */
  target_agent?: AgentId;
  /** Field name to mutate. */
  field: string;
  /** New value (string-encoded). */
  value: string;
}

/** Result of `execute()`. */
export interface ActionResult {
  /** Whether the action succeeded. */
  success: boolean;
  /** Human-readable result message. */
  message: string;
  /** State mutations the plugin requests the engine to apply. */
  mutations: StateMutation[];
  /** Events the plugin wants to emit. */
  events: string[];
  /** Additional data to return to the agent. */
  data: Record<string, string>;
  /** Actual token cost consumed (may differ from estimate). */
  tokens_consumed: number;
}

/** Token cost estimate, returned from `cost_estimate()`. */
export interface TokenCost {
  /** Estimated token consumption. */
  estimated: number;
  /** Confidence level (0.0–1.0). */
  confidence: number;
  /** Human-readable cost breakdown. */
  breakdown?: string;
}

// ─── Error Handling ──────────────────────────────────────────────────────

/** Discriminator for PluginError variants. */
export type PluginErrorKind =
  | "init_failed"
  | "execution_failed"
  | "config_error"
  | "missing_skill"
  | "cost_estimate_failed"
  | "invalid_state"
  | "custom";

/**
 * Structured error type that a plugin can return.
 *
 * Uses a discriminated union based on `kind` so consumers can
 * exhaustively match on error variants.
 */
export type PluginError =
  | { kind: "init_failed"; reason: string }
  | { kind: "execution_failed"; reason: string }
  | { kind: "config_error"; key: string; message: string }
  | { kind: "missing_skill"; skill_id: SkillId }
  | { kind: "cost_estimate_failed"; reason: string }
  | { kind: "invalid_state"; expected: string; actual: string }
  | { kind: "custom"; code: string; message: string };

// ─── API Response Types ──────────────────────────────────────────────────

/** Response from the plugin list endpoint. */
export interface PluginListResponse {
  plugins: PluginInfo[];
  total: number;
  active: number;
}

/** Response from plugin action endpoints (enable/disable/unload). */
export interface PluginActionResponse {
  id: string;
  action: string;
  success: boolean;
  message: string;
}

/** Request body for registering a plugin. */
export interface RegisterPluginRequest {
  id: string;
  name: string;
  version: string;
  description: string;
  author: string;
  priority?: number;
  permissions?: string[];
}

/** Request body for loading WASM into the sandbox. */
export interface LoadWasmRequest {
  plugin_id: string;
  wasm_base64: string;
}

/** Request body for executing a plugin skill. */
export interface ExecuteRequest {
  plugin_id: string;
  skill_id: string;
  agent_id: string;
  params?: Record<string, string>;
}
