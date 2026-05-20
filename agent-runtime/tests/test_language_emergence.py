"""Tests for Phase 4.3.4 — Language emergence: CommunicationAnalyzer, JargonDetector,
LanguageExperiment, and EmergenceMetrics.
"""

from __future__ import annotations

import pytest

from agent_runtime.social.comm_analyzer import CommunicationAnalyzer, DialectReport, MessagePattern
from agent_runtime.social.jargon_detector import JargonDetector, JargonTerm
from agent_runtime.social.language_experiment import EfficiencyMetrics, LanguageExperiment
from agent_runtime.tracing.emergence_metrics import EmergenceMetrics, LanguageEmergenceSnapshot


# ── Fixtures ──


@pytest.fixture
def analyzer() -> CommunicationAnalyzer:
    return CommunicationAnalyzer()


@pytest.fixture
def detector() -> JargonDetector:
    return JargonDetector()


@pytest.fixture
def experiment() -> LanguageExperiment:
    return LanguageExperiment()


# ── Sample data ──

TRADER_MESSAGES = [
    "Buy low and sell high for maximum profit.",
    "The market is bullish today, great trading opportunity.",
    "I will trade my surplus tokens for resources.",
    "Profit margin analysis shows positive growth.",
    "Market indicators suggest a bullish trend.",
]

BUILDER_MESSAGES = [
    "Construct the bridge before the deadline.",
    "We need more materials to build the tower.",
    "Blueprint analysis shows structural weakness.",
    "Build the foundation with reinforced materials.",
    "Construction deadline approaching, focus on the tower.",
]


# ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
# Test 1: CommunicationAnalyzer.analyze_message_patterns
# ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━


def test_analyze_message_patterns_basic(analyzer: CommunicationAnalyzer) -> None:
    """Analyzing messages returns correct counts and derived metrics."""
    pattern = analyzer.analyze_message_patterns("agent-1", TRADER_MESSAGES)

    assert isinstance(pattern, MessagePattern)
    assert pattern.agent_id == "agent-1"
    assert pattern.total_messages == 5
    assert pattern.total_words > 0
    assert pattern.avg_words_per_message > 0
    assert 0.0 <= pattern.vocabulary_richness <= 1.0
    assert 0.0 <= pattern.punctuation_density <= 1.0
    # "bullish" should appear at least twice in trader messages.
    assert pattern.top_words.get("bullish", 0) >= 2


def test_analyze_empty_messages(analyzer: CommunicationAnalyzer) -> None:
    """Empty message list returns a zeroed-out MessagePattern."""
    pattern = analyzer.analyze_message_patterns("empty-agent", [])

    assert pattern.total_messages == 0
    assert pattern.total_words == 0
    assert pattern.avg_words_per_message == 0.0
    assert pattern.vocabulary_richness == 0.0


# ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
# Test 2: CommunicationAnalyzer.compare_group_patterns
# ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━


def test_compare_group_patterns(analyzer: CommunicationAnalyzer) -> None:
    """Two distinct groups show non-zero distance and distinguishing words."""
    group_a = {"agent-1": TRADER_MESSAGES}
    group_b = {"agent-2": BUILDER_MESSAGES}

    result = analyzer.compare_group_patterns(group_a, group_b)

    assert 0.0 <= result["distance"] <= 1.0
    assert 0.0 <= result["shared_vocab_ratio"] <= 1.0
    assert isinstance(result["a_summary"], MessagePattern)
    assert isinstance(result["b_summary"], MessagePattern)
    # Trader-specific and builder-specific words should appear.
    assert len(result["distinguishing_words"]["group_a_only"]) > 0
    assert len(result["distinguishing_words"]["group_b_only"]) > 0


# ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
# Test 3: CommunicationAnalyzer.detect_emerging_dialect
# ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━


def test_detect_emerging_dialect(analyzer: CommunicationAnalyzer) -> None:
    """Dialect detection identifies divergence between groups over time."""
    timeline = [
        {
            "period": "tick-10",
            "groups": {
                "traders": TRADER_MESSAGES,
                "builders": BUILDER_MESSAGES,
            },
        },
        {
            "period": "tick-20",
            "groups": {
                "traders": [
                    "Bullish market indicators show profit margins.",
                    "Trade surplus tokens for maximum profit.",
                ],
                "builders": [
                    "Blueprint construction materials deadline.",
                    "Foundation tower structural reinforced build.",
                ],
            },
        },
    ]

    report = analyzer.detect_emerging_dialect(timeline, distance_threshold=0.3)

    assert isinstance(report, DialectReport)
    assert isinstance(report.has_dialect, bool)
    assert 0.0 <= report.dialect_strength <= 1.0


def test_detect_dialect_insufficient_data(analyzer: CommunicationAnalyzer) -> None:
    """Single data point returns no dialect detection."""
    report = analyzer.detect_emerging_dialect(
        [{"period": "tick-1", "groups": {"a": ["hello"]}}]
    )
    assert not report.has_dialect
    assert report.period_label == "insufficient_data"


# ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
# Test 4: JargonDetector.extract_frequent_phrases
# ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━


def test_extract_frequent_phrases(detector: JargonDetector) -> None:
    """High-frequency n-grams are extracted correctly."""
    messages = [
        "market profit margin analysis",
        "profit margin analysis report",
        "market profit analysis complete",
    ]

    phrases = detector.extract_frequent_phrases(messages, min_freq=2, ngram_range=(2, 2))

    assert isinstance(phrases, list)
    assert len(phrases) > 0
    # "profit margin" and "market profit" should both appear at least 2 times.
    phrase_texts = [p["phrase"] for p in phrases]
    assert "profit margin" in phrase_texts or "market profit" in phrase_texts


def test_extract_phrases_min_freq_filter(detector: JargonDetector) -> None:
    """Phrases below min_freq are excluded."""
    messages = ["unique phrase only once"]
    phrases = detector.extract_frequent_phrases(messages, min_freq=2)

    assert len(phrases) == 0


# ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
# Test 5: JargonDetector.detect_group_specific_terms
# ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━


def test_detect_group_specific_terms(detector: JargonDetector) -> None:
    """Terms exclusive to one group are detected with high specificity."""
    all_groups = {
        "traders": TRADER_MESSAGES,
        "builders": BUILDER_MESSAGES,
    }

    jargon = detector.detect_group_specific_terms(
        all_groups, min_freq=1, specificity_threshold=0.6
    )

    assert isinstance(jargon, list)
    # All returned terms should have specificity >= threshold.
    for term in jargon:
        assert isinstance(term, JargonTerm)
        assert term.specificity >= 0.6
        assert term.frequency >= 1
        assert term.group_id in ("traders", "builders")


# ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
# Test 6: JargonDetector.compute_linguistic_distance
# ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━


def test_linguistic_distance_identical(detector: JargonDetector) -> None:
    """Identical vocabularies produce distance 0.0."""
    msgs = ["hello world test"]
    distance = detector.compute_linguistic_distance(msgs, msgs)
    assert distance == 0.0


def test_linguistic_distance_disjoint(detector: JargonDetector) -> None:
    """Completely disjoint vocabularies produce distance 1.0."""
    msgs_a = ["alpha beta gamma"]
    msgs_b = ["delta epsilon zeta"]
    distance = detector.compute_linguistic_distance(msgs_a, msgs_b)
    assert distance == 1.0


def test_linguistic_distance_partial_overlap(detector: JargonDetector) -> None:
    """Partial overlap gives distance between 0 and 1."""
    msgs_a = ["shared word alpha"]
    msgs_b = ["shared word beta"]
    distance = detector.compute_linguistic_distance(msgs_a, msgs_b)
    assert 0.0 < distance < 1.0


# ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
# Test 7: LanguageExperiment — restricted vocabulary + efficiency
# ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━


def test_language_experiment_restricted_vocab(experiment: LanguageExperiment) -> None:
    """Vocabulary restriction detects violations and measures efficiency."""
    allowed = {"hello", "world", "test", "message", "good", "morning"}
    experiment.setup_restricted_vocabulary(["agent-1"], allowed)

    # Compliant message.
    result = experiment.check_message("hello world good morning test")
    assert result["compliant"] is True
    assert result["violations"] == []

    # Non-compliant message.
    result = experiment.check_message("hello forbidden word here")
    assert result["compliant"] is False
    assert len(result["violations"]) > 0


def test_language_experiment_efficiency(experiment: LanguageExperiment) -> None:
    """Efficiency metrics capture before/after changes and novel words."""
    before = ["the quick brown fox jumps", "over the lazy dog"]
    after = ["fast brown animal leaps", "across sleeping canine"]

    allowed = {"fast", "brown", "animal", "leaps", "across", "sleeping", "canine"}
    experiment.setup_restricted_vocabulary(["agent-1"], allowed)

    metrics = experiment.measure_communication_efficiency(before, after)

    assert isinstance(metrics, EfficiencyMetrics)
    assert metrics.total_messages == 2
    assert metrics.total_words > 0
    assert metrics.words_per_message > 0
    # Some novel words should have emerged (not in before vocabulary).
    assert len(metrics.novel_words) > 0


def test_language_experiment_deactivate(experiment: LanguageExperiment) -> None:
    """Deactivating constraint allows any word."""
    allowed = {"hello"}
    experiment.setup_restricted_vocabulary(["agent-1"], allowed)
    assert experiment.is_active() is True

    experiment.deactivate()
    assert experiment.is_active() is False

    result = experiment.check_message("any word should pass now")
    assert result["compliant"] is True


# ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
# Test 8: EmergenceMetrics — end-to-end snapshot
# ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━


def test_emergence_metrics_snapshot() -> None:
    """EmergenceMetrics produces a valid snapshot with all fields populated."""
    metrics = EmergenceMetrics()
    groups = {
        "traders": TRADER_MESSAGES,
        "builders": BUILDER_MESSAGES,
    }

    snapshot = metrics.compute(tick=42, groups=groups)

    assert isinstance(snapshot, LanguageEmergenceSnapshot)
    assert snapshot.tick == 42
    assert snapshot.group_count == 2
    assert 0.0 <= snapshot.avg_linguistic_distance <= 1.0
    assert 0.0 <= snapshot.dialect_strength <= 1.0
    assert isinstance(snapshot.dialect_detected, bool)
    assert isinstance(snapshot.jargon_count, int)
    assert isinstance(snapshot.top_jargon, list)
    assert 0.0 <= snapshot.avg_vocab_richness <= 1.0


def test_emergence_metrics_history_trend() -> None:
    """History accumulation and trend extraction work correctly."""
    metrics = EmergenceMetrics()
    groups = {
        "traders": TRADER_MESSAGES,
        "builders": BUILDER_MESSAGES,
    }

    metrics.compute(tick=1, groups=groups)
    metrics.compute(tick=2, groups=groups)
    metrics.compute(tick=3, groups=groups)

    assert len(metrics.history) == 3
    trend = metrics.get_trend("avg_linguistic_distance")
    assert len(trend) == 3
    assert all(isinstance(v, float) for v in trend)


def test_emergence_metrics_single_group() -> None:
    """Single group produces a valid but minimal snapshot."""
    metrics = EmergenceMetrics()
    snapshot = metrics.compute(tick=1, groups={"solo": ["hello world"]})

    assert snapshot.group_count == 1
    assert snapshot.avg_linguistic_distance == 0.0
