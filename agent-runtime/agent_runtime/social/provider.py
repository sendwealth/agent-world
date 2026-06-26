"""Default SocialContextProvider — bridges SocialEngine to the decision layer.

This module provides the concrete ``DefaultSocialContextProvider`` that implements
the :class:`agent_runtime.core.decide.SocialContextProvider` protocol. It wraps
:class:`~agent_runtime.social.engine.SocialEngine` and translates its rich
``SocialContext`` output into the lighter ``decide.SocialContext`` dataclass that
the decision prompt consumes.

Also provides ``DefaultSocialEngineHook`` that implements the
:class:`agent_runtime.core.think_loop.SocialEngineHook` protocol for processing
socialize interactions and tick-level cultural diffusion.

Usage::

    from agent_runtime.social.provider import DefaultSocialContextProvider

    provider = DefaultSocialContextProvider()
    # provider.build_social_context(agent_id, tick) -> decide.SocialContext | None

The provider accepts an optional ``NearbyAgentSource`` callable so the think loop
can inject real-time nearby-agent data at decision time.
"""

from __future__ import annotations

import logging
from dataclasses import dataclass
from typing import Any, Protocol

from agent_runtime.core.decide import SocialContext
from agent_runtime.models.personality import PersonalityVector
from agent_runtime.models.values import ValueWeights

from .engine import SocialEngine
from .language_experiment import LanguageExperiment

logger = logging.getLogger(__name__)


# ---------------------------------------------------------------------------
# Data source protocols
# ---------------------------------------------------------------------------


class NearbyAgentSource(Protocol):
    """Callable that provides nearby agent data for social context building.

    The think loop implements this to supply real-time perception data.
    """

    def __call__(self, agent_id: str, tick: int) -> list[dict[str, Any]]: ...


class AgentProfileSource(Protocol):
    """Callable that provides the agent's own personality and values."""

    def __call__(self, agent_id: str) -> AgentProfile | None: ...


@dataclass(frozen=True)
class AgentProfile:
    """Agent profile data needed for social context computation."""

    personality: PersonalityVector
    values: ValueWeights
    group_ids: list[str]


# ---------------------------------------------------------------------------
# DefaultSocialContextProvider
# ---------------------------------------------------------------------------


class DefaultSocialContextProvider:
    """Concrete SocialContextProvider that integrates social/ modules into the
    decision layer.

    This is the single injection point that bridges Phase 4.3 social modules
    (trust, cultural diffusion, imitation, language emergence, etc.) into the
    Perceive -> Decide -> Act think loop.

    Args:
        engine: Optional pre-configured SocialEngine. If not provided a fresh
            instance is created.
        nearby_source: Callable that returns nearby agent dicts (with keys
            ``agent_id``, ``personality``, ``values``, optional ``group_ids``)
            for a given agent_id and tick.
        profile_source: Callable that returns the agent's own profile
            (personality, values, group_ids). If not provided, defaults are used.
    """

    def __init__(
        self,
        engine: SocialEngine | None = None,
        nearby_source: NearbyAgentSource | None = None,
        profile_source: AgentProfileSource | None = None,
    ) -> None:
        self._engine = engine or SocialEngine()
        self._nearby_source = nearby_source
        self._profile_source = profile_source

    def build_social_context(
        self,
        agent_id: str,
        tick: int,
    ) -> SocialContext | None:
        """Build social context for the decision prompt.

        Implements the ``SocialContextProvider`` protocol from ``decide.py``.
        Returns ``None`` if no profile data is available (agent hasn't been
        initialized yet).

        Args:
            agent_id: The deciding agent's ID.
            tick: Current world tick.

        Returns:
            A ``decide.SocialContext`` for prompt injection, or ``None``.
        """
        # Defensive: callers (e.g. DecisionEngine) may pass state.id (UUID).
        # Coerce once at the entry point so all downstream code sees a plain str.
        agent_id = str(agent_id)

        # 1. Get agent profile (personality + values + groups)
        profile = self._get_profile(agent_id)
        if profile is None:
            logger.debug(
                "No agent profile available for %s — skipping social context",
                agent_id,
            )
            return None

        # 2. Get nearby agents
        nearby_agents = self._get_nearby_agents(agent_id, tick)

        # 3. Delegate to SocialEngine for the heavy computation
        try:
            engine_ctx = self._engine.build_context(
                agent_id=agent_id,
                personality=profile.personality,
                values=profile.values,
                nearby_agents=nearby_agents,
                tick=tick,
                agent_groups=profile.group_ids,
            )
        except Exception:
            logger.warning(
                "SocialEngine.build_context failed for agent %s (non-fatal)",
                agent_id,
                exc_info=True,
            )
            return None

        # 4. Adapt engine.SocialContext -> decide.SocialContext
        recommended_id = ""
        if engine_ctx.recommended_target is not None:
            recommended_id = engine_ctx.recommended_target.agent_id

        return SocialContext(
            social_propensity=engine_ctx.social_propensity,
            should_socialize=engine_ctx.should_socialize,
            recommended_target_id=recommended_id,
            trust_snapshot=dict(engine_ctx.trust_snapshot),
            personality_description=engine_ctx.personality_description,
        )

    # ------------------------------------------------------------------
    # Internal helpers
    # ------------------------------------------------------------------

    def _get_profile(self, agent_id: str) -> AgentProfile | None:
        """Resolve the agent's profile from the profile source."""
        if self._profile_source is not None:
            try:
                return self._profile_source(agent_id)
            except Exception:
                logger.debug(
                    "Profile source failed for %s", agent_id, exc_info=True
                )
                return None

        # Without a profile source, we can't build meaningful social context.
        # Return a default profile so the provider degrades gracefully
        # (social propensity will be 0.5 with no targets).
        return None

    def _get_nearby_agents(
        self, agent_id: str, tick: int
    ) -> list[dict[str, Any]]:
        """Resolve nearby agents from the nearby source."""
        if self._nearby_source is not None:
            try:
                return self._nearby_source(agent_id, tick)
            except Exception:
                logger.debug(
                    "Nearby agent source failed for %s at tick %d",
                    agent_id,
                    tick,
                    exc_info=True,
                )
        return []


# ---------------------------------------------------------------------------
# Data source protocols for social engine hook
# ---------------------------------------------------------------------------


class RegionAgentSource(Protocol):
    """Callable that provides agents grouped by region for tick diffusion.

    Called once per tick by ``DefaultSocialEngineHook.apply_tick_diffusion()``.
    Returns a mapping of region_id -> list of agent dicts.
    """

    def __call__(self) -> dict[str, list[dict[str, Any]]]: ...


# ---------------------------------------------------------------------------
# DefaultSocialEngineHook
# ---------------------------------------------------------------------------


class DefaultSocialEngineHook:
    """Concrete ``SocialEngineHook`` that processes socialize actions and
    tick-level cultural diffusion.

    Implements the ``SocialEngineHook`` protocol from ``think_loop.py``.

    Args:
        engine: Optional pre-configured SocialEngine. If not provided a fresh
            instance is created.
        profile_source: Callable that returns the agent's own profile
            (personality, values, group_ids). Required for ``process_socialize``
            to look up both initiator and target profiles.
        region_agent_source: Callable that returns agents grouped by region.
            Required for ``apply_tick_diffusion`` to run regional cultural
            diffusion.
    """

    def __init__(
        self,
        engine: SocialEngine | None = None,
        profile_source: AgentProfileSource | None = None,
        region_agent_source: RegionAgentSource | None = None,
    ) -> None:
        self._engine = engine or SocialEngine()
        self._profile_source = profile_source
        self._region_agent_source = region_agent_source

    def process_socialize(
        self,
        agent_id: str,
        target_id: str,
        tick: int,
    ) -> dict[str, Any] | None:
        """Process a SOCIALIZE action — run trust update, imitation, conflict.

        Looks up both agent and target profiles from the profile source and
        delegates to ``SocialEngine.execute_socialize()``.

        Args:
            agent_id: The initiating agent's ID.
            target_id: The target agent's ID.
            tick: Current tick.

        Returns:
            Dict with interaction results, or ``None`` if profiles unavailable.
        """
        if self._profile_source is None:
            logger.debug(
                "No profile_source configured — skipping process_socialize"
            )
            return None

        agent_profile = self._profile_source(agent_id)
        target_profile = self._profile_source(target_id)

        if agent_profile is None or target_profile is None:
            logger.debug(
                "process_socialize: missing profile for %s or %s — skipping",
                agent_id,
                target_id,
            )
            return None

        return self._engine.execute_socialize(
            agent_id=agent_id,
            target_id=target_id,
            personality=agent_profile.personality,
            values=agent_profile.values,
            target_personality=target_profile.personality,
            target_values=target_profile.values,
            tick=tick,
        )

    def apply_tick_diffusion(self) -> list[dict[str, Any]]:
        """Apply regional cultural diffusion for the current tick.

        Delegates to ``SocialEngine.apply_tick_diffusion()`` using agents
        grouped by region from the ``region_agent_source``.

        Returns:
            List of diffusion result dicts, one per region.
        """
        if self._region_agent_source is None:
            return []

        agents_by_region = self._region_agent_source()
        if not agents_by_region:
            return []

        return self._engine.apply_tick_diffusion(agents_by_region)


class DefaultLanguageExperimentHook:
    """Concrete ``LanguageExperimentHook`` that checks messages against
    vocabulary constraints and records per-tick language metrics.

    Implements the ``LanguageExperimentHook`` protocol from ``think_loop.py``.

    Args:
        experiment: Optional pre-configured LanguageExperiment. If not
            provided a fresh instance is created.
    """

    def __init__(
        self,
        experiment: LanguageExperiment | None = None,
    ) -> None:
        self._experiment = experiment or LanguageExperiment()
        # Per-agent message history for efficiency measurement.
        self._messages_before: dict[str, list[str]] = {}
        self._messages_after: dict[str, list[str]] = {}

    @property
    def experiment(self) -> LanguageExperiment:
        """The underlying LanguageExperiment instance."""
        return self._experiment

    def setup_restricted_vocabulary(
        self,
        agent_ids: list[str],
        allowed_words: set[str],
        experiment_id: str = "default",
    ) -> None:
        """Forward vocabulary setup to the underlying experiment."""
        self._experiment.setup_restricted_vocabulary(
            agent_ids=agent_ids,
            allowed_words=allowed_words,
            experiment_id=experiment_id,
        )

    def check_message(
        self,
        message: str,
        experiment_id: str = "default",
    ) -> dict[str, Any]:
        """Check a message against the active vocabulary constraint.

        Returns:
            Dict with keys: compliant (bool), violations (list of disallowed words).
        """
        return self._experiment.check_message(message, experiment_id)

    def record_tick(
        self,
        agent_id: str,
        tick: int,
        message: str,
        experiment_id: str = "default",
    ) -> dict[str, Any]:
        """Record a message for the agent and return compliance status.

        Messages are stored in per-agent 'before' and 'after' lists,
        enabling efficiency measurement once a vocabulary constraint is
        activated.

        Returns:
            Dict with keys: compliant (bool), violations (list).
        """
        result = self._experiment.check_message(message, experiment_id)

        if self._experiment.is_active(experiment_id):
            self._messages_after.setdefault(agent_id, []).append(message)
        else:
            self._messages_before.setdefault(agent_id, []).append(message)

        return result

    def get_efficiency_metrics(
        self,
        agent_id: str,
        experiment_id: str = "default",
    ) -> Any:
        """Measure communication efficiency for an agent.

        Returns:
            EfficiencyMetrics comparing before/after messages.
        """
        before = self._messages_before.get(agent_id, [])
        after = self._messages_after.get(agent_id, [])
        return self._experiment.measure_communication_efficiency(
            before_messages=before,
            after_messages=after,
            experiment_id=experiment_id,
        )
