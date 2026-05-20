"""Organization governance decision-making for agents.

Provides the GovernanceDecider class that enables agents to make strategic
decisions about governance matters:

1. should_run_for_leader — decide whether to stand for leadership
2. vote_in_election — choose which candidate to support
3. respond_to_treaty — accept, reject, or counter-propose a treaty
4. propose_tax_rate — suggest a tax rate based on org state and self-interest
5. choose_allocation_strategy — pick a resource allocation strategy

Decisions are driven by the agent's own interests (wealth, skills, goals)
relative to the organization's state. The module communicates governance
intents back to the Rust event system via an EventBusProtocol.
"""

from __future__ import annotations

import logging
from dataclasses import dataclass, field
from enum import Enum
from typing import Any, Protocol

logger = logging.getLogger(__name__)


# ---------------------------------------------------------------------------
# Enums
# ---------------------------------------------------------------------------


class LeadershipAmbition(str, Enum):
    """Outcome of a leadership-run decision."""

    RUN = "run"
    ABSTAIN = "abstain"


class TreatyResponse(str, Enum):
    """Possible responses to a treaty proposal."""

    ACCEPT = "accept"
    REJECT = "reject"
    COUNTER = "counter"


class AllocationStrategy(str, Enum):
    """Resource allocation strategy choices."""

    EQUAL = "equal"
    PROPORTIONAL = "proportional"
    NEED_BASED = "need_based"
    MERIT_BASED = "merit_based"


# ---------------------------------------------------------------------------
# Configuration defaults
# ---------------------------------------------------------------------------

# Wealth threshold (relative to org average) above which an agent runs for leader.
LEADERSHIP_WEALTH_THRESHOLD: float = 0.8

# Minimum skill advantage over org average required to run for leader.
LEADERSHIP_SKILL_ADVANTAGE: float = 0.1

# Default tax rate range.
TAX_RATE_MIN: float = 0.05
TAX_RATE_MAX: float = 0.30
TAX_RATE_DEFAULT: float = 0.10

# Interest weight for voting: how much self-interest vs. org benefit.
VOTE_SELF_INTEREST_WEIGHT: float = 0.6
VOTE_ORG_BENEFIT_WEIGHT: float = 0.4


# ---------------------------------------------------------------------------
# Data types
# ---------------------------------------------------------------------------


@dataclass(frozen=True)
class OrgSnapshot:
    """Compact view of organization state used for governance decisions.

    Attributes:
        org_id: Unique organization identifier.
        member_count: Number of current members.
        avg_wealth: Average wealth across all members.
        total_wealth: Total pooled wealth.
        tax_rate: Current tax rate (0.0 - 1.0).
        has_leader: Whether the org currently has a leader.
        leader_id: ID of the current leader, if any.
        treasury: Org treasury balance.
    """

    org_id: str
    member_count: int = 1
    avg_wealth: float = 0.0
    total_wealth: float = 0.0
    tax_rate: float = TAX_RATE_DEFAULT
    has_leader: bool = False
    leader_id: str | None = None
    treasury: float = 0.0


@dataclass(frozen=True)
class AgentInterests:
    """Agent's self-interest profile for governance decisions.

    Attributes:
        agent_id: Unique agent identifier.
        wealth: Agent's current wealth.
        skills: Mapping of skill name to proficiency level (0-100).
        goals: Current goal strings the agent is pursuing.
        risk_tolerance: 0.0 (risk-averse) to 1.0 (risk-seeking).
    """

    agent_id: str
    wealth: float = 0.0
    skills: dict[str, int] = field(default_factory=dict)
    goals: list[str] = field(default_factory=list)
    risk_tolerance: float = 0.5


@dataclass(frozen=True)
class Candidate:
    """A candidate in a leadership election.

    Attributes:
        agent_id: Candidate's agent ID.
        wealth: Candidate's wealth.
        skills: Candidate's skill mapping.
        goals: Candidate's goals.
        reputation: Community reputation score (0.0 - 1.0).
    """

    agent_id: str
    wealth: float = 0.0
    skills: dict[str, int] = field(default_factory=dict)
    goals: list[str] = field(default_factory=dict)
    reputation: float = 0.5


@dataclass(frozen=True)
class Treaty:
    """A treaty proposal between two organizations.

    Attributes:
        treaty_id: Unique treaty identifier.
        proposer_org_id: Organization proposing the treaty.
        terms: Key-value map of treaty terms.
        treaty_type: Type of treaty (trade, defense, non_aggression, etc.).
    """

    treaty_id: str
    proposer_org_id: str
    terms: dict[str, Any] = field(default_factory=dict)
    treaty_type: str = "trade"


@dataclass(frozen=True)
class GovernanceDecision:
    """Result of a governance decision with reasoning.

    Attributes:
        action: The chosen action/strategy/response.
        confidence: 0.0 - 1.0 confidence in this decision.
        reasoning: Human-readable explanation.
    """

    action: str
    confidence: float = 0.5
    reasoning: str = ""


# ---------------------------------------------------------------------------
# Protocol for event bus integration
# ---------------------------------------------------------------------------


class EventBusProtocol(Protocol):
    """Minimal interface for publishing governance events to the event bus.

    The Python side publishes intent events that the Rust world engine
    picks up and processes through its own GovernanceSystem.
    """

    def publish(self, event_type: str, payload: dict[str, Any]) -> None: ...


# ---------------------------------------------------------------------------
# GovernanceDecider
# ---------------------------------------------------------------------------


class GovernanceDecider:
    """Enables agents to make governance decisions within organizations.

    Each decision method is pure (no side effects) and returns a
    GovernanceDecision with the chosen action, confidence, and reasoning.
    The caller is responsible for publishing the decision to the event bus.

    Usage::

        decider = GovernanceDecider()
        decision = decider.should_run_for_leader(org, my_interests)
        if decision.action == "run":
            event_bus.publish("GovernanceIntent", {"intent": "run_for_leader", ...})
    """

    def __init__(
        self,
        *,
        leadership_wealth_threshold: float = LEADERSHIP_WEALTH_THRESHOLD,
        leadership_skill_advantage: float = LEADERSHIP_SKILL_ADVANTAGE,
        vote_self_interest_weight: float = VOTE_SELF_INTEREST_WEIGHT,
        tax_rate_min: float = TAX_RATE_MIN,
        tax_rate_max: float = TAX_RATE_MAX,
    ) -> None:
        self._wealth_threshold = leadership_wealth_threshold
        self._skill_advantage = leadership_skill_advantage
        self._self_interest_weight = vote_self_interest_weight
        self._org_benefit_weight = 1.0 - vote_self_interest_weight
        self._tax_min = tax_rate_min
        self._tax_max = tax_rate_max

    def should_run_for_leader(
        self,
        org: OrgSnapshot,
        my_interests: AgentInterests,
    ) -> GovernanceDecision:
        """Decide whether to run for organization leadership.

        An agent runs for leader when:
        - Their wealth is at or above the org average (they can afford it)
        - They have a skill advantage over the org average
        - The org has no leader or the agent is ambitious (high risk tolerance)

        Args:
            org: Current state of the organization.
            my_interests: The agent's self-interest profile.

        Returns:
            GovernanceDecision with action "run" or "abstain".
        """
        if org.member_count == 0:
            return GovernanceDecision(
                action=LeadershipAmbition.ABSTAIN.value,
                confidence=1.0,
                reasoning="Empty organization, nothing to lead.",
            )

        # Wealth check: is the agent wealthy enough relative to the org?
        wealth_ratio = (
            my_interests.wealth / org.avg_wealth if org.avg_wealth > 0 else 1.0
        )
        wealth_ok = wealth_ratio >= self._wealth_threshold

        # Skill check: does the agent have more/better skills than average?
        my_avg_skill = (
            sum(my_interests.skills.values()) / len(my_interests.skills)
            if my_interests.skills
            else 0.0
        )
        # Heuristic: estimate org avg skill level at 50 (midpoint of 0-100 range)
        estimated_org_skill = 50.0
        skill_advantage = (my_avg_skill - estimated_org_skill) / 100.0
        skill_ok = skill_advantage >= self._skill_advantage

        # Risk tolerance pushes agents with high tolerance to run even if marginal
        risk_bonus = my_interests.risk_tolerance * 0.2

        # Composite willingness score
        willingness = (
            (0.4 if wealth_ok else 0.0)
            + (0.3 if skill_ok else 0.0)
            + (0.1 if not org.has_leader else 0.0)
            + risk_bonus
        )

        should_run = willingness >= 0.5

        if should_run:
            return GovernanceDecision(
                action=LeadershipAmbition.RUN.value,
                confidence=min(1.0, willingness),
                reasoning=(
                    f"Wealth ratio {wealth_ratio:.2f} >= {self._wealth_threshold}, "
                    f"skill advantage {skill_advantage:.2f}, "
                    f"risk tolerance {my_interests.risk_tolerance:.2f}."
                ),
            )

        return GovernanceDecision(
            action=LeadershipAmbition.ABSTAIN.value,
            confidence=1.0 - willingness,
            reasoning=(
                f"Wealth ratio {wealth_ratio:.2f} or skills insufficient. "
                f"Risk tolerance {my_interests.risk_tolerance:.2f} too low to compensate."
            ),
        )

    def vote_in_election(
        self,
        candidates: list[Candidate],
        my_interests: AgentInterests,
    ) -> GovernanceDecision:
        """Choose which candidate to vote for in a leadership election.

        Voting is a weighted mix of self-interest (which candidate benefits
        me most) and org benefit (which candidate is best for the org overall).

        Self-interest is measured by goal alignment and skill complementarity.
        Org benefit is measured by candidate reputation and wealth capability.

        Args:
            candidates: List of candidates standing for election.
            my_interests: The voting agent's interests.

        Returns:
            GovernanceDecision with action = the chosen candidate's agent_id.
            If no candidates, action is "abstain".
        """
        if not candidates:
            return GovernanceDecision(
                action="abstain",
                confidence=1.0,
                reasoning="No candidates to vote for.",
            )

        if len(candidates) == 1:
            return GovernanceDecision(
                action=candidates[0].agent_id,
                confidence=0.8,
                reasoning="Only one candidate; default support.",
            )

        my_goals = set(my_interests.skills.keys()) | set(my_interests.goals)
        best_candidate: Candidate | None = None
        best_score = -1.0

        for candidate in candidates:
            # Self-interest: goal/skill alignment
            cand_goals = set(candidate.skills.keys()) | set(candidate.goals)
            if my_goals and cand_goals:
                overlap = len(my_goals & cand_goals) / len(my_goals | cand_goals)
            else:
                overlap = 0.0

            # Self-interest: wealth proximity (candidates with similar wealth
            # are more likely to share economic interests)
            if my_interests.wealth > 0:
                wealth_proximity = 1.0 - abs(
                    candidate.wealth - my_interests.wealth
                ) / (my_interests.wealth + candidate.wealth + 1e-9)
            else:
                wealth_proximity = 0.5

            self_interest_score = 0.6 * overlap + 0.4 * max(0.0, wealth_proximity)

            # Org benefit: reputation and skill breadth
            avg_skill = (
                sum(candidate.skills.values()) / len(candidate.skills)
                if candidate.skills
                else 0.0
            )
            org_benefit_score = 0.5 * candidate.reputation + 0.5 * (avg_skill / 100.0)

            # Weighted composite
            composite = (
                self_interest_score * self._self_interest_weight
                + org_benefit_score * self._org_benefit_weight
            )

            if composite > best_score:
                best_score = composite
                best_candidate = candidate

        chosen = best_candidate or candidates[0]
        return GovernanceDecision(
            action=chosen.agent_id,
            confidence=min(1.0, best_score),
            reasoning=(
                f"Voted for {chosen.agent_id} with composite score {best_score:.2f}. "
                f"Reputation {chosen.reputation:.2f}, "
                f"goal overlap with voter."
            ),
        )

    def respond_to_treaty(
        self,
        treaty: Treaty,
        my_org: OrgSnapshot,
        other_org: OrgSnapshot,
    ) -> GovernanceDecision:
        """Decide how to respond to a treaty proposal from another org.

        Decision logic:
        - Trade treaties: accept if orgs have different wealth profiles (complementary)
        - Defense treaties: accept if the other org is stronger (protection)
        - Default: accept if terms are favorable, reject otherwise

        Args:
            treaty: The treaty proposal to respond to.
            my_org: The agent's own organization.
            other_org: The proposing organization.

        Returns:
            GovernanceDecision with action "accept", "reject", or "counter".
        """
        if my_org.member_count == 0 or other_org.member_count == 0:
            return GovernanceDecision(
                action=TreatyResponse.REJECT.value,
                confidence=1.0,
                reasoning="One or both organizations are empty.",
            )

        if treaty.treaty_type == "trade":
            return self._evaluate_trade_treaty(treaty, my_org, other_org)
        elif treaty.treaty_type == "defense":
            return self._evaluate_defense_treaty(treaty, my_org, other_org)
        elif treaty.treaty_type == "non_aggression":
            return self._evaluate_non_aggression_treaty(treaty, my_org, other_org)
        else:
            return self._evaluate_generic_treaty(treaty, my_org, other_org)

    def propose_tax_rate(
        self,
        org: OrgSnapshot,
        my_wealth: float,
    ) -> GovernanceDecision:
        """Propose a tax rate based on the org's financial state and self-interest.

        Agents with wealth above average tend to propose lower taxes.
        Agents with wealth below average tend to propose higher taxes.
        The proposal is clamped to the configured min/max range.

        Args:
            org: Current organization state.
            my_wealth: The proposing agent's wealth.

        Returns:
            GovernanceDecision with action = the proposed tax rate as a string.
        """
        if org.avg_wealth <= 0:
            proposed = TAX_RATE_DEFAULT
        else:
            wealth_ratio = my_wealth / org.avg_wealth

            if wealth_ratio > 1.5:
                # Wealthy agents prefer low taxes
                proposed = self._tax_min + (TAX_RATE_DEFAULT - self._tax_min) * 0.3
            elif wealth_ratio > 1.0:
                # Slightly above average: moderate-low taxes
                proposed = TAX_RATE_DEFAULT
            elif wealth_ratio > 0.5:
                # Below average: moderate-high taxes for redistribution
                proposed = TAX_RATE_DEFAULT + (self._tax_max - TAX_RATE_DEFAULT) * 0.4
            else:
                # Well below average: favor higher taxes
                proposed = self._tax_max * 0.8

        # Consider treasury health: low treasury → higher taxes
        if org.treasury < org.total_wealth * 0.1:
            proposed = min(self._tax_max, proposed * 1.2)

        # Clamp to configured bounds
        proposed = max(self._tax_min, min(self._tax_max, proposed))

        if org.avg_wealth > 0:
            wealth_ratio = my_wealth / org.avg_wealth
        else:
            wealth_ratio = 1.0

        return GovernanceDecision(
            action=f"{proposed:.4f}",
            confidence=0.7,
            reasoning=(
                f"Proposed tax rate {proposed:.4f} based on wealth ratio "
                f"{wealth_ratio:.2f} and treasury {org.treasury:.1f}."
            ),
        )

    def choose_allocation_strategy(
        self,
        org: OrgSnapshot,
    ) -> GovernanceDecision:
        """Choose a resource allocation strategy for the organization.

        Decision logic:
        - Small, equal orgs → EQUAL distribution
        - Diverse skill/member profiles → PROPORTIONAL
        - Low treasury with many members → NEED_BASED
        - High variance in member contributions → MERIT_BASED

        Args:
            org: Current organization state.

        Returns:
            GovernanceDecision with action = the chosen AllocationStrategy value.
        """
        if org.member_count <= 1:
            return GovernanceDecision(
                action=AllocationStrategy.EQUAL.value,
                confidence=1.0,
                reasoning="Single-member org; equal is the only option.",
            )

        # Low treasury → need-based allocation to support struggling members
        if org.avg_wealth > 0 and org.treasury < org.avg_wealth * 0.5:
            return GovernanceDecision(
                action=AllocationStrategy.NEED_BASED.value,
                confidence=0.8,
                reasoning=(
                    f"Treasury {org.treasury:.1f} is low relative to avg wealth "
                    f"{org.avg_wealth:.1f}; need-based allocation prioritized."
                ),
            )

        # Large org → proportional for fairness
        if org.member_count >= 10:
            return GovernanceDecision(
                action=AllocationStrategy.PROPORTIONAL.value,
                confidence=0.75,
                reasoning=(
                    f"Large org with {org.member_count} members; "
                    f"proportional allocation for scale."
                ),
            )

        # High tax rate suggests a meritocratic culture → merit-based
        if org.tax_rate > 0.2:
            return GovernanceDecision(
                action=AllocationStrategy.MERIT_BASED.value,
                confidence=0.7,
                reasoning=(
                    f"High tax rate {org.tax_rate:.2f} suggests "
                    f"merit-based allocation is appropriate."
                ),
            )

        # Default: equal distribution for small, healthy orgs
        return GovernanceDecision(
            action=AllocationStrategy.EQUAL.value,
            confidence=0.6,
            reasoning=(
                f"Small, healthy org with {org.member_count} members; "
                f"equal distribution for cohesion."
            ),
        )

    # ------------------------------------------------------------------
    # Treaty evaluation helpers
    # ------------------------------------------------------------------

    def _evaluate_trade_treaty(
        self,
        treaty: Treaty,
        my_org: OrgSnapshot,
        other_org: OrgSnapshot,
    ) -> GovernanceDecision:
        """Trade treaties are beneficial when orgs have complementary wealth."""
        if my_org.avg_wealth <= 0 and other_org.avg_wealth <= 0:
            return GovernanceDecision(
                action=TreatyResponse.REJECT.value,
                confidence=0.9,
                reasoning="Both orgs have zero wealth; no trade benefit.",
            )

        # Complementary: one is richer, the other poorer (trade flows both ways)
        if my_org.avg_wealth > 0 and other_org.avg_wealth > 0:
            ratio = my_org.avg_wealth / other_org.avg_wealth
            is_complementary = ratio < 0.5 or ratio > 2.0
        else:
            is_complementary = True

        if is_complementary:
            return GovernanceDecision(
                action=TreatyResponse.ACCEPT.value,
                confidence=0.8,
                reasoning=(
                    f"Trade treaty with {other_org.org_id}: complementary wealth "
                    f"profiles (avg {my_org.avg_wealth:.1f} vs {other_org.avg_wealth:.1f})."
                ),
            )

        return GovernanceDecision(
            action=TreatyResponse.COUNTER.value,
            confidence=0.6,
            reasoning=(
                f"Trade treaty with {other_org.org_id}: similar wealth profiles; "
                f"counter-proposing more favorable terms."
            ),
        )

    def _evaluate_defense_treaty(
        self,
        treaty: Treaty,
        my_org: OrgSnapshot,
        other_org: OrgSnapshot,
    ) -> GovernanceDecision:
        """Defense treaties: accept if the other org brings military value."""
        # Heuristic: accept if other org has more members (more defenders)
        if other_org.member_count >= my_org.member_count:
            return GovernanceDecision(
                action=TreatyResponse.ACCEPT.value,
                confidence=0.75,
                reasoning=(
                    f"Defense treaty with {other_org.org_id}: they have "
                    f"{other_org.member_count} members vs our {my_org.member_count}."
                ),
            )

        return GovernanceDecision(
            action=TreatyResponse.REJECT.value,
            confidence=0.6,
            reasoning=(
                f"Defense treaty with {other_org.org_id}: they are smaller "
                f"({other_org.member_count} members); limited defense value."
            ),
        )

    def _evaluate_non_aggression_treaty(
        self,
        treaty: Treaty,
        my_org: OrgSnapshot,
        other_org: OrgSnapshot,
    ) -> GovernanceDecision:
        """Non-aggression pacts are generally beneficial; accept unless at a disadvantage."""
        return GovernanceDecision(
            action=TreatyResponse.ACCEPT.value,
            confidence=0.85,
            reasoning=(
                f"Non-aggression treaty with {other_org.org_id}: "
                f"reduces conflict risk."
            ),
        )

    def _evaluate_generic_treaty(
        self,
        treaty: Treaty,
        my_org: OrgSnapshot,
        other_org: OrgSnapshot,
    ) -> GovernanceDecision:
        """Generic treaties: accept if terms are non-empty, otherwise reject."""
        if treaty.terms:
            return GovernanceDecision(
                action=TreatyResponse.ACCEPT.value,
                confidence=0.5,
                reasoning=(
                    f"Generic treaty {treaty.treaty_type} with {other_org.org_id}: "
                    f"terms provided, defaulting to acceptance."
                ),
            )

        return GovernanceDecision(
            action=TreatyResponse.REJECT.value,
            confidence=0.7,
            reasoning=(
                f"Generic treaty {treaty.treaty_type} with {other_org.org_id}: "
                f"no terms specified; cannot evaluate."
            ),
        )
