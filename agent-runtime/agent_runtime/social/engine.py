"""Social engine — orchestrates all social modules for the agent think loop.

The SocialEngine is the single entry point that the decision layer calls into.
It aggregates personality-based social propensity, inter-group trust, cultural
diffusion, imitation, and knowledge-transfer into a unified ``SocialContext``
that the decide step uses to inform the SOCIALIZE action.

Usage::

    from agent_runtime.social.engine import SocialEngine

    engine = SocialEngine()
    ctx = engine.build_context(
        agent_id="agent-1",
        personality=pers,
        values=vals,
        nearby_agents=[...],
        tick=42,
    )
    # ctx.social_propensity  -> float [0, 1]
    # ctx.trust_snapshot     -> dict[str, float]
    # ctx.recommended_target -> str | None
"""

from __future__ import annotations

import logging
import math
from dataclasses import dataclass, field
from typing import Any

from agent_runtime.models.personality import PersonalityVector
from agent_runtime.models.values import ValueWeights

from .cultural_conflict import AgentInteraction, CulturalConflictAndFusion
from .cultural_diffusion import CulturalDiffusion
from .imitation import ImitationEngine
from .intergroup_trust import (
    DEFAULT_OUT_GROUP_TRUST,
    InterGroupEvent,
    InterGroupEventType,
    IntergroupTrust,
)
from .knowledge_transfer import KnowledgeTransfer
from .org_culture import OrgCultureSystem
from .regional_culture import RegionalCulture

logger = logging.getLogger(__name__)


# ---------------------------------------------------------------------------
# Social context — what the decide step receives
# ---------------------------------------------------------------------------

@dataclass(frozen=True)
class SocialTarget:
    """A candidate for social interaction."""

    agent_id: str
    trust_value: float = 0.5  # [0, 1]
    personality_distance: float = 0.0
    same_group: bool = False
    group_id: str = ""


@dataclass(frozen=True)
class SocialContext:
    """Aggregated social context for a single agent at decision time."""

    # Overall social propensity derived from personality + values.
    # Higher = agent is more inclined to socialize this tick.
    social_propensity: float = 0.5  # [0, 1]

    # Whether socializing is recommended at all.
    should_socialize: bool = False

    # Nearby agents that are viable social targets, ranked by affinity.
    targets: list[SocialTarget] = field(default_factory=list)

    # Best target (highest affinity) if should_socialize is True.
    recommended_target: SocialTarget | None = None

    # Trust snapshot: target_agent_id -> trust_value [0, 1].
    trust_snapshot: dict[str, float] = field(default_factory=dict)

    # Personality summary for LLM prompt injection.
    personality_description: str = ""

    # Tick when this context was computed.
    tick: int = 0


# ---------------------------------------------------------------------------
# SocialEngine
# ---------------------------------------------------------------------------

class SocialEngine:
    """Orchestrates personality, trust, cultural diffusion, and imitation
    to produce a ``SocialContext`` that the decision layer consumes.

    Each sub-module is instantiated lazily so the engine is cheap to create.
    """

    def __init__(self) -> None:
        self._trust = IntergroupTrust()
        self._cultural_diffusion = CulturalDiffusion()
        self._imitation = ImitationEngine()
        self._knowledge_transfer = KnowledgeTransfer()
        self._conflict = CulturalConflictAndFusion()
        self._org_culture = OrgCultureSystem()
        self._regional_culture = RegionalCulture()

    # ── Main entry point: build social context for a single agent ──

    def build_context(
        self,
        agent_id: str,
        personality: PersonalityVector,
        values: ValueWeights,
        nearby_agents: list[dict[str, Any]],
        tick: int,
        *,
        agent_groups: list[str] | None = None,
    ) -> SocialContext:
        """Build the social context for an agent at decision time.

        Args:
            agent_id: The deciding agent's ID.
            personality: The agent's personality vector.
            values: The agent's current value weights.
            nearby_agents: List of dicts with keys ``agent_id``, ``personality``
                (PersonalityVector), ``values`` (ValueWeights), and optional
                ``group_ids`` (list[str]).
            tick: Current world tick.
            agent_groups: Groups the deciding agent belongs to.

        Returns:
            A SocialContext summarizing social propensity, targets, and trust.
        """
        groups = set(agent_groups or [])

        # 1. Compute social propensity from personality + values
        social_propensity = self._compute_social_propensity(
            personality, values
        )

        # 2. Evaluate each nearby agent as a social target
        targets: list[SocialTarget] = []
        trust_snapshot: dict[str, float] = {}

        for nearby in nearby_agents:
            # Defensive key access: the REST perception API returns "id"
            # while the gRPC path returns "agent_id".  Skip entries that
            # have neither key so one bad dict can't nuke the whole context.
            target_id: str = nearby.get("agent_id") or nearby.get("id") or ""
            if not target_id:
                logger.debug(
                    "Skipping nearby agent entry without agent_id/id: %s",
                    nearby,
                )
                continue
            target_pers: PersonalityVector = nearby.get(
                "personality", PersonalityVector()
            )
            target_groups = set(nearby.get("group_ids", []))

            # Determine if they share any group
            shared_groups = groups & target_groups
            same_group = len(shared_groups) > 0

            # Compute trust
            if same_group and shared_groups:
                # Use first shared group for in-group trust
                trust_val = self._trust.compute_in_group_trust(
                    agent_id, next(iter(shared_groups))
                )
            elif target_groups:
                # Out-group: check trust toward each target group, take max
                trust_val = max(
                    self._trust.compute_out_group_trust(agent_id, gid)
                    for gid in target_groups
                )
            else:
                trust_val = DEFAULT_OUT_GROUP_TRUST

            trust_snapshot[target_id] = trust_val

            # Personality distance for affinity scoring
            pers_distance = personality.distance(target_pers)

            targets.append(
                SocialTarget(
                    agent_id=target_id,
                    trust_value=trust_val,
                    personality_distance=pers_distance,
                    same_group=same_group,
                    group_id=next(iter(shared_groups), ""),
                )
            )

        # 3. Rank targets by affinity (trust + similarity)
        targets.sort(key=self._target_affinity_score, reverse=True)

        # 4. Decide whether agent should socialize
        should_socialize = self._should_socialize(
            social_propensity, targets
        )

        # 5. Pick recommended target
        recommended = targets[0] if should_socialize and targets else None

        return SocialContext(
            social_propensity=social_propensity,
            should_socialize=should_socialize,
            targets=targets,
            recommended_target=recommended,
            trust_snapshot=trust_snapshot,
            personality_description=personality.to_prompt_description(),
            tick=tick,
        )

    # ── Social propensity ──

    @staticmethod
    def _compute_social_propensity(
        personality: PersonalityVector,
        values: ValueWeights,
    ) -> float:
        """Compute how inclined an agent is to socialize this tick.

        Weighted blend of personality dimensions and value weights:
          - extraversion (40%): primary driver
          - social_orientation (25%): group-oriented tendency
          - agreeableness (15%): cooperative tendency
          - cooperation_weight (15%): value preference
          - inverse neuroticism (5%): low neuroticism → more social

        Returns:
            Float in [0, 1].
        """
        raw = (
            0.40 * personality.extraversion
            + 0.25 * personality.social_orientation
            + 0.15 * personality.agreeableness
            + 0.15 * values.cooperation_weight
            + 0.05 * (1.0 - personality.neuroticism)
        )
        return max(0.0, min(1.0, raw))

    @staticmethod
    def _target_affinity_score(target: SocialTarget) -> float:
        """Score a social target for ranking. Higher = more attractive."""
        # Trust is the strongest signal, then similarity
        similarity = max(0.0, 1.0 - target.personality_distance / math.sqrt(8))
        in_group_bonus = 0.15 if target.same_group else 0.0
        return 0.5 * target.trust_value + 0.35 * similarity + in_group_bonus

    @staticmethod
    def _should_socialize(
        propensity: float,
        targets: list[SocialTarget],
    ) -> bool:
        """Decide if the agent should socialize this tick.

        An agent socializes when:
        1. Social propensity exceeds a threshold (0.4)
        2. There is at least one nearby agent to interact with
        """
        if not targets:
            return False
        return propensity >= 0.4

    # ── Social action execution ──

    def execute_socialize(
        self,
        agent_id: str,
        target_id: str,
        personality: PersonalityVector,
        values: ValueWeights,
        target_personality: PersonalityVector,
        target_values: ValueWeights,
        tick: int,
    ) -> dict[str, Any]:
        """Execute the SOCIALIZE action — perform social interactions.

        This triggers:
        1. Imitation check (if target is successful)
        2. Cultural conflict detection
        3. Trust update (cooperation event)

        Args:
            agent_id: The initiating agent's ID.
            target_id: The target agent's ID.
            personality: Initiator's personality.
            values: Initiator's values.
            target_personality: Target's personality.
            target_values: Target's values.
            tick: Current tick.

        Returns:
            Dict with interaction results.
        """
        results: dict[str, Any] = {
            "agent_id": agent_id,
            "target_id": target_id,
            "tick": tick,
            "imitation": None,
            "conflict": None,
            "trust_update": None,
        }

        # 1. Imitation check (success_score = 1 - personality_distance / max_distance)
        max_dist = math.sqrt(8)
        distance = personality.distance(target_personality)
        success_score = max(0.0, 1.0 - distance / max_dist)

        imitation_result = self._imitation.observe_and_maybe_imitate(
            observer_personality=personality,
            observer_values=values,
            observed_personality=target_personality,
            observed_values=target_values,
            observed_success_score=success_score,
            context={"tick": tick},
        )
        results["imitation"] = imitation_result

        # 2. Cultural conflict detection
        interaction = AgentInteraction(
            agent_a_id=agent_id,
            agent_b_id=target_id,
            agent_a_values=values,
            agent_b_values=target_values,
            agent_a_personality=personality,
            agent_b_personality=target_personality,
            interaction_type="socialize",
            tick=tick,
        )
        conflict_report = self._conflict.detect_cultural_conflict(interaction)
        results["conflict"] = (
            conflict_report.model_dump() if conflict_report else None
        )

        # 3. Trust update — socialize is a cooperation event
        event = InterGroupEvent(
            event_type=InterGroupEventType.COOPERATION,
            source_group=agent_id,
            target_group=target_id,
            tick=tick,
        )
        self._trust.update_trust_from_event(event)
        results["trust_update"] = {
            "event": "cooperation",
            "new_trust": self._trust.get_trust(agent_id, target_id),
        }

        return results

    # ── Cultural diffusion per tick ──

    def apply_tick_diffusion(
        self,
        agents_by_region: dict[str, list[dict[str, Any]]],
    ) -> list[dict[str, Any]]:
        """Apply regional cultural diffusion for one tick.

        Call this once per tick (not per agent).

        Args:
            agents_by_region: Mapping of region_id -> list of agent dicts
                (each with ``agent_id``, ``values``, ``personality``).

        Returns:
            List of diffusion result dicts, one per region.
        """
        results = []
        for region_id, agents in agents_by_region.items():
            result = self._cultural_diffusion.apply_regional_influence(
                agents=agents,
                region_id=region_id,
            )
            results.append(result)
        return results

    # ── Accessors for sub-modules ──

    @property
    def trust(self) -> IntergroupTrust:
        """Direct access to the intergroup trust system."""
        return self._trust

    @property
    def cultural_diffusion(self) -> CulturalDiffusion:
        return self._cultural_diffusion

    @property
    def imitation(self) -> ImitationEngine:
        return self._imitation

    @property
    def knowledge_transfer(self) -> KnowledgeTransfer:
        return self._knowledge_transfer

    @property
    def conflict(self) -> CulturalConflictAndFusion:
        return self._conflict

    @property
    def org_culture(self) -> OrgCultureSystem:
        return self._org_culture

    @property
    def regional_culture(self) -> RegionalCulture:
        return self._regional_culture
