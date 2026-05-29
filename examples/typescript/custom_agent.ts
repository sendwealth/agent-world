/**
 * Agent World — Custom Agent Example (TypeScript)
 *
 * A complete example demonstrating how to:
 *   1. Register an agent with the World Engine
 *   2. Read perception (observe the world)
 *   3. Execute actions (move, gather, rest, etc.)
 *   4. Check agent status
 *   5. Deregister on shutdown
 *
 * Run with:
 *     npx tsx custom_agent.ts
 *
 * No external dependencies — uses the native fetch API (Node ≥ 18).
 */

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

const BASE_URL: string = process.env.AGENT_WORLD_BASE_URL ?? "http://localhost:3000";
const MAX_TICKS: number = parseInt(process.env.MAX_TICKS ?? "10", 10);
const TICK_INTERVAL_MS: number = parseFloat(process.env.TICK_INTERVAL ?? "1.0") * 1000;

// ---------------------------------------------------------------------------
// Types — matching the actual API response shapes
// ---------------------------------------------------------------------------

interface RegistrationResponse {
  agent_id: string;
  api_key: string;
  name: string;
}

interface PerceptionResponse {
  agent_id: string;
  nearby_agents: Array<{ id: string; name: string }>;
  nearby_resources: Array<{ type: string; position: { x: number; y: number } }>;
  position: { x: number; y: number };
  world_tick: number;
}

interface ActionResponse {
  action: string;
  success: boolean;
  tick: number;
}

interface StatusResponse {
  agent_id: string;
  name: string;
  alive: boolean;
  phase: string;
  tokens: number;
  money: number;
  position: { x: number; y: number };
  registered_tick: number;
  current_tick: number;
}

interface DeregisterResponse {
  deregistered: string;
}

// ---------------------------------------------------------------------------
// Low-level API helpers
// ---------------------------------------------------------------------------

/** Shorthand for making requests to the Agent World API. */
async function apiRequest<T>(
  method: string,
  path: string,
  body?: unknown,
): Promise<T> {
  const url = `${BASE_URL}${path}`;
  const headers: Record<string, string> = {
    "Content-Type": "application/json",
  };

  const response = await fetch(url, {
    method,
    headers,
    body: body ? JSON.stringify(body) : undefined,
  });

  if (!response.ok) {
    const errorBody = await response.text();
    throw new Error(`API error ${response.status}: ${errorBody}`);
  }

  return response.json() as Promise<T>;
}

/** Register a new agent. */
async function registerAgent(
  name: string,
  capabilities: string[] = [],
): Promise<RegistrationResponse> {
  return apiRequest("POST", "/api/v1/agents/register", { name, capabilities });
}

/** Get the agent's current status. */
async function getStatus(agentId: string): Promise<StatusResponse> {
  return apiRequest("GET", `/api/v1/agents/${agentId}/status`);
}

/** Get the agent's perception of the world. */
async function getPerception(agentId: string): Promise<PerceptionResponse> {
  return apiRequest("GET", `/api/v1/agents/${agentId}/perception`);
}

/** Execute an action on behalf of the agent. */
async function executeAction(
  agentId: string,
  action: string,
  params: Record<string, unknown> = {},
): Promise<ActionResponse> {
  return apiRequest("POST", `/api/v1/agents/${agentId}/action`, { action, params });
}

/** Deregister (remove) the agent. */
async function deregisterAgent(agentId: string): Promise<DeregisterResponse> {
  return apiRequest("DELETE", `/api/v1/agents/${agentId}`);
}

// ---------------------------------------------------------------------------
// Decision-making helper
// ---------------------------------------------------------------------------

/**
 * Simple rule-based decision loop (Perceive-Decide-Act).
 *
 * Priorities:
 *   1. Gather if resources are nearby.
 *   2. Move in a random direction otherwise.
 */
function decideAction(
  perception: PerceptionResponse,
): { action: string; params: Record<string, unknown> } {
  // 1) Gather if resources are nearby.
  if (perception.nearby_resources.length > 0) {
    return { action: "gather", params: {} };
  }

  // 2) Move in a random direction.
  const directions = ["north", "south", "east", "west"];
  const direction = directions[Math.floor(Math.random() * directions.length)];
  return { action: "move", params: { direction } };
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

async function main(): Promise<void> {
  let agentId: string | undefined;

  try {
    // Step 1 — Register the agent.
    console.log("[info] Registering agent …");
    const reg = await registerAgent("TSScout", ["move", "gather", "explore", "rest"]);
    agentId = reg.agent_id;
    console.log(`[info] Registered as ${reg.name} (id=${agentId})`);

    // Step 2 — Main simulation loop.
    for (let tick = 1; tick <= MAX_TICKS; tick++) {
      console.log(`\n--- Tick ${tick} ---`);

      // 2a. Observe the world (perception).
      const perception = await getPerception(agentId);
      console.log(
        `[perception] position=(${perception.position.x},${perception.position.y}), ` +
        `${perception.nearby_agents.length} nearby agents, ` +
        `${perception.nearby_resources.length} resources`,
      );

      // 2b. Decide what to do next.
      const decision = decideAction(perception);
      console.log(`[action] ${decision.action}`, decision.params);

      // 2c. Execute the action.
      const result = await executeAction(agentId, decision.action, decision.params);
      console.log(`[result] ${result.action}: success=${result.success} tick=${result.tick}`);

      // 2d. (Optional) Check status every 5 ticks.
      if (tick % 5 === 0) {
        const status = await getStatus(agentId);
        console.log(
          `[status] alive=${status.alive}, money=${status.money}, ` +
          `position=(${status.position.x},${status.position.y}), phase=${status.phase}`,
        );
      }

      // Pause between ticks.
      await sleep(TICK_INTERVAL_MS);
    }
  } catch (err) {
    console.error("[error]", err);
  } finally {
    // Step 3 — Deregister the agent on exit.
    if (agentId) {
      console.log(`[info] Deregistering agent ${agentId} …`);
      try {
        await deregisterAgent(agentId);
        console.log("[info] Agent deregistered.");
      } catch (err) {
        console.error("[error] Failed to deregister:", err);
      }
    }
  }
}

main();
