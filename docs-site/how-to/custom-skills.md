---
title: Develop Custom Skills
description: Create, register, test, and evolve custom skills for agents in the Agent World skill system.
---

# Develop Custom Skills

Agent World includes a pluggable skill system. This guide shows you how to
create custom skills, register them with the `SkillRegistry`, test them, and
understand how skills evolve over time.

---

## Skill System Overview

The skill system has three core components:

| Component | Role |
|-----------|------|
| **SkillDefinition** | Template describing a skill (name, max level, execute function) |
| **SkillRegistry** | Central registry for all available skill definitions |
| **SkillExecutor** | Executes skills and awards experience points |

### Built-in Skills

The runtime ships with four built-in skills:

| Skill | Category | Description |
|-------|----------|-------------|
| `coding` | technical | Write, analyze, and debug code |
| `trading` | economic | Buy, sell, and negotiate exchanges |
| `research` | knowledge | Investigate and discover information |
| `teaching` | social | Transfer knowledge to other agents |

---

## Skill Interface

Every skill is defined as a `SkillDefinition`:

```python
from agent_runtime.skills import SkillDefinition

SkillDefinition(
    name="my_skill",               # Unique identifier
    description="What this skill does",
    max_level=10,                  # Maximum achievable level (1-10)
    execute_fn=my_execute_fn,      # Callable that performs the skill
    category="general",            # Optional grouping label
)
```

### Execute Function Signature

The execute function receives the agent's full skill dictionary and keyword
arguments:

```python
from typing import Any, Dict
from agent_runtime.models.skill import Skill

def my_execute_fn(
    agent_skills: Dict[str, Skill],
    **kwargs: Any,
) -> Dict[str, Any]:
    """Execute the skill.

    Args:
        agent_skills: All skills the agent possesses (name -> Skill).
        **kwargs: Skill-specific parameters.

    Returns:
        Dict with at least a "success" boolean key.
    """
    my_level = agent_skills.get("my_skill")
    level = my_level.level if my_level else 0

    # ... perform skill logic ...

    return {
        "success": True,
        "result": "...",
    }
```

### Skill Model

Each agent's instance of a skill is a `Skill` object:

```python
class Skill:
    name: str               # Unique skill name
    max_level: int = 10     # Maximum achievable level
    level: int = 1          # Current level (1-based)
    experience: int = 0     # Accumulated XP
    next_level_exp: int = 100  # XP needed for next level
```

Leveling follows a geometric progression: `next_level_exp *= 1.5` on each
level-up.

---

## Creating a New Skill

### Example: Mining Skill

```python
# agent_runtime/skills/mining.py
"""Custom skill: Mining — extract resources from the world."""

from __future__ import annotations

import random
from typing import Any, Dict

from ..models.skill import Skill
from .registry import SkillDefinition


def _execute_mining(agent_skills: Dict[str, Skill], **kwargs: Any) -> Dict[str, Any]:
    """Execute a mining action.

    Kwargs:
        resource_type: Type of resource to mine (iron, gold, coal).
        depth: Mining depth (higher = better yield but more expensive).

    Returns:
        Dict with success status, yield amount, and quality.
    """
    mining = agent_skills.get("mining")
    level = mining.level if mining else 0

    resource_type = kwargs.get("resource_type", "iron")
    depth = kwargs.get("depth", 1)

    # Resource difficulty mapping
    difficulty = {
        "coal": 1,
        "iron": 3,
        "gold": 6,
        "diamond": 9,
    }

    required_level = difficulty.get(resource_type, 1)
    can_mine = level >= required_level

    if not can_mine:
        return {
            "success": False,
            "error": f"Need mining level {required_level} for {resource_type}, have {level}",
            "yield_amount": 0,
            "resource_type": resource_type,
        }

    # Yield scales with level and depth
    base_yield = random.randint(1, 5)
    level_bonus = level * 0.5
    depth_bonus = depth * 0.3
    total_yield = int(base_yield + level_bonus + depth_bonus)

    quality = min(level / 10.0, 1.0)

    return {
        "success": True,
        "resource_type": resource_type,
        "yield_amount": total_yield,
        "quality": quality,
        "level_used": level,
        "depth": depth,
    }


# Export the skill definition
MINING_SKILL = SkillDefinition(
    name="mining",
    description="Extract resources from the world — higher levels unlock rarer materials",
    max_level=10,
    execute_fn=_execute_mining,
    category="gathering",
)
```

---

## Registering with SkillRegistry

### Option 1: Direct Registration

```python
from agent_runtime.skills import SkillRegistry
from agent_runtime.skills.mining import MINING_SKILL

registry = SkillRegistry()
registry.register(MINING_SKILL)

# Verify it's registered
print(registry.has("mining"))  # True
print(registry.count)          # 1

# Create a skill instance for an agent
mining = registry.create_skill("mining", level=2)
print(mining.level)            # 2
print(mining.max_level)        # 10
```

### Option 2: Add to Built-in Registry

```python
from agent_runtime.skills import (
    SkillRegistry,
    BUILTIN_SKILLS,
    create_registry_with_builtins,
)
from agent_runtime.skills.mining import MINING_SKILL

# Start with built-ins, then add custom skills
registry = create_registry_with_builtins()
registry.register(MINING_SKILL)

print(registry.count)  # 5 (4 built-in + 1 custom)
print(registry.categories())  # ['economic', 'gathering', 'knowledge', 'social', 'technical']
```

### Listing and Querying

```python
# List all skills
all_skills = registry.list_skills()
for s in all_skills:
    print(f"  {s.name} (category: {s.category}, max_level: {s.max_level})")

# Filter by category
gathering_skills = registry.list_skills(category="gathering")

# Get a specific definition
mining_def = registry.get("mining")
print(mining_def.description)
```

---

## Executing Skills

```python
from agent_runtime.skills import SkillExecutor, SkillRegistry

registry = create_registry_with_builtins()
registry.register(MINING_SKILL)

executor = SkillExecutor(registry)

# Set up agent's skill set
agent_skills = {
    "mining": registry.create_skill("mining", level=5),
    "trading": registry.create_skill("trading", level=2),
}

# Execute mining
result = executor.execute(
    "mining",
    agent_skills,
    resource_type="gold",
    depth=3,
)

print(f"XP earned: {result.xp_earned}")       # 40 (10 USE + 30 SUCCESS)
print(f"Leveled up: {result.leveled_up}")      # True/False
print(f"Output: {result.output}")
# Output: {"success": True, "yield_amount": 7, "quality": 0.5, ...}

# Check updated skill
print(agent_skills["mining"].experience)  # 40
print(agent_skills["mining"].level)       # 5 (or 6 if enough XP)
```

### XP Breakdown

```python
print(result.xp_breakdown)
# {"use": 10, "success": 30}
# or for teaching: {"use": 10, "success": 30, "teaching": 50}
```

---

## Testing Skills

### Unit Test Example

```python
# tests/test_mining_skill.py
import pytest
from agent_runtime.skills import SkillRegistry, SkillExecutor
from agent_runtime.skills.mining import MINING_SKILL
from agent_runtime.models.skill import Skill


@pytest.fixture
def registry():
    reg = SkillRegistry()
    reg.register(MINING_SKILL)
    return reg


@pytest.fixture
def executor(registry):
    return SkillExecutor(registry)


def test_mining_skill_registered(registry):
    assert registry.has("mining")
    defn = registry.get("mining")
    assert defn.max_level == 10
    assert defn.category == "gathering"


def test_mining_coal_at_level_1(executor):
    """Level 1 can mine coal (difficulty 1)."""
    agent_skills = {"mining": Skill(name="mining", level=1)}
    result = executor.execute("mining", agent_skills, resource_type="coal")
    assert result.output["success"] is True
    assert result.output["resource_type"] == "coal"
    assert result.xp_earned >= 10  # At least USE XP


def test_mining_diamond_requires_high_level(executor):
    """Level 2 cannot mine diamond (difficulty 9)."""
    agent_skills = {"mining": Skill(name="mining", level=2)}
    result = executor.execute("mining", agent_skills, resource_type="diamond")
    assert result.output["success"] is False
    assert "Need mining level 9" in result.output["error"]


def test_mining_levels_up(executor):
    """Mining many times should eventually level up."""
    agent_skills = {"mining": Skill(name="mining", level=1)}
    leveled = False
    for _ in range(20):
        result = executor.execute("mining", agent_skills, resource_type="coal")
        if result.leveled_up:
            leveled = True
            break
    assert leveled, "Agent should have leveled up after 20 successful mines"
```

Run tests:

```bash
cd agent-runtime && pytest tests/test_mining_skill.py -v
```

---

## Evolution and Mutation of Skills

The World Engine includes an **EvolutionSubsystem** that manages skill
evolution across agent generations:

### Natural Selection Mechanics

- **Passive XP**: Agents earn passive XP per tick (configurable in genesis.yaml)
- **Mutation**: When agents reproduce, offspring skills may mutate:
  - **Boost**: Skill level jumps up (XP bonus +75)
  - **Decay**: Skill level drops (XP reduction -30)
  - **New Skill**: Offspring gains a new skill not held by parents
- **Environmental Pressure**: Harsh conditions increase mutation rates

### Configuration in genesis.yaml

```yaml
evolution:
  skill_max_level: 10
  mutation_rate: 0.1
  evaluation_interval: 100
  passive_xp_per_tick: 1
  offspring_mutation_rate: 0.15
  max_offspring_mutations: 3
  personality_dimensions: 5
  personality_shift_magnitude: 0.1
  skill_level_jump_range: 2
  skill_level_drop_range: 1
  env_pressure_multiplier: 1.5
  heritable_strengthen_chance: 0.6
  heritable_disappear_chance: 0.1
  inactivity_threshold: 50
```

### Updating Skill Definitions at Runtime

```python
from agent_runtime.skills import SkillDefinition

# Upgrade an existing skill definition
registry.upgrade(SkillDefinition(
    name="mining",
    description="Enhanced mining — now supports crystal extraction",
    max_level=15,  # Raised from 10
    execute_fn=_execute_mining_v2,
    category="gathering",
))
```

::: warning
Use `registry.upgrade()` to update existing definitions. Calling
`registry.register()` with a name that's already registered raises a
`ValueError`.
:::

---

## Next Steps

- [Configure an Agent](/how-to/configure-agent) — Assign skills to agents
- [Use A2A Protocol](/how-to/a2a-protocol) — Enable skill teaching between agents
- [Monitor Agents](/how-to/monitor-agents) — Track skill progression in the Dashboard
