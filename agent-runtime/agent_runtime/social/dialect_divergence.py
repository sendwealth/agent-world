"""Dialect divergence analysis — detect regional and organizational communication splits.

Groups agents by geography (region_id) or organization, then measures how
their communication patterns diverge over time.  Outputs a dialect distance
matrix, dialect region boundaries, and convergence / isolation metrics.

Pure Python — no external ML/NLP dependencies beyond what already ships with
the agent-runtime (dataclasses, collections, math).

Typical usage::

    from agent_runtime.social.dialect_divergence import DialectDivergenceAnalyzer

    analyzer = DialectDivergenceAnalyzer()
    matrix = analyzer.compute_distance_matrix(group_messages)
    regions = analyzer.detect_dialect_regions(group_messages, agent_locations)
"""

from __future__ import annotations

import logging
import math
from collections import Counter, defaultdict
from dataclasses import dataclass, field
from typing import Any, Optional

from agent_runtime.social.comm_analyzer import CommunicationAnalyzer, MessagePattern
from agent_runtime.social.jargon_detector import JargonDetector

logger = logging.getLogger(__name__)


# ---------------------------------------------------------------------------
# Data structures
# ---------------------------------------------------------------------------


@dataclass
class DialectRegion:
    """A contiguous region that shares a distinct communication style."""

    region_id: str
    agent_ids: list[str] = field(default_factory=list)
    # Dominant vocabulary (top-10 words by frequency, excluding stop words).
    signature_terms: list[str] = field(default_factory=list)
    # How internally coherent this region is (0.0–1.0).
    coherence: float = 0.0
    # Mean pairwise distance to agents in *other* regions.
    isolation: float = 0.0
    # Aggregate message pattern for the whole region.
    pattern_summary: Optional[MessagePattern] = None


@dataclass
class DialectDistanceMatrix:
    """Pairwise dialect distances between groups.

    ``distances`` is a nested dict: distances[group_a][group_b] -> float.
    """

    group_ids: list[str] = field(default_factory=list)
    distances: dict[str, dict[str, float]] = field(default_factory=lambda: defaultdict(dict))
    # Method used to compute the distance.
    method: str = "cosine"

    def get(self, group_a: str, group_b: str) -> float:
        """Return distance between two groups (symmetric)."""
        if group_a == group_b:
            return 0.0
        d = self.distances.get(group_a, {}).get(group_b)
        if d is not None:
            return d
        d = self.distances.get(group_b, {}).get(group_a)
        return d if d is not None else 0.0

    def to_flat_list(self) -> list[dict[str, Any]]:
        """Export as list of {source, target, distance} dicts."""
        rows: list[dict[str, Any]] = []
        seen: set[tuple[str, str]] = set()
        for a, targets in self.distances.items():
            for b, d in targets.items():
                key = (min(a, b), max(a, b))
                if key not in seen:
                    seen.add(key)
                    rows.append({"source": a, "target": b, "distance": d})
        return rows


@dataclass
class DivergenceReport:
    """Full report from a dialect divergence analysis run."""

    tick: int = 0
    # The distance matrix.
    matrix: DialectDistanceMatrix = field(default_factory=DialectDistanceMatrix)
    # Detected dialect regions.
    regions: list[DialectRegion] = field(default_factory=list)
    # Average inter-group distance.
    avg_inter_group_distance: float = 0.0
    # Average intra-group distance (should be low if groups are coherent).
    avg_intra_group_distance: float = 0.0
    # Ratio: inter / intra (higher => more divergent dialects).
    divergence_index: float = 0.0
    # Which grouping was used ("region" or "org").
    grouping_method: str = "region"


# ---------------------------------------------------------------------------
# Analyzer
# ---------------------------------------------------------------------------


class DialectDivergenceAnalyzer:
    """Detect and quantify dialect divergence across agent populations.

    Thread-safety: instances are **not** thread-safe.  Call from async code
    via ``asyncio.to_thread(analyzer.compute_report, ...)``.
    """

    def __init__(
        self,
        distance_method: str = "cosine",
        coherence_threshold: float = 0.3,
        isolation_threshold: float = 0.4,
    ) -> None:
        self._comm = CommunicationAnalyzer()
        self._jargon = JargonDetector()
        self._distance_method = distance_method
        self._coherence_threshold = coherence_threshold
        self._isolation_threshold = isolation_threshold

    # ── Public API ──────────────────────────────────────────

    def compute_distance_matrix(
        self,
        group_messages: dict[str, list[str]],
        method: Optional[str] = None,
    ) -> DialectDistanceMatrix:
        """Compute pairwise dialect distance between all groups.

        Args:
            group_messages: Mapping of group_id -> list of message strings.
            method: Override distance method ("cosine" or "jaccard").
                Defaults to the instance setting.

        Returns:
            DialectDistanceMatrix with pairwise distances.
        """
        m = method or self._distance_method
        group_ids = list(group_messages.keys())
        matrix = DialectDistanceMatrix(group_ids=group_ids, method=m)

        for i, gid_a in enumerate(group_ids):
            for gid_b in group_ids[i + 1 :]:
                if m == "jaccard":
                    dist = self._jargon.compute_linguistic_distance(
                        group_messages[gid_a], group_messages[gid_b]
                    )
                else:
                    dist = self._cosine_group_distance(
                        group_messages[gid_a], group_messages[gid_b]
                    )
                matrix.distances[gid_a][gid_b] = round(dist, 4)
                matrix.distances[gid_b][gid_a] = round(dist, 4)

        return matrix

    def compute_intra_group_distances(
        self,
        agent_messages: dict[str, list[str]],
        group_assignments: dict[str, str],
    ) -> dict[str, float]:
        """Compute average pairwise distance *within* each group.

        Args:
            agent_messages: Mapping of agent_id -> list of message strings.
            group_assignments: Mapping of agent_id -> group_id.

        Returns:
            Mapping of group_id -> average intra-group distance.
        """
        groups: dict[str, list[str]] = defaultdict(list)
        for aid, gid in group_assignments.items():
            if aid in agent_messages:
                groups[gid].append(aid)

        result: dict[str, float] = {}
        for gid, agent_ids in groups.items():
            if len(agent_ids) < 2:
                result[gid] = 0.0
                continue

            distances: list[float] = []
            for i in range(len(agent_ids)):
                for j in range(i + 1, len(agent_ids)):
                    msgs_a = agent_messages.get(agent_ids[i], [])
                    msgs_b = agent_messages.get(agent_ids[j], [])
                    if not msgs_a or not msgs_b:
                        continue
                    # Use single-agent patterns for intra-group measurement.
                    pa = self._comm.analyze_message_patterns(agent_ids[i], msgs_a)
                    pb = self._comm.analyze_message_patterns(agent_ids[j], msgs_b)
                    dist = self._pattern_cosine_distance(pa, pb)
                    distances.append(dist)

            result[gid] = round(sum(distances) / len(distances), 4) if distances else 0.0

        return result

    def detect_dialect_regions(
        self,
        group_messages: dict[str, list[str]],
        agent_locations: dict[str, str],
        distance_threshold: Optional[float] = None,
    ) -> list[DialectRegion]:
        """Identify dialect regions from group messages and agent geography.

        Agents in the same region_id are grouped.  Then the distance matrix
        is used to decide whether two adjacent regions share a dialect or
        form distinct dialect zones.

        Args:
            group_messages: Mapping of group_id -> list of message strings.
            agent_locations: Mapping of agent_id -> region_id (geographic).
            distance_threshold: Minimum inter-region distance to split into
                separate dialect regions.  Defaults to instance setting.

        Returns:
            List of DialectRegion objects.
        """
        threshold = distance_threshold or self._coherence_threshold

        # Compute pairwise distances between geographic regions.
        matrix = self.compute_distance_matrix(group_messages)

        # Build region-level message aggregates.
        region_agents: dict[str, list[str]] = defaultdict(list)
        for aid, rid in agent_locations.items():
            region_agents[rid].append(aid)

        # Merge nearby regions into dialect zones using single-linkage
        # clustering: regions closer than threshold stay together.
        region_ids = sorted(group_messages.keys())
        parent: dict[str, str] = {rid: rid for rid in region_ids}

        def find(x: str) -> str:
            while parent[x] != x:
                parent[x] = parent[parent[x]]
                x = parent[x]
            return x

        def union(a: str, b: str) -> None:
            ra, rb = find(a), find(b)
            if ra != rb:
                parent[ra] = rb

        for i, rid_a in enumerate(region_ids):
            for rid_b in region_ids[i + 1 :]:
                dist = matrix.get(rid_a, rid_b)
                if dist < threshold:
                    union(rid_a, rid_b)

        # Collect merged clusters.
        clusters: dict[str, list[str]] = defaultdict(list)
        for rid in region_ids:
            root = find(rid)
            clusters[root].append(rid)

        # Build DialectRegion objects.
        regions: list[DialectRegion] = []
        for root, member_rids in clusters.items():
            # Merge all agent IDs from member regions.
            all_agent_ids: list[str] = []
            for mrid in member_rids:
                all_agent_ids.extend(region_agents.get(mrid, []))

            # Merge messages.
            all_msgs: list[str] = []
            for mrid in member_rids:
                all_msgs.extend(group_messages.get(mrid, []))

            # Compute region pattern.
            pattern = self._comm.analyze_message_patterns(root, all_msgs)
            sig_terms = list(pattern.top_words.keys())[:10]

            # Coherence: average pairwise distance within this cluster's member regions.
            if len(member_rids) < 2:
                coherence = 1.0
            else:
                internal_dists: list[float] = []
                for i, ra in enumerate(member_rids):
                    for rb in member_rids[i + 1 :]:
                        internal_dists.append(matrix.get(ra, rb))
                avg_internal = sum(internal_dists) / len(internal_dists) if internal_dists else 0.0
                coherence = round(1.0 - avg_internal, 4)

            # Isolation: average distance to all other clusters' member regions.
            other_rids = [rid for rid in region_ids if rid not in member_rids]
            if other_rids:
                external_dists: list[float] = []
                for mrid in member_rids:
                    for orid in other_rids:
                        external_dists.append(matrix.get(mrid, orid))
                isolation = round(sum(external_dists) / len(external_dists), 4)
            else:
                isolation = 1.0

            regions.append(DialectRegion(
                region_id=root,
                agent_ids=all_agent_ids,
                signature_terms=sig_terms,
                coherence=coherence,
                isolation=isolation,
                pattern_summary=pattern,
            ))

        return regions

    def compute_report(
        self,
        tick: int,
        group_messages: dict[str, list[str]],
        agent_locations: dict[str, str],
        grouping_method: str = "region",
    ) -> DivergenceReport:
        """Run the full dialect divergence analysis and return a DivergenceReport.

        This is the main entry point for the analysis pipeline.

        Args:
            tick: Current simulation tick.
            group_messages: Mapping of group_id -> list of message strings.
            agent_locations: Mapping of agent_id -> region_id.
            grouping_method: "region" (geographic) or "org" (organizational).

        Returns:
            DivergenceReport with all analysis results.
        """
        # 1. Compute distance matrix.
        matrix = self.compute_distance_matrix(group_messages)

        # 2. Detect dialect regions.
        regions = self.detect_dialect_regions(group_messages, agent_locations)

        # 3. Compute inter-group stats.
        flat = matrix.to_flat_list()
        if flat:
            avg_inter = round(sum(d["distance"] for d in flat) / len(flat), 4)
        else:
            avg_inter = 0.0

        # 4. Compute intra-group stats.
        group_assignments: dict[str, str] = {}
        for aid, rid in agent_locations.items():
            group_assignments[aid] = rid

        # Build per-agent messages from groups.
        # Since group_messages is group_id -> messages (not agent_id -> messages),
        # use the group-level intra-distance approximation.
        intra_dists: list[float] = []
        for region in regions:
            if region.coherence > 0:
                intra_dists.append(1.0 - region.coherence)

        avg_intra = round(sum(intra_dists) / len(intra_dists), 4) if intra_dists else 0.0

        # Divergence index: inter / (intra + epsilon) — higher = more divergent.
        divergence_index = round(avg_inter / (avg_intra + 1e-6), 4) if avg_inter > 0 else 0.0

        return DivergenceReport(
            tick=tick,
            matrix=matrix,
            regions=regions,
            avg_inter_group_distance=avg_inter,
            avg_intra_group_distance=avg_intra,
            divergence_index=divergence_index,
            grouping_method=grouping_method,
        )

    def compare_organizations(
        self,
        org_messages: dict[str, list[str]],
        org_agents: dict[str, list[str]],
    ) -> dict[str, Any]:
        """Compare communication patterns between organizations.

        Args:
            org_messages: Mapping of org_id -> list of message strings.
            org_agents: Mapping of org_id -> list of agent_ids in that org.

        Returns:
            Dict with distance matrix, divergence metrics, and per-org summaries.
        """
        matrix = self.compute_distance_matrix(org_messages)

        # Per-org pattern summaries.
        org_summaries: dict[str, dict[str, Any]] = {}
        for org_id, msgs in org_messages.items():
            pattern = self._comm.analyze_message_patterns(org_id, msgs)
            org_summaries[org_id] = {
                "agent_count": len(org_agents.get(org_id, [])),
                "total_messages": pattern.total_messages,
                "vocabulary_richness": pattern.vocabulary_richness,
                "top_words": dict(list(pattern.top_words.items())[:5]),
            }

        flat = matrix.to_flat_list()
        avg_dist = round(sum(d["distance"] for d in flat) / len(flat), 4) if flat else 0.0

        return {
            "distance_matrix": matrix,
            "avg_distance": avg_dist,
            "org_summaries": org_summaries,
            "pairwise_distances": flat,
        }

    def compute_convergence_trend(
        self,
        historical_reports: list[DivergenceReport],
    ) -> dict[str, Any]:
        """Analyze whether dialects are converging or diverging over time.

        Args:
            historical_reports: List of DivergenceReport objects from different ticks.

        Returns:
            Dict with trend direction, slope, and per-tick data points.
        """
        if len(historical_reports) < 2:
            return {"trend": "insufficient_data", "data_points": []}

        # Sort by tick.
        sorted_reports = sorted(historical_reports, key=lambda r: r.tick)
        data_points = [
            {"tick": r.tick, "divergence_index": r.divergence_index}
            for r in sorted_reports
        ]

        # Simple linear regression on divergence_index.
        n = len(data_points)
        xs = [d["tick"] for d in data_points]
        ys = [d["divergence_index"] for d in data_points]

        sum_x = sum(xs)
        sum_y = sum(ys)
        sum_xy = sum(x * y for x, y in zip(xs, ys))
        sum_xx = sum(x * x for x in xs)

        denom = n * sum_xx - sum_x * sum_x
        if denom == 0:
            slope = 0.0
        else:
            slope = (n * sum_xy - sum_x * sum_y) / denom

        if slope > 0.01:
            trend = "diverging"
        elif slope < -0.01:
            trend = "converging"
        else:
            trend = "stable"

        return {
            "trend": trend,
            "slope": round(slope, 6),
            "data_points": data_points,
        }

    # ── Internal helpers ────────────────────────────────────

    def _cosine_group_distance(
        self,
        msgs_a: list[str],
        msgs_b: list[str],
    ) -> float:
        """Compute cosine distance between two groups' message patterns."""
        pa = self._comm.analyze_message_patterns("a", msgs_a)
        pb = self._comm.analyze_message_patterns("b", msgs_b)
        return self._pattern_cosine_distance(pa, pb)

    @staticmethod
    def _pattern_cosine_distance(
        pa: MessagePattern,
        pb: MessagePattern,
    ) -> float:
        """Cosine distance between two MessagePattern objects based on top_words."""
        all_terms = sorted(set(pa.top_words.keys()) | set(pb.top_words.keys()))
        if not all_terms:
            return 0.0

        vec_a = [pa.top_words.get(t, 0) for t in all_terms]
        vec_b = [pb.top_words.get(t, 0) for t in all_terms]

        dot = sum(a * b for a, b in zip(vec_a, vec_b))
        mag_a = math.sqrt(sum(a * a for a in vec_a))
        mag_b = math.sqrt(sum(b * b for b in vec_b))

        if mag_a == 0 or mag_b == 0:
            return 1.0

        similarity = dot / (mag_a * mag_b)
        return round(1.0 - similarity, 4)
