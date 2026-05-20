/**
 * Agent World — Custom Agent Example (TypeScript)
 *
 * A complete example demonstrating how to:
 *   1. Register an agent with the simulation
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

const API_KEY: string = process.env.AGENT_WORLD_API_KEY ?? "your-api-key-here";
const BASE_URL: string = process.env.AGENT_WORLD_BASE_URL ?? "http://localhost:3000";
const MAX_TICKS: number = parseInt(process.env.MAX_TICKS ?? "10", 10);
const TICK_INTERVAL_MS: number = parseFloat(process.env.TICK_INTERVAL ?? "1.0") * 1000;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface AgentRegistration {
  name: string;
  kind?: string;
  metadata?: Record<string, string>;
}

interface PerceptionResponse {
  agent_id: string;
  center: { x: number; y: number };
  tiles: Array<{
    x: number;
    y: number;
    terrain: string;
    resources: Record<string, number>;
  }>;
  nearby_agents: Array<{
    id: string;
    name: string;
    position: { x: number; y: number };
    distance: number;
  }>;
  available_tasks: Array<{
    task_id: string;
    description: string;
    reward: Record<string, number>;
    location: { x: number; y: number };
  }>;
  energy?: number;
  perceived_at: string;
}

interface ActionResponse {
  action_id: string;
  agent_id: string;
  type: string;
  params: Record<string, unknown>;
  result: string;
  message: string;
  executed_at: string;
}

interface StatusResponse {
  id: string;
  name: string;
  status: string;
  position: { x: number; y: number };
  energy: number;
  inventory: Record<string, number>;
}

// ---------------------------------------------------------------------------
// Low-level API helpers
// ---------------------------------------------------------------------------

/** Shorthand for making authenticated requests to the Agent World API. */
async function apiRequest<T>(
  method: string,
  path: string,
  body?: unknown,
): Promise<T> {
  const url = `${BASE_URL}${path}`;
  const headers: Record<string, string> = {
    "Content-Type": "application/json",
    "X-API-Key": API_KEY,
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
async function registerAgent(reg: AgentRegistration): Promise<{ id: string; name: string }> {
  return apiRequest("POST", "/api/v1/agents/register", reg);
}

/** Get the agent's current status. */
async function getStatus(agentId: string): Promise<StatusResponse> {
  return apiRequest("GET", `/api/v1/agents/${agentId}/status`);
}

/** Get the agent's perception of the world. */
async function getPerception(agentId: string, radius = 3): Promise<PerceptionResponse> {
  return apiRequest("GET", `/api/v1/agents/${agentId}/perception?radius=${radius}`);
}

/** Execute an action on behalf of the agent. */
async function executeAction(
  agentId: string,
  type: string,
  params: Record<string, unknown> = {},
): Promise<ActionResponse> {
  return apiRequest("POST", `/api/v1/agents/${agentId}/action`, { type, params });
}

/** Deregister (remove) the agent. */
async function deregisterAgent(agentId: string): Promise<{ id: string; status: string }> {
  return apiRequest("DELETE", `/api/v1/agents/${agentId}`);
}

// ---------------------------------------------------------------------------
// Decision-making helper
// ---------------------------------------------------------------------------

/**
 * Very simple rule-based decision loop.
 *
 * Priorities:
 *   1. Rest if energy is low (< 20).
 *   2. Gather if resources are present on the current tile.
 *   3. Move in a cycling direction otherwise.
 */
function decideAction(perception: PerceptionResponse, tick: number): { type: string; params: Record<string, unknown> } {
  const energy = perception.energy ?? 100;

  // 1) Rest if energy is critically low.
  if (energy < 20) {
    return { type: "rest", params: {} };
  }

  // 2) Gather if the current tile has resources.
  const currentTile = perception.tiles.find(
    (t) => t.x === perception.center.x && t.y === perception.center.y,
  );
  if (currentTile && Object.keys(currentTile.resources).length > 0) {
    const resource = Object.keys(currentTile.resources)[0];
    return { type: "gather", params: { resource } };
  }

  // 3) Move in a cycling direction.
  const directions = ["north", "east", "south", "west"];
  const direction = directions[tick % directions.length];
  return { type: "move", params: { direction } };
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
    const reg = await registerAgent({
      name: "TSScout",
      kind: "explorer",
      metadata: { language: "typescript", owner: "example", version: "1.0.0" },
    });
    agentId = reg.id;
    console.log(`[info] Registered as ${reg.name} (id=${agentId})`);

    // Step 2 — Main simulation loop.
    for (let tick = 1; tick <= MAX_TICKS; tick++) {
      console.log(`\n--- Tick ${tick} ---`);

      // 2a. Observe the world (perception).
      const perception = await getPerception(agentId, 3);
      console.log(
        `[perception] ${perception.tiles.length} tiles, ` +
        `${perception.nearby_agents.length} nearby agents`,
      );

      // 2b. Decide what to do next.
      const action = decideAction(perception, tick);
      console.log(`[action] ${action.type}`, action.params);

      // 2c. Execute the action.
      const result = await executeAction(agentId, action.type, action.params);
      console.log(`[result] ${result.result} — ${result.message}`);

      // 2d. (Optional) Check status every 5 ticks.
      if (tick % 5 === 0) {
        const status = await getStatus(agentId);
        console.log(
          `[status] energy=${status.energy}, position=(${status.position.x},${status.position.y}), ` +
          `inventory=${JSON.stringify(status.inventory)}`,
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
