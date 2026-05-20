"""Cultural conflict detection and fusion effects.

When agents from different cultural backgrounds interact, their value
differences may cause conflict (if above a threshold) or fusion
(gradual blending at cultural boundaries).
"""

from __future__ import annotations

import math
import random
from typing import Any, Dict, List, Optional

from pydantic import BaseModel, Field

from agent_runtime.models.personality import PersonalityVector
from agent_runtime.models.values import ValueWeights

# Value difference threshold above which a conflict is detected.
CONFLICT_THRESHOLD = 0.4

# Maximum fusion adjustment per tick.
MAX_FUSION_DELTA = 0.002


class AgentInteraction(BaseModel):
    """Record of an interaction between two agents."""

    agent_a_id: str
    agent_b_id: str
    agent_a_values: ValueWeights
    agent_b_values: ValueWeights
    agent_a_personality: Optional[PersonalityVector] = None
    agent_b_personality: Optional[PersonalityVector] = None
    interaction_type: str = "generic"
    tick: int = 0


class ConflictReport(BaseModel):
    """Report of a detected cultural conflict."""

    agent_a_id: str
    agent_b_id: str
    conflict_score: float = Field(ge=0.0, le=1.0)
    conflicting_dimensions: List[str]
    value_differences: Dict[str, float]
    tick: int = 0


class CulturalConflictAndFusion:
    """Detect cultural conflicts and apply fusion effects at cultural boundaries.

    Conflict: When two interacting agents have value differences above a threshold,
    a ConflictReport is generated.
    Fusion: Agents at cultural boundaries (close to agents from other cultures)
    gradually blend their values.
    """

    def __init__(self, conflict_threshold: float = CONFLICT_THRESHOLD) -> None:
        self._conflict_threshold = conflict_threshold
        self._conflict_history: List[ConflictReport] = []

    # ── Conflict detection ──

    def detect_cultural_conflict(
        self,
        interaction: AgentInteraction,
    ) -> Optional[ConflictReport]:
        """Detect whether an interaction involves cultural conflict.

        A conflict exists when the value difference in any dimension exceeds
        the conflict threshold.  The conflict score is the maximum normalized
        difference across all dimensions.

        Args:
            interaction: The agent interaction to evaluate.

        Returns:
            A ConflictReport if conflict detected, None otherwise.
        """
        val_names = ValueWeights._dimension_names()
        differences: Dict[str, float] = {}
        conflicting_dims: List[str] = []

        for dim in val_names:
            va = getattr(interaction.agent_a_values, dim)
            vb = getattr(interaction.agent_b_values, dim)
            diff = abs(va - vb)
            differences[dim] = diff
            if diff > self._conflict_threshold:
                conflicting_dims.append(dim)

        if not conflicting_dims:
            return None

        # Conflict score: max normalized difference
        max_diff = max(differences.values())
        conflict_score = min(1.0, max_diff)

        report = ConflictReport(
            agent_a_id=interaction.agent_a_id,
            agent_b_id=interaction.agent_b_id,
            conflict_score=conflict_score,
            conflicting_dimensions=conflicting_dims,
            value_differences=differences,
            tick=interaction.tick,
        )

        self._conflict_history.append(report)
        return report

    # ── Fusion effect ──

    def apply_fusion_effect(
        self,
        border_agents: List[Dict[str, Any]],
        intensity: float = 1.0,
    ) -> Dict[str, Any]:
        """Apply cultural fusion to agents at cultural boundaries.

        Border agents are those who interact frequently with agents from
        other cultures.  Fusion nudges their values toward the average of
        their neighbors, bounded by MAX_FUSION_DELTA per dimension.

        Args:
            border_agents: List of dicts with 'id', 'values' (ValueWeights),
                          and 'neighbor_values' (list of ValueWeights).
            intensity: Multiplier for fusion strength (0.0 to 1.0).

        Returns:
            Dict with 'affected_agents' count, per-agent adjustments, and
            'updated_values' mapping agent_id -> new ValueWeights instance.
        """
        affected = 0
        adjustments: Dict[str, Dict[str, float]] = {}
        updated_values: Dict[str, ValueWeights] = {}

        for agent_data in border_agents:
            agent_values: ValueWeights = agent_data["values"]
            neighbors: List[ValueWeights] = agent_data.get("neighbor_values", [])
            agent_id: str = agent_data["id"]

            if not neighbors:
                updated_values[agent_id] = agent_values
                continue

            n = len(neighbors)
            val_names = ValueWeights._dimension_names()
            agent_adjustments: Dict[str, float] = {}
            updates: Dict[str, float] = {}

            for dim in val_names:
                current = getattr(agent_values, dim)
                neighbor_avg = sum(getattr(nv, dim) for nv in neighbors) / n
                diff = neighbor_avg - current

                # Bounded fusion delta
                delta = max(
                    -MAX_FUSION_DELTA * intensity,
                    min(MAX_FUSION_DELTA * intensity, diff),
                )

                if abs(delta) > 1e-9:
                    updates[dim] = max(0.0, min(1.0, current + delta))
                    agent_adjustments[dim] = delta

            if updates:
                updated_values[agent_id] = agent_values.model_copy(update=updates)
            else:
                updated_values[agent_id] = agent_values

            if agent_adjustments:
                affected += 1
                adjustments[agent_id] = agent_adjustments

        return {
            "affected_agents": affected,
            "total_border_agents": len(border_agents),
            "adjustments": adjustments,
            "updated_values": updated_values,
        }

    # ── Diversity index ──

    def compute_cultural_diversity_index(
        self,
        world_agents: List[Dict[str, Any]],
    ) -> float:
        """Compute world cultural diversity index in [0, 1].

        Uses the average pairwise Euclidean distance between agents' value
        vectors, normalized to [0, 1].
        - 0 = completely homogeneous (all agents identical)
        - 1 = maximum diversity (values maximally spread)

        For efficiency with large populations, uses a sampling approach
        when agent count exceeds 100.

        Args:
            world_agents: List of agent dicts with 'values' (ValueWeights).

        Returns:
            Diversity index in [0, 1].
        """
        n = len(world_agents)
        if n <= 1:
            return 0.0

        values_list = [a["values"] for a in world_agents]
        val_names = ValueWeights._dimension_names()

        # For large populations, sample pairs
        if n > 100:
            n_pairs = 5000
            total_dist = 0.0
            for _ in range(n_pairs):
                i, j = random.sample(range(n), 2)
                dist = math.sqrt(
                    sum(
                        (getattr(values_list[i], d) - getattr(values_list[j], d)) ** 2
                        for d in val_names
                    )
                )
                total_dist += dist
            avg_dist = total_dist / n_pairs
        else:
            total_dist = 0.0
            count = 0
            for i in range(n):
                for j in range(i + 1, n):
                    dist = math.sqrt(
                        sum(
                            (getattr(values_list[i], d) - getattr(values_list[j], d)) ** 2
                            for d in val_names
                        )
                    )
                    total_dist += dist
                    count += 1
            avg_dist = total_dist / count if count > 0 else 0.0

        # Normalize: max possible distance is sqrt(num_dimensions) (each dim 0 vs 1)
        max_possible = math.sqrt(len(val_names))
        if max_possible == 0:
            return 0.0

        return min(1.0, avg_dist / max_possible)

    # ── History access ──

    @property
    def conflict_history(self) -> List[ConflictReport]:
        """Read-only access to conflict history."""
        return list(self._conflict_history)

    def get_conflicts_for_agent(self, agent_id: str) -> List[ConflictReport]:
        """Get all conflicts involving a specific agent."""
        return [
            r
            for r in self._conflict_history
            if r.agent_a_id == agent_id or r.agent_b_id == agent_id
        ]
