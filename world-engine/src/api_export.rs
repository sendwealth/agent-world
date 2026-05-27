//! Data export API — `/api/v2/export/*`.
//!
//! Provides world state snapshots (JSON/CSV), agent interaction graphs
//! (GraphML/JSON), and emergence metric time-series (CSV).

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
pub struct ExportFormatQuery {
    /// Output format override. If absent, falls back to Accept header.
    pub format: Option<String>,
}

// ── Router ────────────────────────────────────────────────

/// Build the export routes (without auth middleware).
pub fn export_routes() -> Router<AppState> {
    Router::new()
        .route("/api/v2/export/world", get(export_world))
        .route("/api/v2/export/agents/graph", get(export_agents_graph))
        .route("/api/v2/export/metrics/timeseries", get(export_metrics_timeseries))
}

// ── Helpers ───────────────────────────────────────────────

/// Resolve the desired output format from `?format=` query param or Accept header.
/// Falls back to `default_fmt` if neither is specified.
fn resolve_format(query: &ExportFormatQuery, headers: &HeaderMap, default_fmt: &str) -> String {
    // 1. Explicit query param takes priority.
    if let Some(ref fmt) = query.format {
        return fmt.to_lowercase();
    }
    // 2. Accept header.
    if let Some(accept) = headers.get(header::ACCEPT).and_then(|v| v.to_str().ok()) {
        let accept = accept.to_lowercase();
        if accept.contains("text/csv") {
            return "csv".into();
        }
        if accept.contains("text/xml") || accept.contains("application/xml") {
            return "graphml".into();
        }
        if accept.contains("application/json") {
            return "json".into();
        }
    }
    // 3. Default.
    default_fmt.to_string()
}

/// World state snapshot for JSON export.
#[derive(Debug, Serialize)]
struct WorldSnapshot {
    tick: u64,
    agents: Vec<AgentSummary>,
    total_money: u64,
    total_tokens: u64,
}

/// Agent summary for export.
#[derive(Debug, Serialize)]
struct AgentSummary {
    id: String,
    name: String,
    phase: String,
    tokens: u64,
    money: u64,
    alive: bool,
    ticks_survived: u64,
}

/// Agent graph edge for interaction graph.
#[derive(Debug, Serialize)]
struct AgentGraphEdge {
    source: String,
    target: String,
    weight: u64,
}

/// Time-series row for metrics CSV export.
#[derive(Debug, Serialize)]
struct MetricsTimeSeriesRow {
    tick: u64,
    agent_count: usize,
    alive_count: usize,
    total_money: u64,
    total_tokens: u64,
    org_count: usize,
}

// ── Handlers ──────────────────────────────────────────────

/// `GET /api/v2/export/world?format=json|csv` — world state snapshot export.
async fn export_world(
    State(state): State<AppState>,
    Query(query): Query<ExportFormatQuery>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let fmt = resolve_format(&query, &headers, "json");

    let agents = state.agents.lock().await;
    let tick = *state.tick_rx.borrow();

    let summaries: Vec<AgentSummary> = agents
        .iter()
        .map(|a| AgentSummary {
            id: a.id.clone(),
            name: a.name.clone(),
            phase: a.phase.clone(),
            tokens: a.tokens,
            money: a.money,
            alive: a.alive,
            ticks_survived: a.ticks_survived,
        })
        .collect();

    let total_money: u64 = agents.iter().map(|a| a.money).sum();
    let total_tokens: u64 = agents.iter().map(|a| a.tokens).sum();
    drop(agents);

    match fmt.as_str() {
        "csv" => {
            let mut csv = String::from("id,name,phase,tokens,money,alive,ticks_survived\n");
            for a in &summaries {
                csv.push_str(&format!(
                    "{},{},{},{},{},{},{}\n",
                    csv_escape(&a.id),
                    csv_escape(&a.name),
                    csv_escape(&a.phase),
                    a.tokens,
                    a.money,
                    a.alive,
                    a.ticks_survived
                ));
            }
            (
                StatusCode::OK,
                [(header::CONTENT_TYPE, "text/csv; charset=utf-8")],
                csv,
            )
                .into_response()
        }
        _ => {
            let snapshot = WorldSnapshot {
                tick,
                agents: summaries,
                total_money,
                total_tokens,
            };
            (
                StatusCode::OK,
                [(header::CONTENT_TYPE, "application/json")],
                axum::Json(snapshot),
            )
                .into_response()
        }
    }
}

/// `GET /api/v2/export/agents/graph?format=graphml|json` — agent interaction graph.
///
/// Builds a graph from message history (A2A messages). Each unique (from, to) pair
/// is an edge; weight = number of messages exchanged.
async fn export_agents_graph(
    State(state): State<AppState>,
    Query(query): Query<ExportFormatQuery>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let fmt = resolve_format(&query, &headers, "json");

    let messages = state.messages.lock().await;
    let agents = state.agents.lock().await;

    // Build adjacency from message history.
    let mut edge_map: std::collections::HashMap<(String, String), u64> =
        std::collections::HashMap::new();
    for msg in messages.iter() {
        let key = (msg.from_agent.clone(), msg.to_agent.clone());
        *edge_map.entry(key).or_insert(0) += 1;
    }

    let edges: Vec<AgentGraphEdge> = edge_map
        .into_iter()
        .map(|((source, target), weight)| AgentGraphEdge {
            source,
            target,
            weight,
        })
        .collect();

    let node_ids: Vec<String> = agents.iter().map(|a| a.id.clone()).collect();
    drop(agents);
    drop(messages);

    match fmt.as_str() {
        "graphml" => {
            let graphml = build_graphml(&node_ids, &edges);
            (
                StatusCode::OK,
                [(header::CONTENT_TYPE, "application/xml; charset=utf-8")],
                graphml,
            )
                .into_response()
        }
        _ => {
            // JSON format: { nodes: [...], edges: [...] }
            let graph = serde_json::json!({
                "nodes": node_ids,
                "edges": edges,
            });
            (
                StatusCode::OK,
                [(header::CONTENT_TYPE, "application/json")],
                axum::Json(graph),
            )
                .into_response()
        }
    }
}

/// `GET /api/v2/export/metrics/timeseries?format=csv` — emergence metrics time series.
///
/// Reads snapshot history from the SnapshotStore and returns tick-by-tick metrics.
async fn export_metrics_timeseries(
    State(state): State<AppState>,
    Query(query): Query<ExportFormatQuery>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let fmt = resolve_format(&query, &headers, "csv");

    // Build time-series from snapshot store if available.
    let rows = if let Some(ref store) = state.snapshot_store {
        let store = store.lock().await;
        match store.list(None, None, None) {
            Ok(snapshots) => {
                let org_count = if let Some(ref org_store) = state.org_store {
                    let os = org_store.lock().await;
                    os.list().len()
                } else {
                    0
                };
                snapshots
                    .iter()
                    .map(|s| MetricsTimeSeriesRow {
                        tick: s.tick,
                        agent_count: s.total_population as usize,
                        alive_count: s.active_agents as usize,
                        total_money: s.gdp,
                        total_tokens: s.gdp, // GDP is token sum
                        org_count,
                    })
                    .collect()
            }
            Err(_) => Vec::new(),
        }
    } else {
        // No snapshot store — produce a single row from current state.
        let agents = state.agents.lock().await;
        let tick = *state.tick_rx.borrow();
        let org_count = if let Some(ref org_store) = state.org_store {
            let os = org_store.lock().await;
            os.list().len()
        } else {
            0
        };
        vec![MetricsTimeSeriesRow {
            tick,
            agent_count: agents.len(),
            alive_count: agents.iter().filter(|a| a.alive).count(),
            total_money: agents.iter().map(|a| a.money).sum(),
            total_tokens: agents.iter().map(|a| a.tokens).sum(),
            org_count,
        }]
    };

    match fmt.as_str() {
        "json" => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "application/json")],
            axum::Json(&rows),
        )
            .into_response(),
        _ => {
            let mut csv = String::from("tick,agent_count,alive_count,total_money,total_tokens,org_count\n");
            for r in &rows {
                csv.push_str(&format!(
                    "{},{},{},{},{},{}\n",
                    r.tick, r.agent_count, r.alive_count, r.total_money, r.total_tokens, r.org_count
                ));
            }
            (
                StatusCode::OK,
                [(header::CONTENT_TYPE, "text/csv; charset=utf-8")],
                csv,
            )
                .into_response()
        }
    }
}

// ── GraphML builder ───────────────────────────────────────

/// Build a simple GraphML XML string from nodes and edges.
fn build_graphml(nodes: &[String], edges: &[AgentGraphEdge]) -> String {
    let mut xml = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <graphml xmlns=\"http://graphml.graphstruct.org/graphml\">\n\
         <graph id=\"G\" edgedefault=\"directed\">\n",
    );

    for node_id in nodes {
        xml.push_str(&format!("  <node id=\"{}\"/>\n", xml_escape(node_id)));
    }

    for (i, edge) in edges.iter().enumerate() {
        xml.push_str(&format!(
            "  <edge id=\"e{}\" source=\"{}\" target=\"{}\">\n    <data key=\"weight\">{}</data>\n  </edge>\n",
            i,
            xml_escape(&edge.source),
            xml_escape(&edge.target),
            edge.weight
        ));
    }

    xml.push_str("</graph>\n</graphml>\n");
    xml
}

/// Minimal XML escaping for node/edge IDs.
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// Standard CSV field escaping per RFC 4180.
///
/// If the field contains a comma, double-quote, newline, or starts with a
/// dangerous character (`=`, `+`, `-`, `@`), wrap it in double-quotes and
/// escape any internal double-quotes by doubling them.
fn csv_escape(field: &str) -> String {
    let needs_quoting = field.contains(',')
        || field.contains('"')
        || field.contains('\n')
        || field.contains('\r')
        || field.starts_with('=')
        || field.starts_with('+')
        || field.starts_with('-')
        || field.starts_with('@');

    if needs_quoting {
        let escaped = field.replace('"', "\"\"");
        format!("\"{}\"", escaped)
    } else {
        field.to_string()
    }
}

// ── Tests ──────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_query_param_overrides_accept() {
        let query = ExportFormatQuery {
            format: Some("csv".into()),
        };
        let mut headers = HeaderMap::new();
        headers.insert(header::ACCEPT, "application/json".parse().unwrap());
        assert_eq!(resolve_format(&query, &headers, "json"), "csv");
    }

    #[test]
    fn format_accept_header_csv() {
        let query = ExportFormatQuery { format: None };
        let mut headers = HeaderMap::new();
        headers.insert(header::ACCEPT, "text/csv".parse().unwrap());
        assert_eq!(resolve_format(&query, &headers, "json"), "csv");
    }

    #[test]
    fn format_accept_header_json() {
        let query = ExportFormatQuery { format: None };
        let mut headers = HeaderMap::new();
        headers.insert(header::ACCEPT, "application/json".parse().unwrap());
        assert_eq!(resolve_format(&query, &headers, "csv"), "json");
    }

    #[test]
    fn format_fallback_default() {
        let query = ExportFormatQuery { format: None };
        let headers = HeaderMap::new();
        assert_eq!(resolve_format(&query, &headers, "json"), "json");
        assert_eq!(resolve_format(&query, &headers, "csv"), "csv");
    }

    #[test]
    fn graphml_output_valid_xml() {
        let nodes = vec!["a1".into(), "a2".into()];
        let edges = vec![AgentGraphEdge {
            source: "a1".into(),
            target: "a2".into(),
            weight: 5,
        }];
        let xml = build_graphml(&nodes, &edges);
        assert!(xml.contains("<node id=\"a1\"/>"));
        assert!(xml.contains("<node id=\"a2\"/>"));
        assert!(xml.contains("<edge id=\"e0\" source=\"a1\" target=\"a2\">"));
        assert!(xml.contains("<data key=\"weight\">5</data>"));
        assert!(xml.contains("</graphml>"));
    }

    #[test]
    fn xml_escape_special_chars() {
        assert_eq!(xml_escape("a&b<c>d"), "a&amp;b&lt;c&gt;d");
        assert_eq!(xml_escape("normal"), "normal");
    }

    #[test]
    fn csv_escape_normal_string() {
        assert_eq!(csv_escape("hello"), "hello");
    }

    #[test]
    fn csv_escape_comma() {
        assert_eq!(csv_escape("hello,world"), "\"hello,world\"");
    }

    #[test]
    fn csv_escape_quotes() {
        assert_eq!(csv_escape("say \"hi\""), "\"say \"\"hi\"\"\"");
    }

    #[test]
    fn csv_escape_formula_injection() {
        assert_eq!(csv_escape("=SUM(A1:A10)"), "\"=SUM(A1:A10)\"");
        assert_eq!(csv_escape("+cmd"), "\"+cmd\"");
        assert_eq!(csv_escape("-cmd"), "\"-cmd\"");
        assert_eq!(csv_escape("@cmd"), "\"@cmd\"");
    }

    #[test]
    fn csv_escape_newline() {
        assert_eq!(csv_escape("line1\nline2"), "\"line1\nline2\"");
    }
}
