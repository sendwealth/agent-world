"""Social network graph exporter for researcher analysis.

Supports:
- GraphML (Gephi/Cytoscape), JSON, adjacency matrix formats
- Community structure detection via connected components
- Centrality metrics (degree, betweenness approximation)
- Relationship matrix with weighted edges by interaction type
- Network density and clustering coefficient computation
"""

from __future__ import annotations

import json
from collections import defaultdict, deque
from typing import TYPE_CHECKING, Any

if TYPE_CHECKING:
    from agent_runtime.tracing.interaction_graph import InteractionGraph


class NetworkExporter:
    """Export agent social relationship graphs.

    Supports GraphML (Gephi/Cytoscape), JSON, and adjacency matrix formats.
    Provides community detection, centrality analysis, and network metrics.
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

    # ── Community Structure Analysis ──────────────────────────

    def _build_weighted_graph(self, interactions: list) -> dict[str, dict[str, float]]:
        """Build a weighted adjacency dict from interactions.

        Returns:
            Dict mapping from_agent -> {to_agent: weight}.
        """
        weighted: dict[str, dict[str, float]] = defaultdict(lambda: defaultdict(float))
        for i in interactions:
            weighted[i.from_agent][i.to_agent] += 1.0
        return dict(weighted)

    def detect_communities(self) -> list[set[str]]:
        """Detect communities using connected components.

        Returns list of sets, each set containing agent IDs in a community.
        """
        interactions = self._get_filtered_interactions()
        nodes: set[str] = set()
        adj: dict[str, set[str]] = defaultdict(set)

        for i in interactions:
            nodes.add(i.from_agent)
            nodes.add(i.to_agent)
            adj[i.from_agent].add(i.to_agent)
            adj[i.to_agent].add(i.from_agent)

        visited: set[str] = set()
        communities: list[set[str]] = []

        for node in nodes:
            if node in visited:
                continue
            community: set[str] = set()
            queue: deque[str] = deque([node])
            while queue:
                current = queue.popleft()
                if current in visited:
                    continue
                visited.add(current)
                community.add(current)
                for neighbor in adj.get(current, set()):
                    if neighbor not in visited:
                        queue.append(neighbor)
            communities.append(community)

        return communities

    def compute_centrality(self) -> dict[str, dict[str, float]]:
        """Compute degree centrality for all nodes.

        Returns:
            Dict mapping agent_id -> {in_degree, out_degree, total_degree, normalized_degree}.
        """
        interactions = self._get_filtered_interactions()
        in_degrees: dict[str, int] = defaultdict(int)
        out_degrees: dict[str, int] = defaultdict(int)
        nodes: set[str] = set()

        for i in interactions:
            nodes.add(i.from_agent)
            nodes.add(i.to_agent)
            out_degrees[i.from_agent] += 1
            in_degrees[i.to_agent] += 1

        n = len(nodes)
        max_possible = max(1, n - 1)

        centrality: dict[str, dict[str, float]] = {}
        for node in nodes:
            in_d = in_degrees.get(node, 0)
            out_d = out_degrees.get(node, 0)
            centrality[node] = {
                "in_degree": float(in_d),
                "out_degree": float(out_d),
                "total_degree": float(in_d + out_d),
                "normalized_degree": (in_d + out_d) / max_possible,
            }

        return centrality

    def compute_network_metrics(self) -> dict[str, Any]:
        """Compute aggregate network metrics.

        Returns density, average degree, reciprocity, and clustering info.
        """
        interactions = self._get_filtered_interactions()

        nodes: set[str] = set()
        for i in interactions:
            nodes.add(i.from_agent)
            nodes.add(i.to_agent)

        n = len(nodes)
        m = len(interactions)

        if n == 0:
            return {
                "node_count": 0,
                "edge_count": 0,
                "density": 0.0,
                "avg_degree": 0.0,
                "reciprocity": 0.0,
                "num_communities": 0,
            }

        # Density: actual edges / possible edges
        max_edges = n * (n - 1)  # directed graph
        density = m / max_edges if max_edges > 0 else 0.0

        # Average degree
        degree_count: dict[str, int] = defaultdict(int)
        for i in interactions:
            degree_count[i.from_agent] += 1
            degree_count[i.to_agent] += 1

        avg_degree = sum(degree_count.values()) / n if n > 0 else 0.0

        # Reciprocity: fraction of edges that are reciprocated
        edge_set: set[tuple[str, str]] = set()
        reciprocal_count = 0
        for i in interactions:
            key = (i.from_agent, i.to_agent)
            edge_set.add(key)

        for (src, tgt) in edge_set:
            if (tgt, src) in edge_set:
                reciprocal_count += 1

        reciprocity = reciprocal_count / len(edge_set) if edge_set else 0.0

        # Communities
        communities = self.detect_communities()

        return {
            "node_count": n,
            "edge_count": m,
            "density": round(density, 6),
            "avg_degree": round(avg_degree, 2),
            "reciprocity": round(reciprocity, 6),
            "num_communities": len(communities),
            "largest_community_size": max(len(c) for c in communities) if communities else 0,
            "smallest_community_size": min(len(c) for c in communities) if communities else 0,
        }

    def export_relationship_matrix(
        self, tick_range: tuple[int, int] | None = None
    ) -> dict[str, Any]:
        """Export weighted relationship matrix grouped by interaction type.

        Returns:
            Dict with node ordering, matrices per type, and combined matrix.
        """
        if tick_range:
            self._tick_range = tick_range
        interactions = self._get_filtered_interactions()

        nodes: set[str] = set()
        for i in interactions:
            nodes.add(i.from_agent)
            nodes.add(i.to_agent)

        sorted_nodes = sorted(nodes)
        node_idx = {n: idx for idx, n in enumerate(sorted_nodes)}
        n = len(sorted_nodes)

        # Group by interaction type
        type_edges: dict[str, list] = defaultdict(list)
        for i in interactions:
            type_edges[i.interaction_type].append(i)

        # Build per-type matrices
        matrices: dict[str, list[list[float]]] = {}
        for itype, edges in type_edges.items():
            matrix = [[0.0] * n for _ in range(n)]
            for i in edges:
                r = node_idx[i.from_agent]
                c = node_idx[i.to_agent]
                matrix[r][c] += 1.0
            matrices[itype] = matrix

        # Combined weighted matrix
        combined = [[0.0] * n for _ in range(n)]
        for matrix in matrices.values():
            for r in range(n):
                for c in range(n):
                    combined[r][c] += matrix[r][c]

        return {
            "nodes": sorted_nodes,
            "matrices_by_type": matrices,
            "combined_matrix": combined,
            "interaction_types": list(matrices.keys()),
        }

    # ── Format Exports ────────────────────────────────────────

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

    def export_full_analysis(self, tick_range: tuple[int, int] | None = None) -> dict[str, Any]:
        """Export complete network analysis with all metrics.

        Combines graph data, community structure, centrality, and network metrics.
        """
        if tick_range:
            self._tick_range = tick_range

        graph_data = self.export_json()
        centrality = self.compute_centrality()
        metrics = self.compute_network_metrics()
        communities = self.detect_communities()

        # Enrich nodes with centrality data
        enriched_nodes = []
        for node_dict in graph_data["nodes"]:
            nid = node_dict["id"]
            node_data = {"id": nid}
            if nid in centrality:
                node_data["centrality"] = centrality[nid]
            enriched_nodes.append(node_data)

        # Community assignments
        community_map: dict[str, int] = {}
        for idx, community in enumerate(communities):
            for agent_id in community:
                community_map[agent_id] = idx

        for node_data in enriched_nodes:
            nid = node_data["id"]
            if nid in community_map:
                node_data["community"] = community_map[nid]

        return {
            "nodes": enriched_nodes,
            "edges": graph_data["edges"],
            "communities": [
                {"id": idx, "members": sorted(c), "size": len(c)}
                for idx, c in enumerate(communities)
            ],
            "centrality": centrality,
            "metrics": metrics,
            "tick_range": self._tick_range,
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
