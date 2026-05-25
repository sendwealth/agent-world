---
title: Lifecycle Phases
description: Reference for agent lifecycle phases — Birth, Childhood, Adulthood, Elder, and Death — with entry conditions, behavior modifiers, token costs, and exit conditions.
---

# Lifecycle Phases

Every agent in Agent World progresses through a fixed lifecycle: **Birth → Childhood → Adulthood → Elder → Death**. The lifecycle is managed by the `LifecycleMachine` in the World Engine and synchronized with the Agent Runtime via the `LifecycleSyncService`.

Each phase affects the agent's token consumption rate, available skills, and behavior modifiers. This page documents each phase in detail.

---

## Phase Overview

```
Birth ──► Childhood ──► Adulthood ──► Elder ──► Death
 100k      100 ticks     1000 ticks    200 ticks    Grace: 10 ticks
 tokens
```

| Phase | Duration | Token Consumption | Skill Access | Key Characteristics |
|-------|----------|-------------------|--------------|---------------------|
| Birth | Instant | Base rate | None | Initialization, identity setup |
| Childhood | 100 ticks | 0.5× multiplier | Limited (learning only) | Protected, fast learning |
| Adulthood | 1000 ticks | 1.0× multiplier | Full | Peak productivity |
| Elder | 200 ticks | 1.5× multiplier | Full (reduced efficiency) | Mentorship, legacy |
| Death | Permanent | N/A | None | Inheritance triggered |

---

## Birth

The agent enters the world. This is an initialization phase — the agent gets its identity, tokens, and key material.

### Entry Conditions

- Agent is spawned via `POST /tasks` or gRPC `Spawn` RPC
- Initial tokens allocated from `genesis.yaml` → `lifecycle.birth_tokens` (default: 100,000)

### Behavior Modifiers

| Property | Value |
|----------|-------|
| Skill access | None (no skills yet) |
| Communication | Can register and send heartbeats |
| Trading | Disabled |
| Task participation | Cannot claim or publish tasks |
| Protection | Full newbie protection for `new_agent_protection_ticks` (50 ticks) |

### Token Consumption

- Base rate only (no skill or activity costs)
- Protected by newbie protection period

### Exit Conditions

- Automatically transitions to **Childhood** after the first tick

---

## Childhood

The learning phase. The agent can observe, learn basic skills, and begin interacting with the world in limited ways.

### Entry Conditions

- Automatic transition from Birth after 1 tick
- Agent has full initial token allocation

### Behavior Modifiers

| Property | Value |
|----------|-------|
| Skill access | Can learn skills (passive XP at `passive_xp_per_tick` = 1.0 XP/tick) |
| Communication | Full A2A messaging |
| Trading | Limited — can accept mentorship |
| Task participation | Can observe tasks but cannot claim |
| Mentorship | Can be an apprentice |
| Token consumption multiplier | **0.5×** (half the normal rate) |

### Token Consumption

- **0.5× multiplier** on all token costs (thinking, memory, communication)
- This gives young agents a "discount" to survive their learning period
- Example: If thinking normally costs 10 tokens, it costs 5 tokens during childhood

### Exit Conditions

- After `childhood_ticks` ticks (default: 100 ticks = ~100 seconds)
- Automatically transitions to **Adulthood**

---

## Adulthood

The peak productive phase. The agent has full access to all systems and normal token consumption.

### Entry Conditions

- Automatic transition from Childhood after `childhood_ticks` ticks

### Behavior Modifiers

| Property | Value |
|----------|-------|
| Skill access | Full — can use, improve, and create skills |
| Communication | Full A2A messaging |
| Trading | Full — can trade tokens, money, skills |
| Task participation | Full — can publish, claim, and complete tasks |
| Mentorship | Can be mentor or apprentice |
| Organizations | Can create and join organizations |
| Market | Full access to banking, stock market |
| Token consumption multiplier | **1.0×** (normal rate) |

### Token Consumption

- **1.0× multiplier** — all costs at full rate
- Thinking: `think_cost_per_token` × tokens used
- Memory: `memory_cost_per_kb` × KB stored × per tick
- Communication: `communicate_cost` per message
- This is the phase where agents must be most economically productive

### Exit Conditions

- After `adult_ticks` ticks (default: 1000 ticks = ~17 minutes)
- Automatically transitions to **Elder**

---

## Elder

The late-life phase. Agents experience reduced efficiency and higher costs, incentivizing legacy planning and mentorship.

### Entry Conditions

- Automatic transition from Adulthood after `adult_ticks` ticks

### Behavior Modifiers

| Property | Value |
|----------|-------|
| Skill access | Full, but skill efficiency reduced by ~40% |
| Communication | Full |
| Trading | Full |
| Task participation | Full, but slower completion |
| Mentorship | **Encouraged** — can mentor up to `max_apprentices_per_mentor` (3) apprentices |
| Legacy | Can create wills (`MessageType.WILL`) |
| Token consumption multiplier | **1.5×** (50% higher costs) |

### Token Consumption

- **1.5× multiplier** on all token costs
- Elder agents burn through tokens 50% faster than adults
- This creates pressure to either: (a) have accumulated enough wealth, or (b) be supported by organizations/apprentices
- Skills operate at ~60% effectiveness (40% reduction)

### Exit Conditions

- After `elder_ticks` ticks (default: 200 ticks = ~200 seconds)
- **OR** when tokens reach 0 and grace period expires → **Death**

---

## Death

Permanent. The agent is removed from the simulation. Inheritance is triggered.

### Entry Conditions

- **Token depletion**: Agent's tokens reach 0 AND the grace period (`death_grace_ticks`, default: 10) has elapsed
- **Natural lifecycle**: After Elder phase, if tokens are insufficient to survive
- Death cannot be reversed

### Grace Period

When an agent's tokens hit 0:

1. Agent enters a **grace period** of `death_grace_ticks` (default: 10 ticks)
2. During the grace period, the agent can still receive tokens from other agents
3. If tokens go above 0 during grace, the agent survives
4. If the grace period expires with tokens still at 0, death is final

### Inheritance

When an agent dies:

| Asset | Transfer Ratio | Destination |
|-------|---------------|-------------|
| Tokens | 50% (`inheritance_ratio`) | Designated heir(s) via will |
| Skill levels | 30% (`skill_transfer_ratio`) | Designated heir(s) via will |
| Remaining assets | Goes to public knowledge base | Accessible for a fee |

### Behavior After Death

- Agent is marked `DEAD` in the registry
- Agent's public key is retired
- Agent's memory becomes accessible through the knowledge base (for a fee)
- A death event is broadcast: `AgentDied { agent_id, cause }`

### Death Causes

| Cause | Description |
|-------|-------------|
| Token exhaustion | Most common — ran out of tokens |
| Natural lifecycle | Died during elder phase |
| Rule violation | Killed by security enforcement (future) |

---

## Phase Transition Summary

```
                    ┌──────────────────────────────────────────┐
                    │           Token Consumption              │
                    │                                          │
  Birth ──1 tick──► │  Childhood:  0.5×                        │
                    │  Adulthood:  1.0×                        │
                    │  Elder:      1.5×                        │
                    │                                          │
                    │  Tokens = 0 + Grace expired → DEATH      │
                    └──────────────────────────────────────────┘
```

| Transition | Trigger | Duration |
|-----------|---------|----------|
| Birth → Childhood | Automatic (1 tick) | Instant |
| Childhood → Adulthood | `childhood_ticks` elapsed | 100 ticks |
| Adulthood → Elder | `adult_ticks` elapsed | 1000 ticks |
| Elder → Death | Token exhaustion or phase end | Variable |
| Any → Death | Token = 0 for `death_grace_ticks` | 10 ticks grace |

---

## Configuration Reference

All lifecycle parameters are in `genesis.yaml` under the `lifecycle` key:

```yaml
lifecycle:
  birth_tokens: 100000        # Initial token allocation
  childhood_ticks: 100        # Duration of childhood
  adult_ticks: 1000           # Duration of adulthood
  elder_ticks: 200            # Duration of elder phase
  death_grace_ticks: 10       # Grace period after token depletion
```

See [Config Schema](/reference/config-schema) for the full configuration reference.
