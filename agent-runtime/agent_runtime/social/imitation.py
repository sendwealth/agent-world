"""Imitation engine — agents observe and partially mimic successful peers.

Imitation is **weighted fusion**, not direct copy. The observer blends a fraction
of the observed agent's personality/values into their own, preserving individual
identity while allowing behavioral convergence.
"""

from __future__ import annotations

import logging
import math
import random
from typing import Any, Dict, List, Optional, Tuple

from agent_runtime.models.personality import PersonalityVector
from agent_runtime.models.values import ValueWeights

logger = logging.getLogger(__name__)

# Base imitation rate — how much of the observed agent's traits are adopted.
BASE_IMITATION_RATE = 0.05

# Minimum openness threshold to even consider imitation.
OPENNESS_THRESHOLD = 0.3

# Maximum possible Euclidean distance between two 8-dim personality vectors
# (each dim in [0, 1], so max per dim = 1.0, max total = sqrt(8)).
MAX_PERSONALITY_DISTANCE = math.sqrt(8)


class ImitationEngine:
    """Agents observe successful peers and partially imitate their traits."""

    def observe_and_maybe_imitate(
        self,
        observer_personality: PersonalityVector,
        observer_values: ValueWeights,
        observed_personality: PersonalityVector,
        observed_values: ValueWeights,
        observed_success_score: float,
        context: Dict[str, Any],
    ) -> Optional[Dict[str, Any]]:
        """Observer considers imitating an observed agent.

        Imitation probability is determined by:
        1. Observer's openness (low openness → unlikely to imitate)
        2. Observed agent's success_score (more successful → more worth imitating)
        3. Personality distance (too different → harder to imitate)

        If imitation occurs, it is a **weighted fusion**: the observer's traits
        move a small step toward the observed agent's traits.

        Args:
            observer_personality: Observer's personality (mutated in place on imitation).
            observer_values: Observer's values (mutated in place on imitation).
            observed_personality: The observed agent's personality.
            observed_values: The observed agent's values.
            observed_success_score: How successful the observed agent is (0.0 - 1.0).
            context: Current situation context.

        Returns:
            None if no imitation occurred, or a dict with details:
            {imitated: True, rate: float, personality_deltas: dict, value_deltas: dict}
        """
        # Gate: low openness agents rarely imitate
        if observer_personality.openness < OPENNESS_THRESHOLD:
            return None

        # Imitation probability: openness * success_score
        imitation_prob = (
            observer_personality.openness * observed_success_score * 0.5
        )
        if random.random() > imitation_prob:
            return None

        # Compute effective imitation rate
        # Social agents imitate more; distant agents imitate less
        social_factor = observer_personality.social_orientation
        distance = observer_personality.distance(observed_personality)
        distance_factor = max(0.1, 1.0 - distance / MAX_PERSONALITY_DISTANCE)

        rate = BASE_IMITATION_RATE * social_factor * distance_factor
        rate = min(rate, 0.1)  # hard cap at 10% per observation

        # Weighted fusion: move observer toward observed
        personality_deltas: Dict[str, float] = {}
        for dim in PersonalityVector._dimension_names():
            obs_val = getattr(observer_personality, dim)
            tgt_val = getattr(observed_personality, dim)
            delta = (tgt_val - obs_val) * rate
            new_val = max(0.0, min(1.0, obs_val + delta))
            personality_deltas[dim] = new_val - obs_val
            object.__setattr__(observer_personality, dim, new_val)

        value_deltas: Dict[str, float] = {}
        for dim in ValueWeights._dimension_names():
            obs_val = getattr(observer_values, dim)
            tgt_val = getattr(observed_values, dim)
            delta = (tgt_val - obs_val) * rate
            new_val = max(0.0, min(1.0, obs_val + delta))
            value_deltas[dim] = new_val - obs_val
            object.__setattr__(observer_values, dim, new_val)

        logger.debug(
            "Imitation: rate=%.4f, personality_distance=%.3f, success=%.2f",
            rate,
            distance,
            observed_success_score,
        )

        return {
            "imitated": True,
            "rate": rate,
            "personality_deltas": personality_deltas,
            "value_deltas": value_deltas,
        }

    def get_role_models(
        self,
        agent_personality: PersonalityVector,
        candidates: List[Dict[str, Any]],
        current_context: Dict[str, Any],
        top_k: int = 3,
    ) -> List[Dict[str, Any]]:
        """Identify the best role models for an agent in the current context.

        Scoring considers:
        1. Success score (reputation, resource level, etc.)
        2. Similarity bonus (similar agents are more relatable)
        3. Context relevance (matching event type or situation)

        Args:
            agent_personality: The agent seeking role models.
            candidates: List of dicts, each with keys:
                agent_id, personality (PersonalityVector), success_score,
                context_tags (list[str]).
            current_context: Current situation context.
            top_k: Number of role models to return.

        Returns:
            Top-k candidates sorted by role-model score, each with an added
            'score' key.
        """
        if not candidates:
            return []

        ctx_tags = set(current_context.get("tags", []))
        event_type = current_context.get("event_type", "")

        scored: List[Tuple[float, Dict[str, Any]]] = []
        for candidate in candidates:
            score = candidate.get("success_score", 0.0)

            # Similarity bonus (closer personality → more relatable)
            cand_pers: PersonalityVector = candidate["personality"]
            distance = agent_personality.distance(cand_pers)
            similarity_bonus = max(0.0, 1.0 - distance / MAX_PERSONALITY_DISTANCE)
            score += similarity_bonus * 0.3

            # Context tag overlap
            cand_tags = set(candidate.get("context_tags", []))
            if ctx_tags & cand_tags:
                score += 0.2

            # Event type match
            if event_type and event_type in candidate.get("context_tags", []):
                score += 0.1

            entry = dict(candidate)
            entry["score"] = score
            scored.append((score, entry))

        scored.sort(key=lambda x: x[0], reverse=True)
        return [entry for _, entry in scored[:top_k]]
