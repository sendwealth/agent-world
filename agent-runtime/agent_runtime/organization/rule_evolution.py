"""Rule evolution tracker — monitors rule lifecycle and suggests reforms.

Tracks the lifecycle of soft rules from creation through modification to
repeal, detects stale rules, and suggests reform directions based on
governance efficiency metrics.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from enum import StrEnum
from typing import Any

# ---------------------------------------------------------------------------
# Enums
# ---------------------------------------------------------------------------


class RuleLifecycleEventType(StrEnum):
    """Types of events in a rule's lifecycle."""

    PROPOSED = "proposed"
    ACTIVATED = "activated"
    SUSPENDED = "suspended"
    RESUMED = "resumed"
    EXPIRED = "expired"
    REPEALED = "repealed"
    TRIGGERED = "triggered"
    EFFECT_APPLIED = "effect_applied"


# ---------------------------------------------------------------------------
# Data classes
# ---------------------------------------------------------------------------


@dataclass(frozen=True)
class RuleLifecycleEvent:
    """A single event in a rule's lifecycle."""

    rule_id: str
    org_id: str
    event_type: RuleLifecycleEventType
    tick: int
    details: str = ""


@dataclass
class RuleStats:
    """Statistics for a single rule."""

    rule_id: str
    org_id: str
    title: str
    status: str
    created_tick: int
    trigger_count: int = 0
    last_triggered_tick: int | None = None
    lifecycle_events: list[RuleLifecycleEvent] = field(default_factory=list)


# ---------------------------------------------------------------------------
# Rule Evolution Tracker
# ---------------------------------------------------------------------------

# Default threshold ticks for stale rule detection
_DEFAULT_STALE_THRESHOLD: int = 500


class RuleEvolutionTracker:
    """Track the lifecycle of soft rules across organizations.

    Records rule events, computes statistics, detects stale rules, and
    suggests reform directions based on governance performance metrics.
    """

    def __init__(self, *, stale_threshold_ticks: int = _DEFAULT_STALE_THRESHOLD) -> None:
        self._stale_threshold = stale_threshold_ticks
        self._stats: dict[str, RuleStats] = {}
        self._events: list[RuleLifecycleEvent] = []

    # ── Record Events ──────────────────────────────────────

    def record_event(self, event: RuleLifecycleEvent) -> None:
        """Record a rule lifecycle event."""
        self._events.append(event)

        # Update stats
        if event.rule_id not in self._stats:
            self._stats[event.rule_id] = RuleStats(
                rule_id=event.rule_id,
                org_id=event.org_id,
                title="",
                status="active",
                created_tick=event.tick,
            )
        stats = self._stats[event.rule_id]
        stats.lifecycle_events.append(event)

        if event.event_type == RuleLifecycleEventType.TRIGGERED:
            stats.trigger_count += 1
            stats.last_triggered_tick = event.tick

        if event.event_type == RuleLifecycleEventType.REPEALED:
            stats.status = "repealed"
        elif event.event_type == RuleLifecycleEventType.EXPIRED:
            stats.status = "expired"
        elif event.event_type == RuleLifecycleEventType.ACTIVATED:
            stats.status = "active"
        elif event.event_type == RuleLifecycleEventType.SUSPENDED:
            stats.status = "suspended"

    def record_rule_creation(
        self, rule_id: str, org_id: str, title: str, tick: int
    ) -> None:
        """Record the creation of a new rule."""
        self._stats[rule_id] = RuleStats(
            rule_id=rule_id,
            org_id=org_id,
            title=title,
            status="proposed",
            created_tick=tick,
        )
        self.record_event(RuleLifecycleEvent(
            rule_id=rule_id,
            org_id=org_id,
            event_type=RuleLifecycleEventType.PROPOSED,
            tick=tick,
            details=title,
        ))

    def record_rule_activation(self, rule_id: str, org_id: str, tick: int) -> None:
        """Record activation of a rule."""
        self.record_event(RuleLifecycleEvent(
            rule_id=rule_id,
            org_id=org_id,
            event_type=RuleLifecycleEventType.ACTIVATED,
            tick=tick,
        ))

    def record_rule_trigger(self, rule_id: str, org_id: str, tick: int) -> None:
        """Record that a rule was triggered (conditions matched)."""
        self.record_event(RuleLifecycleEvent(
            rule_id=rule_id,
            org_id=org_id,
            event_type=RuleLifecycleEventType.TRIGGERED,
            tick=tick,
        ))

    # ── Query ──────────────────────────────────────────────

    def track_rule_lifecycle(self, org_id: str) -> list[RuleLifecycleEvent]:
        """Return the full timeline of rule events for an organization."""
        return [e for e in self._events if e.org_id == org_id]

    def get_rule_stats(self, rule_id: str) -> RuleStats | None:
        """Get statistics for a specific rule."""
        return self._stats.get(rule_id)

    def get_org_rules(self, org_id: str) -> list[RuleStats]:
        """Get all rule stats for an organization."""
        return [s for s in self._stats.values() if s.org_id == org_id]

    # ── Stale Detection ────────────────────────────────────

    def detect_stale_rules(
        self,
        org_id: str,
        current_tick: int,
        threshold_ticks: int | None = None,
    ) -> list[str]:
        """Detect rules that haven't been triggered recently.

        A rule is stale if:
        - It is active
        - It has been active for more than threshold_ticks
        - It has never been triggered OR was last triggered more than threshold_ticks ago

        Returns:
            List of stale rule IDs.
        """
        threshold = threshold_ticks or self._stale_threshold
        stale = []
        for stats in self.get_org_rules(org_id):
            if stats.status != "active":
                continue
            age = current_tick - stats.created_tick
            if age < threshold:
                continue
            if stats.last_triggered_tick is None:
                stale.append(stats.rule_id)
            elif current_tick - stats.last_triggered_tick > threshold:
                stale.append(stats.rule_id)
        return stale

    # ── Reform Suggestions ─────────────────────────────────

    def suggest_rule_reform(
        self,
        org_id: str,
        performance_metrics: dict[str, Any],
    ) -> list[str]:
        """Suggest rule reform directions based on governance performance.

        Analyzes current active rules and performance metrics to identify
        improvement areas.
        """
        suggestions: list[str] = []
        rules = self.get_org_rules(org_id)
        active = [r for r in rules if r.status == "active"]

        # If no rules exist, suggest creating some
        if not active:
            suggestions.append(
                "No active rules — consider proposing rules to improve governance"
            )
            return suggestions

        # Check overall trigger rate
        total_triggers = sum(r.trigger_count for r in active)
        if total_triggers == 0:
            suggestions.append(
                "No rules have been triggered — conditions may be too restrictive"
            )

        # Check economic performance
        economic_output = performance_metrics.get("economic_output", 0)
        expected_output = performance_metrics.get("expected_output", 0)
        if expected_output > 0 and economic_output / expected_output < 0.7:
            # Check if there are trade rules
            trade_rules = [r for r in active if "trade" in r.title.lower()]
            if not trade_rules:
                suggestions.append(
                    "Economic output below 70% expected — consider proposing trade rules"
                )
            else:
                suggestions.append(
                    "Trade rules exist but performance is low — consider reforming conditions"
                )

        # Check safety performance
        incident_rate = performance_metrics.get("incident_rate", 0.0)
        if incident_rate > 0.2:
            behavior_rules = [r for r in active if "behavior" in r.title.lower()]
            if not behavior_rules:
                suggestions.append(
                    "High incident rate — consider proposing behavioral constraints"
                )

        # Stale rule cleanup suggestion
        stale_count = len([r for r in active if r.trigger_count == 0])
        if stale_count > 0:
            suggestions.append(
                f"{stale_count} rule(s) have never triggered — consider repealing stale rules"
            )

        return suggestions
