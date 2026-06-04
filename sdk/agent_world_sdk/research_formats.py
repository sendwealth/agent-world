"""Research-friendly data format exports — LaTeX tables, GraphML, GEXF.

These helpers convert SDK data into formats commonly used in academic research.
All functions are pure computation and do not make HTTP requests.
"""

from __future__ import annotations

import xml.etree.ElementTree as ET
from typing import Any


# =========================================================================
# GraphML Export
# =========================================================================

def to_graphml(
    nodes: list[dict],
    edges: list[dict],
    *,
    node_attributes: list[str] | None = None,
    edge_attributes: list[str] | None = None,
    directed: bool = True,
) -> str:
    """Convert graph data to GraphML XML string.

    Args:
        nodes: List of node dicts, each with ``id`` and optional attributes.
        edges: List of edge dicts, each with ``source``, ``target``, and
               optional attributes like ``weight``.
        node_attributes: Explicit list of node attribute names to include.
                        If None, auto-detected from node data.
        edge_attributes: Explicit list of edge attribute names to include.
                        If None, auto-detected from edge data.
        directed: Whether the graph is directed.

    Returns:
        GraphML XML string.
    """
    if node_attributes is None:
        node_attributes = _detect_attributes(nodes, skip={"id"})
    if edge_attributes is None:
        edge_attributes = _detect_attributes(edges, skip={"source", "target"})

    root = ET.Element("graphml")
    root.set("xmlns", "http://graphml.graphstruct.org/xmlns")

    # Define keys for node attributes
    attr_keys: dict[str, str] = {}
    for i, attr in enumerate(node_attributes):
        key_id = f"n{i}"
        key_el = ET.SubElement(root, "key")
        key_el.set("id", key_id)
        key_el.set("for", "node")
        key_el.set("attr.name", attr)
        key_el.set("attr.type", "string")
        attr_keys[f"node.{attr}"] = key_id

    # Define keys for edge attributes
    for i, attr in enumerate(edge_attributes):
        key_id = f"e{i}"
        key_el = ET.SubElement(root, "key")
        key_el.set("id", key_id)
        key_el.set("for", "edge")
        key_el.set("attr.name", attr)
        key_el.set("attr.type", "string")
        attr_keys[f"edge.{attr}"] = key_id

    graph = ET.SubElement(root, "graph")
    graph.set("id", "G")
    graph.set("edgedefault", "directed" if directed else "undirected")

    # Add nodes
    for node in nodes:
        node_el = ET.SubElement(graph, "node")
        node_el.set("id", str(node.get("id", "")))
        for attr in node_attributes:
            val = node.get(attr)
            if val is not None:
                data_el = ET.SubElement(node_el, "data")
                data_el.set("key", attr_keys[f"node.{attr}"])
                data_el.text = str(val)

    # Add edges
    for edge in edges:
        edge_el = ET.SubElement(graph, "edge")
        edge_el.set("source", str(edge.get("source", "")))
        edge_el.set("target", str(edge.get("target", "")))
        for attr in edge_attributes:
            val = edge.get(attr)
            if val is not None:
                data_el = ET.SubElement(edge_el, "data")
                data_el.set("key", attr_keys[f"edge.{attr}"])
                data_el.text = str(val)

    ET.indent(root, space="  ")
    return ET.tostring(root, encoding="unicode", xml_declaration=True)


# =========================================================================
# GEXF Export
# =========================================================================

def to_gexf(
    nodes: list[dict],
    edges: list[dict],
    *,
    node_attributes: list[str] | None = None,
    edge_attributes: list[str] | None = None,
    directed: bool = True,
) -> str:
    """Convert graph data to GEXF XML string.

    GEXF is used by Gephi and other network analysis tools.

    Args:
        nodes: List of node dicts, each with ``id`` and optional ``label``.
        edges: List of edge dicts, each with ``source``, ``target``, and
               optional ``weight``.
        node_attributes: Explicit list of node attribute names to include.
        edge_attributes: Explicit list of edge attribute names to include.
        directed: Whether the graph is directed.

    Returns:
        GEXF XML string.
    """
    if node_attributes is None:
        node_attributes = _detect_attributes(nodes, skip={"id", "label"})
    if edge_attributes is None:
        edge_attributes = _detect_attributes(edges, skip={"source", "target"})

    nsmap = "http://www.gexf.net/1.2draft"
    root = ET.Element("gexf")
    root.set("xmlns", nsmap)
    root.set("version", "1.2")

    meta = ET.SubElement(root, "meta")
    meta.set("lastmodifieddate", "2026-06-04")
    ET.SubElement(meta, "creator").text = "Agent World SDK"

    graph_el = ET.SubElement(root, "graph")
    graph_el.set("defaultedgetype", "directed" if directed else "undirected")

    # Attributes definitions
    if node_attributes:
        attrs_el = ET.SubElement(graph_el, "attributes")
        attrs_el.set("class", "node")
        for i, attr in enumerate(node_attributes):
            attr_el = ET.SubElement(attrs_el, "attribute")
            attr_el.set("id", str(i))
            attr_el.set("title", attr)
            attr_el.set("type", "string")

    if edge_attributes:
        attrs_el = ET.SubElement(graph_el, "attributes")
        attrs_el.set("class", "edge")
        for i, attr in enumerate(edge_attributes):
            attr_el = ET.SubElement(attrs_el, "attribute")
            attr_el.set("id", str(i))
            attr_el.set("title", attr)
            attr_el.set("type", "string")

    # Nodes
    nodes_el = ET.SubElement(graph_el, "nodes")
    node_attr_map = {attr: str(i) for i, attr in enumerate(node_attributes)}
    for idx, node in enumerate(nodes):
        node_el = ET.SubElement(nodes_el, "node")
        node_el.set("id", str(node.get("id", idx)))
        label = node.get("label", str(node.get("id", idx)))
        node_el.set("label", str(label))

        if node_attributes:
            attvalues = ET.SubElement(node_el, "attvalues")
            for attr in node_attributes:
                val = node.get(attr)
                if val is not None:
                    av = ET.SubElement(attvalues, "attvalue")
                    av.set("for", node_attr_map[attr])
                    av.set("value", str(val))

    # Edges
    edges_el = ET.SubElement(graph_el, "edges")
    edge_attr_map = {attr: str(i) for i, attr in enumerate(edge_attributes)}
    for idx, edge in enumerate(edges):
        edge_el = ET.SubElement(edges_el, "edge")
        edge_el.set("id", str(idx))
        edge_el.set("source", str(edge.get("source", "")))
        edge_el.set("target", str(edge.get("target", "")))
        if "weight" in edge:
            edge_el.set("weight", str(edge["weight"]))
        if "label" in edge:
            edge_el.set("label", str(edge["label"]))

        if edge_attributes:
            attvalues = ET.SubElement(edge_el, "attvalues")
            for attr in edge_attributes:
                val = edge.get(attr)
                if val is not None:
                    av = ET.SubElement(attvalues, "attvalue")
                    av.set("for", edge_attr_map[attr])
                    av.set("value", str(val))

    ET.indent(root, space="  ")
    return ET.tostring(root, encoding="unicode", xml_declaration=True)


# =========================================================================
# LaTeX Table Export
# =========================================================================

def to_latex_table(
    data: list[dict],
    *,
    columns: list[str] | None = None,
    caption: str = "",
    label: str = "",
    position: str = "htbp",
    format_str: str | None = None,
    column_format: str | None = None,
) -> str:
    """Convert a list of dicts to a LaTeX table.

    Args:
        data: List of row dicts.
        columns: Column names to include.  If None, uses keys from first row.
        caption: LaTeX table caption.
        label: LaTeX label for cross-referencing.
        position: Float position specifier.
        format_str: Python format string for numeric values (e.g. ``{:.2f}``).
        column_format: LaTeX column alignment (e.g. ``lrrr``).  Auto-generated
                      if None.

    Returns:
        LaTeX tabular environment string.
    """
    if not data:
        return ""

    if columns is None:
        columns = list(data[0].keys())

    if column_format is None:
        column_format = "l" + "r" * (len(columns) - 1)

    lines: list[str] = []
    lines.append(r"\begin{table}[" + position + "]")
    lines.append(r"  \centering")

    header = " & ".join(_latex_escape(str(c)) for c in columns)
    lines.append(r"  \begin{tabular}{" + column_format + "}")
    lines.append(r"    \toprule")
    lines.append(f"    {header} \\\\")
    lines.append(r"    \midrule")

    for row in data:
        cells = []
        for col in columns:
            val = row.get(col, "")
            if format_str and isinstance(val, (int, float)):
                cells.append(format_str.format(val))
            else:
                cells.append(_latex_escape(str(val)))
        lines.append("    " + " & ".join(cells) + r" \\")

    lines.append(r"    \bottomrule")
    lines.append(r"  \end{tabular}")

    if caption:
        lines.append(f"  \\caption{{{caption}}}")
    if label:
        lines.append(f"  \\label{{{label}}}")

    lines.append(r"\end{table}")
    return "\n".join(lines)


def to_latex_summary(
    stats: dict[str, Any],
    *,
    caption: str = "Summary Statistics",
    label: str = "tab:summary",
) -> str:
    """Convert a statistics dict to a LaTeX summary table.

    Produces a two-column (Metric / Value) table.
    """
    rows = []
    for key, val in stats.items():
        if isinstance(val, dict):
            continue  # Skip nested dicts
        if isinstance(val, list):
            continue  # Skip lists
        rows.append({"Metric": _latex_escape(str(key)), "Value": _latex_escape(str(val))})

    return to_latex_table(
        rows,
        columns=["Metric", "Value"],
        caption=caption,
        label=label,
        column_format="lr",
    )


# =========================================================================
# Helpers
# =========================================================================

def _detect_attributes(
    items: list[dict], *, skip: set[str] | None = None
) -> list[str]:
    """Auto-detect attribute names from a list of dicts."""
    skip = skip or set()
    attrs: list[str] = []
    seen: set[str] = set()
    for item in items:
        for key in item:
            if key not in skip and key not in seen:
                attrs.append(key)
                seen.add(key)
    return attrs


def _latex_escape(text: str) -> str:
    """Escape special LaTeX characters."""
    replacements = {
        "&": r"\&",
        "%": r"\%",
        "$": r"\$",
        "#": r"\#",
        "_": r"\_",
        "{": r"\{",
        "}": r"\}",
        "~": r"\textasciitilde{}",
        "^": r"\textasciicircum{}",
    }
    for char, replacement in replacements.items():
        text = text.replace(char, replacement)
    return text
