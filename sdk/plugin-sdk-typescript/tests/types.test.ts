/**
 * Agent World Plugin SDK — Type Tests
 *
 * These tests verify that the exported types are correctly structured
 * and that values conform to the expected interfaces at compile time.
 */

import { describe, it, expect } from "vitest";
import {
  MutationKind,
  type PluginInfo,
  type AgentSnapshot,
  type WorldContext,
  type ActionContext,
  type ActionResult,
  type StateMutation,
  type TokenCost,
  type PluginError,
} from "../src/index.js";

// ─── Helper factories ────────────────────────────────────────────────────

function makePluginInfo(overrides?: Partial<PluginInfo>): PluginInfo {
  return {
    id: "test/plugin",
    name: "Test Plugin",
    version: "0.1.0",
    description: "A test plugin",
    author: "Test Author",
    min_engine_version: "1.0.0",
    required_skills: [],
    tags: ["test"],
    ...overrides,
  };
}

function makeAgentSnapshot(overrides?: Partial<AgentSnapshot>): AgentSnapshot {
  return {
    id: "agent-1",
    name: "Test Agent",
    phase: "idle",
    money: 100,
    tokens: 500,
    reputation: 0.85,
    skills: { coding: 10 },
    alive: true,
    age: 42,
    ...overrides,
  };
}

function makeWorldContext(overrides?: Partial<WorldContext>): WorldContext {
  return {
    tick: 1,
    agent: makeAgentSnapshot(),
    visible_agents: [],
    globals: {},
    recent_events: [],
    ...overrides,
  };
}

function makeActionContext(overrides?: Partial<ActionContext>): ActionContext {
  return {
    world: makeWorldContext(),
    params: {},
    config: {},
    ...overrides,
  };
}

function makeActionResult(overrides?: Partial<ActionResult>): ActionResult {
  return {
    success: true,
    message: "ok",
    mutations: [],
    events: [],
    data: {},
    tokens_consumed: 0,
    ...overrides,
  };
}

// ─── Tests ───────────────────────────────────────────────────────────────

describe("MutationKind enum", () => {
  it("has all expected values matching snake_case serialization", () => {
    expect(MutationKind.CreditTokens).toBe("credit_tokens");
    expect(MutationKind.DebitTokens).toBe("debit_tokens");
    expect(MutationKind.CreditMoney).toBe("credit_money");
    expect(MutationKind.DebitMoney).toBe("debit_money");
    expect(MutationKind.SetSkill).toBe("set_skill");
    expect(MutationKind.AdjustReputation).toBe("adjust_reputation");
    expect(MutationKind.SetGlobal).toBe("set_global");
    expect(MutationKind.EmitEvent).toBe("emit_event");
  });

  it("has exactly 8 variants", () => {
    expect(Object.keys(MutationKind).length).toBe(8);
  });
});

describe("PluginInfo", () => {
  it("creates a valid PluginInfo with all required fields", () => {
    const info = makePluginInfo();
    expect(info.id).toBe("test/plugin");
    expect(info.version).toBe("0.1.0");
    expect(info.required_skills).toEqual([]);
    expect(info.tags).toEqual(["test"]);
  });

  it("supports optional config_schema", () => {
    const info = makePluginInfo({
      config_schema: '{"type":"object"}',
    });
    expect(info.config_schema).toBe('{"type":"object"}');
  });
});

describe("AgentSnapshot", () => {
  it("creates a valid snapshot", () => {
    const agent = makeAgentSnapshot();
    expect(agent.id).toBe("agent-1");
    expect(agent.alive).toBe(true);
    expect(agent.skills["coding"]).toBe(10);
    expect(typeof agent.reputation).toBe("number");
  });
});

describe("WorldContext", () => {
  it("creates a valid world context", () => {
    const world = makeWorldContext();
    expect(world.tick).toBe(1);
    expect(world.agent).toBeDefined();
    expect(world.visible_agents).toEqual([]);
    expect(world.globals).toEqual({});
    expect(world.recent_events).toEqual([]);
  });

  it("supports undefined agent", () => {
    const world = makeWorldContext({ agent: undefined });
    expect(world.agent).toBeUndefined();
  });
});

describe("ActionContext", () => {
  it("creates a valid action context", () => {
    const ctx = makeActionContext();
    expect(ctx.world).toBeDefined();
    expect(ctx.params).toEqual({});
    expect(ctx.config).toEqual({});
  });

  it("supports params and config", () => {
    const ctx = makeActionContext({
      params: { key: "value" },
      config: { mode: "test" },
    });
    expect(ctx.params["key"]).toBe("value");
    expect(ctx.config["mode"]).toBe("test");
  });
});

describe("StateMutation", () => {
  it("creates a mutation with all fields", () => {
    const mutation: StateMutation = {
      kind: MutationKind.CreditTokens,
      target_agent: "agent-1",
      field: "tokens",
      value: "100",
    };
    expect(mutation.kind).toBe(MutationKind.CreditTokens);
    expect(mutation.target_agent).toBe("agent-1");
    expect(mutation.field).toBe("tokens");
    expect(mutation.value).toBe("100");
  });

  it("allows undefined target_agent", () => {
    const mutation: StateMutation = {
      kind: MutationKind.SetGlobal,
      field: "my_key",
      value: "my_value",
    };
    expect(mutation.target_agent).toBeUndefined();
  });
});

describe("ActionResult", () => {
  it("creates a successful result", () => {
    const result = makeActionResult();
    expect(result.success).toBe(true);
    expect(result.tokens_consumed).toBe(0);
  });

  it("creates a result with mutations and events", () => {
    const result = makeActionResult({
      success: false,
      message: "Something went wrong",
      mutations: [
        { kind: MutationKind.DebitTokens, field: "tokens", value: "50" },
      ],
      events: ["error_occurred"],
      tokens_consumed: 50,
    });
    expect(result.success).toBe(false);
    expect(result.mutations).toHaveLength(1);
    expect(result.events).toContain("error_occurred");
  });
});

describe("TokenCost", () => {
  it("creates a cost estimate", () => {
    const cost: TokenCost = {
      estimated: 100,
      confidence: 0.95,
      breakdown: "Complex computation",
    };
    expect(cost.estimated).toBe(100);
    expect(cost.confidence).toBeCloseTo(0.95);
    expect(cost.breakdown).toBe("Complex computation");
  });

  it("allows optional breakdown", () => {
    const cost: TokenCost = { estimated: 50, confidence: 1.0 };
    expect(cost.breakdown).toBeUndefined();
  });
});

describe("PluginError", () => {
  it("creates all error variants", () => {
    const errors: PluginError[] = [
      { kind: "init_failed", reason: "bad config" },
      { kind: "execution_failed", reason: "timeout" },
      { kind: "config_error", key: "api_key", message: "missing" },
      { kind: "missing_skill", skill_id: "nonexistent" },
      { kind: "cost_estimate_failed", reason: "unknown" },
      { kind: "invalid_state", expected: "active", actual: "shutdown" },
      { kind: "custom", code: "E001", message: "something" },
    ];

    expect(errors).toHaveLength(7);
    expect(errors[0]!.kind).toBe("init_failed");
    expect(errors[6]!.kind).toBe("custom");
  });
});
