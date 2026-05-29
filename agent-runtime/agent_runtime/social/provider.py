"""Default SocialContextProvider — bridges SocialEngine to the decision layer.

This module provides the concrete ``DefaultSocialContextProvider`` that implements
the :class:`agent_runtime.core.decide.SocialContextProvider` protocol. It wraps
:class:`~agent_runtime.social.engine.SocialEngine` and translates its rich
``SocialContext`` output into the lighter ``decide.SocialContext`` dataclass that
the decision prompt consumes.

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
from typing import Any, Dict, List, Optional, Protocol

from agent_runtime.core.decide import SocialContext
from agent_runtime.models.personality import PersonalityVector
from agent_runtime.models.values import ValueWeights

from .engine import SocialEngine

logger = logging.getLogger(__name__)


# ---------------------------------------------------------------------------
# Data source protocols
# ---------------------------------------------------------------------------


class NearbyAgentSource(Protocol):
    """Callable that provides nearby agent data for social context building.

    The think loop implements this to supply real-time perception data.
    """

    def __call__(self, agent_id: str, tick: int) -> List[Dict[str, Any]]: ...


class AgentProfileSource(Protocol):
    """Callable that provides the agent's own personality and values."""

    def __call__(self, agent_id: str) -> Optional[AgentProfile]: ...


@dataclass(frozen=True)
class AgentProfile:
    """Agent profile data needed for social context computation."""

    personality: PersonalityVector
    values: ValueWeights
    group_ids: List[str]


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
    ) -> List[Dict[str, Any]]:
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
