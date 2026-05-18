"""Reflection engine — LLM-driven self-reflection for agent learning.

The reflection module is called periodically by the think loop (every N ticks)
and performs three key functions:

1. **Reflection content generation** — Uses LLM to analyze recent behaviour,
   outcomes, and resource trends to produce a structured self-assessment.
2. **Strategy update** — Adjusts the agent's survival strategy preferences
   based on reflection insights.
3. **Experience writing** — Stores learned experiences into long-term memory
   for future retrieval and decision-making.

Token cost model:
- Each reflection call costs the LLM token consumption (variable) plus a
  fixed overhead of **10 tokens** for processing.

The reflection is skipped entirely when the agent is in PANIC or URGENT
survival mode to conserve tokens.

Usage::

    from agent_runtime.core.reflect import ReflectionEngine, ReflectionConfig
    from agent_runtime.memory.long_term import LongTermMemory

    engine = ReflectionEngine(
        llm_provider=my_llm,
        long_term_memory=LongTermMemory(),
        cost_tracker=my_cost_tracker,
    )
    await engine.reflect(agent_state, tick=100)
"""

from __future__ import annotations

import json
import logging
import re
from dataclasses import dataclass, field
from enum import Enum
from typing import Any, Protocol

from agent_runtime.llm.base import LLMMessage, LLMProvider
from agent_runtime.llm.cost import CostTracker
from agent_runtime.memory.long_term import LongTermMemory
from agent_runtime.memory.short_term import ShortTermMemory
from agent_runtime.memory.working_memory import WorkingMemory
from agent_runtime.models.agent_state import AgentState

logger = logging.getLogger(__name__)

# ---------------------------------------------------------------------------
# Constants
# ---------------------------------------------------------------------------

_REFLECTION_TOKEN_OVERHEAD: int = 10  # Fixed token cost for reflection processing


# ---------------------------------------------------------------------------
# Enums
# ---------------------------------------------------------------------------


class ReflectionCategory(str, Enum):
    """Categories for reflection-generated memories."""

    STRATEGY = "strategy"        # Strategy adjustments
    EXPERIENCE = "experience"    # Learned experiences
    INSIGHT = "insight"          # General insights about the world


# ---------------------------------------------------------------------------
# Configuration
# # ---------------------------------------------------------------------------


@dataclass
class ReflectionConfig:
    """Configuration for the reflection engine.

    Attributes:
        token_overhead: Fixed token cost deducted per reflection.
        max_recent_actions: Number of recent actions to include in prompt.
        max_strategy_memories: Max strategy memories to recall for context.
    """

    token_overhead: int = _REFLECTION_TOKEN_OVERHEAD
    max_recent_actions: int = 10
    max_strategy_memories: int = 5


# ---------------------------------------------------------------------------
# Reflection result
# ---------------------------------------------------------------------------


@dataclass(frozen=True)
class ReflectionResult:
    """Outcome of a single reflection cycle.

    Attributes:
        tick: Tick at which reflection occurred.
        analysis: The LLM-generated analysis text.
        strategy_adjustments: List of strategy adjustments made.
        memories_stored: Number of memories written to long-term store.
        token_cost: Total tokens consumed by this reflection.
        skipped: Whether reflection was skipped (e.g., due to low tokens).
        skip_reason: Reason for skipping, if skipped.
    """

    tick: int
    analysis: str = ""
    strategy_adjustments: list[str] = field(default_factory=list)
    memories_stored: int = 0
    token_cost: int = 0
    skipped: bool = False
    skip_reason: str = ""


# ---------------------------------------------------------------------------
# Agent state protocol (decoupled from concrete AgentState)
# ---------------------------------------------------------------------------


class AgentStateProtocol(Protocol):
    """Minimal interface the reflection engine needs from agent state."""

    @property
    def tokens(self) -> int: ...

    @property
    def max_tokens(self) -> int: ...

    @property
    def name(self) -> str: ...

    @property
    def health(self) -> float: ...

    @property
    def money(self) -> float: ...

    @property
    def reputation(self) -> float: ...

    @property
    def tick(self) -> int: ...


# ---------------------------------------------------------------------------
# Prompt template
# ---------------------------------------------------------------------------

_REFLECTION_PROMPT_TEMPLATE = """\
You are {name}, an autonomous agent reflecting on your recent behaviour.

## Current State (Tick {tick})
- Health: {health:.0f}/100
- Tokens: {tokens}/{max_tokens} ({token_ratio:.0%})
- Money: {money:.1f}
- Reputation: {reputation:.1f}

## Recent Actions
{recent_actions}

## Current Strategy Context
{strategy_context}

## Task
Analyze your recent actions and outcomes. Produce a structured reflection with:
1. What went well and what didn't
2. Strategy adjustments for the future
3. Key experiences worth remembering

Respond with ONLY a JSON object (no markdown):
{{"analysis": "<your self-assessment>", "strategy_adjustments": ["<adjustment1>", ...], \
"experiences": [{{"content": "<experience>", "importance": <0.0-1.0>, "category": \
"<strategy|experience|insight>"}}]}}

Reflect now:"""


def build_reflection_prompt(
    state: AgentState,
    tick: int,
    recent_actions: list[dict[str, Any]],
    strategy_context: str,
) -> str:
    """Build the reflection prompt from agent state and recent history."""
    # Format recent actions
    if recent_actions:
        actions_section = "\n".join(
            f"  - Tick {a.get('tick', '?')}: {a.get('action', '?')} "
            f"-> {a.get('status', '?')}"
            for a in recent_actions[-10:]
        )
    else:
        actions_section = "  No recent actions recorded."

    max_t = state.max_tokens if state.max_tokens > 0 else 1
    token_ratio = state.tokens / max_t

    return _REFLECTION_PROMPT_TEMPLATE.format(
        name=state.name,
        tick=tick,
        health=state.health,
        tokens=state.tokens,
        max_tokens=state.max_tokens,
        token_ratio=token_ratio,
        money=state.money,
        reputation=state.reputation,
        recent_actions=actions_section,
        strategy_context=strategy_context,
    )


# ---------------------------------------------------------------------------
# Response parsing
# ---------------------------------------------------------------------------


def parse_reflection_response(raw: str) -> dict[str, Any]:
    """Parse the LLM reflection response into a structured dict.

    Handles markdown code fences and extracts JSON.

    Returns a dict with keys: analysis, strategy_adjustments, experiences.
    Raises ValueError if parsing fails.
    """
    cleaned = raw.strip()

    # Strip code fences
    if cleaned.startswith("```"):
        cleaned = re.sub(r"^```(?:json)?\s*\n?", "", cleaned, count=1)
        cleaned = re.sub(r"\n?```\s*$", "", cleaned)
        cleaned = cleaned.strip()

    try:
        data = json.loads(cleaned)
    except json.JSONDecodeError as e:
        raise ValueError(f"Failed to parse reflection response as JSON: {e}") from e

    if not isinstance(data, dict):
        raise ValueError("Reflection response must be a JSON object")

    # Ensure required keys with defaults
    data.setdefault("analysis", "")
    data.setdefault("strategy_adjustments", [])
    data.setdefault("experiences", [])

    # Validate types
    if not isinstance(data["analysis"], str):
        data["analysis"] = str(data["analysis"])
    if not isinstance(data["strategy_adjustments"], list):
        data["strategy_adjustments"] = []
    if not isinstance(data["experiences"], list):
        data["experiences"] = []

    # Normalize experiences
    normalized_experiences = []
    for exp in data["experiences"]:
        if isinstance(exp, dict):
            normalized_experiences.append({
                "content": str(exp.get("content", "")),
                "importance": max(0.0, min(1.0, float(exp.get("importance", 0.7)))),
                "category": str(exp.get("category", "experience")),
            })
        elif isinstance(exp, str):
            normalized_experiences.append({
                "content": exp,
                "importance": 0.7,
                "category": "experience",
            })
    data["experiences"] = normalized_experiences

    return data


# ---------------------------------------------------------------------------
# ReflectionEngine
# ---------------------------------------------------------------------------


class ReflectionEngine:
    """LLM-driven self-reflection engine.

    Called periodically by the think loop. Analyzes recent behaviour,
    adjusts strategies, and writes experiences to long-term memory.

    Usage::

        engine = ReflectionEngine(
            llm_provider=my_llm,
            long_term_memory=LongTermMemory(),
            short_term_memory=ShortTermMemory(),
            working_memory=WorkingMemory(),
            cost_tracker=CostTracker(),
        )
        result = await engine.reflect(agent_state, tick=100)
    """

    def __init__(
        self,
        llm_provider: LLMProvider,
        long_term_memory: LongTermMemory,
        *,
        short_term_memory: ShortTermMemory | None = None,
        working_memory: WorkingMemory | None = None,
        cost_tracker: CostTracker | None = None,
        config: ReflectionConfig | None = None,
    ) -> None:
        self._llm = llm_provider
        self._ltm = long_term_memory
        self._stm = short_term_memory
        self._wm = working_memory
        self._cost_tracker = cost_tracker
        self._config = config or ReflectionConfig()
        self._action_history: list[dict[str, Any]] = []

    # ------------------------------------------------------------------
    # Public API
    # ------------------------------------------------------------------

    def record_action(
        self,
        tick: int,
        action: str,
        status: str,
        token_cost: int = 0,
        reasoning: str = "",
    ) -> None:
        """Record an action for future reflection analysis.

        Called by the think loop after each action execution.
        """
        self._action_history.append({
            "tick": tick,
            "action": action,
            "status": status,
            "token_cost": token_cost,
            "reasoning": reasoning,
        })
        # Keep only the most recent actions
        max_keep = self._config.max_recent_actions * 3  # keep 3x for full context
        if len(self._action_history) > max_keep:
            self._action_history = self._action_history[-max_keep:]

    async def reflect(self, state: AgentState, tick: int) -> ReflectionResult:
        """Execute a reflection cycle.

        Steps:
        1. Check if reflection should be skipped (low tokens)
        2. Build reflection prompt
        3. Call LLM for analysis
        4. Parse response
        5. Write experiences to long-term memory
        6. Update strategy preferences
        7. Track token costs

        Args:
            state: Current agent state.
            tick: Current tick number.

        Returns:
            ReflectionResult with the outcome of the reflection.
        """
        # Skip reflection if tokens are critically low (< 20% of max)
        overhead = self._config.token_overhead
        if state.max_tokens > 0 and state.tokens < state.max_tokens * 0.2:
            logger.info(
                "Skipping reflection at tick %d: tokens too low (%d/%d)",
                tick, state.tokens, state.max_tokens,
            )
            return ReflectionResult(
                tick=tick,
                skipped=True,
                skip_reason="Token ratio below 20%, conserving resources",
            )

        # Deduct overhead tokens
        try:
            state.adjust_tokens(-overhead)
        except ValueError:
            return ReflectionResult(
                tick=tick,
                skipped=True,
                skip_reason=f"Not enough tokens for reflection overhead ({overhead})",
            )

        # Gather context
        strategy_context = self._get_strategy_context()
        recent_actions = self._action_history[-self._config.max_recent_actions:]

        # Build prompt
        prompt = build_reflection_prompt(state, tick, recent_actions, strategy_context)

        # Call LLM
        total_tokens_used = overhead
        try:
            response = await self._llm.chat(
                [LLMMessage(role="user", content=prompt)]
            )
            total_tokens_used += response.usage.total_tokens

            # Record LLM cost
            if self._cost_tracker is not None:
                await self._cost_tracker.record(response)

        except Exception:
            logger.exception("Reflection LLM call failed at tick %d", tick)
            return ReflectionResult(
                tick=tick,
                token_cost=total_tokens_used,
                analysis="Reflection failed: LLM call error",
            )

        # Parse response
        try:
            parsed = parse_reflection_response(response.content)
        except ValueError as e:
            logger.warning("Failed to parse reflection response at tick %d: %s", tick, e)
            return ReflectionResult(
                tick=tick,
                token_cost=total_tokens_used,
                analysis="Reflection failed: parse error",
            )

        # Write experiences to long-term memory
        memories_stored = 0
        for exp in parsed.get("experiences", []):
            content = exp["content"]
            if not content:
                continue
            self._ltm.store(
                content=content,
                category=exp.get("category", "experience"),
                importance=exp.get("importance", 0.7),
                source="reflection",
                tick=tick,
                metadata={"analysis_tick": tick},
            )
            memories_stored += 1

        # Store the analysis itself as an insight if it's meaningful
        analysis = parsed.get("analysis", "")
        if analysis and len(analysis) > 20:
            self._ltm.store(
                content=analysis,
                category="insight",
                importance=0.6,
                source="reflection",
                tick=tick,
                metadata={"reflection_tick": tick},
            )
            memories_stored += 1

        # Store strategy adjustments
        strategy_adjustments = parsed.get("strategy_adjustments", [])
        for adj in strategy_adjustments:
            self._ltm.store(
                content=adj,
                category="strategy",
                importance=0.8,
                source="reflection",
                tick=tick,
                metadata={"reflection_tick": tick},
            )

        logger.info(
            "Reflection at tick %d: analysis=%d chars, adjustments=%d, memories=%d, tokens=%d",
            tick,
            len(analysis),
            len(strategy_adjustments),
            memories_stored,
            total_tokens_used,
        )

        return ReflectionResult(
            tick=tick,
            analysis=analysis,
            strategy_adjustments=strategy_adjustments,
            memories_stored=memories_stored,
            token_cost=total_tokens_used,
        )

    # ------------------------------------------------------------------
    # Properties
    # ------------------------------------------------------------------

    @property
    def action_history(self) -> list[dict[str, Any]]:
        """Read-only access to recorded action history."""
        return list(self._action_history)

    @property
    def config(self) -> ReflectionConfig:
        return self._config

    # ------------------------------------------------------------------
    # Private helpers
    # ------------------------------------------------------------------

    def _get_strategy_context(self) -> str:
        """Build a context string from existing strategy memories."""
        if self._stm is not None:
            # Try to recall strategy-related memories from short-term
            try:
                strategies = self._stm.search("strategy", top_k=3)
                if strategies:
                    return "\n".join(
                        f"  - {s.content} (importance: {s.importance:.2f})"
                        for s in strategies
                    )
            except Exception:
                pass

        # Fallback: check long-term memory for strategies
        if self._ltm is not None:
            try:
                strategies = self._ltm.get_recent(top_k=3, category="strategy")
                if strategies:
                    return "\n".join(
                        f"  - {s.content} (importance: {s.importance:.2f})"
                        for s in strategies
                    )
            except Exception:
                pass

        return "  No prior strategy context available."

    def clear_history(self) -> None:
        """Clear the recorded action history."""
        self._action_history.clear()
