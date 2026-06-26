"""Tests for the personality system: PersonalityVector, ValueWeights, and ExperienceAccumulator.

Covers: creation, validation, mutation, distance, serialization, prompt generation,
experience recording, value updates, and personality drift.
"""

import math

import pytest
from pydantic import ValidationError

from agent_runtime.core.experience import Experience, ExperienceAccumulator
from agent_runtime.models.personality import PersonalityVector
from agent_runtime.models.values import MAX_ADJUSTMENT, ValueWeights

# ============================================================
# PersonalityVector tests
# ============================================================


class TestPersonalityVectorCreation:
    def test_default_creation(self):
        pv = PersonalityVector()
        assert pv.openness == 0.5
        assert pv.conscientiousness == 0.5
        assert pv.extraversion == 0.5
        assert pv.agreeableness == 0.5
        assert pv.neuroticism == 0.5
        assert pv.risk_tolerance == 0.5
        assert pv.social_orientation == 0.5
        assert pv.greed == 0.5

    def test_custom_creation(self):
        pv = PersonalityVector(openness=0.9, greed=0.1)
        assert pv.openness == 0.9
        assert pv.greed == 0.1

    def test_bounds_validation(self):
        with pytest.raises(ValidationError):
            PersonalityVector(openness=1.5)
        with pytest.raises(ValidationError):
            PersonalityVector(openness=-0.1)

    def test_random_creation(self):
        pv = PersonalityVector.random()
        for dim in PersonalityVector._dimension_names():
            val = getattr(pv, dim)
            assert 0.0 <= val <= 1.0

    def test_random_with_seed(self):
        import random
        rng = random.Random(42)
        pv1 = PersonalityVector.random(rng)
        rng2 = random.Random(42)
        pv2 = PersonalityVector.random(rng2)
        assert pv1 == pv2


class TestPersonalityVectorMutation:
    def test_mutate_returns_new_instance(self):
        pv = PersonalityVector()
        mutated = pv.mutate(rate=0.05)
        assert mutated is not pv

    def test_mutate_stays_in_bounds(self):
        pv = PersonalityVector(openness=0.99)
        for _ in range(100):
            pv = pv.mutate(rate=0.1)
            assert 0.0 <= pv.openness <= 1.0

    def test_mutate_with_experience_offsets(self):
        pv = PersonalityVector(openness=0.5)
        mutated = pv.mutate(
            rate=0.05,
            experience_offsets={"openness": 0.04},
        )
        # Should have shifted openness upward (offset + noise)
        assert mutated.openness > 0.4  # rough check


class TestPersonalityVectorDistance:
    def test_distance_to_self(self):
        pv = PersonalityVector()
        assert pv.distance(pv) == 0.0

    def test_distance_symmetry(self):
        pv1 = PersonalityVector(openness=0.9)
        pv2 = PersonalityVector(openness=0.1)
        assert abs(pv1.distance(pv2) - pv2.distance(pv1)) < 1e-9

    def test_distance_max(self):
        pv1 = PersonalityVector(
            openness=0.0, conscientiousness=0.0, extraversion=0.0,
            agreeableness=0.0, neuroticism=0.0, risk_tolerance=0.0,
            social_orientation=0.0, greed=0.0,
        )
        pv2 = PersonalityVector(
            openness=1.0, conscientiousness=1.0, extraversion=1.0,
            agreeableness=1.0, neuroticism=1.0, risk_tolerance=1.0,
            social_orientation=1.0, greed=1.0,
        )
        expected = math.sqrt(8)  # 8 dimensions, each diff = 1
        assert abs(pv1.distance(pv2) - expected) < 1e-9


class TestPersonalityVectorPrompt:
    def test_balanced_prompt(self):
        pv = PersonalityVector()
        desc = pv.to_prompt_description()
        assert "balanced" in desc.lower()

    def test_high_openness_prompt(self):
        pv = PersonalityVector(openness=0.9)
        desc = pv.to_prompt_description()
        assert "curious" in desc.lower() or "exploring" in desc.lower()


class TestPersonalityVectorSerialization:
    def test_to_storage_dict_roundtrip(self):
        pv = PersonalityVector(openness=0.8, greed=0.3)
        d = pv.to_storage_dict()
        assert isinstance(d, dict)
        assert d["openness"] == 0.8
        assert d["greed"] == 0.3

        restored = PersonalityVector.from_storage_dict(d)
        assert restored == pv

    def test_json_roundtrip(self):
        pv = PersonalityVector(openness=0.7, extraversion=0.3)
        json_str = pv.model_dump_json()
        restored = PersonalityVector.model_validate_json(json_str)
        assert restored == pv

    def test_from_storage_dict_tolerant(self):
        """Extra keys are ignored, missing keys get defaults."""
        d = {"openness": 0.9, "unknown_key": 42}
        pv = PersonalityVector.from_storage_dict(d)
        assert pv.openness == 0.9
        assert pv.greed == 0.5  # default


# ============================================================
# ValueWeights tests
# ============================================================


class TestValueWeightsCreation:
    def test_default_creation(self):
        vw = ValueWeights()
        assert vw.survival_priority == 0.5
        assert vw.cooperation_weight == 0.5
        assert vw.competition_weight == 0.5
        assert vw.exploration_drive == 0.5
        assert vw.tradition_adherence == 0.3
        assert vw.innovation_tendency == 0.3

    def test_bounds_validation(self):
        with pytest.raises(ValidationError):
            ValueWeights(survival_priority=1.5)


class TestValueWeightsExperience:
    def test_cooperation_positive_raises_cooperation(self):
        vw = ValueWeights()
        vw.update_from_experience("cooperation", 1.0)
        assert vw.cooperation_weight > 0.5

    def test_betrayal_raises_competition(self):
        vw = ValueWeights()
        vw.update_from_experience("betrayal", -1.0)
        assert vw.competition_weight > 0.5

    def test_exploration_positive_raises_drive(self):
        vw = ValueWeights()
        vw.update_from_experience("exploration", 1.0)
        assert vw.exploration_drive > 0.5

    def test_death_witness_raises_survival(self):
        vw = ValueWeights()
        vw.update_from_experience("death_witness", -1.0)
        assert vw.survival_priority > 0.5

    def test_max_adjustment_cap(self):
        """Single event should not adjust more than MAX_ADJUSTMENT."""
        vw = ValueWeights()
        old_coop = vw.cooperation_weight
        vw.update_from_experience("cooperation", 1.0)
        delta = abs(vw.cooperation_weight - old_coop)
        assert delta <= MAX_ADJUSTMENT + 1e-9

    def test_values_stay_in_bounds(self):
        vw = ValueWeights()
        for event in ["trade", "cooperation", "betrayal", "exploration", "death_witness"]:
            for _ in range(100):
                vw.update_from_experience(event, 1.0)
                for dim in ValueWeights._dimension_names():
                    val = getattr(vw, dim)
                    assert 0.0 <= val <= 1.0, f"{dim}={val} out of bounds after {event}"


class TestValueWeightsDecay:
    def test_decay_moves_toward_midpoint(self):
        vw = ValueWeights(cooperation_weight=0.9)
        vw.decay(rate=0.5)
        assert vw.cooperation_weight < 0.9
        assert vw.cooperation_weight > 0.5  # moved toward 0.5

    def test_survival_priority_exempt_from_decay(self):
        vw = ValueWeights(survival_priority=0.9)
        old_survival = vw.survival_priority
        vw.decay(rate=0.5)
        assert vw.survival_priority == old_survival  # unchanged


class TestValueWeightsSerialization:
    def test_roundtrip(self):
        vw = ValueWeights(cooperation_weight=0.7, exploration_drive=0.8)
        d = vw.to_storage_dict()
        restored = ValueWeights.from_storage_dict(d)
        assert restored == vw


class TestValueWeightsPrompt:
    def test_balanced_prompt(self):
        vw = ValueWeights()
        summary = vw.to_prompt_summary()
        assert "balanced" in summary.lower()


# ============================================================
# ExperienceAccumulator tests
# ============================================================


class TestExperience:
    def test_create_experience(self):
        exp = Experience(
            tick=1,
            event_type="cooperation",
            partner_id="agent-2",
            outcome=0.8,
            context={"location": "market"},
            learned="Trading is profitable",
        )
        assert exp.tick == 1
        assert exp.event_type == "cooperation"
        assert exp.outcome == 0.8

    def test_outcome_bounds(self):
        with pytest.raises(ValidationError):
            Experience(tick=1, event_type="trade", outcome=2.0)


class TestExperienceAccumulator:
    def test_record_updates_values(self):
        pv = PersonalityVector()
        vw = ValueWeights()
        acc = ExperienceAccumulator(pv, vw)

        exp = Experience(tick=1, event_type="cooperation", outcome=1.0)
        acc.record(exp)

        assert vw.cooperation_weight > 0.5

    def test_record_mutates_personality(self):
        pv = PersonalityVector(openness=0.5)
        vw = ValueWeights()
        acc = ExperienceAccumulator(pv, vw)

        exp = Experience(tick=1, event_type="exploration", outcome=1.0)
        acc.record(exp)

        # Personality should have been mutated (new instance)
        assert acc.personality is not pv

    def test_history_bounded(self):
        pv = PersonalityVector()
        vw = ValueWeights()
        acc = ExperienceAccumulator(pv, vw, max_history=5)

        for i in range(10):
            acc.record(Experience(tick=i, event_type="trade", outcome=0.5))

        assert acc.experience_count == 5

    def test_get_relevant_experiences(self):
        pv = PersonalityVector()
        vw = ValueWeights()
        acc = ExperienceAccumulator(pv, vw)

        acc.record(Experience(tick=1, event_type="trade", outcome=0.5))
        acc.record(Experience(tick=2, event_type="cooperation", outcome=0.8))
        acc.record(Experience(tick=3, event_type="betrayal", outcome=-0.5))
        acc.record(Experience(tick=4, event_type="trade", outcome=-0.3))

        # Query for trade events
        results = acc.get_relevant_experiences(
            {"event_type": "trade", "outcome": 0.5},
            top_k=2,
        )
        assert len(results) == 2
        # First should be a trade match
        assert results[0].event_type == "trade"

    def test_get_relevant_experiences_empty(self):
        pv = PersonalityVector()
        vw = ValueWeights()
        acc = ExperienceAccumulator(pv, vw)
        results = acc.get_relevant_experiences({}, top_k=5)
        assert results == []

    def test_personality_snapshot(self):
        pv = PersonalityVector(openness=0.8)
        vw = ValueWeights(cooperation_weight=0.7)
        acc = ExperienceAccumulator(pv, vw)

        acc.record(Experience(tick=1, event_type="trade", outcome=0.5))
        snapshot = acc.get_personality_snapshot()

        assert "personality" in snapshot
        assert "values" in snapshot
        assert "experience_count" in snapshot
        assert snapshot["experience_count"] == 1
        assert "prompt_description" in snapshot
        assert "value_summary" in snapshot

    def test_history_read_only(self):
        pv = PersonalityVector()
        vw = ValueWeights()
        acc = ExperienceAccumulator(pv, vw)
        acc.record(Experience(tick=1, event_type="trade", outcome=0.5))

        hist = acc.history
        assert len(hist) == 1
        # Modifying returned list should not affect internal state
        hist.clear()
        assert acc.experience_count == 1

    def test_multiple_events_accumulate(self):
        pv = PersonalityVector()
        vw = ValueWeights()
        acc = ExperienceAccumulator(pv, vw)

        for i in range(20):
            acc.record(Experience(
                tick=i,
                event_type="cooperation",
                outcome=0.9,
            ))

        # After many positive cooperation events, cooperation should be high
        assert acc.values.cooperation_weight > 0.7
