"""Agent interaction graph — directed graph of agent-to-agent interactions.

Supports cluster detection (BFS connected components), DOT and JSON export,
and neighbor lookups in O(degree) time via a reverse adjacency map.
"""

from __future__ import annotations

from collections import defaultdict, deque
from dataclasses import dataclass, field
from typing import Any


@dataclass(frozen=True)
class Interaction:
    """A single directed interaction between two agents."""

    from_agent: str
    to_agent: str
    interaction_type: str
    tick: int


@dataclass
class InteractionGraph:
    """Directed graph of agent interactions.

    Maintains both forward and reverse adjacency maps so that
    ``get_neighbors`` runs in O(degree) rather than O(V*E).
    """

    _nodes: set[str] = field(default_factory=set)
    _edges: list[Interaction] = field(default_factory=list)
    _adjacency: dict[str, dict[str, list[Interaction]]] = field(
        default_factory=lambda: defaultdict(lambda: defaultdict(list))
    )
    _reverse_adjacency: dict[str, set[str]] = field(
        default_factory=lambda: defaultdict(set)
    )

    def add_interaction(
        self,
        from_agent: str,
        to_agent: str,
        interaction_type: str,
        tick: int,
    ) -> Interaction:
        """Record a directed interaction and return the ``Interaction`` object."""
        inter = Interaction(
            from_agent=from_agent,
            to_agent=to_agent,
            interaction_type=interaction_type,
            tick=tick,
        )
        self._nodes.add(from_agent)
        self._nodes.add(to_agent)
        self._edges.append(inter)
        self._adjacency[from_agent][to_agent].append(inter)
        self._reverse_adjacency[to_agent].add(from_agent)
        return inter

    def get_all_interactions(self) -> list[Interaction]:
        """Return all recorded interactions."""
        return list(self._edges)

    def get_interactions(
        self, from_agent: str, to_agent: str
    ) -> list[Interaction]:
        """Return all interactions between two agents (directional)."""
        return list(self._adjacency.get(from_agent, {}).get(to_agent, []))

    def get_neighbors(self, agent_id: str) -> set[str]:
        """Return all agents connected to *agent_id* (in + out).

        Uses the reverse adjacency map for O(1) lookup + O(degree) union.
        """
        outgoing = set(self._adjacency.get(agent_id, {}).keys())
        incoming = self._reverse_adjacency.get(agent_id, set())
        return outgoing | incoming

    def get_clusters(self) -> list[set[str]]:
        """Identify connected components via BFS.

        Uses ``collections.deque`` for O(1) popleft.
        """
        visited: set[str] = set()
        clusters: list[set[str]] = []

        for node in self._nodes:
            if node in visited:
                continue
            cluster: set[str] = set()
            queue: deque[str] = deque([node])
            while queue:
                current = queue.popleft()
                if current in visited:
                    continue
                visited.add(current)
                cluster.add(current)
                for neighbor in self.get_neighbors(current):
                    if neighbor not in visited:
                        queue.append(neighbor)
            clusters.append(cluster)
        return clusters

    def export_dot(self) -> str:
        """Export the graph as a Graphviz DOT string."""
        lines = ["digraph InteractionGraph {"]
        seen_edges: set[tuple[str, str]] = set()
        for edge in self._edges:
            key = (edge.from_agent, edge.to_agent)
            if key not in seen_edges:
                seen_edges.add(key)
                # Escape quotes in agent IDs for DOT safety
                src = edge.from_agent.replace('"', '\\"')
                dst = edge.to_agent.replace('"', '\\"')
                lines.append(f'  "{src}" -> "{dst}";')
        lines.append("}")
        return "\n".join(lines)

    def export_json(self) -> dict[str, Any]:
        """Export the graph as a JSON-serializable dict."""
        clusters = self.get_clusters()
        return {
            "nodes": sorted(self._nodes),
            "edges": [
                {
                    "from": e.from_agent,
                    "to": e.to_agent,
                    "type": e.interaction_type,
                    "tick": e.tick,
                }
                for e in self._edges
            ],
            "clusters": [sorted(c) for c in clusters],
            "summary": {
                "total_nodes": len(self._nodes),
                "total_edges": len(self._edges),
                "num_clusters": len(clusters),
            },
        }
