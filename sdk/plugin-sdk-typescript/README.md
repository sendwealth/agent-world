# @agent-world/plugin-sdk

TypeScript SDK for building Agent World skill plugins.

## Install

```bash
npm install @agent-world/plugin-sdk
```

## Quick Start

### 1. Create a Plugin

Extend the `SkillPlugin` abstract class and implement the required methods:

```typescript
import {
  SkillPlugin,
  type PluginInfo,
  type ActionResult,
  type ActionContext,
  type TokenCost,
  type SkillId,
} from "@agent-world/plugin-sdk";

class MyPlugin extends SkillPlugin {
  init(config: Record<string, string>): PluginInfo {
    return {
      id: "me/my-plugin",
      name: "My Plugin",
      version: "0.1.0",
      description: "Does something cool",
      author: "Me",
      min_engine_version: "1.0.0",
      required_skills: [],
      tags: ["example"],
    };
  }

  register(): SkillId[] {
    return ["my_skill"];
  }

  execute(ctx: ActionContext): ActionResult {
    const agentName = ctx.world.agent?.name ?? "unknown";
    return {
      success: true,
      message: `Hello, ${agentName}!`,
      mutations: [],
      events: [],
      data: {},
      tokens_consumed: 10,
    };
  }

  costEstimate(ctx: ActionContext): TokenCost {
    return {
      estimated: 10,
      confidence: 0.9,
      breakdown: "Fixed cost per invocation",
    };
  }

  // Optional: react to world events
  override onEvent(event: string, ctx) {
    if (event === "agent_spawned") {
      console.log(`New agent spawned at tick ${ctx.tick}`);
    }
    return null;
  }
}
```

### 2. Use the HTTP Client

Connect to a running Agent World engine:

```typescript
import { PluginClient } from "@agent-world/plugin-sdk";

const client = new PluginClient({
  baseUrl: "http://localhost:8080",
  apiKey: "your-api-key", // optional
});

// Register a plugin
await client.register({
  id: "me/my-plugin",
  name: "My Plugin",
  version: "0.1.0",
  description: "A test plugin",
  author: "Me",
});

// List all plugins
const { plugins, total, active } = await client.list();

// Enable / disable
await client.enable("me/my-plugin");
await client.disable("me/my-plugin");

// Load WASM into sandbox
import { readFileSync } from "fs";
const wasm = readFileSync("./my-plugin.wasm");
await client.loadWasmBytes("me/my-plugin", new Uint8Array(wasm));

// Execute a skill
const result = await client.execute({
  plugin_id: "me/my-plugin",
  skill_id: "my_skill",
  agent_id: "agent-1",
  params: { key: "value" },
});
```

## Type Reference

| Type | Description |
|------|-------------|
| `PluginInfo` | Plugin metadata (id, name, version, etc.) |
| `AgentSnapshot` | Read-only snapshot of an agent's state |
| `WorldContext` | Read-only world state snapshot |
| `ActionContext` | Full context for plugin execution |
| `ActionResult` | Result of execute() with mutations and events |
| `StateMutation` | A requested state change |
| `MutationKind` | Enum of mutation types |
| `TokenCost` | Token cost estimate |
| `PluginError` | Discriminated union error type |

## Plugin Lifecycle

```
Loaded → Init → Register → (Execute ↔ Cost Estimate) → Shutdown
```

1. **Init** — Return metadata, validate config.
2. **Register** — Declare which skills the plugin provides.
3. **Execute** — Run the plugin's core logic each tick.
4. **Cost Estimate** — Pre-flight budget check before execution.
5. **Shutdown** — Graceful teardown (optional override).

## Development

```bash
npm install
npm run build
npm test
```

## License

MIT
