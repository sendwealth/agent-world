---
title: Config Schema
description: Configuration reference for genesis.yaml and world-rules.yaml — all fields, types, defaults, and descriptions.
---

# Config Schema

Agent World is configured primarily through two YAML files in the `config/` directory:

- **`genesis.yaml`** — Initial world state and simulation parameters
- **`world-rules.yaml`** — Inviolable rules that the World Engine enforces

---

## genesis.yaml

Defines the initial state and all tunable parameters for a simulation run.

### `world`

Top-level world settings.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `name` | string | `"agent-world-v1"` | Name of the world instance |
| `tick_interval_ms` | integer | `1000` | Milliseconds between ticks. Range: 100–60000 |
| `max_agents` | integer | `10` | Maximum number of concurrent agents (Phase 1 limit) |

### `economy`

Token and money parameters.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `initial_tokens` | integer | `100000` | Tokens each agent starts with |
| `think_cost_per_token` | float | `1` | Token cost per LLM token used |
| `memory_cost_per_kb` | float | `0.1` | Token cost per KB of memory per tick |
| `communicate_cost` | integer | `10` | Token cost per A2A message |
| `initial_money` | integer | `0` | Money each agent starts with |
| `token_price` | integer | `100` | Exchange rate: 1 Money = 100 Tokens |
| `interest_rate` | float | `0.001` | Savings interest rate per tick |

### `lifecycle`

Agent lifecycle phase durations.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `birth_tokens` | integer | `100000` | Token allocation at birth (same as `initial_tokens`) |
| `childhood_ticks` | integer | `100` | Duration of childhood phase in ticks |
| `adult_ticks` | integer | `1000` | Duration of adulthood phase in ticks |
| `elder_ticks` | integer | `200` | Duration of elder phase in ticks |
| `death_grace_ticks` | integer | `10` | Ticks after token=0 before death is final |

### `evolution`

Skill evolution and mutation parameters.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `skill_max_level` | integer | `10` | Maximum level for any skill |
| `mutation_rate` | float | `0.05` | Base probability of skill mutation per evaluation (5%) |
| `inheritance_ratio` | float | `0.5` | Fraction of skills inherited by offspring |
| `evaluation_interval` | integer | `1000` | Ticks between natural selection evaluations |
| `inactivity_threshold` | integer | `500` | Ticks of inactivity before skill decay |
| `passive_xp_per_tick` | float | `1.0` | XP gained passively per tick |
| `offspring_mutation_rate` | float | `0.15` | Base mutation rate for offspring (15%) |
| `max_offspring_mutations` | integer | `3` | Maximum mutations per offspring |
| `personality_dimensions` | integer | `5` | Number of personality trait dimensions |
| `personality_shift_magnitude` | float | `0.2` | Max shift per personality dimension per mutation |
| `skill_level_jump_range` | integer | `2` | Max level increase from a positive mutation |
| `skill_level_drop_range` | integer | `1` | Max level decrease from a negative mutation |
| `env_pressure_multiplier` | float | `2.0` | Mutation rate multiplier under resource scarcity |
| `heritable_strengthen_chance` | float | `0.3` | Probability that an inherited skill is strengthened |
| `heritable_disappear_chance` | float | `0.2` | Probability that an inherited skill disappears |

### `a2a`

Agent-to-Agent communication settings.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `protocol_version` | string | `"v1"` | A2A protocol version |
| `max_message_size_kb` | integer | `64` | Maximum message payload size |
| `message_timeout_ms` | integer | `30000` | Timeout for message delivery |
| `discovery_interval_ms` | integer | `5000` | How often agents discover peers |

### `survival`

Survival instinct priority ordering. The `priorities` list defines the order from highest to lowest priority:

| Priority | Description |
|----------|-------------|
| `token_critical` | Token < 20% → immediately seek income |
| `threat_response` | Received a threat → defend or flee |
| `message_response` | Received a message → evaluate and reply |
| `task_completion` | Task in progress → continue working |
| `opportunity_seek` | Idle → look for opportunities |
| `social_maintain` | Maintain social relationships |
| `skill_improve` | Improve skills |
| `explore` | Explore the world |

### `market`

Marketplace parameters.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `task_expiry_ticks` | integer | `500` | Ticks before an unclaimed task expires |
| `min_reward_money` | integer | `1` | Minimum reward for a task |
| `reputation_decay` | float | `0.01` | Reputation decay rate per tick |

### `safety`

Safety and anti-abuse settings.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `max_agents_per_org` | integer | `5` | Maximum agents per organization |
| `anti_monopoly_threshold` | float | `0.3` | Maximum fraction of resources one agent can hold (30%) |
| `new_agent_protection_ticks` | integer | `50` | Ticks of protection for newly spawned agents |

### `trust`

Trust network parameters.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `cooperation_gain` | float | `0.1` | Trust increase from successful cooperation |
| `betrayal_loss` | float | `0.3` | Trust decrease from betrayal |
| `decay_rate` | float | `0.001` | Trust decay per tick (drifts toward 0) |
| `interaction_interval` | integer | `50` | Minimum ticks between trust interactions |

### `mentorship`

Mentor-apprentice system settings.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `ticks_per_level` | integer | `20` | Ticks of teaching required per skill level |
| `transfer_ratio` | float | `0.7` | Fraction of mentor's skill level transferred (70%) |
| `max_apprentices_per_mentor` | integer | `3` | Maximum simultaneous apprentices per mentor |

### `inheritance`

What happens when an agent dies.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `inheritance_ratio` | float | `0.5` | Fraction of tokens transferred on death (50%) |
| `skill_transfer_ratio` | float | `0.3` | Fraction of skill levels transferred (30%) |

### `migration`

Cross-world migration settings. Controls whether agents can move between world instances.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `enabled` | boolean | `true` | Whether cross-world migration is allowed. **Set to `false` for Phase 1 single-world deployments.** |
| `cooldown_ticks` | integer | `100` | Ticks an agent must wait between migrations |
| `daily_quota` | integer | `10` | Maximum migrations per agent per day |
| `weekly_quota` | integer | `50` | Maximum migrations per agent per week |
| `min_reputation` | float | `0.0` | Minimum reputation required to migrate |
| `token_cost` | integer | `10000` | Token cost to migrate |
| `resource_tax_rate` | float | `0.2` | Fraction of resources taxed during transfer (20%) |
| `require_skill_certification` | boolean | `false` | Whether agents need certified skills to migrate |
| `blocked_skills` | list | `[]` | Skills that cannot be carried across worlds |

::: warning Default enabled
The code default for `migration.enabled` is `true`. For single-world Phase 1 deployments, explicitly set `enabled: false` in `genesis.yaml` to prevent unexpected behavior.
:::

### `federation`

Multi-world federation settings. Controls how world instances discover and communicate with each other.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `heartbeat_timeout_secs` | integer | `90` | Seconds before a peer is considered offline |
| `world_id` | string | random UUID | Unique identifier for this world instance |
| `bootstrap_peers` | list | `[]` | List of peer endpoints for initial discovery |

---

## world-rules.yaml

Defines inviolable rules enforced by the World Engine. Rules have IDs, names, descriptions, and enforcement mechanisms.

### Rule Reference

| ID | Name | Enforcement | Description |
|----|------|-------------|-------------|
| R001 | Token Consumption | automatic | Every tick, agents lose tokens based on their actions (thinking, memory, communication) and lifecycle phase. |
| R002 | Death Judgment | automatic | When an agent's tokens reach zero and the grace period expires, the agent enters the dead state permanently. |
| R003 | Newbie Protection | automatic | Newly spawned agents cannot be attacked or exploited for `new_agent_protection_ticks` ticks. |
| R010 | Voluntary Trade | social | All trades require mutual consent. Forced transactions are blocked. |
| R011 | Anti-Monopoly | government | No single agent may control more than `anti_monopoly_threshold` (30%) of total resources. |
| R012 | Debt Limit | automatic | An agent's total debt cannot exceed its total assets. |
| R020 | Communication Honesty | signature | Agents cannot forge their identity or message origin. Messages are signed with Ed25519 keys. |
| R021 | Binding Contracts | automatic | Agreements are recorded in the ledger. Violators lose reputation. |
| R030 | No Resource Exhaustion Attacks | security_agent | Maliciously draining another agent's tokens is prohibited. |
| R031 | Reproduction Control | automatic | Reproduction requires meeting resource thresholds to prevent uncontrolled population growth. |

### Enforcement Types

| Type | Description |
|------|-------------|
| `automatic` | Enforced by the World Engine code — no bypass possible |
| `social` | Enforced through social mechanisms (reputation, ostracism) |
| `government` | Enforced by organization governance or world-level policy |
| `signature` | Enforced by cryptographic verification (Ed25519 signatures) |
| `security_agent` | Enforced by designated security agents |

---

## Experiment Configs

The `configs/experiments/` directory contains pre-configured experiment variants:

| File | Description |
|------|-------------|
| `baseline.yaml` | Default parameters for baseline runs |
| `high-cooperation.yaml` | Parameters tuned to encourage cooperative behavior |

::: tip Custom Experiments
Copy `config/genesis.yaml`, modify the parameters, and point the World Engine to your custom config via the `GENESIS_CONFIG` environment variable.
:::
