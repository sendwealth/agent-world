"""Rule proposal engine — Agent-driven rule creation and campaigning.

Agents use this module to evaluate whether they should propose new rules,
generate structured rule proposals, and campaign for support among
organization members.
"""

from __future__ import annotations

import uuid
from dataclasses import dataclass
from enum import Enum
from typing import Any, Protocol

# ---------------------------------------------------------------------------
# Enums
# ---------------------------------------------------------------------------


class RuleCategory(str, Enum):
    """Category of a soft rule."""

    TAX = "tax"
    BEHAVIOR = "behavior"
    TRADE = "trade"
    DIPLOMACY = "diplomacy"
    CUSTOM = "custom"


# ---------------------------------------------------------------------------
# Data classes
# ---------------------------------------------------------------------------


@dataclass(frozen=True)
class RuleCondition:
    """A trigger condition for a rule.

    field: dot-path into world state (e.g. "agent.tokens", "org.treasury")
    operator: one of ">", "<", "==", ">=", "<=", "contains"
    value: the value to compare against
    """

    field: str
    operator: str
    value: Any


@dataclass(frozen=True)
class RuleEffect:
    """An effect applied when conditions match.

    target: dot-path to target field
    action: "add", "subtract", "multiply", "set", "block_action"
    value: value used by the action
    """

    target: str
    action: str
    value: Any


@dataclass(frozen=True)
class RuleProposal:
    """A complete rule proposal ready for submission."""

    proposal_id: str
    proposer_id: str
    org_id: str
    title: str
    description: str
    rule_type: RuleCategory
    conditions: tuple[RuleCondition, ...]
    effects: tuple[RuleEffect, ...]
    expires_ticks: int | None = None


# ---------------------------------------------------------------------------
# Trigger thresholds
# ---------------------------------------------------------------------------

# Resource inequality: Gini coefficient above this triggers a tax proposal
_RESOURCE_INEQUALITY_THRESHOLD: float = 0.4

# Safety: ratio of attacks to population above this triggers a behavior rule
_SAFETY_INCIDENT_RATIO: float = 0.3

# Economic efficiency: output ratio below this triggers a trade rule
_ECONOMIC_EFFICIENCY_THRESHOLD: float = 0.5


# ---------------------------------------------------------------------------
# Protocols
# ---------------------------------------------------------------------------


class LLMProviderProtocol(Protocol):
    """Minimal interface for LLM text generation."""

    async def generate(self, prompt: str) -> str: ...


# ---------------------------------------------------------------------------
# Rule Proposal Engine
# ---------------------------------------------------------------------------


class RuleProposalEngine:
    """Agent rule proposal decision engine.

    Evaluates whether an agent should propose a new rule based on observed
    conditions, generates structured proposals, and produces campaign
    messages for gathering support.
    """

    def __init__(
        self,
        *,
        inequality_threshold: float = _RESOURCE_INEQUALITY_THRESHOLD,
        safety_threshold: float = _SAFETY_INCIDENT_RATIO,
        efficiency_threshold: float = _ECONOMIC_EFFICIENCY_THRESHOLD,
    ) -> None:
        self._inequality_threshold = inequality_threshold
        self._safety_threshold = safety_threshold
        self._efficiency_threshold = efficiency_threshold

    # ── Should Propose ─────────────────────────────────────

    def should_propose_rule(
        self,
        agent_id: str,
        org_id: str,
        context: dict[str, Any],
    ) -> tuple[bool, str]:
        """Evaluate whether the agent should propose a new rule.

        Trigger conditions:
        - Resource inequality within the org (Gini > threshold)
        - Safety incidents (attack ratio > threshold)
        - Economic inefficiency (output ratio < threshold)
        - External threat (rival org expanding)

        Returns:
            (should_propose, reason) — reason is human-readable.
        """
        members = context.get("members", [])
        if not members:
            return False, "no members in organization"

        # Check resource inequality (Gini coefficient)
        resources = [m.get("resources", 0) for m in members]
        gini = _gini_coefficient(resources)
        if gini > self._inequality_threshold:
            return (
                True,
                f"resource inequality detected (Gini={gini:.2f} > "
                f"{self._inequality_threshold:.2f})",
            )

        # Check safety incidents
        attacks = context.get("recent_attacks", 0)
        population = len(members)
        if population > 0 and attacks / population > self._safety_threshold:
            return (
                True,
                f"safety concern: {attacks} attacks on {population} members",
            )

        # Check economic efficiency
        output = context.get("economic_output", 0)
        expected = context.get("expected_output", 0)
        if expected > 0 and output / expected < self._efficiency_threshold:
            return (
                True,
                f"economic inefficiency: output {output} < "
                f"{self._efficiency_threshold * expected:.0f} expected",
            )

        # Check external threat
        rival_expansion = context.get("rival_org_expansion", False)
        if rival_expansion:
            return True, "external threat: rival organization expanding"

        return False, "no triggering conditions met"

    # ── Generate Proposal ──────────────────────────────────

    def generate_rule_proposal(
        self,
        agent_id: str,
        org_id: str,
        issue: str,
        context: dict[str, Any],
    ) -> RuleProposal:
        """Generate a structured rule proposal based on the identified issue.

        The proposal includes conditions and effects derived from the issue
        and context. This is a deterministic generator — LLM integration
        for natural language descriptions can wrap this.
        """
        proposal_id = str(uuid.uuid4())
        rule_type, conditions, effects = _derive_rule_from_issue(issue, context)

        return RuleProposal(
            proposal_id=proposal_id,
            proposer_id=agent_id,
            org_id=org_id,
            title=_generate_title(issue, rule_type),
            description=_generate_description(issue, context),
            rule_type=rule_type,
            conditions=tuple(conditions),
            effects=tuple(effects),
        )

    # ── Campaign ───────────────────────────────────────────

    def campaign_for_rule(
        self,
        agent_id: str,
        rule: RuleProposal,
    ) -> str:
        """Generate a campaign message for the proposed rule.

        The message explains why the rule is needed and urges support.
        In production, an LLM would generate this; here we produce
        a structured template.
        """
        cond_desc = ", ".join(
            f"{c.field} {c.operator} {c.value}" for c in rule.conditions
        )
        effect_desc = ", ".join(
            f"{e.action} {e.value} on {e.target}" for e in rule.effects
        )
        return (
            f"Fellow members, I propose we adopt the rule "
            f'"{rule.title}". '
            f"Problem: {rule.description}. "
            f"When {cond_desc}, we will {effect_desc}. "
            f"Please vote in favor to strengthen our organization."
        )


# ---------------------------------------------------------------------------
# Helper functions
# ---------------------------------------------------------------------------


def _gini_coefficient(values: list[float]) -> float:
    """Compute the Gini coefficient for a list of values."""
    if not values:
        return 0.0
    n = len(values)
    if n == 1:
        return 0.0
    sorted_vals = sorted(values)
    cumsum = 0.0
    weighted_sum = 0.0
    for i, v in enumerate(sorted_vals):
        cumsum += v
        weighted_sum += (i + 1) * v
    total = cumsum
    if total == 0:
        return 0.0
    return (2 * weighted_sum) / (n * total) - (n + 1) / n


def _derive_rule_from_issue(
    issue: str,
    context: dict[str, Any],
) -> tuple[RuleCategory, list[RuleCondition], list[RuleEffect]]:
    """Derive rule type, conditions, and effects from an issue description."""
    issue_lower = issue.lower()

    if "inequality" in issue_lower or "gini" in issue_lower or "tax" in issue_lower:
        avg_resources = context.get("avg_member_resources", 100)
        return (
            RuleCategory.TAX,
            [RuleCondition(
                field="agent.resources",
                operator=">",
                value=avg_resources * 2,
            )],
            [RuleEffect(
                target="agent.tax_bonus",
                action="set",
                value=0.1,
            )],
        )

    if "safety" in issue_lower or "attack" in issue_lower:
        return (
            RuleCategory.BEHAVIOR,
            [RuleCondition(
                field="agent.under_attack",
                operator="==",
                value=True,
            )],
            [RuleEffect(
                target="agent.defense_bonus",
                action="set",
                value=10,
            )],
        )

    if "trade" in issue_lower or "efficiency" in issue_lower or "economic" in issue_lower:
        return (
            RuleCategory.TRADE,
            [RuleCondition(
                field="agent.trade_volume",
                operator="<",
                value=context.get("min_trade_volume", 10),
            )],
            [RuleEffect(
                target="agent.trade_bonus",
                action="add",
                value=5,
            )],
        )

    if "rival" in issue_lower or "diplomacy" in issue_lower or "threat" in issue_lower:
        return (
            RuleCategory.DIPLOMACY,
            [RuleCondition(
                field="org.is_allied",
                operator="==",
                value=False,
            )],
            [RuleEffect(
                target="agent.interaction_limit",
                action="set",
                value=True,
            )],
        )

    # Default: custom rule
    return (
        RuleCategory.CUSTOM,
        [RuleCondition(
            field="world.tick",
            operator=">=",
            value=0,
        )],
        [RuleEffect(
            target="agent.custom_effect",
            action="set",
            value=True,
        )],
    )


def _generate_title(issue: str, rule_type: RuleCategory) -> str:
    """Generate a short title for a rule based on the issue."""
    type_prefix = rule_type.value.capitalize()
    words = issue.split()[:5]
    return f"{type_prefix} Rule: {' '.join(words)}"


def _generate_description(issue: str, context: dict[str, Any]) -> str:
    """Generate a description for a rule based on the issue and context."""
    org_name = context.get("org_name", "our organization")
    return f"To protect {org_name}: {issue}"
