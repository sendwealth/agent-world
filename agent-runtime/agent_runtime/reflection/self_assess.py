"""Self-assessment — LLM-driven reflection and strategy evolution.

The :class:`ReflectionEngine` implements the ``ReflectionProvider`` protocol
from ``agent_runtime.core.think_loop``.  Every *N* ticks (configurable,
default 10) it:

1. Collects recent action history and outcomes from the ActionExecutor.
2. Builds a reflection prompt summarising behaviour and resource trends.
3. Calls an LLM to evaluate strategy effectiveness.
4. Parses the LLM response into a :class:`StrategyAdjustment`.
5. Records the adjustment in the memory system for future decisions.
6. Deducts a configurable token cost from the agent.

If the LLM call fails, the engine falls back to a rule-based assessment
that analyses success/failure ratios and resource trends without any LLM
invocation.

Usage::

    from agent_runtime.reflection.self_assess import ReflectionEngine, ReflectionEngineConfig

    engine = ReflectionEngine(
        config=ReflectionEngineConfig(token_cost=15),
        memory=short_term_memory,
    )
    # Plug into ThinkLoop:
    loop = ThinkLoop(..., reflection_provider=engine)
"""

from __future__ import annotations

import json
import logging
import re
from collections import deque
from dataclasses import dataclass, field
from enum import StrEnum
from typing import Protocol

from agent_runtime.core.act import ActionResult, ActionStatus
from agent_runtime.llm.base import LLMMessage, LLMProvider
from agent_runtime.memory.short_term import ShortTermMemoryProtocol
from agent_runtime.models.agent_state import AgentState

logger = logging.getLogger(__name__)

# ---------------------------------------------------------------------------
# Constants
# ---------------------------------------------------------------------------

#: Maximum number of adjustment history entries retained.
_MAX_ADJUSTMENT_HISTORY = 1000

#: Maximum length for sanitised agent names in prompts.
_MAX_NAME_LENGTH = 64


# ---------------------------------------------------------------------------
# Data types
# ---------------------------------------------------------------------------


class BehaviorStrategy(StrEnum):
    """High-level strategy modes an agent can adopt.

    These influence decision-making by biasing action selection.
    """

    CONSERVATIVE = "conservative"   # Prioritise safety, minimise costs
    BALANCED = "balanced"           # Neutral, explore equally
    AGGRESSIVE = "aggressive"       # Maximise gains, accept higher costs
    SOCIAL = "social"               # Focus on communication & collaboration
    EXPLORATORY = "exploratory"     # Prioritise learning & discovery


# Default strategy weights for each mode — used by decision providers
# to bias action selection.  Maps action-type value to a weight multiplier.
_STRATEGY_WEIGHTS: dict[BehaviorStrategy, dict[str, float]] = {
    BehaviorStrategy.CONSERVATIVE: {
        "rest": 2.0,
        "explore": 0.5,
        "send_message": 0.8,
        "claim_task": 0.6,
        "submit_task": 1.0,
        "propose_deal": 0.3,
        "teach_skill": 0.5,
    },
    BehaviorStrategy.BALANCED: {
        "rest": 1.0,
        "explore": 1.0,
        "send_message": 1.0,
        "claim_task": 1.0,
        "submit_task": 1.0,
        "propose_deal": 1.0,
        "teach_skill": 1.0,
    },
    BehaviorStrategy.AGGRESSIVE: {
        "rest": 0.3,
        "explore": 1.5,
        "send_message": 1.0,
        "claim_task": 2.0,
        "submit_task": 1.5,
        "propose_deal": 2.0,
        "teach_skill": 1.2,
    },
    BehaviorStrategy.SOCIAL: {
        "rest": 0.8,
        "explore": 0.8,
        "send_message": 2.5,
        "claim_task": 0.8,
        "submit_task": 1.0,
        "propose_deal": 1.5,
        "teach_skill": 2.0,
    },
    BehaviorStrategy.EXPLORATORY: {
        "rest": 0.5,
        "explore": 2.5,
        "send_message": 1.0,
        "claim_task": 1.2,
        "submit_task": 0.8,
        "propose_deal": 0.8,
        "teach_skill": 1.0,
    },
}


@dataclass(frozen=True)
class StrategyAdjustment:
    """A strategy change produced by the reflection process.

    Attributes:
        previous_strategy: The strategy that was active before reflection.
        new_strategy: The strategy adopted after reflection.
        reasoning: Why the change was made.
        confidence: Confidence in the adjustment (0-100).
        resource_delta: Change in token balance over the analysed window.
        success_rate: Fraction of successful actions in the window (0.0-1.0).
        action_counts: How many times each action type was executed.
        tick: The tick at which this adjustment was made.
    """

    previous_strategy: BehaviorStrategy
    new_strategy: BehaviorStrategy
    reasoning: str
    confidence: int = 50
    resource_delta: int = 0
    success_rate: float = 0.0
    action_counts: dict[str, int] = field(default_factory=dict)
    tick: int = 0


@dataclass(frozen=True)
class ReflectionResult:
    """Complete output of a reflection cycle.

    Attributes:
        adjustment: The strategy adjustment (may be no-op if strategy unchanged).
        token_cost: Tokens consumed by this reflection.
        method: How the reflection was performed ("llm" or "rule_based").
        memory_stored: Whether the result was persisted to memory.
    """

    adjustment: StrategyAdjustment
    token_cost: int
    method: str
    memory_stored: bool


# ---------------------------------------------------------------------------
# Configuration
# ---------------------------------------------------------------------------


@dataclass
class ReflectionEngineConfig:
    """Configuration for the reflection engine.

    Attributes:
        token_cost: Token cost per reflection cycle.
        analysis_window: Number of recent actions to analyse.
        default_strategy: Starting strategy for new agents.
        memory_importance: Importance score for reflection memories (0.0-1.0).
        max_history: Maximum number of strategy adjustments to retain.
        dominance_threshold: Fraction of actions that must be one type to
            trigger a strategy switch (0.0-1.0).
        low_success_threshold: Success rate below which the agent switches
            to conservative (0.0-1.0).
        high_success_threshold: Success rate above which the agent may
            switch to aggressive (0.0-1.0).
        spending_threshold: Token spending delta below which the agent is
            considered to be actively spending (negative value).
        rule_based_confidence: Default confidence for rule-based adjustments.
    """

    token_cost: int = 20
    analysis_window: int = 50
    default_strategy: BehaviorStrategy = BehaviorStrategy.BALANCED
    memory_importance: float = 0.8
    max_history: int = _MAX_ADJUSTMENT_HISTORY
    dominance_threshold: float = 0.4
    low_success_threshold: float = 0.3
    high_success_threshold: float = 0.8
    spending_threshold: int = -50
    rule_based_confidence: int = 70


# ---------------------------------------------------------------------------
# Metrics dataclass (module-level for reusability)
# ---------------------------------------------------------------------------


@dataclass(frozen=True)
class _Metrics:
    """Computed metrics from recent actions."""

    action_count: int = 0
    success_rate: float = 0.0
    resource_delta: int = 0
    action_counts: dict[str, int] = field(default_factory=dict)


# ---------------------------------------------------------------------------
# Protocols
# ---------------------------------------------------------------------------


class ActionHistoryProvider(Protocol):
    """Provides access to recent action execution history."""

    @property
    def history(self) -> list[ActionResult]: ...


# ---------------------------------------------------------------------------
# Prompt template
# ---------------------------------------------------------------------------

_REFLECTION_PROMPT = """\
You are {name}, an autonomous agent reflecting on your recent behaviour.

## Current State (Tick {tick})
- Health: {health:.0f}/100
- Tokens: {tokens} / {max_tokens}
- Money: {money:.1f}
- Reputation: {reputation:.1f}

## Recent Activity Summary
- Actions analysed: {action_count}
- Success rate: {success_rate:.0%}
- Token balance change: {resource_delta:+d}
- Action breakdown: {action_breakdown}

## Current Strategy: {current_strategy}

## Available Strategies
- conservative: Minimise costs, prioritise safety
- balanced: Neutral approach, explore equally
- aggressive: Maximise gains, accept higher costs
- social: Focus on communication & collaboration
- exploratory: Prioritise learning & discovery

Based on your recent performance, evaluate your current strategy and \
recommend whether to keep it or switch. Consider:
1. Are you running out of tokens too fast? → lean toward conservative
2. Are you successful at tasks and deals? → stay aggressive or social
3. Are you exploring enough to find opportunities? → lean toward exploratory
4. Are you collaborating effectively? → lean toward social
5. Is everything stable? → stay balanced

Respond with ONLY a JSON object:
{{"strategy": "<strategy_name>", "reasoning": "<why>", "confidence": <0-100>}}
"""


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def _sanitise_name(name: str) -> str:
    """Sanitise an agent name for safe interpolation into LLM prompts.

    Strips newlines and other control characters and truncates to a
    reasonable length to mitigate prompt injection.
    """
    # Replace newlines and common control chars with spaces
    safe = re.sub(r"[\r\n\t]", " ", name)
    # Remove any remaining control characters
    safe = re.sub(r"[\x00-\x1f\x7f]", "", safe)
    # Truncate
    return safe[:_MAX_NAME_LENGTH]


# ---------------------------------------------------------------------------
# ReflectionEngine
# ---------------------------------------------------------------------------


class ReflectionEngine:
    """LLM-driven reflection engine that evaluates and adjusts agent strategy.

    Implements the ``ReflectionProvider`` protocol so it can be plugged into
    the ThinkLoop directly.

    Usage::

        engine = ReflectionEngine(
            config=ReflectionEngineConfig(token_cost=15),
            memory=short_term_memory,
            llm_provider=my_llm,
        )
        # Use in ThinkLoop:
        loop = ThinkLoop(..., reflection_provider=engine)
    """

    def __init__(
        self,
        *,
        config: ReflectionEngineConfig | None = None,
        memory: ShortTermMemoryProtocol | None = None,
        llm_provider: LLMProvider | None = None,
        action_history_provider: ActionHistoryProvider | None = None,
    ) -> None:
        self._config = config or ReflectionEngineConfig()
        self._memory = memory
        self._llm = llm_provider
        self._action_history = action_history_provider
        self._current_strategy: BehaviorStrategy = self._config.default_strategy
        self._adjustment_history: deque[StrategyAdjustment] = deque(
            maxlen=self._config.max_history,
        )

    # ------------------------------------------------------------------
    # Properties
    # ------------------------------------------------------------------

    @property
    def current_strategy(self) -> BehaviorStrategy:
        """The agent's current behaviour strategy."""
        return self._current_strategy

    @property
    def strategy_weights(self) -> dict[str, float]:
        """Action weight multipliers for the current strategy."""
        return dict(_STRATEGY_WEIGHTS.get(
            self._current_strategy,
            _STRATEGY_WEIGHTS[BehaviorStrategy.BALANCED],
        ))

    @property
    def adjustment_history(self) -> list[StrategyAdjustment]:
        """History of all strategy adjustments made."""
        return list(self._adjustment_history)

    @property
    def config(self) -> ReflectionEngineConfig:
        return self._config

    # ------------------------------------------------------------------
    # ReflectionProvider protocol
    # ------------------------------------------------------------------

    async def reflect(self, state: AgentState, tick: int) -> None:
        """Run a reflection cycle.

        This is the main entry point called by ThinkLoop every N ticks.
        It analyses recent behaviour, evaluates strategy, and updates
        the agent's approach.

        Args:
            state: Current agent state.
            tick: Current tick number.
        """
        # Deduct tokens upfront to avoid race conditions
        if self._config.token_cost > 0:
            if state.tokens < self._config.token_cost:
                logger.debug(
                    "Skipping reflection at tick %d: insufficient tokens (%d < %d)",
                    tick, state.tokens, self._config.token_cost,
                )
                return
            try:
                state.adjust_tokens(-self._config.token_cost)
            except ValueError:
                logger.warning(
                    "Reflection token deduction failed at tick %d", tick,
                )
                return

        # Gather action history
        recent_actions = self._get_recent_actions()

        # Compute metrics from action history
        metrics = self._compute_metrics(recent_actions)

        # Try LLM-based reflection, fall back to rule-based
        if self._llm is not None:
            result = await self._reflect_with_llm(state, tick, metrics)
        else:
            result = self._reflect_rule_based(state, tick, metrics)

        # Update current strategy
        self._current_strategy = result.adjustment.new_strategy
        self._adjustment_history.append(result.adjustment)

        # Store in memory system
        memory_stored = self._store_to_memory(result, tick)

        if memory_stored:
            logger.info(
                "Reflection at tick %d: %s → %s (%s, confidence=%d, cost=%d tokens, memory=saved)",
                tick,
                result.adjustment.previous_strategy.value,
                result.adjustment.new_strategy.value,
                result.adjustment.reasoning[:80],
                result.adjustment.confidence,
                self._config.token_cost,
            )
        else:
            logger.warning(
                "Reflection at tick %d: %s → %s (memory save FAILED)",
                tick,
                result.adjustment.previous_strategy.value,
                result.adjustment.new_strategy.value,
            )

    # ------------------------------------------------------------------
    # Metric computation
    # ------------------------------------------------------------------

    def _get_recent_actions(self) -> list[ActionResult]:
        """Retrieve recent actions from the action history provider."""
        if self._action_history is None:
            return []
        history = self._action_history.history
        window = self._config.analysis_window
        return history[-window:] if len(history) > window else history

    def _compute_metrics(self, actions: list[ActionResult]) -> _Metrics:
        """Analyse recent action history and compute performance metrics."""
        if not actions:
            return _Metrics()

        success_count = sum(
            1 for a in actions if a.status == ActionStatus.SUCCESS
        )
        total_cost = sum(a.token_cost for a in actions)
        action_counts: dict[str, int] = {}
        for a in actions:
            name = a.action_type.value
            action_counts[name] = action_counts.get(name, 0) + 1

        return _Metrics(
            action_count=len(actions),
            success_rate=success_count / len(actions) if actions else 0.0,
            resource_delta=-total_cost,
            action_counts=action_counts,
        )

    # ------------------------------------------------------------------
    # LLM-based reflection
    # ------------------------------------------------------------------

    async def _reflect_with_llm(
        self,
        state: AgentState,
        tick: int,
        metrics: _Metrics,
    ) -> ReflectionResult:
        """Use the LLM to evaluate strategy and recommend adjustments."""
        prompt = self._build_prompt(state, tick, metrics)

        assert self._llm is not None
        try:
            response = await self._llm.chat(
                [LLMMessage(role="user", content=prompt)],
                max_tokens=256,
                temperature=0.3,
            )
            adjustment = self._parse_llm_response(response.content, tick, metrics)
            method = "llm"
        except Exception as e:
            logger.warning(
                "LLM reflection failed at tick %d (%s), falling back to rule-based",
                tick, e,
            )
            adjustment = self._rule_based_adjustment(tick, metrics)
            method = "rule_based_fallback"

        return ReflectionResult(
            adjustment=adjustment,
            token_cost=self._config.token_cost,
            method=method,
            memory_stored=False,
        )

    def _build_prompt(
        self,
        state: AgentState,
        tick: int,
        metrics: _Metrics,
    ) -> str:
        """Build the reflection prompt from agent state and metrics."""
        breakdown = ", ".join(
            f"{name}: {count}"
            for name, count in sorted(metrics.action_counts.items())
        ) or "none"

        return _REFLECTION_PROMPT.format(
            name=_sanitise_name(state.name),
            tick=tick,
            health=state.health,
            tokens=state.tokens,
            max_tokens=state.max_tokens,
            money=state.money,
            reputation=state.reputation,
            action_count=metrics.action_count,
            success_rate=metrics.success_rate,
            resource_delta=metrics.resource_delta,
            action_breakdown=breakdown,
            current_strategy=self._current_strategy.value,
        )

    def _parse_llm_response(
        self,
        raw: str,
        tick: int,
        metrics: _Metrics,
    ) -> StrategyAdjustment:
        """Parse the LLM response into a StrategyAdjustment."""
        # Strip code fences
        text = raw.strip()
        if text.startswith("```"):
            text = re.sub(r"^```(?:json)?\s*\n?", "", text, count=1)
            text = re.sub(r"\n?```\s*$", "", text)
            text = text.strip()

        try:
            data = json.loads(text)
        except json.JSONDecodeError:
            logger.warning("Failed to parse LLM reflection response as JSON")
            return self._rule_based_adjustment(tick, metrics)

        strategy_name = data.get("strategy", "")
        try:
            new_strategy = BehaviorStrategy(strategy_name)
        except ValueError:
            logger.warning(
                "Unknown strategy '%s' from LLM, keeping current",
                strategy_name,
            )
            new_strategy = self._current_strategy

        reasoning = data.get("reasoning", "LLM reflection")
        if not isinstance(reasoning, str):
            reasoning = str(reasoning)
        # Truncate reasoning to prevent unbounded memory storage
        reasoning = reasoning[:500]

        confidence = data.get("confidence", 50)
        try:
            confidence = max(0, min(100, int(confidence)))
        except (TypeError, ValueError):
            confidence = 50

        return StrategyAdjustment(
            previous_strategy=self._current_strategy,
            new_strategy=new_strategy,
            reasoning=reasoning,
            confidence=confidence,
            resource_delta=metrics.resource_delta,
            success_rate=metrics.success_rate,
            action_counts=dict(metrics.action_counts),
            tick=tick,
        )

    # ------------------------------------------------------------------
    # Rule-based reflection (no LLM)
    # ------------------------------------------------------------------

    def _reflect_rule_based(
        self,
        state: AgentState,
        tick: int,
        metrics: _Metrics,
    ) -> ReflectionResult:
        """Perform reflection using deterministic rules (no LLM)."""
        adjustment = self._rule_based_adjustment(tick, metrics)
        return ReflectionResult(
            adjustment=adjustment,
            token_cost=self._config.token_cost,
            method="rule_based",
            memory_stored=False,
        )

    def _rule_based_adjustment(
        self,
        tick: int,
        metrics: _Metrics,
    ) -> StrategyAdjustment:
        """Determine strategy using deterministic rules.

        Evaluation order (most specific first):
        1. No actions → keep current (nothing to analyse)
        2. Dominant messaging (>{dominance_threshold}) → social
        3. Dominant exploring (>{dominance_threshold}) → exploratory
        4. Low success rate (<{low_success_threshold}) → conservative
        5. High success rate (>{high_success_threshold}) + active spending → aggressive
        6. Otherwise → keep current
        """
        cfg = self._config
        new_strategy = self._current_strategy
        reasoning = "No significant change detected."

        if metrics.action_count == 0:
            # No actions to analyse — keep current strategy, no forced switch
            reasoning = "No recent actions to analyse — keeping current strategy."
        elif (
            metrics.action_counts.get("send_message", 0)
            > metrics.action_count * cfg.dominance_threshold
        ):
            new_strategy = BehaviorStrategy.SOCIAL
            reasoning = "High messaging activity — switching to social strategy."
        elif (
            metrics.action_counts.get("explore", 0)
            > metrics.action_count * cfg.dominance_threshold
        ):
            new_strategy = BehaviorStrategy.EXPLORATORY
            reasoning = "High exploration activity — switching to exploratory strategy."
        elif metrics.success_rate < cfg.low_success_threshold:
            new_strategy = BehaviorStrategy.CONSERVATIVE
            reasoning = (
                f"Low success rate ({metrics.success_rate:.0%}) — "
                f"switching to conservative to minimise losses."
            )
        elif (
            metrics.success_rate > cfg.high_success_threshold
            and metrics.resource_delta < cfg.spending_threshold
        ):
            # Successful but spending — push aggressive to capitalise
            new_strategy = BehaviorStrategy.AGGRESSIVE
            reasoning = (
                f"High success rate ({metrics.success_rate:.0%}) with active spending — "
                f"switching to aggressive to maximise gains."
            )

        # If nothing changed, note it
        if new_strategy == self._current_strategy and metrics.action_count > 0:
            reasoning = (
                f"Current strategy '{self._current_strategy.value}' is performing "
                f"well (success: {metrics.success_rate:.0%}). No change needed."
            )

        return StrategyAdjustment(
            previous_strategy=self._current_strategy,
            new_strategy=new_strategy,
            reasoning=reasoning,
            confidence=cfg.rule_based_confidence,
            resource_delta=metrics.resource_delta,
            success_rate=metrics.success_rate,
            action_counts=dict(metrics.action_counts),
            tick=tick,
        )

    # ------------------------------------------------------------------
    # Memory integration
    # ------------------------------------------------------------------

    def _store_to_memory(self, result: ReflectionResult, tick: int) -> bool:
        """Persist the reflection result to the memory system.

        Returns True if stored successfully, False otherwise.
        """
        if self._memory is None:
            return False

        adj = result.adjustment
        content = (
            f"Strategy adjustment at tick {tick}: "
            f"{adj.previous_strategy.value} → {adj.new_strategy.value}. "
            f"Reason: {adj.reasoning} "
            f"(success_rate={adj.success_rate:.0%}, "
            f"resource_delta={adj.resource_delta:+d}, "
            f"confidence={adj.confidence}, method={result.method})"
        )

        try:
            self._memory.store(
                content=content,
                importance=self._config.memory_importance,
                tick=tick,
            )
            return True
        except Exception as e:
            logger.warning("Failed to store reflection in memory: %s", e)
            return False

    # ------------------------------------------------------------------
    # Utility
    # ------------------------------------------------------------------

    def reset_strategy(self) -> None:
        """Reset the current strategy to the default."""
        self._current_strategy = self._config.default_strategy

    def clear_history(self) -> None:
        """Clear all recorded strategy adjustments."""
        self._adjustment_history.clear()
