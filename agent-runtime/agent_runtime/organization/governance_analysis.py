"""Governance analysis — stability, comparison, prediction, and reporting.

Provides the GovernanceAnalyzer class that operates on historical governance
event data to produce analytical insights:

1. analyze_org_stability — measure governance stability of a single org
2. compare_governance_models — compare multiple orgs' governance patterns
3. predict_leadership_change — estimate leadership turnover risk
4. export_governance_report — generate a structured report for export
"""

from __future__ import annotations

import json
from dataclasses import dataclass, field
from enum import Enum
from typing import Any

# ---------------------------------------------------------------------------
# Data types
# ---------------------------------------------------------------------------


class StabilityLevel(str, Enum):
    """Governance stability classification."""

    STABLE = "stable"
    MODERATE = "moderate"
    UNSTABLE = "unstable"
    TURBULENT = "turbulent"


class ExportFormat(str, Enum):
    """Supported export formats."""

    JSON = "json"
    CSV = "csv"
    MARKDOWN = "markdown"


@dataclass(frozen=True)
class GovernanceEventData:
    """A single governance event for analysis."""

    event_type: str
    org_id: str
    tick: int
    details: dict[str, Any] = field(default_factory=dict)


@dataclass(frozen=True)
class OrgGovernanceSnapshot:
    """Aggregated governance snapshot for a single org."""

    org_id: str
    total_elections: int = 0
    total_leadership_changes: int = 0
    total_tax_collected: int = 0
    total_tax_events: int = 0
    total_distributed: int = 0
    total_distribution_events: int = 0
    avg_tax_rate: float = 0.0
    treaties_signed: int = 0
    treaties_broken: int = 0
    active_treaties: int = 0
    relation_changes: int = 0
    first_event_tick: int | None = None
    last_event_tick: int | None = None
    member_count: int = 0


@dataclass(frozen=True)
class StabilityReport:
    """Result of stability analysis for a single org."""

    org_id: str
    stability_score: float
    stability_level: StabilityLevel
    leadership_stability: float
    fiscal_stability: float
    diplomatic_stability: float
    analysis_ticks: int
    factors: dict[str, str] = field(default_factory=dict)


@dataclass(frozen=True)
class GovernanceComparison:
    """Result of comparing governance models across orgs."""

    orgs: list[OrgGovernanceSnapshot]
    rankings: dict[str, int]
    most_stable_org: str | None
    least_stable_org: str | None
    insights: list[str] = field(default_factory=list)


@dataclass(frozen=True)
class LeadershipPrediction:
    """Result of leadership change prediction."""

    org_id: str
    risk_score: float
    confidence: float
    factors: dict[str, str] = field(default_factory=dict)


# ---------------------------------------------------------------------------
# Analyzer
# ---------------------------------------------------------------------------


class GovernanceAnalyzer:
    """Analyzes governance event data to produce insights.

    Accepts pre-collected event data and org snapshots (typically from
    the Rust metrics collector via the API). All methods are pure
    computation — no I/O or side effects.
    """

    def __init__(
        self,
        *,
        leadership_change_weight: float = 0.4,
        fiscal_weight: float = 0.3,
        diplomacy_weight: float = 0.3,
        stability_thresholds: tuple[float, float, float] = (0.7, 0.4, 0.2),
    ) -> None:
        self._leadership_weight = leadership_change_weight
        self._fiscal_weight = fiscal_weight
        self._diplomacy_weight = diplomacy_weight
        self._stable_threshold, self._moderate_threshold, self._unstable_threshold = (
            stability_thresholds
        )

    # ── Public API ───────────────────────────────────────────

    def analyze_org_stability(
        self,
        org: OrgGovernanceSnapshot,
        events: list[GovernanceEventData],
        ticks: int = 0,
    ) -> StabilityReport:
        """Analyze governance stability for a single organization."""
        if ticks <= 0:
            ticks = self._effective_ticks(org)

        leadership_score = self._leadership_stability(org, ticks)
        fiscal_score = self._fiscal_stability(org, ticks)
        diplomacy_score = self._diplomacy_stability(org, ticks)

        composite = (
            self._leadership_weight * leadership_score
            + self._fiscal_weight * fiscal_score
            + self._diplomacy_weight * diplomacy_score
        )
        composite = max(0.0, min(1.0, composite))

        factors: dict[str, str] = {}
        if leadership_score < 0.5:
            factors["leadership"] = "High leadership turnover detected"
        if fiscal_score < 0.5:
            factors["fiscal"] = "Irregular tax/distribution patterns"
        if diplomacy_score < 0.5:
            factors["diplomacy"] = "Frequent treaty violations or hostile relations"

        return StabilityReport(
            org_id=org.org_id,
            stability_score=round(composite, 4),
            stability_level=self._classify_stability(composite),
            leadership_stability=round(leadership_score, 4),
            fiscal_stability=round(fiscal_score, 4),
            diplomatic_stability=round(diplomacy_score, 4),
            analysis_ticks=ticks,
            factors=factors,
        )

    def compare_governance_models(
        self,
        orgs: list[OrgGovernanceSnapshot],
        events: list[GovernanceEventData],
    ) -> GovernanceComparison:
        """Compare governance models across multiple organizations."""
        reports = []
        for org in orgs:
            report = self.analyze_org_stability(org, events)
            reports.append((org, report))

        ranked = sorted(reports, key=lambda x: x[1].stability_score, reverse=True)
        rankings = {
            org.org_id: rank + 1 for rank, (org, _) in enumerate(ranked)
        }

        most_stable = ranked[0][0].org_id if ranked else None
        least_stable = ranked[-1][0].org_id if ranked and len(ranked) > 1 else None

        insights = self._generate_comparison_insights(reports)

        return GovernanceComparison(
            orgs=orgs,
            rankings=rankings,
            most_stable_org=most_stable,
            least_stable_org=least_stable,
            insights=insights,
        )

    def predict_leadership_change(
        self,
        org: OrgGovernanceSnapshot,
        events: list[GovernanceEventData],
    ) -> LeadershipPrediction:
        """Predict the risk of a leadership change in the near future."""
        ticks = self._effective_ticks(org)
        org_events = [e for e in events if e.org_id == org.org_id]

        election_rate = org.total_elections / max(ticks, 1)
        freq_factor = min(1.0, election_rate * 10)

        avg_tax = org.total_tax_collected / max(org.total_tax_events, 1)
        fiscal_pressure = max(0.0, 1.0 - avg_tax / 1000)

        total_diplomacy = org.treaties_signed + org.treaties_broken + org.relation_changes
        diplomacy_rate = total_diplomacy / max(ticks, 1)
        diplomacy_pressure = min(1.0, diplomacy_rate * 20)

        leadership_events = [
            e for e in org_events
            if e.event_type in ("leadership_changed", "leadership_election_started")
        ]
        recent_changes = (
            len(leadership_events[-3:]) if len(leadership_events) >= 3
            else len(leadership_events)
        )
        recency_factor = min(1.0, recent_changes / 3.0)

        risk_score = (
            0.3 * freq_factor
            + 0.2 * fiscal_pressure
            + 0.2 * diplomacy_pressure
            + 0.3 * recency_factor
        )
        risk_score = max(0.0, min(1.0, risk_score))

        total_data = org.total_elections + org.total_tax_events + total_diplomacy
        confidence = min(1.0, total_data / 50.0)

        factors: dict[str, str] = {}
        if freq_factor > 0.5:
            factors["frequency"] = "High election frequency"
        if fiscal_pressure > 0.5:
            factors["fiscal"] = "Fiscal pressure detected"
        if recency_factor > 0.5:
            factors["recency"] = "Recent leadership instability"

        return LeadershipPrediction(
            org_id=org.org_id,
            risk_score=round(risk_score, 4),
            confidence=round(confidence, 4),
            factors=factors,
        )

    def export_governance_report(
        self,
        orgs: list[OrgGovernanceSnapshot],
        events: list[GovernanceEventData],
        fmt: ExportFormat = ExportFormat.JSON,
    ) -> str:
        """Export a governance report for one or more organizations."""
        reports = []
        comparisons = []
        for org in orgs:
            stability = self.analyze_org_stability(org, events)
            prediction = self.predict_leadership_change(org, events)
            reports.append((org, stability, prediction))

        if len(orgs) > 1:
            comparison = self.compare_governance_models(orgs, events)
            comparisons.append(comparison)

        if fmt == ExportFormat.JSON:
            return self._export_json(reports, comparisons)
        elif fmt == ExportFormat.CSV:
            return self._export_csv(reports)
        elif fmt == ExportFormat.MARKDOWN:
            return self._export_markdown(reports, comparisons)
        else:
            raise ValueError(f"Unsupported export format: {fmt}")

    # ── Private helpers ──────────────────────────────────────

    def _effective_ticks(self, org: OrgGovernanceSnapshot) -> int:
        if org.first_event_tick is not None and org.last_event_tick is not None:
            span = org.last_event_tick - org.first_event_tick
            return max(span, 1)
        return max(org.total_tax_events + org.total_elections, 1)

    def _leadership_stability(self, org: OrgGovernanceSnapshot, ticks: int) -> float:
        if org.total_elections == 0:
            return 1.0
        change_rate = org.total_leadership_changes / max(ticks, 1)
        stability = max(0.0, 1.0 - change_rate * 10)
        return min(1.0, stability)

    def _fiscal_stability(self, org: OrgGovernanceSnapshot, ticks: int) -> float:
        if org.total_tax_events == 0:
            return 0.5
        tax_rate = org.total_tax_events / max(ticks, 1)
        consistency = min(1.0, tax_rate * 5)
        if org.total_tax_collected > 0:
            balance = org.total_distributed / org.total_tax_collected
            balance_score = 1.0 - abs(1.0 - min(balance, 2.0))
        else:
            balance_score = 0.5
        return 0.6 * consistency + 0.4 * balance_score

    def _diplomacy_stability(self, org: OrgGovernanceSnapshot, ticks: int) -> float:
        if org.treaties_signed == 0 and org.treaties_broken == 0:
            return 0.5
        total_treaties = org.treaties_signed + org.treaties_broken
        break_ratio = org.treaties_broken / max(total_treaties, 1)
        treaty_stability = 1.0 - break_ratio
        active_score = min(1.0, org.active_treaties / 5.0)
        relation_rate = org.relation_changes / max(ticks, 1)
        relation_stability = max(0.0, 1.0 - relation_rate * 20)
        return 0.4 * treaty_stability + 0.3 * active_score + 0.3 * relation_stability

    def _classify_stability(self, score: float) -> StabilityLevel:
        if score >= self._stable_threshold:
            return StabilityLevel.STABLE
        elif score >= self._moderate_threshold:
            return StabilityLevel.MODERATE
        elif score >= self._unstable_threshold:
            return StabilityLevel.UNSTABLE
        else:
            return StabilityLevel.TURBULENT

    def _generate_comparison_insights(
        self,
        reports: list[tuple[OrgGovernanceSnapshot, StabilityReport]],
    ) -> list[str]:
        insights: list[str] = []
        if not reports:
            return insights
        by_leadership = sorted(reports, key=lambda x: x[1].leadership_stability, reverse=True)
        by_fiscal = sorted(reports, key=lambda x: x[1].fiscal_stability, reverse=True)
        by_diplomacy = sorted(reports, key=lambda x: x[1].diplomatic_stability, reverse=True)

        if len(reports) >= 2:
            best_lead = by_leadership[0]
            if best_lead[1].leadership_stability > 0:
                insights.append(
                    f"{best_lead[0].org_id} has the most stable leadership "
                    f"(score: {best_lead[1].leadership_stability:.2f})"
                )
            best_fiscal = by_fiscal[0]
            if best_fiscal[1].fiscal_stability > 0:
                insights.append(
                    f"{best_fiscal[0].org_id} has the strongest fiscal governance "
                    f"(score: {best_fiscal[1].fiscal_stability:.2f})"
                )
            best_dip = by_diplomacy[0]
            if best_dip[1].diplomatic_stability > 0:
                insights.append(
                    f"{best_dip[0].org_id} has the most stable diplomacy "
                    f"(score: {best_dip[1].diplomatic_stability:.2f})"
                )
        return insights

    def _export_json(
        self,
        reports: list[tuple[OrgGovernanceSnapshot, StabilityReport, LeadershipPrediction]],
        comparisons: list[GovernanceComparison],
    ) -> str:
        data: dict[str, Any] = {"organizations": []}
        for org, stability, prediction in reports:
            data["organizations"].append({
                "org_id": org.org_id,
                "stability": {
                    "score": stability.stability_score,
                    "level": stability.stability_level.value,
                    "leadership": stability.leadership_stability,
                    "fiscal": stability.fiscal_stability,
                    "diplomacy": stability.diplomatic_stability,
                    "factors": stability.factors,
                },
                "prediction": {
                    "risk_score": prediction.risk_score,
                    "confidence": prediction.confidence,
                    "factors": prediction.factors,
                },
                "metrics": {
                    "elections": org.total_elections,
                    "leadership_changes": org.total_leadership_changes,
                    "tax_collected": org.total_tax_collected,
                    "distributed": org.total_distributed,
                    "treaties_signed": org.treaties_signed,
                    "treaties_broken": org.treaties_broken,
                },
            })
        if comparisons:
            data["comparison"] = {
                "rankings": comparisons[0].rankings,
                "insights": comparisons[0].insights,
            }
        return json.dumps(data, indent=2, ensure_ascii=False)

    def _export_csv(
        self,
        reports: list[tuple[OrgGovernanceSnapshot, StabilityReport, LeadershipPrediction]],
    ) -> str:
        lines = [
            "org_id,stability_score,stability_level,leadership_stability,"
            "fiscal_stability,diplomatic_stability,risk_score,confidence,"
            "elections,leadership_changes,tax_collected,treaties_signed,treaties_broken"
        ]
        for org, stability, prediction in reports:
            lines.append(
                f"{org.org_id},{stability.stability_score},{stability.stability_level.value},"
                f"{stability.leadership_stability},{stability.fiscal_stability},"
                f"{stability.diplomatic_stability},{prediction.risk_score},"
                f"{prediction.confidence},{org.total_elections},"
                f"{org.total_leadership_changes},{org.total_tax_collected},"
                f"{org.treaties_signed},{org.treaties_broken}"
            )
        return "\n".join(lines)

    def _export_markdown(
        self,
        reports: list[tuple[OrgGovernanceSnapshot, StabilityReport, LeadershipPrediction]],
        comparisons: list[GovernanceComparison],
    ) -> str:
        lines = ["# Governance Report\n"]
        for org, stability, prediction in reports:
            lines.append(f"## {org.org_id}\n")
            lines.append(
                f"- **Stability**: "
                f"{stability.stability_level.value} "
                f"({stability.stability_score:.2f})"
            )
            lines.append(f"  - Leadership: {stability.leadership_stability:.2f}")
            lines.append(f"  - Fiscal: {stability.fiscal_stability:.2f}")
            lines.append(f"  - Diplomacy: {stability.diplomatic_stability:.2f}")
            lines.append(
                f"- **Leadership Change Risk**: "
                f"{prediction.risk_score:.2f} "
                f"(confidence: {prediction.confidence:.2f})"
            )
            if stability.factors:
                lines.append("- **Risk Factors**:")
                for key, val in stability.factors.items():
                    lines.append(f"  - {key}: {val}")
            lines.append("")
        if comparisons:
            lines.append("## Comparison\n")
            for insight in comparisons[0].insights:
                lines.append(f"- {insight}")
        return "\n".join(lines)
