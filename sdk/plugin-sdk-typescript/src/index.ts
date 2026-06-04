/**
 * Agent World Plugin SDK — Barrel Export
 *
 * Re-exports all public types, the abstract SkillPlugin class,
 * and the PluginClient HTTP client from a single entry point.
 *
 * @example
 * ```typescript
 * import { SkillPlugin, type PluginInfo, type ActionResult } from "@agent-world/plugin-sdk";
 * ```
 */

// Types
export type {
  PluginId,
  SemVer,
  SkillId,
  AgentId,
  PluginInfo,
  AgentSnapshot,
  WorldContext,
  ActionContext,
  ActionResult,
  StateMutation,
  TokenCost,
  PluginError,
  PluginErrorKind,
  PluginListResponse,
  PluginActionResponse,
  RegisterPluginRequest,
  LoadWasmRequest,
  ExecuteRequest,
} from "./types.js";

// Enum
export { MutationKind } from "./types.js";

// Abstract base class
export { SkillPlugin } from "./plugin.js";

// HTTP client
export { PluginClient, PluginClientError } from "./client.js";
export type { PluginClientOptions } from "./client.js";
