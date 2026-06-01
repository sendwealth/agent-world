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
    """Sends a heartbeat to the server and returns the server tick."""

    async def heartbeat(self) -> int: ...


class CulturalInfluenceHook(Protocol):
    """Optional hook for applying cultural influence during the think cycle.

    Called once per tick after reflection. Implementations can nudge the
    agent's values/personality based on regional, organizational, or peer
    cultural context.
    """

    def apply(self, state: AgentState, tick: int) -> None: ...


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
        (ActionType.GATHER, 3),   # adds money in World Engine
        (ActionType.MOVE, 3),     # changes position in World Engine
        (ActionType.EXPLORE, 2),
        (ActionType.REST, 1),
    ]

    _RESOURCE_TYPES: list[str] = ["food", "wood", "stone", "iron"]
    _DIRECTIONS: list[str] = ["north", "south", "east", "west"]

    def __init__(self, executor: ActionExecutor) -> None:
        self._executor = executor

    def _weighted_choice(self, affordable: list[ActionType]) -> ActionType:
        """Pick a random action weighted by _ACTION_WEIGHTS."""
        weights = {
            at: w for at, w in self._ACTION_WEIGHTS if at in affordable
        }
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
        affordable = [
            at for at in all_actions if self._executor.can_afford(at, state)
        ]

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
        model_registry: Any | None = None,
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
        if social_context_provider is not None:
            self._inject_social_provider(social_context_provider)

        # Group identity provider — optional, injects cultural influence
        self._group_identity = group_identity
        # Cultural influence hook — optional, nudges values/personality each tick
        self._cultural_hook = cultural_hook
        # Model registry — optional, enables runtime model hot-swap
        self._model_registry = model_registry
        self._last_model_version: int = 0
        # Perception cache — avoids redundant RPC when environment unchanged
        self._perception_cache: Perception | None = None
        self._perception_cache_time: float = 0.0

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

    def _inject_social_provider(self, provider: SocialContextProvider) -> None:
        """Inject the social context provider into the decision provider.

        Handles two patterns:
        1. If the decision_provider wraps a DecisionEngine (e.g. LLMDecisionProvider),
           set the social_provider on the inner engine.
        2. If the decision_provider is wrapped in an async adapter, unwrap first.
        """
        target = self._decision

        # Unwrap AsyncDecisionProvider if present
        if hasattr(target, "inner"):
            target = target.inner

        # If the decision provider has a settable _engine with social_provider
        if hasattr(target, "_engine"):
            engine = target._engine
            if hasattr(engine, "_social_provider"):
                object.__setattr__(engine, "_social_provider", provider)
                logger.info(
                    "Injected SocialContextProvider into DecisionEngine"
                )
                return

        logger.debug(
            "SocialContextProvider provided but decision_provider does not "
            "support injection — social context will not influence decisions"
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
        """
        self._tick += 1
        think_start = time.monotonic()

        # --- Lifecycle gate: Dead agents do nothing ---
        if is_terminal(self.state.phase):
            logger.info(
                "Tick %d: agent is Dead — stopping think loop", self._tick
            )
            self.stop()
            return

        # 0. Heartbeat (optional — sends liveness ping and syncs server tick)
        if self._heartbeat is not None and self.config.heartbeat_enabled:
            try:
                self._server_tick = await self._heartbeat.heartbeat()
                logger.debug(
                    "Tick %d: heartbeat ok — server_tick=%d",
                    self._tick,
                    self._server_tick,
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

        # 2. Survival assessment (synchronous, no LLM)
        survival_action = self.survival.assess(self.state)

        if survival_action.mode in (SurvivalMode.PANIC, SurvivalMode.URGENT):
            logger.warning(
                "Tick %d: survival mode=%s — executing emergency actions",
                self._tick,
                survival_action.mode.value,
            )
            metrics.survival_actions.inc()
            # Pass the world_client as the A2A client for emergency broadcasts
            a2a = self._world_client if self._world_client is not None else None
            await self.survival.execute(survival_action, self.state, a2a_client=a2a)
            return  # Skip normal decision

        # 3. Decide
        with trace_phase("decide", str(self.state.id)):
            decision = await self._decision.decide(self.state, perception, survival_action)

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
                reasoning=(
                    f"Action blocked by phase {self.state.phase.value}, resting instead."
                ),
            )

        logger.debug(
            "Tick %d: decided — action=%s reason=%s",
            self._tick,
            decision.action_type.value,
            decision.reasoning,
        )

        # 5. Act
        with trace_phase("act", str(self.state.id)):
            await self._act(decision)

        # 6. Reflect (periodic)
        if self.config.reflect_interval > 0 and self._tick % self.config.reflect_interval == 0:
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

        # 8. Record think-loop duration
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
            if hasattr(self._perception, '_drain_messages'):
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

    async def _act(self, decision: Decision) -> None:
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

    async def submit_task(
        self, task_id: str, result: dict[str, Any]
    ) -> dict[str, Any]:
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
