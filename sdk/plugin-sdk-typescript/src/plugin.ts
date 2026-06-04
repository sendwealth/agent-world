/**
 * Agent World Plugin SDK — Abstract SkillPlugin class
 *
 * All skill plugins must extend this class and implement the abstract methods.
 * The engine calls these methods during the plugin lifecycle:
 *
 *   Loaded → Init → Register → (Execute ↔ Cost Estimate) → Shutdown
 */

import type {
  PluginInfo,
  ActionContext,
  ActionResult,
  WorldContext,
  TokenCost,
  PluginError,
  SkillId,
} from "./types.js";

/**
 * Abstract base class for Agent World skill plugins.
 *
 * Subclass this to create a plugin that the WASM runtime can load
 * and invoke during simulation ticks.
 *
 * @example
 * ```typescript
 * class MyPlugin extends SkillPlugin {
 *   init(config: Record<string, string>): PluginInfo {
 *     return {
 *       id: "me/my-plugin",
 *       name: "My Plugin",
 *       version: "0.1.0",
 *       description: "Does something cool",
 *       author: "Me",
 *       min_engine_version: "1.0.0",
 *       required_skills: [],
 *       tags: ["example"],
 *     };
 *   }
 *
 *   register(): SkillId[] {
 *     return ["my_skill"];
 *   }
 *
 *   execute(ctx: ActionContext): ActionResult {
 *     return {
 *       success: true,
 *       message: "Hello from my plugin!",
 *       mutations: [],
 *       events: [],
 *       data: {},
 *       tokens_consumed: 10,
 *     };
 *   }
 *
 *   costEstimate(ctx: ActionContext): TokenCost {
 *     return { estimated: 10, confidence: 1.0 };
 *   }
 * }
 * ```
 */
export abstract class SkillPlugin {
  // ─── Abstract Methods (must be implemented) ──────────────────────────

  /**
   * Return plugin metadata. Called once after loading / WASM instantiation.
   *
   * Use this to validate configuration and prepare internal state.
   *
   * @param config - Plugin configuration key-value pairs.
   * @returns Plugin metadata or a PluginError on failure.
   */
  abstract init(config: Record<string, string>): PluginInfo | PluginError;

  /**
   * Return the list of skill IDs this plugin provides.
   * Called after `init()` succeeds. The engine registers these
   * skills in the world's skill tree.
   */
  abstract register(): SkillId[];

  /**
   * Execute the plugin's core logic.
   *
   * Receives an `ActionContext` with world state and parameters.
   * Returns an `ActionResult` with success status and any requested
   * state mutations.
   *
   * @param ctx - The full execution context.
   * @returns The action result or a PluginError on failure.
   */
  abstract execute(ctx: ActionContext): ActionResult | PluginError;

  /**
   * Estimate the token cost of execution *before* calling `execute()`.
   *
   * The engine uses this to decide whether to proceed based on the
   * agent's remaining token budget. Return a cost of 0 for free actions.
   *
   * @param ctx - The execution context (read-only).
   * @returns The token cost estimate or a PluginError on failure.
   */
  abstract costEstimate(ctx: ActionContext): TokenCost | PluginError;

  // ─── Default Methods (optional overrides) ────────────────────────────

  /**
   * Graceful shutdown. Called when the engine is stopping or
   * unloading the plugin. Override to release resources.
   *
   * Default implementation is a no-op.
   */
  shutdown(): void {
    // no-op by default
  }

  /**
   * Handle a world event. Called when an event matching the
   * plugin's subscriptions fires.
   *
   * Override to react to events. Return an `ActionResult` to request
   * state changes, or `null` / `undefined` to do nothing.
   *
   * @param _event - The event name.
   * @param _ctx - The current world context.
   * @returns An optional ActionResult, or null/undefined to ignore.
   */
  onEvent(_event: string, _ctx: WorldContext): ActionResult | null {
    return null;
  }
}
