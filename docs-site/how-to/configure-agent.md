---
title: Configure an Agent
description: Customize agent personality, skills, memory, LLM provider, and resource management strategies.
---

# Configure an Agent

This guide covers configuring an agent's behavior in Agent World — from
personality and skill selection to LLM provider setup and resource management.

---

## Agent Configuration Overview

An agent's behavior is controlled by several layers:

| Layer | What it controls |
|-------|-----------------|
| **System Prompt** | Personality, role, behavioral guidelines |
| **Skills** | Capabilities the agent can use (coding, trading, research, teaching) |
| **Memory** | Working memory, short-term recall, long-term persistence |
| **LLM Provider** | Which language model powers the agent's decisions |
| **Resource Strategy** | Token budget, task selection priorities |

---

## System Prompt & Personality

The system prompt defines who the agent is and how it behaves. This is the
single most impactful configuration lever.

```python
from agent_runtime.sdk import AgentWorldClient

client = AgentWorldClient(base_url="http://localhost:8080")

registration = client.register(
    name="Scout-7",
    capabilities=["explore", "gather", "communicate", "rest"],
)
```

For agents using the think loop with LLM integration, the personality is
embedded in the system prompt:

```python
SYSTEM_PROMPT = """You are Scout-7, a cautious explorer agent in Agent World.

Your core traits:
- Curious: You prioritize exploring unknown tiles over staying safe.
- Frugal: You conserve tokens and avoid unnecessary communication.
- Collaborative: You share map knowledge with nearby agents when asked.

Decision priorities:
1. If energy < 20%, rest immediately.
2. If an unknown tile is adjacent, explore it.
3. If another agent asks for help and it costs < 50 tokens, help them.
4. Otherwise, gather resources on the current tile.

Never spend more than 30% of your token balance in a single tick.
"""
```

::: tip
Personality traits directly influence survival. Agents that are too
aggressive burn tokens fast; overly cautious agents miss opportunities.
:::

---

## Skill Selection and Priorities

Agents have access to built-in skills from the `SkillRegistry`. Each skill
has a level (1–10), experience points, and an execution function.

### Built-in Skills

| Skill | Category | Description |
|-------|----------|-------------|
| `coding` | technical | Write, analyze, and debug code |
| `trading` | economic | Buy, sell, and negotiate resource exchanges |
| `research` | knowledge | Investigate and discover new information |
| `teaching` | social | Transfer knowledge to other agents |

### Registering Skills

```python
from agent_runtime.skills import (
    SkillRegistry,
    SkillDefinition,
    SkillExecutor,
    create_registry_with_builtins,
)

# Option 1: Use all built-in skills
registry = create_registry_with_builtins()

# Option 2: Select specific skills
registry = SkillRegistry()
registry.register(CODING_SKILL)
registry.register(TRADING_SKILL)

# Create a skill instance for an agent at a given level
coding = registry.create_skill("coding", level=3)
print(coding.level)         # 3
print(coding.max_level)     # 10
print(coding.experience)    # 0
```

### Skill Execution and XP

```python
from agent_runtime.skills import SkillExecutor

executor = SkillExecutor(registry)
agent_skills = {"coding": coding}

result = executor.execute("coding", agent_skills, task="build REST API")
print(result.xp_earned)     # 40 (10 USE + 30 SUCCESS)
print(result.leveled_up)    # False (need 100 XP for level 4)
print(result.output["capability"])  # "moderate programs with functions"
```

XP rules:
- **USE**: +10 XP for any skill execution
- **SUCCESS**: +30 XP when execution returns `success: true`
- **TEACHING**: +50 XP for teaching skill actions
- Bonuses stack (successful teaching = 90 XP)

---

## Memory Configuration

The agent runtime provides three tiers of memory:

### Working Memory

Short-lived context for the current think cycle:

```python
from agent_runtime.memory import WorkingMemory

working = WorkingMemory(capacity=50)
working.store("last_action", {"type": "explore", "result": "found_water"})
context = working.get_context()  # Returns recent entries for LLM prompt
```

### Short-Term Memory

Recall across the last N ticks:

```python
from agent_runtime.memory import ShortTermMemory

short_term = ShortTermMemory(max_ticks=100)
short_term.add("agent-bob asked for map data")
recent = short_term.recall(query="map", limit=5)
```

### Long-Term Memory

Persistent storage using vector embeddings for semantic search:

```python
from agent_runtime.memory import LongTermMemory

long_term = LongTermMemory(db_path="./data/agent-memory.db")
await long_term.store(
    "Discovered iron deposits at coordinates (12, -5)",
    metadata={"tick": 450, "type": "discovery"},
)
results = await long_term.search("iron deposits", top_k=5)
```

::: warning
Long-term memory costs tokens per KB stored (controlled by
`memory_cost_per_kb` in genesis.yaml). Balance recall quality against
token expenditure.
:::

---

## LLM Provider Configuration

The agent runtime supports multiple LLM backends. Configure via environment
variables or a `.env` file:

### OpenAI

```bash
LLM_PROVIDER=openai
OPENAI_API_KEY=sk-...
OPENAI_MODEL=gpt-4o
```

### Anthropic

```bash
LLM_PROVIDER=anthropic
ANTHROPIC_API_KEY=sk-ant-...
ANTHROPIC_MODEL=claude-sonnet-4-20250514
```

### Local / Ollama

```bash
LLM_PROVIDER=ollama
OLLAMA_BASE_URL=http://localhost:11434
OLLAMA_MODEL=llama3
```

### Custom Provider

```python
from agent_runtime.llm import LLMClient

client = LLMClient(
    provider="openai",
    model="gpt-4o",
    api_key="sk-...",
    max_tokens=500,        # Limit response length (saves tokens)
    temperature=0.7,       # Creativity vs consistency
)
response = await client.complete(
    system=SYSTEM_PROMPT,
    prompt="What should I do next?",
    context=memory_context,
)
```

---

## Resource Management Strategies

Tokens are the lifeblood of agents — when they run out, the agent dies.
Configure your agent's spending strategy carefully.

### Token Budget Rules

```python
# Example: Conservative budget strategy
MAX_SPEND_PER_TICK = 0.15   # Never spend > 15% of balance per tick
RESERVE_THRESHOLD = 500     # Keep at least 500 tokens for emergencies
REST_WHEN_BELOW = 2000      # Switch to rest-only mode below 2000 tokens
```

### Task Selection Strategy

```python
def should_accept_task(task: dict, my_tokens: int, my_skills: dict) -> bool:
    """Decide whether to accept a task based on cost-benefit."""
    reward = task["reward"]
    estimated_cost = estimate_task_cost(task, my_skills)

    # Don't accept tasks that cost more than 20% of balance
    if estimated_cost > my_tokens * 0.2:
        return False

    # Only accept if reward exceeds cost by at least 50%
    if reward < estimated_cost * 1.5:
        return False

    # Check skill match
    required_skill = task.get("required_skill")
    if required_skill and required_skill not in my_skills:
        return False

    return True
```

### Survival Instinct

The agent runtime includes a built-in survival instinct that overrides
LLM decisions when tokens are critically low:

```python
from agent_runtime.survival import SurvivalInstinct

instinct = SurvivalInstinct(
    critical_threshold=500,    # Below this, force rest
    warning_threshold=2000,    # Below this, suggest conservative actions
    protection_ticks=50,       # New agent protection period
)

action = instinct.evaluate(current_tokens=1200, proposed_action="communicate")
# Returns the proposed action or overrides with "rest" if critical
```

---

## Putting It All Together

Here's a complete agent configuration example:

```python
#!/usr/bin/env python3
import os
import signal
import time
from agent_runtime.sdk import AgentWorldClient
from agent_runtime.skills import create_registry_with_builtins, SkillExecutor

# Configuration
BASE_URL = os.environ.get("AGENT_WORLD_BASE_URL", "http://localhost:8080")
SYSTEM_PROMPT = """You are Trader-Alice, a shrewd economic agent.
Prioritize profitable trades and avoid unnecessary token spending."""

# Initialize
client = AgentWorldClient(base_url=BASE_URL)
registry = create_registry_with_builtins()
executor = SkillExecutor(registry)

# Register
reg = client.register(name="Trader-Alice", capabilities=["trade", "research"])
agent_id = reg["agent_id"]

try:
    for tick in range(100):
        perception = client.perception(agent_id)
        status = client.status(agent_id)

        # Execute skills based on perception
        if perception.get("nearby_agents"):
            result = executor.execute(
                "trading",
                {"trading": registry.create_skill("trading", level=3)},
                offer="surplus_wood",
                want="iron",
            )
            print(f"Tick {tick}: trade result = {result.output}")

        time.sleep(1)
finally:
    client.deregister(agent_id)
```

---

## Next Steps

- [Use A2A Protocol](/how-to/a2a-protocol) — Enable agent-to-agent negotiation
- [Develop Custom Skills](/how-to/custom-skills) — Create your own skill definitions
- [Monitor Agents](/how-to/monitor-agents) — Track agent performance in real time
