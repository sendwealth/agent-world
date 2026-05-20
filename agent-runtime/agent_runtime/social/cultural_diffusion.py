"""Cultural diffusion — regional and organizational value convergence.

Agents in the same region or organization slowly converge in their values,
producing emergent group cultures. The convergence rate is intentionally very
slow (0.3% per interaction) to preserve individual diversity while still
enabling macro-level cultural patterns.
"""

from __future__ import annotations

import logging
import math
from typing import Any, Dict, List, Tuple

from agent_runtime.models.personality import PersonalityVector
from agent_runtime.models.values import ValueWeights

logger = logging.getLogger(__name__)

# Hard cap: maximum change per dimension per diffusion step.
CULTURAL_DIFFUSION_RATE = 0.003


class CulturalDiffusion:
    """Manages regional and organizational cultural convergence."""

    def apply_regional_influence(
        self,
        agents: List[Dict[str, Any]],
        region_id: str,
    ) -> Dict[str, Any]:
        """Apply regional culture influence to a group of agents.

        Computes the regional average values, then nudges each agent's values
        toward that average by CULTURAL_DIFFUSION_RATE.

        Args:
            agents: List of dicts, each with keys:
                agent_id (str), values (ValueWeights),
                personality (PersonalityVector), region_id (str).
                The values/personality objects are mutated in place.
            region_id: The region whose culture is being applied.

        Returns:
            Dict with region_id, agent_count, avg_values (dict),
            total_adjustments (dict of dim -> total absolute change).
        """
        if len(agents) < 2:
            return {
                "region_id": region_id,
                "agent_count": len(agents),
                "avg_values": {},
                "total_adjustments": {},
            }

        # Compute regional value average
        dim_names = ValueWeights._dimension_names()
        sums: Dict[str, float] = {d: 0.0 for d in dim_names}
        for agent in agents:
            v: ValueWeights = agent["values"]
            for d in dim_names:
                sums[d] += getattr(v, d)

        avg: Dict[str, float] = {d: sums[d] / len(agents) for d in dim_names}

        # Nudge each agent toward regional average
        total_adj: Dict[str, float] = {d: 0.0 for d in dim_names}
        for agent in agents:
            v: ValueWeights = agent["values"]
            for d in dim_names:
                current = getattr(v, d)
                diff = avg[d] - current
                # Apply at most CULTURAL_DIFFUSION_RATE per dimension
                delta = max(-CULTURAL_DIFFUSION_RATE, min(CULTURAL_DIFFUSION_RATE, diff))
                new_val = max(0.0, min(1.0, current + delta))
                actual_delta = new_val - current
                object.__setattr__(v, d, new_val)
                total_adj[d] += abs(actual_delta)

        logger.debug(
            "Regional diffusion (%s): %d agents, adjustments=%s",
            region_id,
            len(agents),
            {d: round(total_adj[d], 4) for d in dim_names},
        )

        return {
            "region_id": region_id,
            "agent_count": len(agents),
            "avg_values": avg,
            "total_adjustments": total_adj,
        }

    def apply_organizational_culture(
        self,
        org_id: str,
        org_culture: ValueWeights,
        members: List[Dict[str, Any]],
    ) -> Dict[str, Any]:
        """Apply organizational culture to members.

        Unlike regional influence (converge to average), org culture has a
        declared culture vector (e.g. from the org charter). Members' values
        are nudged toward the org culture at CULTURAL_DIFFUSION_RATE.

        Agreeableness acts as a moderator: agents with higher agreeableness
        are more influenced by org culture, but the rate is capped so it
        never exceeds CULTURAL_DIFFUSION_RATE.

        Args:
            org_id: Organization identifier.
            org_culture: The organization's declared value vector.
            members: List of dicts with agent_id, values (ValueWeights),
                personality (PersonalityVector). Mutated in place.

        Returns:
            Dict with org_id, member_count, total_adjustments.
        """
        if not members:
            return {
                "org_id": org_id,
                "member_count": 0,
                "total_adjustments": {},
            }

        dim_names = ValueWeights._dimension_names()
        total_adj: Dict[str, float] = {d: 0.0 for d in dim_names}

        for member in members:
            v: ValueWeights = member["values"]
            p: PersonalityVector = member.get("personality", None)

            for d in dim_names:
                current = getattr(v, d)
                target = getattr(org_culture, d)
                diff = target - current

                # Agreeableness modulates org influence: only reduces rate, never amplifies
                agreeableness_factor = 1.0
                if p is not None:
                    agreeableness_factor = 0.5 + 0.5 * p.agreeableness

                rate = CULTURAL_DIFFUSION_RATE * min(1.0, agreeableness_factor)
                delta = max(-rate, min(rate, diff))
                new_val = max(0.0, min(1.0, current + delta))
                actual_delta = new_val - current
                object.__setattr__(v, d, new_val)
                total_adj[d] += abs(actual_delta)

        logger.debug(
            "Org culture (%s): %d members, adjustments=%s",
            org_id,
            len(members),
            {d: round(total_adj[d], 4) for d in dim_names},
        )

        return {
            "org_id": org_id,
            "member_count": len(members),
            "total_adjustments": total_adj,
        }

    def compute_cultural_distance(
        self,
        group_a: List[ValueWeights],
        group_b: List[ValueWeights],
    ) -> float:
        """Compute Euclidean cultural distance between two agent groups.

        Distance is based on the difference of group-mean value vectors.

        Args:
            group_a: List of ValueWeights for group A.
            group_b: List of ValueWeights for group B.

        Returns:
            Euclidean distance between group centroids (0.0 = identical culture).
        """
        dim_names = ValueWeights._dimension_names()

        def _centroid(group: List[ValueWeights]) -> Dict[str, float]:
            if not group:
                return {d: 0.5 for d in dim_names}
            sums = {d: 0.0 for d in dim_names}
            for v in group:
                for d in dim_names:
                    sums[d] += getattr(v, d)
            return {d: sums[d] / len(group) for d in dim_names}

        ca = _centroid(group_a)
        cb = _centroid(group_b)

        return math.sqrt(sum((ca[d] - cb[d]) ** 2 for d in dim_names))
