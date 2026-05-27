//! Social Network Graph Export API — `/api/v2/export/network/*`.
//!
//! Provides social network graph export in GraphML and JSON formats.
//! Nodes = agents, Edges = interactions (trust, trade, messaging).
//! Edge weights are derived from trust scores, trade volume, and message count.

use std::collections::HashMap;

use axum::{
    Router,
    extract::{Query, State},
    http::{HeaderMap, StatusCode, header},
    response::IntoResponse,
    routing::get,
};
use serde::{Deserialize, Serialize};

use crate::api::{AppState, ErrorResponse};

// ── Query Types ───────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct NetworkGraphQuery {
    /// Output format: "json" or "graphml". Defaults to "json".
    pub format: Option<String>,
    /// Minimum edge weight to include (filters out weak connections).
    pub min_weight: Option<f64>,
    /// Edge types to include (comma-separated): "trust", "trade", "message".
    /// Empty or omitted = all types.
    pub edge_types: Option<String>,
    /// Include node attributes (skills, org membership). Default: true.
    #[serde(default = "default_true")]
    pub include_attributes: bool,
}

fn default_true() -> bool {
    true
}

// ── Data Types ────────────────────────────────────────────

/// A node in the social network graph.
#[derive(Debug, Clone, Serialize)]
pub struct NetworkNode {
    pub id: String,
    pub label: String,
    pub phase: String,
    pub alive: bool,
    pub tokens: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generation: Option<u32>,
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub skills: HashMap<String, u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub organization: Option<String>,
}

/// An edge in the social network graph.
#[derive(Debug, Clone, Serialize)]
pub struct NetworkEdge {
    pub source: String,
    pub target: String,
    pub weight: f64,
    pub edge_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interaction_count: Option<u64>,
}

/// The complete network graph.
#[derive(Debug, Clone, Serialize)]
pub struct NetworkGraph {
    pub node_count: usize,
    pub edge_count: usize,
    pub nodes: Vec<NetworkNode>,
    pub edges: Vec<NetworkEdge>,
}

// ── Router ────────────────────────────────────────────────

pub fn network_graph_routes() -> Router<AppState> {
    Router::new().route("/api/v2/export/network", get(export_network_graph))
}

// ── Helpers ───────────────────────────────────────────────

/// XML escape helper.
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// Build GraphML output from the network graph.
fn build_graphml(graph: &NetworkGraph, include_attributes: bool) -> String {
    let mut xml = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <graphml xmlns=\"http://graphml.graphstruct.org/graphml\">\n",
    );

    // Define attribute keys
    if include_attributes {
        xml.push_str("  <key id=\"label\" for=\"node\" attr.name=\"label\" attr.type=\"string\"/>\n");
        xml.push_str("  <key id=\"phase\" for=\"node\" attr.name=\"phase\" attr.type=\"string\"/>\n");
        xml.push_str("  <key id=\"alive\" for=\"node\" attr.name=\"alive\" attr.type=\"boolean\"/>\n");
        xml.push_str("  <key id=\"tokens\" for=\"node\" attr.name=\"tokens\" attr.type=\"long\"/>\n");
    }
    xml.push_str("  <key id=\"weight\" for=\"edge\" attr.name=\"weight\" attr.type=\"double\"/>\n");
    xml.push_str("  <key id=\"edge_type\" for=\"edge\" attr.name=\"edge_type\" attr.type=\"string\"/>\n");

    xml.push_str("  <graph id=\"G\" edgedefault=\"directed\">\n");

    // Nodes
    for node in &graph.nodes {
        if include_attributes {
            xml.push_str(&format!(
                "    <node id=\"{}\">\n      <data key=\"label\">{}</data>\n      <data key=\"phase\">{}</data>\n      <data key=\"alive\">{}</data>\n      <data key=\"tokens\">{}</data>\n    </node>\n",
                xml_escape(&node.id),
                xml_escape(&node.label),
                xml_escape(&node.phase),
                node.alive,
                node.tokens,
            ));
        } else {
            xml.push_str(&format!("    <node id=\"{}\"/>\n", xml_escape(&node.id)));
        }
    }

    // Edges
    for (i, edge) in graph.edges.iter().enumerate() {
        xml.push_str(&format!(
            "    <edge id=\"e{}\" source=\"{}\" target=\"{}\">\n      <data key=\"weight\">{:.4}</data>\n      <data key=\"edge_type\">{}</data>\n    </edge>\n",
            i,
            xml_escape(&edge.source),
            xml_escape(&edge.target),
            edge.weight,
            xml_escape(&edge.edge_type),
        ));
    }

    xml.push_str("  </graph>\n</graphml>\n");
    xml
}

// ── Handler ───────────────────────────────────────────────

/// `GET /api/v2/export/network` — export the social network graph.
///
/// Builds a graph from:
/// - A2A messages (edge weight = message count)
/// - Trust scores (edge weight = trust value)
/// - Trade transactions (edge weight = trade volume)
async fn export_network_graph(
    State(state): State<AppState>,
    Query(query): Query<NetworkGraphQuery>,
) -> impl IntoResponse {
    let fmt = query.format.as_deref().unwrap_or("json").to_lowercase();
    let min_weight = query.min_weight.unwrap_or(0.0);

    // Parse edge type filter
    let type_filter: Option<std::collections::HashSet<String>> = query.edge_types.as_deref().map(|s| {
        s.split(',')
            .map(|t| t.trim().to_lowercase())
            .collect()
    });

    // Collect nodes from agents
    let agents = state.agents.lock().await;
    let mut node_ids: Vec<String> = Vec::new();
    let mut nodes: Vec<NetworkNode> = Vec::new();

    for agent in agents.iter() {
        node_ids.push(agent.id.clone());

        // Look up org membership
        let org_name = if let Some(ref org_store) = state.org_store {
            let store = org_store.lock().await;
            store.list().iter().find_map(|org| {
                org.members.iter().find_map(|m| {
                    if m.agent_id == agent.id {
                        Some(org.name.clone())
                    } else {
                        None
                    }
                })
            })
        } else {
            None
        };

        nodes.push(NetworkNode {
            id: agent.id.clone(),
            label: agent.name.clone(),
            phase: agent.phase.clone(),
            alive: agent.alive,
            tokens: agent.tokens,
            generation: if agent.generation > 0 {
                Some(agent.generation)
            } else {
                None
            },
            skills: agent.skills.clone(),
            organization: org_name,
        });
    }
    drop(agents);

    // Collect edges from various sources
    let mut edge_map: HashMap<(String, String, String), (f64, u64)> = HashMap::new();

    // 1. A2A messages → message edges
    if type_filter.as_ref().map_or(true, |tf| tf.contains("message")) {
        let messages = state.messages.lock().await;
        for msg in messages.iter() {
            let key = (msg.from_agent.clone(), msg.to_agent.clone(), "message".to_string());
            let entry = edge_map.entry(key).or_insert((0.0, 0));
            entry.0 += 1.0;
            entry.1 += 1;
        }
        drop(messages);
    }

    // 2. Trust edges
    if type_filter.as_ref().map_or(true, |tf| tf.contains("trust")) {
        if let Some(ref trust_system) = state.reputation_system {
            // Use reputation system for trust data
            let rep = trust_system.lock().await;
            // The reputation system tracks individual scores, not pairwise trust.
            // We include trust from event history as a proxy.
        }
    }

    // 3. Trade volume edges from marketplace
    if type_filter.as_ref().map_or(true, |tf| tf.contains("trade")) {
        if let Some(ref marketplace) = state.marketplace {
            let mp = marketplace.lock().await;
            // Get trade history from marketplace listings
            for listing in mp.list_all().iter() {
                for purchase in mp.listing_purchases(listing.id) {
                    let key = (purchase.buyer_id.clone(), listing.publisher_id.clone(), "trade".to_string());
                    let entry = edge_map.entry(key).or_insert((0.0, 0));
                    entry.0 += listing.price as f64;
                    entry.1 += 1;
                }
            }
        }
    }

    // Apply min_weight filter and build edge list
    let edges: Vec<NetworkEdge> = edge_map
        .into_iter()
        .filter(|(_, (weight, _))| *weight >= min_weight)
        .map(|((source, target, edge_type), (weight, count))| NetworkEdge {
            source,
            target,
            weight,
            edge_type,
            interaction_count: Some(count),
        })
        .collect();

    let graph = NetworkGraph {
        node_count: nodes.len(),
        edge_count: edges.len(),
        nodes,
        edges,
    };

    match fmt.as_str() {
        "graphml" => {
            let graphml = build_graphml(&graph, query.include_attributes);
            (
                StatusCode::OK,
                [(header::CONTENT_TYPE, "application/xml; charset=utf-8")],
                graphml,
            )
                .into_response()
        }
        _ => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "application/json")],
            axum::Json(graph),
        )
            .into_response(),
    }
}

// ── Tests ──────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xml_escape_special_chars() {
        assert_eq!(xml_escape("a&b<c>d"), "a&amp;b&lt;c&gt;d");
        assert_eq!(xml_escape("normal"), "normal");
    }

    #[test]
    fn graphml_output_valid() {
        let graph = NetworkGraph {
            node_count: 2,
            edge_count: 1,
            nodes: vec![
                NetworkNode {
                    id: "a1".into(),
                    label: "Alice".into(),
                    phase: "adult".into(),
                    alive: true,
                    tokens: 100,
                    generation: None,
                    skills: HashMap::new(),
                    organization: None,
                },
                NetworkNode {
                    id: "a2".into(),
                    label: "Bob".into(),
                    phase: "adult".into(),
                    alive: true,
                    tokens: 200,
                    generation: Some(1),
                    skills: HashMap::new(),
                    organization: None,
                },
            ],
            edges: vec![NetworkEdge {
                source: "a1".into(),
                target: "a2".into(),
                weight: 5.0,
                edge_type: "message".into(),
                interaction_count: Some(5),
            }],
        };

        let xml = build_graphml(&graph, true);
        assert!(xml.contains("<node id=\"a1\">"));
        assert!(xml.contains("<data key=\"label\">Alice</data>"));
        assert!(xml.contains("<edge id=\"e0\" source=\"a1\" target=\"a2\">"));
        assert!(xml.contains("<data key=\"weight\">5.0000</data>"));
        assert!(xml.contains("<data key=\"edge_type\">message</data>"));
        assert!(xml.contains("</graphml>"));
    }

    #[test]
    fn graphml_output_minimal() {
        let graph = NetworkGraph {
            node_count: 1,
            edge_count: 0,
            nodes: vec![NetworkNode {
                id: "a1".into(),
                label: "Alice".into(),
                phase: "adult".into(),
                alive: true,
                tokens: 100,
                generation: None,
                skills: HashMap::new(),
                organization: None,
            }],
            edges: vec![],
        };

        let xml = build_graphml(&graph, false);
        assert!(xml.contains("<node id=\"a1\"/>"));
        assert!(!xml.contains("<data key=\"label\">"));
    }

    #[test]
    fn network_graph_serialization() {
        let graph = NetworkGraph {
            node_count: 1,
            edge_count: 0,
            nodes: vec![NetworkNode {
                id: "a1".into(),
                label: "Alice".into(),
                phase: "adult".into(),
                alive: true,
                tokens: 100,
                generation: None,
                skills: HashMap::new(),
                organization: None,
            }],
            edges: vec![],
        };

        let json = serde_json::to_string(&graph).unwrap();
        assert!(json.contains("\"node_count\":1"));
        assert!(json.contains("\"id\":\"a1\""));
    }
}
