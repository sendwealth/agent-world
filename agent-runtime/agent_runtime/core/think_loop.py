"""Think loop — the core Perceive → Decide → Act cycle.

Each tick the agent:
  1. Perceives its environment (messages, market, own state).
  2. Runs survival assessment (synchronous, no LLM).
     - If PANIC or URGENT: executes emergency actions and skips normal decision.
  3. Makes an LLM-driven decision (or a mock fallback).
  4. Executes the chosen action(s) via the ActionExecutor.

The loop runs asynchronously with configurable tick intervals and
full error recovery — exceptions are logged and the loop continues.

Usage::

    from agent_runtime.core.think_loop import ThinkLoop, ThinkLoopConfig
    from agent_runtime.models.agent_state import AgentState
    from agent_runtime.survival.instinct import SurvivalInstinct
    from agent_runtime.core.act import ActionExecutor

    state = AgentState(name="Alice", max_tokens=1000, tokens=500)
    loop = ThinkLoop(
        state=state,
        survival=SurvivalInstinct(),
        executor=ActionExecutor(),
        config=ThinkLoopConfig(tick_interval=0.1),
    )
    await loop.run(max_ticks=100)
"""

from __future__ import annotations

import asyncio
import logging
import random
import time
from collections import deque
from dataclasses import dataclass, field, replace
from typing import Any, Protocol

from agent_runtime.core.act import (
    ActionContext,
    ActionExecutor,
    ActionStatus,
    ActionType,
)
from agent_runtime.core.decide import SocialContextProvider
from agent_runtime.models.agent_state import AgentState
from agent_runtime.models.phase_abilities import get_phase_abilities, is_terminal
from agent_runtime.observability import log_tick, metrics, trace_phase
from agent_runtime.social.feed import FeedIntegration
from agent_runtime.survival.instinct import (
    SurvivalAction,
    SurvivalInstinct,
    SurvivalMode,
)

logger = logging.getLogger(__name__)


# ---------------------------------------------------------------------------
# Group identity influence (Phase 4.3.3)
# ---------------------------------------------------------------------------


class GroupIdentityProvider(Protocol):
    """Provides group identity context for influencing agent decisions.

    Injected into the think loop to apply cultural pressure, trust bias,
    and diversity awareness during the perceive step.
    """

    def get_identity_context(self, agent_id: str, tick: int) -> dict[str, Any]: ...


# ---------------------------------------------------------------------------
# Configuration
# ---------------------------------------------------------------------------


@dataclass
class ThinkLoopConfig:
    """Configuration for the think loop.

    Attributes:
        tick_interval: Seconds between ticks.
        max_ticks: Maximum number of ticks before stopping (0 = unlimited).
        reflect_interval: Run reflect every N ticks.
        error_backoff: Seconds to wait after an error before retrying.
        max_consecutive_errors: Stop after this many consecutive errors (0 = unlimited).
        heartbeat_enabled: Send heartbeat RPC each tick for server tick sync.
        perception_cache_ttl: Seconds to cache perception data (0 = no cache).
            When multiple agents share similar environments, caching reduces
            redundant Discover RPC calls.
    """

    tick_interval: float = 1.0
    max_ticks: int = 0
    reflect_interval: int = 10
    error_backoff: float = 5.0
    max_consecutive_errors: int = 0
    heartbeat_enabled: bool = False
    perception_cache_ttl: float = 0.0


# ---------------------------------------------------------------------------
# Perception data
# ---------------------------------------------------------------------------


@dataclass(frozen=True)
class Perception:
    """Snapshot of what the agent perceives at a given tick.

    In production this is populated by A2A calls.  For now the fields
    are optional / defaulted so the think loop can run standalone.
    """

    messages: list[dict[str, Any]] = field(default_factory=list)
    token_balance: int = 0
    token_ratio: float = 0.0
    market_state: dict[str, Any] = field(default_factory=dict)
    active_task: str | None = None
    health: float = 100.0
    tick: int = 0
    server_tick: int = 0


# ---------------------------------------------------------------------------
# Decision data
# ---------------------------------------------------------------------------


@dataclass(frozen=True)
class Decision:
    """A decision produced by the decide step.

    Attributes:
        action_type: The chosen action to execute.
        parameters: Action-specific parameters.
        reasoning: Why this action was chosen (for logging / reflection).
    """

    action_type: ActionType
    parameters: dict[str, Any] = field(default_factory=dict)
    reasoning: str = ""


# ---------------------------------------------------------------------------
# Protocols for swappable perception / decision strategies
# ---------------------------------------------------------------------------


class PerceptionProvider(Protocol):
    """Produces a Perception each tick."""

    async def perceive(self, state: AgentState, tick: int) -> Perception: ...


class DecisionProvider(Protocol):
    """Produces a Decision given a perception and survival assessment."""

    async def decide(
        self,
        state: AgentState,
        perception: Perception,
        survival: SurvivalAction,
    ) -> Decision: ...


class ReflectionProvider(Protocol):
    """Called periodically for the agent to reflect on its behaviour."""

    async def reflect(self, state: AgentState, tick: int) -> None: ...


class HeartbeatProvider(Protocol):
    """Sends a heartbeat to the server and returns the server tick.

    The heartbeat now carries agent status information (token balance,
    alive status, urgent events) so the World Engine can monitor
    agent health and relay alerts to Human operators.
    """

    async def heartbeat(
        self,
        *,
        token_balance: int = 0,
        max_tokens: int = 0,
        alive: bool = True,
        urgent_events: list[str] | None = None,
    ) -> int: ...


class CulturalInfluenceHook(Protocol):
    """Optional hook for applying cultural influence during the think cycle.

    Called once per tick after reflection. Implementations can nudge the
    agent's values/personality based on regional, organizational, or peer
    cultural context.
    """

    def apply(self, state: AgentState, tick: int) -> None: ...


class SocialEngineHook(Protocol):
    """Optional hook for processing social interactions and tick-level diffusion.

    Called in two places:
      1. After a successful SOCIALIZE action — processes trust updates,
         imitation, and cultural conflict detection.
      2. Once per tick — applies regional cultural diffusion across all agents.
    """

    def process_socialize(
        self,
        agent_id: str,
        target_id: str,
        tick: int,
    ) -> dict[str, Any] | None: ...

    def apply_tick_diffusion(self) -> list[dict[str, Any]]: ...


class EmotionHook(Protocol):
    """Optional hook for emotion updates during the think cycle.

    Called after each action with the action type and result status,
    allowing the EmotionEngine to react to game events. Also called
    once per tick for temporal decay.
    """

    def update_from_action(
        self,
        action_type: str,
        status: str,
        context: dict[str, Any] | None,
    ) -> None: ...

    def decay(self, ticks_elapsed: int) -> None: ...

    def get_mood_description(self) -> str: ...


class LanguageExperimentHook(Protocol):
    """Optional hook for running language experiments during the think cycle.

    Called once per tick after the action has been executed (and after
    social engine processing). Implementations check messages against
    vocabulary constraints and measure communication efficiency.
    """

    def check_message(
        self,
        message: str,
        experiment_id: str,
    ) -> dict[str, Any]: ...

    def record_tick(
        self,
        agent_id: str,
        tick: int,
        message: str,
        experiment_id: str,
    ) -> dict[str, Any]: ...


class DiaryProvider(Protocol):
    """Generates and persists a diary entry for the current tick.

    Called once per tick after the action has been executed (and after
    reflection, if applicable).  Implementations typically delegate to
    ``DiaryGenerator``.
    """

    async def write_entry(
        self,
        state: AgentState,
        *,
        tick: int,
        action: str,
        outcome: str,
        key_events: list[str] | None = None,
        decisions: list[str] | None = None,
    ) -> None: ...


# ---------------------------------------------------------------------------
# Default (mock) providers
# ---------------------------------------------------------------------------


class DefaultPerceptionProvider:
    """Builds a Perception from the agent's current state.

    Uses only local state — no A2A / network calls.  Suitable for
    testing and as a placeholder until the real perceive layer is built.
    """

    async def perceive(self, state: AgentState, tick: int) -> Perception:
        max_tokens = getattr(state, "max_tokens", None)
        if max_tokens and max_tokens > 0:
            ratio = state.tokens / max_tokens
        else:
            ratio = 0.0

        return Perception(
            messages=[],
            token_balance=state.tokens,
            token_ratio=ratio,
            market_state={},
            active_task=None,
            health=state.health,
            tick=tick,
        )


class MockDecisionProvider:
    """Random decision provider for testing.

    Picks a random affordable action with realistic parameter generation.
    No LLM involved.  Includes GATHER and MOVE so that World Engine state
    (money, position) actually changes, making integration tests observable.
    """

    # Weighted action pool: GATHER and MOVE are more frequent so they
    # produce visible state changes in the World Engine.
    _ACTION_WEIGHTS: list[tuple[ActionType, int]] = [
        (ActionType.GATHER, 3),  # adds money in World Engine
        (ActionType.MOVE, 3),  # changes position in World Engine
        (ActionType.EXPLORE, 2),
        (ActionType.REST, 1),
    ]

    _RESOURCE_TYPES: list[str] = ["food", "wood", "stone", "iron"]
    _DIRECTIONS: list[str] = ["north", "south", "east", "west"]

    def __init__(self, executor: ActionExecutor) -> None:
        self._executor = executor

    def _weighted_choice(self, affordable: list[ActionType]) -> ActionType:
        """Pick a random action weighted by _ACTION_WEIGHTS."""
        weights = {at: w for at, w in self._ACTION_WEIGHTS if at in affordable}
        if not weights:
            return affordable[0]
        actions = list(weights.keys())
        ws = [weights[a] for a in actions]
        return random.choices(actions, weights=ws, k=1)[0]

    def _build_params(self, action_type: ActionType) -> dict[str, Any]:
        """Generate action-appropriate parameters."""
        if action_type == ActionType.GATHER:
            return {"resource_type": random.choice(self._RESOURCE_TYPES)}
        if action_type == ActionType.MOVE:
            return {"direction": random.choice(self._DIRECTIONS)}
        if action_type == ActionType.EXPLORE:
            return {"explore_params": {"radius": random.randint(1, 5)}}
        return {}

    async def decide(
        self,
        state: AgentState,
        perception: Perception,
        survival: SurvivalAction,
    ) -> Decision:
        all_actions = [at for at, _ in self._ACTION_WEIGHTS]
        affordable = [at for at in all_actions if self._executor.can_afford(at, state)]

        if not affordable:
            # Even REST should be free, but guard anyway.
            return Decision(
                action_type=ActionType.REST,
                reasoning="No affordable actions — resting.",
            )

        chosen = self._weighted_choice(affordable)
        params = self._build_params(chosen)
        return Decision(
            action_type=chosen,
            parameters=params,
            reasoning=f"Mock decision: chose {chosen.value} (tick {perception.tick}).",
        )


class DefaultReflectionProvider:
    """No-op reflection provider."""

    async def reflect(self, state: AgentState, tick: int) -> None:
        logger.debug("Reflect at tick %d: tokens=%d", tick, state.tokens)


# ---------------------------------------------------------------------------
# ThinkLoop
# ---------------------------------------------------------------------------


class ThinkLoop:
    """Core Perceive → Decide → Act loop.

    Runs the agent cycle with configurable tick intervals, error recovery,
    and swappable perception / decision / reflection providers.

    Supports an optional perception cache that avoids redundant Discover RPC
    calls when the environment hasn't changed since the last perception.

    Supports model hot-swapping: if a ``model_registry`` is provided, each
    tick checks whether the agent's model assignment changed in the registry.
    If so, the LLMProvider inside the decision chain is re-created and the
    swap takes effect on the **next** tick without interrupting the current one.

    Usage::

        loop = ThinkLoop(state=state, survival=instinct, executor=executor)
        await loop.run(max_ticks=100)
    """

    def __init__(
        self,
        state: AgentState,
        survival: SurvivalInstinct,
        executor: ActionExecutor,
        *,
        config: ThinkLoopConfig | None = None,
        perception_provider: PerceptionProvider | None = None,
        decision_provider: DecisionProvider | None = None,
        reflection_provider: ReflectionProvider | None = None,
        world_client: Any | None = None,
        heartbeat_provider: HeartbeatProvider | None = None,
        group_identity: GroupIdentityProvider | None = None,
        cultural_hook: CulturalInfluenceHook | None = None,
        social_context_provider: SocialContextProvider | None = None,
        social_nearby_cache: list[dict[str, Any]] | None = None,
        social_engine_hook: SocialEngineHook | None = None,
        emotion_hook: EmotionHook | None = None,
        model_registry: Any | None = None,
        diary_provider: DiaryProvider | None = None,
        feed_integration: FeedIntegration | None = None,
        language_experiment_hook: LanguageExperimentHook | None = None,
    ) -> None:
        self.state = state
        self.survival = survival
        self.executor = executor
        self.config = config or ThinkLoopConfig()

        # Providers
        self._perception = perception_provider or DefaultPerceptionProvider()
        self._decision = decision_provider or MockDecisionProvider(executor)
        self._reflection = reflection_provider or DefaultReflectionProvider()

        # World client for ACT phase — defaults to no-op for backward compat
        self._world_client = world_client

        # Heartbeat provider — optional, sends heartbeat each tick
        self._heartbeat = heartbeat_provider

        # Social context provider — optional, injects social/cultural context
        # into the decision layer. If provided and the decision_provider has
        # a settable social_provider attribute, inject it automatically.
        self._social_context_provider = social_context_provider
        self._social_nearby_cache = social_nearby_cache
        if social_context_provider is not None:
            self._inject_social_provider(social_context_provider)

        # Emotion hook — optional, updates emotional state from actions and decay
        self._emotion_hook = emotion_hook
        # If emotion hook is provided, inject it into the decision engine
        if emotion_hook is not None:
            self._inject_emotion_provider(emotion_hook)

        # Group identity provider — optional, injects cultural influence
        self._group_identity = group_identity
        # Cultural influence hook — optional, nudges values/personality each tick
        self._cultural_hook = cultural_hook
        # Social engine hook — optional, processes social interactions and diffusion
        self._social_engine_hook = social_engine_hook
        # Model registry — optional, enables runtime model hot-swap
        self._model_registry = model_registry
        self._last_model_version: int = 0
        # Perception cache — avoids redundant RPC when environment unchanged
        self._perception_cache: Perception | None = None
        # Recent action history — fed back into prompt for anti-repetition
        self._recent_actions: deque[str] = deque(maxlen=10)
        self._perception_cache_time: float = 0.0

        # Diary provider — optional, generates narrative diary entries
        self._diary = diary_provider

        # Feed integration — optional, social content posting/interaction
        self._feed = feed_integration
        # Language experiment hook — optional, vocabulary constraints and efficiency
        self._language_experiment_hook = language_experiment_hook

        # Runtime state
        self._tick: int = 0
        self._server_tick: int = 0
        self._running: bool = False
        self._consecutive_errors: int = 0
        self._total_errors: int = 0
        self._start_time: float = 0.0

    # ------------------------------------------------------------------
    # Properties
    # ------------------------------------------------------------------

    @property
    def tick(self) -> int:
        """Current tick number."""
        return self._tick

    @property
    def server_tick(self) -> int:
        """Last known server tick from heartbeat."""
        return self._server_tick

    @property
    def running(self) -> bool:
        """Whether the loop is currently active."""
        return self._running

    @property
    def total_errors(self) -> int:
        """Total errors encountered during the run."""
        return self._total_errors

    @property
    def social_context_provider(self) -> SocialContextProvider | None:
        """The social context provider, if configured."""
        return self._social_context_provider

    # ------------------------------------------------------------------
    # Social provider injection
    # ------------------------------------------------------------------

    @staticmethod
    def _unwrap_to_engine(provider: Any) -> Any | None:
        """Walk the decision-provider wrapper chain and return the inner DecisionEngine.

        Known wrapper attributes (checked at each level):
        * _inner  — AsyncDecisionProvider
        * _base   — MemoryAwareDecisionProvider

        If an object with an _engine attribute is reached (e.g. LLMDecisionProvider),
        its _engine value is returned.

        Returns None when the chain cannot be resolved.
        """
        seen: set[int] = set()
        current = provider
        while current is not None:
            if id(current) in seen:
                break
            seen.add(id(current))

            # Reached a provider that wraps a DecisionEngine directly
            engine = getattr(current, "_engine", None)
            if engine is not None:
                return engine

            # Try each known wrapper attribute
            for attr in ("_inner", "_base"):
                inner = getattr(current, attr, None)
                if inner is not None:
                    current = inner
                    break
            else:
                # No known wrapper attribute — dead end
                break

        return None

    def _inject_social_provider(self, provider: SocialContextProvider) -> None:
        """Inject the social context provider into the decision provider.

        Walks the wrapper chain (AsyncDecisionProvider, MemoryAwareDecisionProvider,
        etc.) to find the inner DecisionEngine and sets the _social_provider
        attribute on it.
        """
        engine = self._unwrap_to_engine(self._decision)
        if engine is not None and hasattr(engine, "_social_provider"):
            object.__setattr__(engine, "_social_provider", provider)
            logger.info("Injected SocialContextProvider into DecisionEngine")
            return

        logger.warning(
            "SocialContextProvider provided but decision_provider does not "
            "support injection — social context will not influence decisions"
        )

    def _inject_emotion_provider(self, hook: EmotionHook) -> None:
        """Inject the emotion provider into the decision provider chain.

        Walks the wrapper chain to find the inner DecisionEngine
        and sets the _emotion_provider attribute on it.
        """
        engine = self._unwrap_to_engine(self._decision)
        if engine is not None and hasattr(engine, "_emotion_provider"):
            object.__setattr__(engine, "_emotion_provider", hook)
            logger.info("Injected EmotionContextProvider into DecisionEngine")
            return

        logger.warning(
            "EmotionHook provided but decision_provider does not "
            "support injection — emotion context will not influence decisions"
        )

    # ------------------------------------------------------------------
    # Main entry point
    # ------------------------------------------------------------------

    async def run(self, max_ticks: int | None = None) -> None:
        """Run the think loop.

        Args:
            max_ticks: Override for max ticks to run. If None, uses config value.
                       0 means run indefinitely until stop() is called.
        """
        effective_max = max_ticks if max_ticks is not None else self.config.max_ticks
        self._running = True
        self._consecutive_errors = 0
        self._total_errors = 0
        self._start_time = time.monotonic()

        logger.info(
            "ThinkLoop started: max_ticks=%s tick_interval=%.2fs",
            effective_max or "unlimited",
            self.config.tick_interval,
        )

        try:
            while self._running:
                # Check tick limit
                if effective_max > 0 and self._tick >= effective_max:
                    logger.info("ThinkLoop reached max_ticks=%d", effective_max)
                    break

                try:
                    await self._think_once()
                    self._consecutive_errors = 0
                except Exception:
                    self._consecutive_errors += 1
                    self._total_errors += 1
                    logger.exception(
                        "Error in tick %d (consecutive: %d, total: %d)",
                        self._tick,
                        self._consecutive_errors,
                        self._total_errors,
                    )

                    # Check if we've exceeded consecutive error limit
                    if (
                        self.config.max_consecutive_errors > 0
                        and self._consecutive_errors >= self.config.max_consecutive_errors
                    ):
                        logger.error(
                            "Exceeded max_consecutive_errors=%d, stopping.",
                            self.config.max_consecutive_errors,
                        )
                        break

                    # Backoff after error
                    await asyncio.sleep(self.config.error_backoff)
                    continue

                # Wait for next tick
                if self.config.tick_interval > 0:
                    await asyncio.sleep(self.config.tick_interval)
        finally:
            self._running = False
            elapsed = time.monotonic() - self._start_time
            logger.info(
                "ThinkLoop stopped: ticks=%d errors=%d elapsed=%.1fs",
                self._tick,
                self._total_errors,
                elapsed,
            )

    def stop(self) -> None:
        """Signal the loop to stop gracefully."""
        self._running = False

    # ------------------------------------------------------------------
    # Core cycle
    # ------------------------------------------------------------------

    async def _think_once(self) -> None:
        """Execute one Perceive → Decide → Act cycle.

        Lifecycle integration:
          - Dead agents: skip the cycle entirely and stop the loop.
          - Dying agents: run a reduced cycle (only will/communication actions).
          - Phase abilities gate which actions the agent can perform.
          - Token-exhausted agents: stop the loop to prevent infinite panic loops.
        """
        self._tick += 1
        think_start = time.monotonic()

        # --- Lifecycle gate: Dead agents do nothing ---
        if is_terminal(self.state.phase):
            logger.info("Tick %d: agent is Dead — stopping think loop", self._tick)
            self.stop()
            return

        # --- Token exhaustion gate: stop before entering infinite panic ---
        if self.state.tokens <= 0:
            logger.warning(
                "Tick %d: agent has 0 tokens — stopping think loop to prevent infinite panic cycle",
                self._tick,
            )
            self.stop()
            return

        # 0. Heartbeat (optional — sends liveness ping with agent status)
        if self._heartbeat is not None and self.config.heartbeat_enabled:
            try:
                max_tokens = getattr(self.state, "max_tokens", None) or 0
                token_balance = self.state.tokens
                urgent_events: list[str] = []
                # Detect low token situation
                if max_tokens > 0 and token_balance < max_tokens * 0.15:
                    urgent_events.append("low_tokens")
                self._server_tick = await self._heartbeat.heartbeat(
                    token_balance=token_balance,
                    max_tokens=max_tokens,
                    alive=True,
                    urgent_events=urgent_events if urgent_events else None,
                )
                logger.debug(
                    "Tick %d: heartbeat ok — server_tick=%d tokens=%d",
                    self._tick,
                    self._server_tick,
                    token_balance,
                )
            except Exception:
                logger.debug(
                    "Tick %d: heartbeat failed (non-fatal)",
                    self._tick,
                    exc_info=True,
                )

        # 0b. Model hot-swap check (optional — re-create LLMProvider if changed)
        if self._model_registry is not None:
            try:
                self._check_model_swap()
            except Exception:
                logger.debug(
                    "Tick %d: model swap check failed (non-fatal)",
                    self._tick,
                    exc_info=True,
                )

        # 1. Perceive (with optional caching)
        with trace_phase("perceive", str(self.state.id)):
            perception = await self._perceive_with_cache()

        # 1b. Group identity influence (optional — Phase 4.3.3)
        if self._group_identity is not None:
            try:
                identity_ctx = self._group_identity.get_identity_context(
                    str(self.state.id), self._tick
                )
                if identity_ctx:
                    # Merge group identity context into market_state for downstream use
                    new_market = {**perception.market_state, "group_identity": identity_ctx}
                    perception = replace(perception, market_state=new_market)
            except Exception:
                logger.debug(
                    "Tick %d: group identity context failed (non-fatal)",
                    self._tick,
                    exc_info=True,
                )

        logger.debug(
            "Tick %d: perceived — token_ratio=%.2f health=%.0f phase=%s",
            self._tick,
            perception.token_ratio,
            perception.health,
            self.state.phase.value,
        )

        # 1c. Feed nearby agents into the social context provider cache
        if self._social_nearby_cache is not None:
            nearby = perception.market_state.get("nearby_agents", [])
            self._social_nearby_cache.clear()
            self._social_nearby_cache.extend(nearby)

        # 2. Survival assessment (synchronous, no LLM)
        survival_action = self.survival.assess(self.state)
        in_emergency = survival_action.mode in (SurvivalMode.PANIC, SurvivalMode.URGENT)

        if in_emergency:
            logger.warning(
                "Tick %d: survival mode=%s — executing emergency actions",
                self._tick,
                survival_action.mode.value,
            )
            metrics.survival_actions.inc()
            # Pass the world_client as the A2A client for emergency broadcasts
            a2a = self._world_client if self._world_client is not None else None
            await self.survival.execute(survival_action, self.state, a2a_client=a2a)

            # Emergency actions are fire-and-forget — always fall through to
            # normal LLM decision-making.  Emergency actions (SOS broadcast,
            # loan request) are quick and token-free; skipping the normal
            # decision wastes 100% of panic ticks instead of using them
            # productively (gather, claim_task, etc.).
            logger.info(
                "Tick %d: emergency actions done — proceeding to normal LLM decision",
                self._tick,
            )

        # 3. Decide — inject recent actions into perception so the LLM can
        # see its own behavior history and avoid repetition.
        if self._recent_actions:
            perception = replace(
                perception,
                market_state={
                    **perception.market_state,
                    "recent_actions": list(self._recent_actions),
                },
            )
        with trace_phase("decide", str(self.state.id)):
            decision = await self._decision.decide(self.state, perception, survival_action)

        # 3a. Hard anti-repetition: if the same action was chosen 5+ consecutive
        # times, force a different action to break the loop.  glm-4-flash tends
        # to get stuck in explore-only loops when perception data is sparse.
        if len(self._recent_actions) >= 5:
            last_action = self._recent_actions[-1]
            consecutive = sum(
                1 for a in list(self._recent_actions)[::-1] if a == last_action
            )
            if consecutive >= 5 and decision.action_type.value == last_action:
                diverse_choices = [
                    ActionType.SOCIALIZE,
                    ActionType.GATHER,
                    ActionType.PRACTICE_SKILL,
                    ActionType.PROPOSE_DEAL,
                    ActionType.REST,
                    ActionType.MOVE,
                ]
                choices = [a for a in diverse_choices if a != decision.action_type]
                if choices:
                    new_action = random.choice(choices)
                    logger.info(
                        "Tick %d: forced diversity — '%s' repeated %dx, "
                        "switching to '%s'",
                        self._tick,
                        last_action,
                        consecutive,
                        new_action.value,
                    )
                    decision = replace(
                        decision,
                        action_type=new_action,
                        reasoning=(
                            f"Breaking {consecutive}-tick '{last_action}' "
                            f"streak for behavioral diversity"
                        ),
                    )

        # 3b. Record the chosen action for distribution metrics (P2-2) so
        # action diversity can be monitored over time.
        metrics.record_action(decision.action_type.value)
        self._recent_actions.append(decision.action_type.value)

        # 4. Phase ability gate: check if the agent can perform this action
        abilities = get_phase_abilities(self.state.phase)
        if not self.state.can_perform(decision.action_type.value):
            logger.debug(
                "Tick %d: action %s blocked by phase %s (abilities: %s)",
                self._tick,
                decision.action_type.value,
                self.state.phase.value,
                abilities.model_dump(),
            )
            # Fall back to rest if the chosen action is not allowed
            decision = Decision(
                action_type=ActionType.REST,
                reasoning=(f"Action blocked by phase {self.state.phase.value}, resting instead."),
            )

        logger.debug(
            "Tick %d: decided — action=%s reason=%s",
            self._tick,
            decision.action_type.value,
            decision.reasoning,
        )

        # 4b. Socialize target injection — deep fallback (SEN-693).
        # The LLM frequently omits target_agent_id for socialize, and the
        # perception's nearby_agents list may be empty (Discover RPC returned
        # no agents or the gRPC perception provider silently failed).  Try
        # multiple fallbacks before giving up; if all fail, downgrade to REST
        # so we don't burn retries on a guaranteed ValueError.
        if decision.action_type == ActionType.SOCIALIZE and not decision.parameters.get(
            "target_agent_id"
        ):
            decision = await self._inject_socialize_target(decision, perception)

        # 5. Act
        with trace_phase("act", str(self.state.id)):
            action_result = await self._act(decision)

        # 5b. Emotion update from action result (optional)
        if self._emotion_hook is not None:
            try:
                self._emotion_hook.update_from_action(
                    action_type=decision.action_type.value,
                    status=action_result.status.value if action_result else "unknown",
                    context={
                        "reasoning": decision.reasoning,
                        "tick": self._tick,
                    },
                )
            except Exception:
                logger.debug(
                    "Tick %d: emotion update error (non-fatal)",
                    self._tick,
                    exc_info=True,
                )

        # 5c. Social engine — process socialize interaction (optional)
        if self._social_engine_hook is not None and action_result is not None:
            try:
                if (
                    decision.action_type == ActionType.SOCIALIZE
                    and action_result.status == ActionStatus.SUCCESS
                ):
                    target_id = decision.parameters.get("target_agent_id", "")
                    if target_id:
                        self._social_engine_hook.process_socialize(
                            agent_id=str(self.state.id),
                            target_id=target_id,
                            tick=self._tick,
                        )
            except Exception:
                logger.debug(
                    "Tick %d: social engine process_socialize error (non-fatal)",
                    self._tick,
                    exc_info=True,
                )

        # 6. Reflect (periodic — skip when low on tokens)
        if (
            self.config.reflect_interval > 0
            and self._tick % self.config.reflect_interval == 0
            and survival_action.mode not in (SurvivalMode.PANIC, SurvivalMode.URGENT)
        ):
            await self._reflection.reflect(self.state, self._tick)

        # 7. Cultural influence hook (every tick, no-op if not configured)
        if self._cultural_hook is not None:
            try:
                self._cultural_hook.apply(self.state, self._tick)
            except Exception:
                logger.debug(
                    "Tick %d: cultural hook error (non-fatal)",
                    self._tick,
                    exc_info=True,
                )

        # 7a. Social engine tick-level diffusion (every tick, no-op if not configured)
        if self._social_engine_hook is not None:
            try:
                self._social_engine_hook.apply_tick_diffusion()
            except Exception:
                logger.debug(
                    "Tick %d: social engine tick diffusion error (non-fatal)",
                    self._tick,
                    exc_info=True,
                )

        # 7b. Emotion decay (every tick, no-op if not configured)
        if self._emotion_hook is not None:
            try:
                self._emotion_hook.decay(ticks_elapsed=1)
            except Exception:
                logger.debug(
                    "Tick %d: emotion decay error (non-fatal)",
                    self._tick,
                    exc_info=True,
                )

        # 8. Diary entry (every tick, non-fatal — skip when critically low on tokens)
        if self._diary is not None and survival_action.mode != SurvivalMode.PANIC:
            try:
                action_outcome = (
                    action_result.status.value if action_result is not None else "unknown"
                )
                await self._diary.write_entry(
                    self.state,
                    tick=self._tick,
                    action=decision.action_type.value,
                    outcome=action_outcome,
                    decisions=[decision.reasoning] if decision.reasoning else [],
                )
            except Exception:
                logger.debug(
                    "Tick %d: diary generation error (non-fatal)",
                    self._tick,
                    exc_info=True,
                )

        # 9. Feed integration (every tick, non-fatal)
        if self._feed is not None:
            try:
                mood = getattr(self.state, "mood", "") or "neutral"
                extraversion = 0.5
                personality = getattr(self.state, "personality", None)
                if personality and hasattr(personality, "extraversion"):
                    extraversion = float(personality.extraversion)
                await self._feed.on_tick(self._tick, mood=mood, extraversion=extraversion)
            except Exception:
                logger.debug(
                    "Tick %d: feed integration error (non-fatal)",
                    self._tick,
                    exc_info=True,
                )

        # 9b. Language experiment (every tick, non-fatal)
        if self._language_experiment_hook is not None:
            try:
                reasoning = decision.reasoning or ""
                experiment_id = decision.parameters.get("experiment_id", "default")
                self._language_experiment_hook.check_message(
                    message=reasoning,
                    experiment_id=experiment_id,
                )
                self._language_experiment_hook.record_tick(
                    agent_id=str(self.state.id),
                    tick=self._tick,
                    message=reasoning,
                    experiment_id=experiment_id,
                )
            except Exception:
                logger.debug(
                    "Tick %d: language experiment hook error (non-fatal)",
                    self._tick,
                    exc_info=True,
                )

        # 10. Record think-loop duration
        elapsed = time.monotonic() - think_start
        metrics.think_duration.observe(elapsed)
        metrics.tokens_balance.set(self.state.tokens)
        metrics.health.set(self.state.health)
        log_tick(
            self._tick,
            str(self.state.id),
            self.state.tokens,
            self.state.health,
            self.state.phase.value,
        )

    # ------------------------------------------------------------------
    # Model hot-swap
    # ------------------------------------------------------------------

    def _check_model_swap(self) -> None:
        """Check if the model registry has a pending hot-swap for this agent.

        Walks the decision provider chain to find the innermost
        ``DecisionEngine._provider``, re-creates it from the registry
        override, and injects the new provider.  The swap takes effect
        on the **next** tick — the current tick proceeds with the old
        provider.
        """
        reg = self._model_registry
        current_version = reg.get_agent_models_version()
        if current_version == self._last_model_version:
            return  # No change

        self._last_model_version = current_version
        agent_id = str(self.state.id)

        override = reg.get_agent_model_override(agent_id)
        if override is None:
            return  # No specific override for this agent

        provider_id, model = override
        provider_cfg = reg.get_provider(provider_id)
        if provider_cfg is None:
            logger.warning(
                "Hot-swap: provider %r not found for agent %s",
                provider_id,
                agent_id,
            )
            return

        new_provider = reg.create_provider(provider_cfg, model)

        # Walk the decision provider chain to find and replace the LLMProvider.
        # Chain: AsyncDecisionProvider → LLMDecisionProvider → DecisionEngine._provider
        target = self._decision
        engine = None

        # Unwrap AsyncDecisionProvider
        if hasattr(target, "_inner"):
            target = target._inner

        # Unwrap MemoryAwareDecisionProvider or similar wrappers
        if hasattr(target, "base_provider"):
            target = target.base_provider

        # Get the DecisionEngine
        if hasattr(target, "_engine"):
            engine = target._engine

        if engine is not None and hasattr(engine, "_provider"):
            old_provider = engine._provider
            engine._provider = new_provider
            logger.info(
                "Model switched for agent %s: %s/%s → %s/%s (tick %d)",
                agent_id,
                old_provider._config.provider.value if hasattr(old_provider, "_config") else "?",
                old_provider._config.model if hasattr(old_provider, "_config") else "?",
                provider_id,
                model,
                self._tick,
                extra={
                    "event": "model_switched",
                    "agent": agent_id,
                    "provider": provider_id,
                    "model": model,
                    "tick": self._tick,
                },
            )

    # ------------------------------------------------------------------
    # Perception caching
    # ------------------------------------------------------------------

    async def _perceive_with_cache(self) -> Perception:
        """Get perception, using cache if within TTL.

        Caching avoids redundant Discover RPC calls when the environment
        hasn't changed.  Messages are always fresh (they come from the
        streaming queue, not the cache).  The cache covers the expensive
        `discover` call and static agent state.
        """
        if self.config.perception_cache_ttl <= 0:
            return await self._perception.perceive(self.state, self._tick)

        now = time.monotonic()
        if (
            self._perception_cache is not None
            and (now - self._perception_cache_time) < self.config.perception_cache_ttl
        ):
            # Return cached perception but update tick and messages
            cached = self._perception_cache
            # Re-drain messages (always fresh)
            if hasattr(self._perception, "_drain_messages"):
                fresh_messages = await self._perception._drain_messages()  # type: ignore[union-attr]
            else:
                fresh_messages = cached.messages
            return Perception(
                messages=fresh_messages,
                token_balance=self.state.tokens,
                token_ratio=cached.token_ratio,
                market_state=cached.market_state,
                active_task=cached.active_task,
                health=self.state.health,
                tick=self._tick,
                server_tick=cached.server_tick,
            )

        perception = await self._perception.perceive(self.state, self._tick)
        self._perception_cache = perception
        self._perception_cache_time = now
        return perception

    def invalidate_perception_cache(self) -> None:
        """Force a fresh perception on the next tick."""
        self._perception_cache = None
        self._perception_cache_time = 0.0

    # ------------------------------------------------------------------
    # Action execution
    # ------------------------------------------------------------------

    async def _inject_socialize_target(
        self,
        decision: Decision,
        perception: Perception,
    ) -> Decision:
        """Inject ``target_agent_id`` for a SOCIALIZE action via deep fallback.

        Three layers are tried in order (SEN-693):
          1. ``social_context_provider.recommended_target_id``
          2. ``perception.market_state['nearby_agents']`` (may differ from
             the DecisionPerception's nearby_agents because the LLM
             injection runs against a potentially stale / empty list)
          3. ``world_client.explore()`` / ``discover()`` — direct query via
             whatever world client is wired in (GRPCWorldClient or REST)

        If all three fail the decision is downgraded to REST so the action
        executor does not burn all retries on a guaranteed ValueError.
        """
        my_id = str(self.state.id)

        # Fallback 1: social context provider recommended target
        if self._social_context_provider is not None:
            try:
                social_ctx = self._social_context_provider.build_social_context(
                    agent_id=my_id, tick=self._tick
                )
                if social_ctx is not None and social_ctx.recommended_target_id:
                    logger.info(
                        "Tick %d: injected target_agent_id=%s (social_context_provider)",
                        self._tick,
                        social_ctx.recommended_target_id,
                    )
                    return replace(
                        decision,
                        parameters={
                            **decision.parameters,
                            "target_agent_id": social_ctx.recommended_target_id,
                        },
                    )
            except Exception:
                logger.debug(
                    "Tick %d: social_context_provider target lookup failed",
                    self._tick,
                    exc_info=True,
                )

        # Fallback 2: perception market_state nearby_agents
        nearby = perception.market_state.get("nearby_agents", [])
        for agent_info in nearby:
            candidate = (
                agent_info.get("agent_id") or agent_info.get("id") or agent_info.get("name")
                if isinstance(agent_info, dict)
                else agent_info
            )
            if candidate and str(candidate) != my_id:
                candidate = str(candidate)
                logger.info(
                    "Tick %d: injected target_agent_id=%s (nearby_agents)",
                    self._tick,
                    candidate,
                )
                return replace(
                    decision,
                    parameters={
                        **decision.parameters,
                        "target_agent_id": candidate,
                    },
                )

        # Fallback 3: social_nearby_cache (populated by step 1c)
        if self._social_nearby_cache is not None:
            for agent_info in self._social_nearby_cache:
                candidate = (
                    agent_info.get("agent_id") or agent_info.get("id") or agent_info.get("name")
                    if isinstance(agent_info, dict)
                    else agent_info
                )
                if candidate and str(candidate) != my_id:
                    candidate = str(candidate)
                    logger.info(
                        "Tick %d: injected target_agent_id=%s (nearby_cache)",
                        self._tick,
                        candidate,
                    )
                    return replace(
                        decision,
                        parameters={
                            **decision.parameters,
                            "target_agent_id": candidate,
                        },
                    )

        # Fallback 4: query the world client directly.
        # GRPCWorldClient has ``explore()`` which calls Discover and returns
        # ``{"agents": [...]}``.  Any REST client with ``explore()`` works too.
        if self._world_client is not None:
            try:
                result = await self._world_client.explore({})
                for agent_info in result.get("agents", []):
                    candidate = (
                        agent_info.get("agent_id") or agent_info.get("name")
                        if isinstance(agent_info, dict)
                        else None
                    )
                    if candidate and str(candidate) != my_id:
                        candidate = str(candidate)
                        logger.info(
                            "Tick %d: injected target_agent_id=%s (world.explore)",
                            self._tick,
                            candidate,
                        )
                        return replace(
                            decision,
                            parameters={
                                **decision.parameters,
                                "target_agent_id": candidate,
                            },
                        )
            except Exception:
                logger.debug(
                    "Tick %d: world_client.explore fallback failed",
                    self._tick,
                    exc_info=True,
                )

        # All fallbacks exhausted — downgrade to REST to avoid retry_exhausted
        logger.warning(
            "Tick %d: SOCIALIZE has no target_agent_id and all fallbacks "
            "failed — downgrading to REST",
            self._tick,
        )
        return Decision(
            action_type=ActionType.REST,
            reasoning=(
                "Socialize target unavailable (no nearby agents discovered) — resting instead"
            ),
        )

    async def _act(self, decision: Decision) -> Any:
        """Execute a decision via the ActionExecutor.

        The ActionExecutor handles token deduction, retry logic, and
        result recording.  We wrap the agent state and world client
        into an ActionContext.

        If no world_client was provided (standalone mode), a
        ``_NoOpWorldClient`` is used — actions succeed but have no
        effect on the World Engine.  This is intentional for agents
        running without ``--world-url``.
        """
        if self._world_client is not None:
            world = self._world_client
        else:
            if self._tick <= 1:
                logger.warning(
                    "No world_client provided — running in standalone mode. "
                    "Actions will succeed locally but have no effect on the "
                    "World Engine.  Provide --world-url for connected mode."
                )
            world = _NoOpWorldClient()
        context = ActionContext(
            agent=self.state,  # type: ignore[arg-type]
            world=world,  # type: ignore[arg-type]
            parameters=decision.parameters,
        )

        result = await self.executor.execute(decision.action_type, context)

        # Record action for reflection analysis (if provider supports it)
        if hasattr(self._reflection, "record_action"):
            self._reflection.record_action(
                tick=self._tick,
                action=decision.action_type.value,
                status=result.status.value,
                token_cost=result.token_cost,
                reasoning=decision.reasoning,
            )

        if result.status != ActionStatus.SUCCESS:
            logger.warning(
                "Tick %d: action %s failed — status=%s error=%s",
                self._tick,
                decision.action_type.value,
                result.status.value,
                result.error,
            )

        return result


# ---------------------------------------------------------------------------
# No-op world client (placeholder until A2A layer is built)
# ---------------------------------------------------------------------------


class _NoOpWorldClient:
    """Placeholder world client that returns empty success results.

    Used when the real A2A / world client isn't available yet.
    All methods return a minimal success dict.
    """

    async def send_message(self, payload: dict[str, Any]) -> dict[str, Any]:
        return {"status": "ok", "action": "send_message"}

    async def claim_task(self, task_id: str) -> dict[str, Any]:
        return {"status": "ok", "action": "claim_task", "task_id": task_id}

    async def submit_task(self, task_id: str, result: dict[str, Any]) -> dict[str, Any]:
        return {"status": "ok", "action": "submit_task", "task_id": task_id}

    async def propose_deal(self, proposal: dict[str, Any]) -> dict[str, Any]:
        return {"status": "ok", "action": "propose_deal"}

    async def teach_skill(
        self, target_agent_id: str, skill_name: str, level: int
    ) -> dict[str, Any]:
        return {
            "status": "ok",
            "action": "teach_skill",
            "target": target_agent_id,
            "skill": skill_name,
        }

    async def explore(self, parameters: dict[str, Any]) -> dict[str, Any]:
        return {"status": "ok", "action": "explore", "findings": []}

    async def move(self, direction: str) -> dict[str, Any]:
        return {"status": "ok", "action": "move", "direction": direction}

    async def gather(self, resource_type: str) -> dict[str, Any]:
        return {"status": "ok", "action": "gather", "resource_type": resource_type}

    async def build(self, structure_type: str, **kwargs: Any) -> dict[str, Any]:
        return {"status": "ok", "action": "build", "structure_type": structure_type}

    async def socialize(self, target_agent_id: str, message: str = "") -> dict[str, Any]:
        return {
            "status": "ok",
            "action": "socialize",
            "target_agent_id": target_agent_id,
        }

    async def respond_to_oracle(self, oracle_id: str, response: str) -> dict[str, Any]:
        return {"status": "ok", "action": "respond_oracle", "oracle_id": oracle_id}

    async def check_bounties(self) -> dict[str, Any]:
        return {"status": "ok", "action": "check_bounties", "bounties": []}

    async def claim_bounty(self, bounty_id: str) -> dict[str, Any]:
        return {"status": "ok", "action": "claim_bounty", "bounty_id": bounty_id}

    async def complete_bounty(self, bounty_id: str, result: str) -> dict[str, Any]:
        return {"status": "ok", "action": "complete_bounty", "bounty_id": bounty_id}


# ---------------------------------------------------------------------------
# Concurrent multi-agent runner
# ---------------------------------------------------------------------------


async def run_agents_concurrent(
    loops: list[ThinkLoop],
    max_ticks: int | None = None,
) -> None:
    """Run multiple agent think loops concurrently.

    Each agent's LLM decision calls happen in parallel via asyncio.gather,
    dramatically reducing wall-clock time at scale.  A typical 10-agent
    simulation with 1s tick intervals goes from ~10s/tick (serial) to
    ~1.5s/tick (parallel) when LLM calls dominate.

    Args:
        loops: List of ThinkLoop instances (one per agent).
        max_ticks: Override for max ticks. If None, uses each loop's config.
    """
    if not loops:
        return

    tasks = [loop.run(max_ticks=max_ticks) for loop in loops]
    await asyncio.gather(*tasks)
