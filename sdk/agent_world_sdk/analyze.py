"""Analysis utilities — pure computation, no API calls.

These helpers operate on data dicts returned by the SDK or loaded from
exported files. They do not make HTTP requests.
"""

from __future__ import annotations

import math
from collections import Counter
from typing import Any


class AnalyzeModule:
    """Lightweight analysis functions for Agent World data."""

    def cultural_diversity(self, data: list[dict]) -> dict[str, Any]:
        """Compute cultural diversity metrics from a list of agent profiles.

        Each agent dict should have a ``phase`` key (and optionally others).

        Returns a dict with:
        - ``shannon_entropy`` — entropy of the phase distribution
        - ``unique_phases`` — number of distinct phases
        - ``phase_counts`` — dict of phase -> count
        - ``simpson_index`` — 1 - sum(p_i^2), probability that two randomly
          chosen agents belong to different phases
        """
        if not data:
            return {
                "shannon_entropy": 0.0,
                "unique_phases": 0,
                "phase_counts": {},
                "simpson_index": 0.0,
            }

        phase_counts: Counter[str] = Counter(
            agent.get("phase", "unknown") for agent in data
        )
        total = len(data)
        unique = len(phase_counts)

        # Shannon entropy
        entropy = 0.0
        for count in phase_counts.values():
            p = count / total
            if p > 0:
                entropy -= p * math.log2(p)

        # Simpson diversity index (1 - D)
        d = sum((c / total) ** 2 for c in phase_counts.values())
        simpson = 1.0 - d

        return {
            "shannon_entropy": round(entropy, 4),
            "unique_phases": unique,
            "phase_counts": dict(phase_counts),
            "simpson_index": round(simpson, 4),
        }

    def trust_network(self, edges: list[dict]) -> dict[str, Any]:
        """Analyse a trust / interaction network from an edge list.

        Each edge dict should have ``source``, ``target``, and optionally
        ``weight``.

        Returns:
        - ``node_count`` — number of unique agents
        - ``edge_count`` — number of edges
        - ``avg_degree`` — average connections per node
        - ``density`` — edge_count / (node_count * (node_count - 1)) for directed
        """
        if not edges:
            return {
                "node_count": 0,
                "edge_count": 0,
                "avg_degree": 0.0,
                "density": 0.0,
            }

        nodes: set[str] = set()
        for edge in edges:
            nodes.add(str(edge.get("source", "")))
            nodes.add(str(edge.get("target", "")))
        nodes.discard("")

        node_count = len(nodes)
        edge_count = len(edges)
        avg_degree = (edge_count / node_count) if node_count > 0 else 0.0
        density = (
            edge_count / (node_count * (node_count - 1))
            if node_count > 1
            else 0.0
        )

        return {
            "node_count": node_count,
            "edge_count": edge_count,
            "avg_degree": round(avg_degree, 4),
            "density": round(density, 6),
        }
