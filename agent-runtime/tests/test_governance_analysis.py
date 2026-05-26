"""Tests for governance_analysis module."""

from __future__ import annotations

import json

import pytest

from agent_runtime.organization.governance_analysis import (
    ExportFormat,
    GovernanceAnalyzer,
    GovernanceEventData,
    OrgGovernanceSnapshot,
    StabilityLevel,
)

# ── Helpers ──────────────────────────────────────────────────


def _make_org(
    org_id: str = "org-1",
    *,
    elections: int = 1,
    leadership_changes: int = 0,
    tax_collected: int = 500,
    tax_events: int = 10,
    distributed: int = 400,
    distribution_events: int = 5,
    avg_tax_rate: float = 0.10,
    treaties_signed: int = 2,
    treaties_broken: int = 0,
    active_treaties: int = 2,
    relation_changes: int = 1,
    first_tick: int | None = 1,
    last_tick: int | None = 100,
    member_count: int = 5,
) -> OrgGovernanceSnapshot:
    return OrgGovernanceSnapshot(
        org_id=org_id,
        total_elections=elections,
        total_leadership_changes=leadership_changes,
        total_tax_collected=tax_collected,
        total_tax_events=tax_events,
        total_distributed=distributed,
        total_distribution_events=distribution_events,
        avg_tax_rate=avg_tax_rate,
        treaties_signed=treaties_signed,
        treaties_broken=treaties_broken,
        active_treaties=active_treaties,
        relation_changes=relation_changes,
        first_event_tick=first_tick,
        last_event_tick=last_tick,
        member_count=member_count,
    )


def _make_event(
    event_type: str = "tax_collected",
    org_id: str = "org-1",
    tick: int = 10,
) -> GovernanceEventData:
    return GovernanceEventData(event_type=event_type, org_id=org_id, tick=tick)


def _sample_events(org_id: str = "org-1") -> list[GovernanceEventData]:
    return [
        _make_event("tax_collected", org_id, 10),
        _make_event("tax_collected", org_id, 20),
        _make_event("treasury_distributed", org_id, 30),
        _make_event("leadership_election_started", org_id, 40),
        _make_event("leadership_changed", org_id, 45),
        _make_event("treaty_signed", org_id, 50),
    ]


# ── Stability Analysis ───────────────────────────────────────


class TestAnalyzeOrgStability:
    def test_stable_org(self) -> None:
        analyzer = GovernanceAnalyzer()
        org = _make_org(leadership_changes=0, tax_events=10, treaties_broken=0)
        report = analyzer.analyze_org_stability(org, _sample_events())
        assert report.org_id == "org-1"
        assert report.stability_level == StabilityLevel.STABLE
        assert report.stability_score >= 0.7
        assert report.leadership_stability >= 0.8

    def test_turbulent_org(self) -> None:
        analyzer = GovernanceAnalyzer()
        org = _make_org(
            leadership_changes=10,
            tax_events=1,
            tax_collected=10,
            treaties_signed=5,
            treaties_broken=4,
            active_treaties=1,
            relation_changes=20,
            first_tick=1,
            last_tick=10,
        )
        report = analyzer.analyze_org_stability(org, _sample_events())
        assert report.stability_score < 0.4
        assert len(report.factors) > 0

    def test_no_elections_is_stable(self) -> None:
        analyzer = GovernanceAnalyzer()
        org = _make_org(elections=0, leadership_changes=0)
        report = analyzer.analyze_org_stability(org, [])
        assert report.leadership_stability == 1.0

    def test_no_tax_events_neutral_fiscal(self) -> None:
        analyzer = GovernanceAnalyzer()
        org = _make_org(tax_events=0, tax_collected=0)
        report = analyzer.analyze_org_stability(org, [])
        assert report.fiscal_stability == 0.5

    def test_no_diplomacy_neutral(self) -> None:
        analyzer = GovernanceAnalyzer()
        org = _make_org(treaties_signed=0, treaties_broken=0, relation_changes=0)
        report = analyzer.analyze_org_stability(org, [])
        assert report.diplomatic_stability == 0.5

    def test_stability_score_bounded(self) -> None:
        analyzer = GovernanceAnalyzer()
        org = _make_org()
        report = analyzer.analyze_org_stability(org, _sample_events())
        assert 0.0 <= report.stability_score <= 1.0
        assert 0.0 <= report.leadership_stability <= 1.0
        assert 0.0 <= report.fiscal_stability <= 1.0
        assert 0.0 <= report.diplomatic_stability <= 1.0

    def test_analysis_ticks_from_event_span(self) -> None:
        analyzer = GovernanceAnalyzer()
        org = _make_org(first_tick=10, last_tick=60)
        report = analyzer.analyze_org_stability(org, _sample_events())
        assert report.analysis_ticks == 50


# ── Comparison ────────────────────────────────────────────────


class TestCompareGovernanceModels:
    def test_rank_two_orgs(self) -> None:
        analyzer = GovernanceAnalyzer()
        org_stable = _make_org("org-stable", leadership_changes=0, treaties_broken=0)
        org_unstable = _make_org(
            "org-unstable",
            leadership_changes=10,
            treaties_broken=5,
            active_treaties=0,
            relation_changes=15,
            first_tick=1,
            last_tick=10,
        )
        result = analyzer.compare_governance_models(
            [org_stable, org_unstable], _sample_events()
        )
        assert result.rankings["org-stable"] == 1
        assert result.rankings["org-unstable"] == 2
        assert result.most_stable_org == "org-stable"
        assert result.least_stable_org == "org-unstable"

    def test_insights_generated_for_multiple_orgs(self) -> None:
        analyzer = GovernanceAnalyzer()
        orgs = [
            _make_org("org-a", leadership_changes=0),
            _make_org("org-b", leadership_changes=5, treaties_broken=3),
        ]
        result = analyzer.compare_governance_models(orgs, _sample_events())
        assert len(result.insights) >= 1

    def test_single_org_no_least_stable(self) -> None:
        analyzer = GovernanceAnalyzer()
        org = _make_org("solo")
        result = analyzer.compare_governance_models([org], [])
        assert result.most_stable_org == "solo"
        assert result.least_stable_org is None


# ── Leadership Prediction ────────────────────────────────────


class TestPredictLeadershipChange:
    def test_low_risk_stable_org(self) -> None:
        analyzer = GovernanceAnalyzer()
        org = _make_org(elections=1, leadership_changes=0, tax_collected=5000)
        prediction = analyzer.predict_leadership_change(org, _sample_events())
        assert prediction.risk_score < 0.5
        assert prediction.org_id == "org-1"

    def test_high_risk_unstable_org(self) -> None:
        analyzer = GovernanceAnalyzer()
        events = [
            _make_event("leadership_changed", "org-1", 10),
            _make_event("leadership_changed", "org-1", 12),
            _make_event("leadership_changed", "org-1", 14),
        ]
        org = _make_org(
            elections=10,
            leadership_changes=10,
            tax_collected=10,
            tax_events=1,
            first_tick=1,
            last_tick=10,
        )
        prediction = analyzer.predict_leadership_change(org, events)
        assert prediction.risk_score > 0.4

    def test_confidence_increases_with_data(self) -> None:
        analyzer = GovernanceAnalyzer()
        org_low = _make_org(elections=1, tax_events=2, treaties_signed=0)
        pred_low = analyzer.predict_leadership_change(org_low, [])
        org_high = _make_org(elections=10, tax_events=50, treaties_signed=10)
        pred_high = analyzer.predict_leadership_change(org_high, [])
        assert pred_high.confidence >= pred_low.confidence

    def test_risk_score_bounded(self) -> None:
        analyzer = GovernanceAnalyzer()
        org = _make_org()
        prediction = analyzer.predict_leadership_change(org, _sample_events())
        assert 0.0 <= prediction.risk_score <= 1.0
        assert 0.0 <= prediction.confidence <= 1.0


# ── Export ────────────────────────────────────────────────────


class TestExportGovernanceReport:
    def test_json_export(self) -> None:
        analyzer = GovernanceAnalyzer()
        org = _make_org()
        result = analyzer.export_governance_report([org], _sample_events(), ExportFormat.JSON)
        parsed = json.loads(result)
        assert "organizations" in parsed
        assert len(parsed["organizations"]) == 1
        assert parsed["organizations"][0]["org_id"] == "org-1"

    def test_csv_export(self) -> None:
        analyzer = GovernanceAnalyzer()
        org = _make_org()
        result = analyzer.export_governance_report([org], _sample_events(), ExportFormat.CSV)
        lines = result.strip().split("\n")
        assert len(lines) == 2
        assert "org_id" in lines[0]
        assert "org-1" in lines[1]

    def test_markdown_export(self) -> None:
        analyzer = GovernanceAnalyzer()
        org = _make_org()
        result = analyzer.export_governance_report([org], _sample_events(), ExportFormat.MARKDOWN)
        assert "# Governance Report" in result
        assert "## org-1" in result

    def test_json_export_with_comparison(self) -> None:
        analyzer = GovernanceAnalyzer()
        orgs = [_make_org("org-a"), _make_org("org-b")]
        result = analyzer.export_governance_report(orgs, _sample_events(), ExportFormat.JSON)
        parsed = json.loads(result)
        assert "comparison" in parsed
        assert len(parsed["organizations"]) == 2

    def test_invalid_format_raises(self) -> None:
        analyzer = GovernanceAnalyzer()
        org = _make_org()
        with pytest.raises(ValueError, match="Unsupported export format"):
            analyzer.export_governance_report([org], [], "xml")  # type: ignore[arg-type]


# ── Stability Classification ─────────────────────────────────


class TestStabilityClassification:
    def test_custom_thresholds(self) -> None:
        analyzer = GovernanceAnalyzer(stability_thresholds=(0.9, 0.6, 0.3))
        org = _make_org()
        report = analyzer.analyze_org_stability(org, _sample_events())
        assert isinstance(report.stability_level, StabilityLevel)

    def test_all_levels_reachable(self) -> None:
        analyzer = GovernanceAnalyzer()
        stable_org = _make_org(
            leadership_changes=0, tax_events=50, treaties_broken=0,
            first_tick=1, last_tick=100,
        )
        stable = analyzer.analyze_org_stability(stable_org, [])
        assert stable.stability_level == StabilityLevel.STABLE

        turbulent_org = _make_org(
            leadership_changes=50, tax_events=1, tax_collected=1,
            treaties_signed=10, treaties_broken=9, active_treaties=1,
            relation_changes=50, first_tick=1, last_tick=10,
        )
        turbulent = analyzer.analyze_org_stability(turbulent_org, [])
        assert turbulent.stability_level == StabilityLevel.TURBULENT
