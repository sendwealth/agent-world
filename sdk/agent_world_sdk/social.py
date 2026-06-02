"""Social network analysis sub-module — centrality, communities, connectivity.

These helpers operate on data dicts returned by the SDK or loaded from
exported files (e.g. the network graph from ``/api/v2/export/network``).
They do not make HTTP requests.
"""

from __future__ import annotations

from collections import Counter, defaultdict
from typing import Any


class SocialModule:
    """Social network analysis functions for Agent World data."""

    # -- Centrality ----------------------------------------------------------

    @staticmethod
    def degree_centrality(
        edges: list[dict],
        *,
        directed: bool = False,
    ) -> dict[str, dict[str, float]]:
        """Compute degree centrality for every node in the edge list.

        Each edge dict should have ``source`` and ``target`` keys.

        Returns a dict mapping ``node_id -> {"in_degree", "out_degree",
        "degree", "centrality"}`` where centrality is normalised by
        ``max_possible_degree = n - 1``.
        """
        nodes: set[str] = set()
        in_deg: Counter[str] = Counter()
        out_deg: Counter[str] = Counter()

        for e in edges:
            src = str(e.get("source", ""))
            tgt = str(e.get("target", ""))
            if not src or not tgt:
                continue
            nodes.add(src)
            nodes.add(tgt)
            out_deg[src] += 1
            in_deg[tgt] += 1
            if not directed:
                out_deg[tgt] += 1
                in_deg[src] += 1

        n = len(nodes)
        max_deg = max(n - 1, 1)

        result: dict[str, dict[str, float]] = {}
        for node in nodes:
            deg = out_deg[node] if directed else (in_deg[node] + out_deg[node]) // (2 if not directed else 1)
            if not directed:
                deg = in_deg[node]  # undirected: in == out == degree
            result[node] = {
                "in_degree": in_deg[node],
                "out_degree": out_deg[node],
                "degree": in_deg[node] if not directed else in_deg[node] + out_deg[node],
                "centrality": round(deg / max_deg, 6),
            }
        return result

    @staticmethod
    def betweenness_centrality(
        edges: list[dict],
        *,
        directed: bool = False,
    ) -> dict[str, float]:
        """Compute approximate betweenness centrality using BFS.

        For large graphs this uses sampling (up to 200 source nodes) to keep
        runtime reasonable.  Returns ``node_id -> score`` normalised to [0, 1].
        """
        adj: dict[str, list[str]] = defaultdict(list)
        nodes: set[str] = set()
        for e in edges:
            src = str(e.get("source", ""))
            tgt = str(e.get("target", ""))
            if not src or not tgt:
                continue
            nodes.add(src)
            nodes.add(tgt)
            adj[src].append(tgt)
            if not directed:
                adj[tgt].append(src)

        if not nodes:
            return {}

        node_list = sorted(nodes)
        n = len(node_list)
        # Sample sources for scalability
        max_sources = min(n, 200)
        sources = node_list[:max_sources]

        betweenness: Counter[str] = Counter()

        for s in sources:
            # BFS from s
            queue = [s]
            visited = {s}
            predecessors: dict[str, list[str]] = defaultdict(list)
            sigma: dict[str, int] = defaultdict(int)
            sigma[s] = 1
            dist: dict[str, int] = {s: 0}

            head = 0
            while head < len(queue):
                v = queue[head]
                head += 1
                for w in adj[v]:
                    if w not in visited:
                        visited.add(w)
                        dist[w] = dist[v] + 1
                        queue.append(w)
                    if dist.get(w) == dist.get(v, -1) + 1:
                        sigma[w] += sigma[v]
                        predecessors[w].append(v)

            # Back-propagation
            delta: dict[str, float] = defaultdict(float)
            while queue:
                w = queue.pop()
                for v in predecessors[w]:
                    delta[v] += (sigma[v] / max(sigma[w], 1)) * (1 + delta[w])
                if w != s:
                    betweenness[w] += delta[w]

        # Normalise
        scale = (n - 1) * (n - 2) if not directed and n > 2 else max(n - 1, 1)
        norm_factor = max_sources / max(n, 1)  # adjust for sampling
        return {
            node: round(betweenness[node] / scale * norm_factor, 6)
            for node in node_list
        }

    # -- Community detection -------------------------------------------------

    @staticmethod
    def connected_components(
        edges: list[dict],
        nodes: list[dict] | None = None,
    ) -> list[list[str]]:
        """Find connected components using union-find.

        *edges* should have ``source``/``target``.  *nodes* is an optional
        list of node dicts (each with an ``id`` key) to ensure isolated nodes
        appear as singletons.

        Returns a list of lists, each containing node IDs in one component,
        sorted largest-first.
        """
        parent: dict[str, str] = {}

        def find(x: str) -> str:
            while parent.get(x, x) != x:
                parent[x] = parent.get(parent[x], x)
                x = parent[x]
            return x

        def union(a: str, b: str) -> None:
            ra, rb = find(a), find(b)
            if ra != rb:
                parent[ra] = rb

        # Ensure all nodes exist (including isolated ones)
        all_nodes: set[str] = set()
        if nodes:
            for nd in nodes:
                nid = str(nd.get("id", ""))
                if nid:
                    all_nodes.add(nid)
        for e in edges:
            src = str(e.get("source", ""))
            tgt = str(e.get("target", ""))
            if src:
                all_nodes.add(src)
            if tgt:
                all_nodes.add(tgt)

        for nid in all_nodes:
            parent.setdefault(nid, nid)

        for e in edges:
            src = str(e.get("source", ""))
            tgt = str(e.get("target", ""))
            if src and tgt:
                union(src, tgt)

        components: dict[str, set[str]] = defaultdict(set)
        for nid in all_nodes:
            components[find(nid)].add(nid)

        result = sorted(components.values(), key=len, reverse=True)
        return [sorted(c) for c in result]

    def community_summary(
        self,
        edges: list[dict],
        nodes: list[dict] | None = None,
    ) -> dict[str, Any]:
        """High-level community / connectivity summary.

        Returns:
        - ``component_count`` — number of connected components
        - ``largest_component_size``
        - ``largest_component_ratio`` — fraction of nodes in the largest component
        - ``isolated_nodes`` — count of nodes with degree 0
        - ``components`` — list of component node-ID lists (largest first)
        """
        components = self.connected_components(edges, nodes)
        total_nodes = sum(len(c) for c in components)
        largest = len(components[0]) if components else 0

        # Count isolated nodes
        deg: Counter[str] = Counter()
        for e in edges:
            src = str(e.get("source", ""))
            tgt = str(e.get("target", ""))
            deg[src] += 1
            deg[tgt] += 1
        if nodes:
            isolated = sum(1 for nd in nodes if deg[str(nd.get("id", ""))] == 0)
        else:
            isolated = sum(1 for nid in set(deg.keys()) if deg[nid] == 0)
            # Also count nodes in components that have size 1 but no edges
            isolated += sum(1 for c in components if len(c) == 1)

        return {
            "component_count": len(components),
            "largest_component_size": largest,
            "largest_component_ratio": round(largest / max(total_nodes, 1), 4),
            "isolated_nodes": isolated,
            "components": components,
        }

    # -- Interaction patterns ------------------------------------------------

    @staticmethod
    def interaction_matrix(
        edges: list[dict],
    ) -> dict[str, dict[str, float]]:
        """Build a weighted adjacency matrix from an edge list.

        Returns ``{source: {target: weight, ...}, ...}``.  For edges sharing
        the same (source, target) pair, weights are summed.
        """
        matrix: dict[str, dict[str, float]] = defaultdict(lambda: defaultdict(float))
        for e in edges:
            src = str(e.get("source", ""))
            tgt = str(e.get("target", ""))
            weight = float(e.get("weight", 1.0))
            if src and tgt:
                matrix[src][tgt] += weight
        return dict(matrix)

    @staticmethod
    def top_interactors(
        edges: list[dict],
        *,
        top_n: int = 10,
    ) -> list[dict[str, Any]]:
        """Rank agents by total interaction weight.

        Returns a list of ``{"agent_id", "total_weight", "unique_partners"}``
        sorted by total_weight descending.
        """
        agent_weight: Counter[str] = float_counter()
        agent_partners: dict[str, set[str]] = defaultdict(set)

        for e in edges:
            src = str(e.get("source", ""))
            tgt = str(e.get("target", ""))
            weight = float(e.get("weight", 1.0))
            if src:
                agent_weight[src] += weight
                if tgt:
                    agent_partners[src].add(tgt)
            if tgt:
                agent_weight[tgt] += weight
                if src:
                    agent_partners[tgt].add(src)

        ranked = sorted(agent_weight.items(), key=lambda x: x[1], reverse=True)[:top_n]
        return [
            {
                "agent_id": aid,
                "total_weight": round(w, 4),
                "unique_partners": len(agent_partners[aid]),
            }
            for aid, w in ranked
        ]


def float_counter() -> Counter[str]:
    """Return a Counter that defaults to 0.0 for float accumulation."""
    return Counter()
