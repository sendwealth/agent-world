"""Social network graph exporter for researcher analysis."""

from __future__ import annotations

import json
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from agent_runtime.tracing.interaction_graph import InteractionGraph


class NetworkExporter:
    """Export agent social relationship graphs.

    Supports GraphML (Gephi/Cytoscape), JSON, and adjacency matrix formats.
    """

    def __init__(self, graph: InteractionGraph) -> None:
        self._graph = graph
        self._tick_range: tuple[int, int] | None = None

    def filter_tick_range(self, start: int, end: int) -> NetworkExporter:
        """Filter interactions to a tick range (inclusive)."""
        self._tick_range = (start, end)
        return self

    def _get_filtered_interactions(self) -> list:
        """Get interactions filtered by tick range."""
        all_interactions = self._graph.get_all_interactions()
        if not self._tick_range:
            return all_interactions
        start, end = self._tick_range
        return [i for i in all_interactions if start <= i.tick <= end]

    def export_graphml(self, tick_range: tuple[int, int] | None = None) -> str:
        """Export as GraphML format (readable by Gephi/Cytoscape).

        GraphML is the standard academic format for graph data.
        Uses only stdlib — no xml.etree to keep dependencies minimal.
        """
        if tick_range:
            self._tick_range = tick_range
        interactions = self._get_filtered_interactions()

        # Collect nodes
        nodes: set[str] = set()
        for i in interactions:
            nodes.add(i.from_agent)
            nodes.add(i.to_agent)

        # Build GraphML XML manually (no new deps)
        lines = [
            '<?xml version="1.0" encoding="UTF-8"?>',
            '<graphml xmlns="http://graphml.graphstruct.org/xmlns">',
            '  <key id="label" for="node" attr.name="label" attr.type="string"/>',
            '  <key id="weight" for="edge" attr.name="weight" attr.type="double"/>',
            '  <key id="interaction_type" for="edge"'
            ' attr.name="interaction_type" attr.type="string"/>',
            '  <key id="tick" for="edge" attr.name="tick" attr.type="int"/>',
            '  <graph id="G" edgedefault="directed">',
        ]

        for node in sorted(nodes):
            escaped = node.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")
            lines.append(f'    <node id="{escaped}">')
            lines.append(f'      <data key="label">{escaped}</data>')
            lines.append('    </node>')

        for i in interactions:
            f = i.from_agent.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")
            t = i.to_agent.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")
            itype = (
                i.interaction_type.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")
            )
            lines.append(f'    <edge source="{f}" target="{t}">')
            lines.append(f'      <data key="interaction_type">{itype}</data>')
            lines.append(f'      <data key="tick">{i.tick}</data>')
            lines.append('    </edge>')

        lines.append('  </graph>')
        lines.append('</graphml>')
        return '\n'.join(lines)

    def export_json(self, tick_range: tuple[int, int] | None = None) -> dict:
        """Export as JSON with nodes, edges, and attributes.

        Returns dict with 'nodes', 'edges', 'summary' keys.
        """
        if tick_range:
            self._tick_range = tick_range
        interactions = self._get_filtered_interactions()

        nodes: set[str] = set()
        edges: list[dict] = []

        for i in interactions:
            nodes.add(i.from_agent)
            nodes.add(i.to_agent)
            edges.append({
                "source": i.from_agent,
                "target": i.to_agent,
                "type": i.interaction_type,
                "tick": i.tick,
            })

        return {
            "nodes": [{"id": n} for n in sorted(nodes)],
            "edges": edges,
            "summary": {
                "node_count": len(nodes),
                "edge_count": len(edges),
                "tick_range": self._tick_range,
            },
        }

    def export_adjacency_matrix(self, tick: int) -> list[list[float]]:
        """Export adjacency matrix for a single tick.

        Returns a 2D list where matrix[i][j] = weight of edge from node_i to node_j.
        """
        self._tick_range = (tick, tick)
        interactions = self._get_filtered_interactions()

        # Collect all nodes
        nodes: set[str] = set()
        for i in interactions:
            nodes.add(i.from_agent)
            nodes.add(i.to_agent)

        sorted_nodes = sorted(nodes)
        node_idx = {n: idx for idx, n in enumerate(sorted_nodes)}
        n = len(sorted_nodes)

        # Initialize zero matrix
        matrix = [[0.0] * n for _ in range(n)]

        # Fill with edge weights (count interactions)
        for i in interactions:
            r = node_idx[i.from_agent]
            c = node_idx[i.to_agent]
            matrix[r][c] += 1.0

        return matrix

    def export_json_string(self, tick_range: tuple[int, int] | None = None) -> str:
        """Export as JSON string."""
        return json.dumps(self.export_json(tick_range), indent=2, ensure_ascii=False)
