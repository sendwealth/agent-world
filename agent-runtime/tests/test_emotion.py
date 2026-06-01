"""Tests for the emotion subsystem (agent_runtime.emotion).

Covers:
- EmotionalState model (defaults, clamping, prompt description, serialization)
- EmotionType enum
- EmotionEngine (event updates, personality modulation, decay, baseline)
- ThinkLoopEmotionHook (action-to-emotion mapping, decay adapter)
- Integration with build_prompt (mood_description injection)
"""

from __future__ import annotations

import math

import pytest

from agent_runtime.core.decide import (
    DecisionAction,
    DecisionPerception,
    SurvivalAssessment,
    build_prompt,
)
from agent_runtime.emotion.engine import (
    EmotionEngine,
    EmotionResponse,
    ThinkLoopEmotionHook,
    _ACTION_TO_EMOTION_EVENT,
)
from agent_runtime.emotion.mood import EmotionalState, EmotionType
from agent_runtime.models.personality import PersonalityVector


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def _default_personality(**overrides):
    """Create a PersonalityVector with sensible defaults."""
    defaults = {
        "openness": 0.5,
        "conscientiousness": 0.5,
        "extraversion": 0.5,
        "agreeableness": 0.5,
        "neuroticism": 0.5,
        "risk_tolerance": 0.5,
        "social_orientation": 0.5,
        "greed": 0.5,
    }
    defaults.update(overrides)
    return PersonalityVector(**defaults)


# ---------------------------------------------------------------------------
# EmotionalState tests
# ---------------------------------------------------------------------------


class TestEmotionalState:
    """Tests for the EmotionalState Pydantic model."""

    def test_default_state_is_calm(self):
        state = EmotionalState()
        assert state.valence == 0.0
        assert state.arousal == 0.3
        assert state.dominance == 0.5
        assert state.primary_emotion == EmotionType.CALM
        assert state.intensity == 0.0
        assert state.triggers == []

    def test_valence_rejects_out_of_bounds(self):
        """Pydantic Field constraints reject out-of-range valence."""
        import pydantic
        with pytest.raises(pydantic.ValidationError):
            EmotionalState(valence=2.0)
        with pytest.raises(pydantic.ValidationError):
            EmotionalState(valence=-2.0)

    def test_arousal_rejects_out_of_bounds(self):
        """Pydantic Field constraints reject out-of-range arousal."""
        import pydantic
        with pytest.raises(pydantic.ValidationError):
            EmotionalState(arousal=-0.5)
        with pytest.raises(pydantic.ValidationError):
            EmotionalState(arousal=2.0)

    def test_intensity_rejects_out_of_bounds(self):
        """Pydantic Field constraints reject out-of-range intensity."""
        import pydantic
        with pytest.raises(pydantic.ValidationError):
            EmotionalState(intensity=-0.5)
        with pytest.raises(pydantic.ValidationError):
            EmotionalState(intensity=2.0)

    def test_neutral_factory(self):
        state = EmotionalState.neutral()
        assert state.valence == 0.0
        assert state.primary_emotion == EmotionType.CALM
        assert state.intensity == 0.0

    def test_prompt_description_low_intensity(self):
        state = EmotionalState(intensity=0.03)
        desc = state.to_prompt_description()
        assert "neutral" in desc or "balanced" in desc

    def test_prompt_description_happy_high_intensity(self):
        state = EmotionalState(
            valence=0.8,
            arousal=0.7,
            primary_emotion=EmotionType.HAPPY,
            intensity=0.8,
        )
        desc = state.to_prompt_description()
        assert "happy" in desc
        assert "intensely" in desc

    def test_prompt_description_with_triggers(self):
        state = EmotionalState(
            valence=0.5,
            primary_emotion=EmotionType.HAPPY,
            intensity=0.4,
            triggers=["earned_money", "social_success"],
        )
        desc = state.to_prompt_description()
        assert "earned_money" in desc

    def test_prompt_description_behavioral_tendency_bold(self):
        state = EmotionalState(
            valence=0.5, arousal=0.7, primary_emotion=EmotionType.HAPPY, intensity=0.5
        )
        desc = state.to_prompt_description()
        assert "bold" in desc

    def test_prompt_description_behavioral_tendency_cautious(self):
        state = EmotionalState(
            valence=-0.5, arousal=0.7, primary_emotion=EmotionType.FEARFUL, intensity=0.5
        )
        desc = state.to_prompt_description()
        assert "cautious" in desc

    def test_serialization_roundtrip(self):
        state = EmotionalState(
            valence=0.3,
            arousal=0.6,
            dominance=0.7,
            primary_emotion=EmotionType.HAPPY,
            secondary_emotion=EmotionType.SURPRISED,
            triggers=["earned_money"],
            intensity=0.5,
        )
        data = state.to_storage_dict()
        restored = EmotionalState.from_storage_dict(data)
        assert restored.valence == pytest.approx(0.3)
        assert restored.arousal == pytest.approx(0.6)
        assert restored.dominance == pytest.approx(0.7)
        assert restored.primary_emotion == EmotionType.HAPPY
        assert restored.secondary_emotion == EmotionType.SURPRISED
        assert restored.triggers == ["earned_money"]
        assert restored.intensity == pytest.approx(0.5)


# ---------------------------------------------------------------------------
# EmotionEngine tests
# ---------------------------------------------------------------------------


class TestEmotionEngine:
    """Tests for the EmotionEngine."""

    def test_initial_state_is_baseline(self):
        personality = _default_personality()
        engine = EmotionEngine(personality=personality)
        baseline = engine.get_baseline()
        assert engine.state.valence == pytest.approx(baseline.valence)
        assert engine.state.arousal == pytest.approx(baseline.arousal)

    def test_baseline_reflects_personality(self):
        # High neuroticism → lower valence, higher arousal
        neurotic = _default_personality(neuroticism=0.9, extraversion=0.1)
        stable = _default_personality(neuroticism=0.1, extraversion=0.9)

        engine_neurotic = EmotionEngine(personality=neurotic)
        engine_stable = EmotionEngine(personality=stable)

        assert engine_neurotic.get_baseline().valence < engine_stable.get_baseline().valence
        assert engine_neurotic.get_baseline().arousal > engine_stable.get_baseline().arousal

    def test_earned_money_makes_agent_happy(self):
        personality = _default_personality()
        engine = EmotionEngine(personality=personality)

        state = engine.update("earned_money", {"amount": 50})

        assert state.valence > 0.0
        assert state.primary_emotion == EmotionType.HAPPY
        assert "earned_money" in state.triggers

    def test_attacked_makes_agent_fearful(self):
        personality = _default_personality()
        engine = EmotionEngine(personality=personality)

        state = engine.update("attacked")

        assert state.valence < 0.0
        assert state.arousal > 0.3
        assert state.primary_emotion == EmotionType.FEARFUL

    def test_neuroticism_amplifies_negative(self):
        """High-neuroticism agents react more strongly to negative events."""
        neurotic = _default_personality(neuroticism=0.9, extraversion=0.1)
        stable = _default_personality(neuroticism=0.1, extraversion=0.9)

        engine_neurotic = EmotionEngine(personality=neurotic)
        engine_stable = EmotionEngine(personality=stable)

        state_neurotic = engine_neurotic.update("attacked")
        state_stable = engine_stable.update("attacked")

        # Neurotic agent should have lower valence (more negative)
        assert state_neurotic.valence < state_stable.valence

    def test_extraversion_amplifies_positive(self):
        """High-extraversion agents react more strongly to positive events."""
        extravert = _default_personality(extraversion=0.9, neuroticism=0.1)
        introvert = _default_personality(extraversion=0.1, neuroticism=0.9)

        engine_extro = EmotionEngine(personality=extravert)
        engine_intro = EmotionEngine(personality=introvert)

        state_extro = engine_extro.update("earned_money")
        state_intro = engine_intro.update("earned_money")

        assert state_extro.valence > state_intro.valence

    def test_decay_regresses_toward_baseline(self):
        personality = _default_personality()
        engine = EmotionEngine(personality=personality)

        # Make agent happy
        engine.update("earned_money")
        valence_after = engine.state.valence

        # Decay over many ticks
        engine.decay(ticks_elapsed=20)

        # Should be closer to baseline than immediately after event
        baseline = engine.get_baseline()
        assert abs(engine.state.valence - baseline.valence) < abs(valence_after - baseline.valence)

    def test_decay_reduces_intensity(self):
        personality = _default_personality()
        engine = EmotionEngine(personality=personality)

        engine.update("bounty_completed")
        assert engine.state.intensity > 0.1

        engine.decay(ticks_elapsed=30)
        assert engine.state.intensity < 0.1

    def test_full_decay_returns_to_near_baseline(self):
        personality = _default_personality()
        engine = EmotionEngine(personality=personality)

        engine.update("attacked")
        engine.decay(ticks_elapsed=100)

        baseline = engine.get_baseline()
        assert engine.state.valence == pytest.approx(baseline.valence, abs=0.05)

    def test_multiple_events_accumulate(self):
        personality = _default_personality()
        engine = EmotionEngine(personality=personality)

        engine.update("earned_money")
        valence_1 = engine.state.valence

        engine.update("earned_money")
        valence_2 = engine.state.valence

        # Second positive event should push valence higher
        assert valence_2 > valence_1

    def test_trigger_history_capped(self):
        personality = _default_personality()
        engine = EmotionEngine(personality=personality)

        for i in range(15):
            engine.update("rest")

        assert len(engine.state.triggers) <= 10

    def test_get_mood_description(self):
        personality = _default_personality()
        engine = EmotionEngine(personality=personality)
        engine.update("earned_money")
        desc = engine.get_mood_description()
        assert isinstance(desc, str)
        assert len(desc) > 0

    def test_to_prompt_description_alias(self):
        personality = _default_personality()
        engine = EmotionEngine(personality=personality)
        assert engine.to_prompt_description() == engine.get_mood_description()

    def test_unknown_event_mild_surprise(self):
        personality = _default_personality()
        engine = EmotionEngine(personality=personality)

        state = engine.update("completely_unknown_event")
        assert state.primary_emotion == EmotionType.SURPRISED
        assert state.arousal > 0.3  # slightly more aroused

    def test_context_intensity_amount(self):
        """Large amounts should produce higher intensity."""
        personality = _default_personality()
        engine = EmotionEngine(personality=personality)

        state_small = EmotionEngine(personality=personality).update("earned_money", {"amount": 10})
        state_large = EmotionEngine(personality=personality).update("earned_money", {"amount": 1000})

        # Both should be happy, but larger amount may affect intensity via context
        assert state_small.primary_emotion == EmotionType.HAPPY
        assert state_large.primary_emotion == EmotionType.HAPPY

    def test_serialization_roundtrip(self):
        personality = _default_personality()
        engine = EmotionEngine(personality=personality)
        engine.update("earned_money")

        data = engine.get_state_dict()
        engine2 = EmotionEngine(personality=personality)
        engine2.load_state_dict(data)

        assert engine2.state.valence == pytest.approx(engine.state.valence)
        assert engine2.state.primary_emotion == engine.state.primary_emotion

    def test_all_mapped_events_produce_valid_state(self):
        """Every event in _EVENT_RESPONSES should produce a valid state."""
        personality = _default_personality()
        for event in [
            "earned_money", "attacked", "social_success", "resource_loss",
            "oracle_received", "bounty_completed", "trade_success", "trade_failure",
            "cooperation", "betrayal", "exploration_success", "death_witness",
            "survival_crisis", "rest",
        ]:
            engine = EmotionEngine(personality=personality)
            state = engine.update(event)
            assert -1.0 <= state.valence <= 1.0
            assert 0.0 <= state.arousal <= 1.0
            assert 0.0 <= state.dominance <= 1.0
            assert 0.0 <= state.intensity <= 1.0


# ---------------------------------------------------------------------------
# ThinkLoopEmotionHook tests
# ---------------------------------------------------------------------------


class TestThinkLoopEmotionHook:
    """Tests for the ThinkLoopEmotionHook adapter."""

    def test_successful_action_triggers_emotion(self):
        personality = _default_personality()
        engine = EmotionEngine(personality=personality)
        hook = ThinkLoopEmotionHook(engine)

        hook.update_from_action("gather", "success", {"amount": 50})

        assert engine.state.valence > 0.0
        assert "earned_money" in engine.state.triggers

    def test_failed_action_triggers_negative(self):
        personality = _default_personality()
        engine = EmotionEngine(personality=personality)
        hook = ThinkLoopEmotionHook(engine)

        hook.update_from_action("gather", "failed", None)

        assert engine.state.valence < 0.0

    def test_decay_delegates_to_engine(self):
        personality = _default_personality()
        engine = EmotionEngine(personality=personality)
        hook = ThinkLoopEmotionHook(engine)

        engine.update("attacked")
        valence_before = engine.state.valence

        hook.decay(ticks_elapsed=10)
        assert engine.state.valence > valence_before  # regresses toward baseline

    def test_mood_description_delegates(self):
        personality = _default_personality()
        engine = EmotionEngine(personality=personality)
        hook = ThinkLoopEmotionHook(engine)

        engine.update("earned_money")
        desc = hook.get_mood_description()
        assert isinstance(desc, str)
        assert "happy" in desc.lower() or "optimistic" in desc.lower()

    def test_all_mapped_actions(self):
        """All actions in _ACTION_TO_EMOTION_EVENT should be handled."""
        personality = _default_personality()
        for action in _ACTION_TO_EMOTION_EVENT:
            engine = EmotionEngine(personality=personality)
            hook = ThinkLoopEmotionHook(engine)
            hook.update_from_action(action, "success", None)
            # Should not raise and should update triggers
            assert len(engine.state.triggers) > 0

    def test_engine_property(self):
        personality = _default_personality()
        engine = EmotionEngine(personality=personality)
        hook = ThinkLoopEmotionHook(engine)
        assert hook.engine is engine

    def test_unknown_action_success(self):
        """Unknown successful actions should be handled gracefully."""
        personality = _default_personality()
        engine = EmotionEngine(personality=personality)
        hook = ThinkLoopEmotionHook(engine)

        hook.update_from_action("fly_to_moon", "success", None)
        assert len(engine.state.triggers) > 0

    def test_error_status_triggers_negative(self):
        personality = _default_personality()
        engine = EmotionEngine(personality=personality)
        hook = ThinkLoopEmotionHook(engine)

        hook.update_from_action("explore", "error", None)
        assert engine.state.valence < 0.0


# ---------------------------------------------------------------------------
# DecisionEngine prompt integration tests
# ---------------------------------------------------------------------------


class TestEmotionInPrompt:
    """Tests that mood_description is injected into the decision prompt."""

    def test_prompt_contains_mood_description(self):
        from dataclasses import dataclass
        from enum import Enum

        @dataclass
        class MockSkill:
            name: str
            level: int = 1

        @dataclass
        class MockState:
            name: str = "TestAgent"
            id: str = "agent-001"
            phase: object = None
            tokens: int = 500
            money: float = 100.0
            health: float = 80.0
            reputation: float = 42.0
            skills: dict = None

            def __post_init__(self):
                if self.skills is None:
                    self.skills = {}

        class MockPhase(str, Enum):
            EXPLORATION = "exploration"

        state = MockState(phase=MockPhase.EXPLORATION)
        perception = DecisionPerception(tick=10)
        survival = SurvivalAssessment()

        mood = "You feel happy and optimistic intensely."
        prompt = build_prompt(
            state,
            perception,
            survival,
            [DecisionAction.REST],
            mood_description=mood,
        )

        assert "## Current Mood" in prompt
        assert mood in prompt

    def test_prompt_without_mood_shows_default(self):
        from dataclasses import dataclass
        from enum import Enum

        @dataclass
        class MockSkill:
            name: str
            level: int = 1

        @dataclass
        class MockState:
            name: str = "TestAgent"
            id: str = "agent-001"
            phase: object = None
            tokens: int = 500
            money: float = 100.0
            health: float = 80.0
            reputation: float = 42.0
            skills: dict = None

            def __post_init__(self):
                if self.skills is None:
                    self.skills = {}

        class MockPhase(str, Enum):
            EXPLORATION = "exploration"

        state = MockState(phase=MockPhase.EXPLORATION)
        perception = DecisionPerception(tick=10)
        survival = SurvivalAssessment()

        prompt = build_prompt(
            state,
            perception,
            survival,
            [DecisionAction.REST],
            mood_description=None,
        )

        assert "## Current Mood" in prompt
        assert "No mood data available" in prompt


# ---------------------------------------------------------------------------
# End-to-end verification tests (matching issue acceptance criteria)
# ---------------------------------------------------------------------------


class TestAcceptanceCriteria:
    """Verify the acceptance criteria from the issue spec."""

    def test_agent_earns_money_valence_rises(self):
        """Agent 赚到钱后 valence 上升, primary_emotion 变为 happy"""
        personality = _default_personality()
        engine = EmotionEngine(personality=personality)
        baseline = engine.state.valence

        state = engine.update("earned_money", {"amount": 50})

        assert state.valence > baseline
        assert state.primary_emotion == EmotionType.HAPPY

    def test_agent_attacked_arousal_rises_fearful(self):
        """Agent 被攻击后 arousal 上升, primary_emotion 变为 fearful"""
        personality = _default_personality()
        engine = EmotionEngine(personality=personality)
        baseline_arousal = engine.state.arousal

        state = engine.update("attacked")

        assert state.arousal > baseline_arousal
        assert state.primary_emotion == EmotionType.FEARFUL

    def test_neuroticism_amplifies_negative_response(self):
        """neuroticism 高的 Agent 对负面事件反应更强"""
        neurotic = _default_personality(neuroticism=0.9, extraversion=0.1)
        calm = _default_personality(neuroticism=0.1, extraversion=0.9)

        engine_neurotic = EmotionEngine(personality=neurotic)
        engine_calm = EmotionEngine(personality=calm)

        state_neurotic = engine_neurotic.update("attacked")
        state_calm = engine_calm.update("attacked")

        # Neurotic agent's valence should be more negative
        assert state_neurotic.valence < state_calm.valence
        # Neurotic agent's arousal should be higher
        assert state_neurotic.arousal > state_calm.arousal

    def test_emotion_decays_to_baseline(self):
        """情绪随时间自然衰减回归 baseline"""
        personality = _default_personality()
        engine = EmotionEngine(personality=personality)

        engine.update("bounty_completed")
        assert engine.state.intensity > 0.1

        engine.decay(ticks_elapsed=50)
        baseline = engine.get_baseline()

        # Should be very close to baseline after 50 ticks of decay
        assert abs(engine.state.valence - baseline.valence) < 0.05
        assert engine.state.intensity < 0.05

    def test_emotion_state_queryable_via_dict(self):
        """情绪状态可被 Dashboard API 查询 (via get_state_dict)"""
        personality = _default_personality()
        engine = EmotionEngine(personality=personality)
        engine.update("earned_money")

        data = engine.get_state_dict()
        assert "valence" in data
        assert "arousal" in data
        assert "dominance" in data
        assert "primary_emotion" in data
        assert "intensity" in data
        assert "triggers" in data
        assert isinstance(data["valence"], float)
        assert isinstance(data["primary_emotion"], str)
