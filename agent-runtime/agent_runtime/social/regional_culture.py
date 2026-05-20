"""Regional culture clustering — auto-detect cultural clusters from agent similarity.

Agents that are spatially close and share similar personality/value profiles
naturally cluster into cultural regions.  A simple K-means implementation
is used (no external dependencies).
"""

from __future__ import annotations

import math
import random
from typing import Any, Dict, List, Optional

from pydantic import BaseModel, Field

from agent_runtime.models.personality import PersonalityVector
from agent_runtime.models.values import ValueWeights


class Cluster(BaseModel):
    """A cultural cluster of agents."""

    cluster_id: str
    center_personality: PersonalityVector
    center_values: ValueWeights
    agent_ids: List[str] = Field(default_factory=list)
    region_id: str = ""


class RegionalCulture:
    """Detect and maintain cultural clusters based on agent similarity and proximity.

    Uses a simple K-means variant that combines personality distance and
    value-weight distance for clustering.
    """

    def __init__(self, n_clusters: int = 5, max_iterations: int = 20) -> None:
        self._n_clusters = n_clusters
        self._max_iterations = max_iterations
        self._clusters: Dict[str, Cluster] = {}

    # ── Public API ──

    def compute_regional_culture(
        self,
        region_id: str,
        agents: List[Dict[str, Any]],
    ) -> Dict[str, Any]:
        """Compute the aggregate culture for a specific region.

        Args:
            region_id: Region identifier.
            agents: List of agent dicts with 'personality' (PersonalityVector)
                    and 'values' (ValueWeights) fields.

        Returns:
            Dict with aggregate personality, values, and agent count.
        """
        if not agents:
            return {"region_id": region_id, "agent_count": 0}

        n = len(agents)
        personalities = [a["personality"] for a in agents]
        values = [a["values"] for a in agents]

        # Average each personality dimension
        dim_names = PersonalityVector._dimension_names()
        avg_p: Dict[str, float] = {}
        for dim in dim_names:
            avg_p[dim] = sum(getattr(p, dim) for p in personalities) / n

        # Average each value dimension
        val_names = ValueWeights._dimension_names()
        avg_v: Dict[str, float] = {}
        for dim in val_names:
            avg_v[dim] = sum(getattr(v, dim) for v in values) / n

        return {
            "region_id": region_id,
            "agent_count": n,
            "aggregate_personality": avg_p,
            "aggregate_values": avg_v,
        }

    def detect_cultural_clusters(
        self,
        world_agents: List[Dict[str, Any]],
        n_clusters: Optional[int] = None,
    ) -> List[Cluster]:
        """Auto-detect cultural clusters using K-means on personality + value vectors.

        Args:
            world_agents: List of agent dicts with 'id', 'personality' (PersonalityVector),
                         'values' (ValueWeights), and optional 'region_id' fields.
            n_clusters: Override for number of clusters.

        Returns:
            List of Cluster objects with assigned agent_ids.
        """
        k = n_clusters or self._n_clusters

        if len(world_agents) <= k:
            # Fewer agents than clusters — each agent is its own cluster
            clusters: List[Cluster] = []
            for i, agent in enumerate(world_agents):
                cluster = Cluster(
                    cluster_id=f"cluster_{i}",
                    center_personality=agent["personality"],
                    center_values=agent["values"],
                    agent_ids=[agent["id"]],
                    region_id=agent.get("region_id", ""),
                )
                clusters.append(cluster)
            self._clusters = {c.cluster_id: c for c in clusters}
            return clusters

        # Build feature vectors for K-means
        features = self._build_feature_vectors(world_agents)

        # Initialize centroids by random selection
        rng = random.Random(42)
        initial_indices = rng.sample(range(len(world_agents)), k)
        centroids = [features[i] for i in initial_indices]

        # K-means iterations
        assignments: List[int] = [0] * len(world_agents)

        for _ in range(self._max_iterations):
            # Assign each agent to nearest centroid
            new_assignments: List[int] = []
            for feat in features:
                min_dist = float("inf")
                best = 0
                for ci, centroid in enumerate(centroids):
                    d = self._euclidean_distance(feat, centroid)
                    if d < min_dist:
                        min_dist = d
                        best = ci
                new_assignments.append(best)

            # Check convergence
            if new_assignments == assignments:
                break
            assignments = new_assignments

            # Update centroids
            new_centroids: List[List[float]] = []
            for ci in range(k):
                members = [
                    features[j] for j in range(len(features)) if assignments[j] == ci
                ]
                if members:
                    centroid = [
                        sum(m[d] for m in members) / len(members)
                        for d in range(len(members[0]))
                    ]
                else:
                    centroid = centroids[ci]
                new_centroids.append(centroid)
            centroids = new_centroids

        # Build Cluster objects
        result: List[Cluster] = []
        for ci in range(k):
            member_indices = [j for j in range(len(world_agents)) if assignments[j] == ci]
            member_agents = [world_agents[j] for j in member_indices]
            member_ids = [a["id"] for a in member_agents]

            # Compute cluster center from actual members
            if member_agents:
                personalities = [a["personality"] for a in member_agents]
                values = [a["values"] for a in member_agents]
                n = len(member_agents)

                dim_names = PersonalityVector._dimension_names()
                center_p = PersonalityVector(**{
                    dim: sum(getattr(p, dim) for p in personalities) / n
                    for dim in dim_names
                })
                val_names = ValueWeights._dimension_names()
                center_v = ValueWeights(**{
                    dim: sum(getattr(v, dim) for v in values) / n
                    for dim in val_names
                })
            else:
                center_p = PersonalityVector()
                center_v = ValueWeights()

            cluster = Cluster(
                cluster_id=f"cluster_{ci}",
                center_personality=center_p,
                center_values=center_v,
                agent_ids=member_ids,
                region_id=member_agents[0].get("region_id", "") if member_agents else "",
            )
            result.append(cluster)

        self._clusters = {c.cluster_id: c for c in result}
        return result

    def get_cluster_boundary(
        self,
        cluster_a: Cluster,
        cluster_b: Cluster,
    ) -> Dict[str, Any]:
        """Compute boundary characteristics between two cultural clusters.

        Returns the distance between cluster centers and the dimensions
        with the largest cultural differences.
        """
        p_dist = cluster_a.center_personality.distance(cluster_b.center_personality)

        val_names = ValueWeights._dimension_names()
        val_diffs: Dict[str, float] = {}
        for dim in val_names:
            va = getattr(cluster_a.center_values, dim)
            vb = getattr(cluster_b.center_values, dim)
            val_diffs[dim] = abs(va - vb)

        # Top 3 dimensions with largest differences
        sorted_diffs = sorted(val_diffs.items(), key=lambda x: x[1], reverse=True)
        top_differences = sorted_diffs[:3]

        return {
            "cluster_a": cluster_a.cluster_id,
            "cluster_b": cluster_b.cluster_id,
            "personality_distance": p_dist,
            "value_differences": val_diffs,
            "top_differences": top_differences,
            "combined_agent_count": len(cluster_a.agent_ids) + len(cluster_b.agent_ids),
        }

    # ── Accessors ──

    def get_cluster(self, cluster_id: str) -> Optional[Cluster]:
        """Get a cluster by ID."""
        return self._clusters.get(cluster_id)

    def find_agent_cluster(self, agent_id: str) -> Optional[Cluster]:
        """Find which cluster an agent belongs to."""
        for cluster in self._clusters.values():
            if agent_id in cluster.agent_ids:
                return cluster
        return None

    # ── Internals ──

    def _build_feature_vectors(
        self, agents: List[Dict[str, Any]]
    ) -> List[List[float]]:
        """Combine personality + value dimensions into flat feature vectors."""
        vectors: List[List[float]] = []
        for agent in agents:
            p = agent["personality"]
            v = agent["values"]
            feat: List[float] = []
            for dim in PersonalityVector._dimension_names():
                feat.append(getattr(p, dim))
            for dim in ValueWeights._dimension_names():
                feat.append(getattr(v, dim))
            vectors.append(feat)
        return vectors

    @staticmethod
    def _euclidean_distance(a: List[float], b: List[float]) -> float:
        return math.sqrt(sum((x - y) ** 2 for x, y in zip(a, b)))
