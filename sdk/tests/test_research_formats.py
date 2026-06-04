"""Tests for research_formats module — GraphML, GEXF, LaTeX exports."""

from __future__ import annotations

import xml.etree.ElementTree as ET

import pytest

from agent_world_sdk.research_formats import (
    to_graphml,
    to_gexf,
    to_latex_table,
    to_latex_summary,
)


# -- Fixtures ---------------------------------------------------------------

@pytest.fixture
def sample_nodes():
    return [
        {"id": "a1", "label": "Alice", "phase": "Adult"},
        {"id": "a2", "label": "Bob", "phase": "Elder"},
        {"id": "a3", "label": "Carol", "phase": "Child"},
    ]


@pytest.fixture
def sample_edges():
    return [
        {"source": "a1", "target": "a2", "weight": "3.0", "type": "trade"},
        {"source": "a2", "target": "a3", "weight": "1.5", "type": "trust"},
        {"source": "a1", "target": "a3", "weight": "2.0", "type": "trade"},
    ]


# =========================================================================
# GraphML
# =========================================================================

class TestGraphML:
    def test_valid_xml(self, sample_nodes, sample_edges):
        result = to_graphml(sample_nodes, sample_edges)
        # Should be valid XML
        root = ET.fromstring(result)
        assert "graphml" in root.tag

    def test_nodes_present(self, sample_nodes, sample_edges):
        result = to_graphml(sample_nodes, sample_edges)
        root = ET.fromstring(result)
        ns = {"g": "http://graphml.graphstruct.org/xmlns"}
        nodes = root.findall(".//g:node", ns)
        assert len(nodes) == 3

    def test_edges_present(self, sample_nodes, sample_edges):
        result = to_graphml(sample_nodes, sample_edges)
        root = ET.fromstring(result)
        ns = {"g": "http://graphml.graphstruct.org/xmlns"}
        edges = root.findall(".//g:edge", ns)
        assert len(edges) == 3

    def test_node_attributes(self, sample_nodes, sample_edges):
        result = to_graphml(sample_nodes, sample_edges, node_attributes=["phase"])
        assert "phase" in result

    def test_edge_weight(self, sample_nodes, sample_edges):
        result = to_graphml(sample_nodes, sample_edges, edge_attributes=["weight"])
        assert "weight" in result

    def test_directed_default(self, sample_nodes, sample_edges):
        result = to_graphml(sample_nodes, sample_edges)
        assert "directed" in result

    def test_undirected(self, sample_nodes, sample_edges):
        result = to_graphml(sample_nodes, sample_edges, directed=False)
        assert "undirected" in result

    def test_empty_graph(self):
        result = to_graphml([], [])
        root = ET.fromstring(result)
        assert "graphml" in root.tag

    def test_xml_declaration(self, sample_nodes, sample_edges):
        result = to_graphml(sample_nodes, sample_edges)
        assert result.startswith("<?xml")


# =========================================================================
# GEXF
# =========================================================================

class TestGEXF:
    def test_valid_xml(self, sample_nodes, sample_edges):
        result = to_gexf(sample_nodes, sample_edges)
        root = ET.fromstring(result)
        assert "gexf" in root.tag

    def test_nodes_present(self, sample_nodes, sample_edges):
        result = to_gexf(sample_nodes, sample_edges)
        root = ET.fromstring(result)
        ns = {"g": "http://www.gexf.net/1.2draft"}
        nodes = root.findall(".//g:node", ns)
        assert len(nodes) == 3

    def test_edges_present(self, sample_nodes, sample_edges):
        result = to_gexf(sample_nodes, sample_edges)
        root = ET.fromstring(result)
        ns = {"g": "http://www.gexf.net/1.2draft"}
        edges = root.findall(".//g:edge", ns)
        assert len(edges) == 3

    def test_creator_metadata(self, sample_nodes, sample_edges):
        result = to_gexf(sample_nodes, sample_edges)
        assert "Agent World SDK" in result

    def test_empty(self):
        result = to_gexf([], [])
        root = ET.fromstring(result)
        assert "gexf" in root.tag

    def test_node_labels(self, sample_nodes, sample_edges):
        result = to_gexf(sample_nodes, sample_edges)
        assert "Alice" in result
        assert "Bob" in result

    def test_edge_weight(self, sample_nodes, sample_edges):
        result = to_gexf(sample_nodes, sample_edges, edge_attributes=["weight"])
        assert "weight" in result


# =========================================================================
# LaTeX Table
# =========================================================================

class TestLatexTable:
    def test_basic_table(self):
        data = [
            {"Name": "Alice", "Score": 95, "Rank": 1},
            {"Name": "Bob", "Score": 87, "Rank": 2},
        ]
        result = to_latex_table(data)
        assert r"\begin{table}" in result
        assert r"\end{table}" in result
        assert r"\begin{tabular}" in result
        assert "Alice" in result
        assert "Bob" in result

    def test_custom_columns(self):
        data = [
            {"a": 1, "b": 2, "c": 3},
            {"a": 4, "b": 5, "c": 6},
        ]
        result = to_latex_table(data, columns=["a", "b"])
        assert "a" in result
        assert "b" in result
        # c should not be in the header
        assert r"a & b \\" in result

    def test_caption_and_label(self):
        data = [{"x": 1}]
        result = to_latex_table(data, caption="Test Table", label="tab:test")
        assert r"\caption{Test Table}" in result
        assert r"\label{tab:test}" in result

    def test_empty_data(self):
        result = to_latex_table([])
        assert result == ""

    def test_special_chars_escaped(self):
        data = [{"text": "a & b", "val": "100%"}]
        result = to_latex_table(data)
        assert r"\&" in result
        assert r"\%" in result

    def test_format_str(self):
        data = [{"val": 3.14159}]
        result = to_latex_table(data, format_str="{:.2f}")
        assert "3.14" in result

    def test_custom_column_format(self):
        data = [{"a": 1, "b": 2}]
        result = to_latex_table(data, column_format="lr")
        assert "{lr}" in result


class TestLatexSummary:
    def test_basic_summary(self):
        stats = {
            "mean": 42.5,
            "std_dev": 12.3,
            "count": 100,
        }
        result = to_latex_summary(stats)
        assert r"\begin{table}" in result
        assert "mean" in result
        assert "42.5" in result

    def test_skips_nested(self):
        stats = {
            "simple": 1,
            "nested": {"a": 1},
            "list_val": [1, 2],
        }
        result = to_latex_summary(stats)
        assert "simple" in result
        assert "nested" not in result
        assert "list_val" not in result
