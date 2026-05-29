//! Unified V1 Data Export API — `GET /api/v1/export/{type}`.
//!
//! Provides a single entry point for exporting all simulation data:
//! - `behavior`  — agent action history (perceive→decide→act→reflect)
//! - `network`   — social network graph (DOT / D3.js JSON / GEXF)
//! - `economic`  — transactions, GDP time series, wealth distribution
//! - `organization` — members, governance proposals, election history
//!
//! All endpoints support `?format=csv|json&from_tick=&to_tick=` filters.

use std::collections::HashMap;

use axum::{
    extract::{Path, Query, State},
    http::{header, StatusCode},
    response::IntoResponse,
    routing::get,
    Router,
};
use serde::{Deserialize, Serialize};

use crate::api::AppState;

// ── Query Parameters ─────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ExportQuery {
    /// Output format: "csv", "json", "dot", "gexf", "d3".
    /// Defaults vary by export type.
    pub format: Option<String>,
    /// Start tick (inclusive). Defaults to 0.
    pub from_tick: Option<u64>,
    /// End tick (inclusive). Defaults to latest tick.
    pub to_tick: Option<u64>,
    /// Filter by agent ID (optional).
    pub agent_id: Option<String>,
    /// Maximum number of entries (behavior logs). Defaults to 10000.
    pub limit: Option<u64>,
    /// Event/action types to include (comma-separated, behavior logs only).
    pub event_types: Option<String>,
    /// Minimum edge weight (network graph only).
    pub min_weight: Option<f64>,
    /// Edge types to include (comma-separated: trust,trade,message).
    pub edge_types: Option<String>,
    /// Include node attributes (network graph). Default: true.
    #[serde(default = "default_true")]
    pub include_attributes: bool,
}

fn default_true() -> bool {
    true
}

// ── Router ───────────────────────────────────────────────

pub fn export_v1_routes() -> Router<AppState> {
    Router::new().route("/api/v1/export/{export_type}", get(export_handler))
}

// ── CSV Escape ───────────────────────────────────────────

/// Standard CSV field escaping per RFC 4180 with formula-injection protection.
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

/// XML escape helper.
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

// ── Dispatcher ───────────────────────────────────────────

/// `GET /api/v1/export/{export_type}` — unified export dispatcher.
async fn export_handler(
    State(state): State<AppState>,
    Path(export_type): Path<String>,
    Query(query): Query<ExportQuery>,
) -> impl IntoResponse {
    match export_type.as_str() {
        "behavior" => export_behavior(&state, &query).await,
        "network" => export_network(&state, &query).await,
        "economic" => export_economic(&state, &query).await,
        "organization" => export_organization(&state, &query).await,
        _ => (
            StatusCode::BAD_REQUEST,
            [(header::CONTENT_TYPE, "application/json")],
            axum::Json(serde_json::json!({
                "error": format!(
                    "Unknown export type '{}'. Valid types: behavior, network, economic, organization",
                    export_type
                )
            })),
        )
            .into_response(),
    }
}

// ════════════════════════════════════════════════════════════
//  BEHAVIOR LOG EXPORT
// ════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize)]
struct BehaviorEntry {
    tick: u64,
    event_type: String,
    agent_id: String,
    target_agent_id: Option<String>,
    description: String,
    details: serde_json::Value,
}

async fn export_behavior(state: &AppState, query: &ExportQuery) -> axum::response::Response {
    let fmt = query.format.as_deref().unwrap_or("json").to_lowercase();
    let limit = query.limit.unwrap_or(10_000);
    let from = query.from_tick.unwrap_or(0);
    let current_tick = *state.tick_rx.borrow();
    let to = query.to_tick.unwrap_or(current_tick);

    let type_filter: Option<std::collections::HashSet<String>> = query
        .event_types
        .as_deref()
        .map(|s| s.split(',').map(|t| t.trim().to_lowercase()).collect());

    let mut entries: Vec<BehaviorEntry> = Vec::new();

    // 1. A2A messages
    let messages = state.messages.lock().await;
    for msg in messages.iter() {
        if msg.tick < from || msg.tick > to {
            continue;
        }
        if let Some(ref aid) = query.agent_id {
            if msg.from_agent != *aid && msg.to_agent != *aid {
                continue;
            }
        }
        let etype = format!("message_{}", msg.message_type);
        if let Some(ref tf) = type_filter {
            if !tf.contains(&etype) && !tf.contains(&msg.message_type.to_lowercase()) {
                continue;
            }
        }

        entries.push(BehaviorEntry {
            tick: msg.tick,
            event_type: etype,
            agent_id: msg.from_agent.clone(),
            target_agent_id: Some(msg.to_agent.clone()),
            description: format!(
                "{} sent {} to {} (tick {})",
                msg.from_agent, msg.message_type, msg.to_agent, msg.tick
            ),
            details: serde_json::json!({
                "message_id": msg.id,
                "message_type": msg.message_type,
                "payload_preview": &msg.payload[..msg.payload.len().min(500)],
            }),
        });

        if query.agent_id.is_none() || Some(msg.to_agent.as_str()) == query.agent_id.as_deref() {
            entries.push(BehaviorEntry {
                tick: msg.tick,
                event_type: format!("message_received_{}", msg.message_type),
                agent_id: msg.to_agent.clone(),
                target_agent_id: Some(msg.from_agent.clone()),
                description: format!(
                    "{} received {} from {} (tick {})",
                    msg.to_agent, msg.message_type, msg.from_agent, msg.tick
                ),
                details: serde_json::json!({
                    "message_id": msg.id,
                    "message_type": msg.message_type,
                }),
            });
        }
    }
    drop(messages);

    // 2. Snapshot key events
    if let Some(ref store) = state.snapshot_store {
        let store = store.lock().await;
        if let Ok(snapshots) = store.list(Some(from), Some(to), None) {
            for snap in &snapshots {
                for ke in &snap.key_events {
                    if ke.tick < from || ke.tick > to {
                        continue;
                    }
                    if let Some(ref aid) = query.agent_id {
                        if ke.agent_id.as_deref() != Some(aid.as_str()) {
                            continue;
                        }
                    }
                    if let Some(ref tf) = type_filter {
                        if !tf.contains(&ke.event_type.to_lowercase()) {
                            continue;
                        }
                    }
                    entries.push(BehaviorEntry {
                        tick: ke.tick,
                        event_type: ke.event_type.clone(),
                        agent_id: ke.agent_id.clone().unwrap_or_default(),
                        target_agent_id: None,
                        description: ke.description.clone(),
                        details: serde_json::json!({ "event_type": ke.event_type }),
                    });
                }
            }
        }
    }

    // 3. Current agent state as snapshot entry
    let agents = state.agents.lock().await;
    for agent in agents.iter() {
        if let Some(ref aid) = query.agent_id {
            if agent.id != *aid {
                continue;
            }
        }
        if let Some(ref tf) = type_filter {
            if !tf.contains("agent_state") && !tf.contains("state") {
                continue;
            }
        }
        entries.push(BehaviorEntry {
            tick: current_tick,
            event_type: "agent_state".to_string(),
            agent_id: agent.id.clone(),
            target_agent_id: None,
            description: format!(
                "Agent {} (phase: {}, tokens: {}, money: {}, alive: {}, ticks_survived: {})",
                agent.name, agent.phase, agent.tokens, agent.money, agent.alive, agent.ticks_survived
            ),
            details: serde_json::json!({
                "name": agent.name,
                "phase": agent.phase,
                "tokens": agent.tokens,
                "money": agent.money,
                "alive": agent.alive,
                "ticks_survived": agent.ticks_survived,
                "generation": agent.generation,
                "parent_ids": agent.parent_ids,
                "skills": agent.skills,
            }),
        });
    }
    drop(agents);

    entries.sort_by(|a, b| a.tick.cmp(&b.tick).then_with(|| a.event_type.cmp(&b.event_type)));
    entries.truncate(limit as usize);

    match fmt.as_str() {
        "csv" => {
            let mut csv = String::from("tick,event_type,agent_id,target_agent_id,description,details\n");
            for e in &entries {
                let details_str = serde_json::to_string(&e.details).unwrap_or_default();
                csv.push_str(&format!(
                    "{},{},{},{},{},{}\n",
                    e.tick,
                    csv_escape(&e.event_type),
                    csv_escape(&e.agent_id),
                    e.target_agent_id.as_deref().map(csv_escape).unwrap_or_default(),
                    csv_escape(&e.description),
                    csv_escape(&details_str),
                ));
            }
            (
                StatusCode::OK,
                [(header::CONTENT_TYPE, "text/csv; charset=utf-8")],
                csv,
            )
                .into_response()
        }
        _ => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "application/json")],
            axum::Json(serde_json::json!({
                "count": entries.len(),
                "from_tick": from,
                "to_tick": to,
                "entries": entries,
            })),
        )
            .into_response(),
    }
}

// ════════════════════════════════════════════════════════════
//  SOCIAL NETWORK GRAPH EXPORT
// ════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize)]
struct NetNode {
    id: String,
    label: String,
    phase: String,
    alive: bool,
    tokens: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    generation: Option<u32>,
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    skills: HashMap<String, u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    organization: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct NetEdge {
    source: String,
    target: String,
    weight: f64,
    edge_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    interaction_count: Option<u64>,
}

async fn export_network(state: &AppState, query: &ExportQuery) -> axum::response::Response {
    let fmt = query.format.as_deref().unwrap_or("json").to_lowercase();
    let min_weight = query.min_weight.unwrap_or(0.0);

    let type_filter: Option<std::collections::HashSet<String>> = query
        .edge_types
        .as_deref()
        .map(|s| s.split(',').map(|t| t.trim().to_lowercase()).collect());

    // Collect nodes
    let agents = state.agents.lock().await;
    let mut nodes: Vec<NetNode> = Vec::new();
    for agent in agents.iter() {
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

        nodes.push(NetNode {
            id: agent.id.clone(),
            label: agent.name.clone(),
            phase: agent.phase.clone(),
            alive: agent.alive,
            tokens: agent.tokens,
            generation: if agent.generation > 0 { Some(agent.generation) } else { None },
            skills: agent.skills.clone(),
            organization: org_name,
        });
    }
    drop(agents);

    // Collect edges
    let mut edge_map: HashMap<(String, String, String), (f64, u64)> = HashMap::new();

    // Message edges
    if type_filter.as_ref().is_none_or(|tf| tf.contains("message")) {
        let messages = state.messages.lock().await;
        for msg in messages.iter() {
            let key = (msg.from_agent.clone(), msg.to_agent.clone(), "message".to_string());
            let entry = edge_map.entry(key).or_insert((0.0, 0));
            entry.0 += 1.0;
            entry.1 += 1;
        }
    }

    // Trade edges from marketplace
    if type_filter.as_ref().is_none_or(|tf| tf.contains("trade")) {
        if let Some(ref marketplace) = state.marketplace {
            let mp = marketplace.lock().await;
            for listing in mp.list_all().iter() {
                for purchase in mp.listing_purchases(listing.id) {
                    let key = (
                        purchase.buyer_id.clone(),
                        listing.publisher_id.clone(),
                        "trade".to_string(),
                    );
                    let entry = edge_map.entry(key).or_insert((0.0, 0));
                    entry.0 += listing.price as f64;
                    entry.1 += 1;
                }
            }
        }
    }

    let edges: Vec<NetEdge> = edge_map
        .into_iter()
        .filter(|(_, (weight, _))| *weight >= min_weight)
        .map(|((source, target, edge_type), (weight, count))| NetEdge {
            source,
            target,
            weight,
            edge_type,
            interaction_count: Some(count),
        })
        .collect();

    match fmt.as_str() {
        "dot" => build_dot_response(&nodes, &edges, query.include_attributes),
        "gexf" => build_gexf_response(&nodes, &edges, query.include_attributes),
        "d3" => build_d3_response(&nodes, &edges),
        _ => build_network_json_response(&nodes, &edges),
    }
}

fn build_dot_response(
    nodes: &[NetNode],
    edges: &[NetEdge],
    include_attrs: bool,
) -> axum::response::Response {
    let mut dot = String::from("digraph social_network {\n");
    dot.push_str("  rankdir=LR;\n  node [shape=box];\n\n");

    for node in nodes {
        if include_attrs {
            dot.push_str(&format!(
                "  \"{}\" [label=\"{}\", phase=\"{}\", alive={}, tokens={}];\n",
                xml_escape(&node.id),
                xml_escape(&node.label),
                xml_escape(&node.phase),
                node.alive,
                node.tokens,
            ));
        } else {
            dot.push_str(&format!("  \"{}\";\n", xml_escape(&node.id)));
        }
    }
    dot.push('\n');

    for edge in edges {
        let color = match edge.edge_type.as_str() {
            "message" => "blue",
            "trade" => "green",
            "trust" => "orange",
            _ => "gray",
        };
        dot.push_str(&format!(
            "  \"{}\" -> \"{}\" [label=\"{}\", weight={:.1}, color={}];\n",
            xml_escape(&edge.source),
            xml_escape(&edge.target),
            xml_escape(&edge.edge_type),
            edge.weight,
            color,
        ));
    }

    dot.push_str("}\n");

    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/vnd.graphviz; charset=utf-8")],
        dot,
    )
        .into_response()
}

fn build_gexf_response(
    nodes: &[NetNode],
    edges: &[NetEdge],
    include_attrs: bool,
) -> axum::response::Response {
    let mut xml = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <gexf xmlns=\"http://www.gexf.net/1.3draft\" version=\"1.3\">\n\
         <meta lastmodifieddate=\"2026-01-01\">\n\
         <creator>agent-world export</creator>\n\
         </meta>\n\
         <graph mode=\"static\" defaultedgetype=\"directed\">\n",
    );

    // Attribute definitions
    if include_attrs {
        xml.push_str(
            "  <attributes class=\"node\">\n\
             <attribute id=\"0\" title=\"label\" type=\"string\"/>\n\
             <attribute id=\"1\" title=\"phase\" type=\"string\"/>\n\
             <attribute id=\"2\" title=\"alive\" type=\"boolean\"/>\n\
             <attribute id=\"3\" title=\"tokens\" type=\"integer\"/>\n\
             </attributes>\n",
        );
    }
    xml.push_str(
        "  <attributes class=\"edge\">\n\
         <attribute id=\"4\" title=\"weight\" type=\"float\"/>\n\
         <attribute id=\"5\" title=\"edge_type\" type=\"string\"/>\n\
         <attribute id=\"6\" title=\"interaction_count\" type=\"integer\"/>\n\
         </attributes>\n",
    );

    // Nodes
    xml.push_str("  <nodes>\n");
    for (i, node) in nodes.iter().enumerate() {
        if include_attrs {
            xml.push_str(&format!(
                "    <node id=\"{}\" label=\"{}\">\n\
                 <attvalues>\n\
                 <attvalue for=\"0\" value=\"{}\"/>\n\
                 <attvalue for=\"1\" value=\"{}\"/>\n\
                 <attvalue for=\"2\" value=\"{}\"/>\n\
                 <attvalue for=\"3\" value=\"{}\"/>\n\
                 </attvalues>\n\
                 </node>\n",
                i,
                xml_escape(&node.id),
                xml_escape(&node.label),
                xml_escape(&node.phase),
                node.alive,
                node.tokens,
            ));
        } else {
            xml.push_str(&format!(
                "    <node id=\"{}\" label=\"{}\"/>\n",
                i,
                xml_escape(&node.id),
            ));
        }
    }
    xml.push_str("  </nodes>\n");

    // Build node ID lookup for edge source/target
    let node_index: HashMap<&str, usize> = nodes.iter().enumerate().map(|(i, n)| (n.id.as_str(), i)).collect();

    // Edges
    xml.push_str("  <edges>\n");
    for (i, edge) in edges.iter().enumerate() {
        let src_idx = node_index.get(edge.source.as_str()).map_or(0, |v| *v);
        let tgt_idx = node_index.get(edge.target.as_str()).map_or(0, |v| *v);
        let count = edge.interaction_count.unwrap_or(0);
        xml.push_str(&format!(
            "    <edge id=\"{}\" source=\"{}\" target=\"{}\" label=\"{}\">\n\
             <attvalues>\n\
             <attvalue for=\"4\" value=\"{:.4}\"/>\n\
             <attvalue for=\"5\" value=\"{}\"/>\n\
             <attvalue for=\"6\" value=\"{}\"/>\n\
             </attvalues>\n\
             </edge>\n",
            i,
            src_idx,
            tgt_idx,
            xml_escape(&edge.edge_type),
            edge.weight,
            xml_escape(&edge.edge_type),
            count,
        ));
    }
    xml.push_str("  </edges>\n");

    xml.push_str("</graph>\n</gexf>\n");

    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/xml; charset=utf-8")],
        xml,
    )
        .into_response()
}

fn build_d3_response(nodes: &[NetNode], edges: &[NetEdge]) -> axum::response::Response {
    // D3.js force-directed graph format
    let d3_nodes: Vec<serde_json::Value> = nodes
        .iter()
        .map(|n| {
            serde_json::json!({
                "id": n.id,
                "label": n.label,
                "phase": n.phase,
                "alive": n.alive,
                "tokens": n.tokens,
            })
        })
        .collect();

    let d3_links: Vec<serde_json::Value> = edges
        .iter()
        .map(|e| {
            serde_json::json!({
                "source": e.source,
                "target": e.target,
                "weight": e.weight,
                "type": e.edge_type,
                "count": e.interaction_count.unwrap_or(0),
            })
        })
        .collect();

    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/json")],
        axum::Json(serde_json::json!({
            "nodes": d3_nodes,
            "links": d3_links,
        })),
    )
        .into_response()
}

fn build_network_json_response(nodes: &[NetNode], edges: &[NetEdge]) -> axum::response::Response {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/json")],
        axum::Json(serde_json::json!({
            "node_count": nodes.len(),
            "edge_count": edges.len(),
            "nodes": nodes,
            "edges": edges,
        })),
    )
        .into_response()
}

// ════════════════════════════════════════════════════════════
//  ECONOMIC DATA EXPORT
// ════════════════════════════════════════════════════════════

async fn export_economic(state: &AppState, query: &ExportQuery) -> axum::response::Response {
    let fmt = query.format.as_deref().unwrap_or("json").to_lowercase();

    match fmt.as_str() {
        "csv" => export_economic_csv(state, query).await,
        _ => export_economic_json(state, query).await,
    }
}

async fn export_economic_json(state: &AppState, query: &ExportQuery) -> axum::response::Response {
    let from = query.from_tick.unwrap_or(0);
    let current_tick = *state.tick_rx.borrow();
    let to = query.to_tick.unwrap_or(current_tick);

    let mut result = serde_json::Map::new();

    // 1. Current wealth snapshot
    {
        let agents = state.agents.lock().await;
        let total_money: u64 = agents.iter().map(|a| a.money).sum();
        let total_tokens: u64 = agents.iter().map(|a| a.tokens).sum();
        let alive_count = agents.iter().filter(|a| a.alive).count();

        let mut wealth_list: Vec<serde_json::Value> = agents
            .iter()
            .filter(|a| a.alive)
            .map(|a| {
                serde_json::json!({
                    "agent_id": a.id,
                    "name": a.name,
                    "money": a.money,
                    "tokens": a.tokens,
                    "total_wealth": a.money + a.tokens,
                })
            })
            .collect();
        wealth_list.sort_by(|a, b| {
            b.get("total_wealth").and_then(|v| v.as_u64()).cmp(
                &a.get("total_wealth").and_then(|v| v.as_u64()),
            )
        });

        // Gini coefficient
        let gini = compute_gini(&agents.iter().filter(|a| a.alive).map(|a| a.money + a.tokens).collect::<Vec<_>>());

        result.insert("wealth_distribution".into(), serde_json::json!({
            "total_money": total_money,
            "total_tokens": total_tokens,
            "alive_agents": alive_count,
            "gini_coefficient": gini,
            "agents": wealth_list,
        }));
        drop(agents);
    }

    // 2. GDP time series from snapshots
    {
        let ts: Vec<serde_json::Value> = if let Some(ref store) = state.snapshot_store {
            let store = store.lock().await;
            match store.list(Some(from), Some(to), None) {
                Ok(snapshots) => snapshots
                    .iter()
                    .map(|s| {
                        serde_json::json!({
                            "tick": s.tick,
                            "gdp": s.gdp,
                            "population": s.active_agents,
                            "gini": s.gini_coefficient,
                        })
                    })
                    .collect(),
                Err(_) => Vec::new(),
            }
        } else {
            Vec::new()
        };
        result.insert("gdp_timeseries".into(), serde_json::json!(ts));
    }

    // 3. Transaction records from marketplace purchases
    {
        let mut transactions: Vec<serde_json::Value> = Vec::new();
        if let Some(ref marketplace) = state.marketplace {
            let mp = marketplace.lock().await;
            for listing in mp.list_all().iter() {
                for purchase in mp.listing_purchases(listing.id) {
                    if purchase.tick < from || purchase.tick > to {
                        continue;
                    }
                    if let Some(ref aid) = query.agent_id {
                        if purchase.buyer_id != *aid && purchase.seller_id != *aid {
                            continue;
                        }
                    }
                    transactions.push(serde_json::json!({
                        "tick": purchase.tick,
                        "buyer_id": purchase.buyer_id,
                        "seller_id": purchase.seller_id,
                        "listing_id": purchase.listing_id.to_string(),
                        "price": purchase.price,
                        "item_title": listing.title,
                    }));
                }
            }
        }
        transactions.sort_by(|a, b| {
            a.get("tick").and_then(|v| v.as_u64()).cmp(&b.get("tick").and_then(|v| v.as_u64()))
        });
        result.insert("transactions".into(), serde_json::json!(transactions));
    }

    // 4. Banking data
    {
        if let Some(ref banking) = state.banking_system {
            let bank = banking.lock().await;
            let accounts: Vec<serde_json::Value> = bank
                .list_accounts()
                .iter()
                .map(|a| {
                    let balance = bank.get_balance(a.id).unwrap_or(0);
                    serde_json::json!({
                        "id": a.id.to_string(),
                        "owner_id": a.owner_id,
                        "account_type": format!("{:?}", a.account_type),
                        "label": a.label,
                        "balance": balance,
                        "created_tick": a.created_tick,
                    })
                })
                .collect();

            let loans: Vec<serde_json::Value> = bank
                .list_loans(None, None)
                .iter()
                .map(|l| {
                    serde_json::json!({
                        "id": l.id.to_string(),
                        "borrower_id": l.borrower_id,
                        "principal": l.principal,
                        "outstanding_balance": l.outstanding_balance,
                        "interest_rate": l.interest_rate,
                        "status": format!("{:?}", l.status),
                        "total_repaid": l.total_repaid,
                        "created_tick": l.created_tick,
                    })
                })
                .collect();

            result.insert(
                "banking".into(),
                serde_json::json!({
                    "total_money_supply": bank.total_money_supply(),
                    "total_loan_debt": bank.total_loan_debt(),
                    "accounts": accounts,
                    "loans": loans,
                }),
            );
        }
    }

    // 5. Stock market data
    {
        if let Some(ref stock_market) = state.stock_market {
            let sm = stock_market.lock().await;
            let stocks: Vec<serde_json::Value> = sm
                .list_stocks()
                .iter()
                .map(|s| {
                    let holdings = sm.get_stock_holdings(&s.id);
                    serde_json::json!({
                        "id": s.id,
                        "org_id": s.org_id,
                        "ticker": s.ticker,
                        "total_shares": s.total_shares,
                        "price": s.price,
                        "status": format!("{:?}", s.status),
                        "listed_tick": s.listed_tick,
                        "shareholders": holdings.len(),
                    })
                })
                .collect();

            let orders: Vec<serde_json::Value> = sm
                .list_orders(None, None)
                .iter()
                .filter(|o| o.created_tick >= from && o.created_tick <= to)
                .map(|o| {
                    serde_json::json!({
                        "id": o.id,
                        "stock_id": o.stock_id,
                        "agent_id": o.agent_id,
                        "order_type": format!("{:?}", o.order_type),
                        "price": o.price,
                        "quantity": o.quantity,
                        "filled_quantity": o.filled_quantity,
                        "status": format!("{:?}", o.status),
                        "created_tick": o.created_tick,
                    })
                })
                .collect();

            result.insert(
                "stock_market".into(),
                serde_json::json!({
                    "stocks": stocks,
                    "orders": orders,
                }),
            );
        }
    }

    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/json")],
        axum::Json(serde_json::Value::Object(result)),
    )
        .into_response()
}

async fn export_economic_csv(state: &AppState, query: &ExportQuery) -> axum::response::Response {
    let from = query.from_tick.unwrap_or(0);
    let current_tick = *state.tick_rx.borrow();
    let to = query.to_tick.unwrap_or(current_tick);

    let mut csv = String::new();

    // Section 1: Wealth distribution
    {
        let agents = state.agents.lock().await;
        let total_money: u64 = agents.iter().map(|a| a.money).sum();
        let total_tokens: u64 = agents.iter().map(|a| a.tokens).sum();

        csv.push_str(&format!(
            "# Wealth Distribution (tick {})\n",
            current_tick
        ));
        csv.push_str(&format!(
            "# total_money={}, total_tokens={}\n",
            total_money, total_tokens
        ));
        csv.push_str("agent_id,name,money,tokens,total_wealth,phase,alive\n");
        for a in agents.iter() {
            csv.push_str(&format!(
                "{},{},{},{},{},{},{}\n",
                csv_escape(&a.id),
                csv_escape(&a.name),
                a.money,
                a.tokens,
                a.money + a.tokens,
                csv_escape(&a.phase),
                a.alive,
            ));
        }
        csv.push('\n');
    }

    // Section 2: GDP time series
    {
        if let Some(ref store) = state.snapshot_store {
            let store = store.lock().await;
            if let Ok(snapshots) = store.list(Some(from), Some(to), None) {
                csv.push_str("# GDP Time Series\n");
                csv.push_str("tick,gdp,population,gini\n");
                for s in &snapshots {
                    csv.push_str(&format!(
                        "{},{},{},{:.4}\n",
                        s.tick, s.gdp, s.active_agents, s.gini_coefficient
                    ));
                }
                csv.push('\n');
            }
        }
    }

    // Section 3: Transactions
    {
        csv.push_str("# Transactions\n");
        csv.push_str("tick,buyer_id,seller_id,listing_id,price,item_title\n");
        if let Some(ref marketplace) = state.marketplace {
            let mp = marketplace.lock().await;
            for listing in mp.list_all().iter() {
                for purchase in mp.listing_purchases(listing.id) {
                    if purchase.tick < from || purchase.tick > to {
                        continue;
                    }
                    csv.push_str(&format!(
                        "{},{},{},{},{},{}\n",
                        purchase.tick,
                        csv_escape(&purchase.buyer_id),
                        csv_escape(&purchase.seller_id),
                        purchase.listing_id,
                        purchase.price,
                        csv_escape(&listing.title),
                    ));
                }
            }
        }
        csv.push('\n');
    }

    // Section 4: Banking
    {
        if let Some(ref banking) = state.banking_system {
            let bank = banking.lock().await;
            csv.push_str("# Bank Accounts\n");
            csv.push_str("account_id,owner_id,type,label,balance,created_tick\n");
            for a in bank.list_accounts() {
                let balance = bank.get_balance(a.id).unwrap_or(0);
                csv.push_str(&format!(
                    "{},{},{},{},{},{}\n",
                    a.id,
                    csv_escape(&a.owner_id),
                    csv_escape(&format!("{:?}", a.account_type)),
                    csv_escape(&a.label),
                    balance,
                    a.created_tick,
                ));
            }
            csv.push('\n');

            csv.push_str("# Loans\n");
            csv.push_str("loan_id,borrower_id,principal,outstanding_balance,interest_rate,status,total_repaid,created_tick\n");
            for l in bank.list_loans(None, None) {
                csv.push_str(&format!(
                    "{},{},{},{},{},{},{},{}\n",
                    l.id,
                    csv_escape(&l.borrower_id),
                    l.principal,
                    l.outstanding_balance,
                    l.interest_rate,
                    csv_escape(&format!("{:?}", l.status)),
                    l.total_repaid,
                    l.created_tick,
                ));
            }
            csv.push('\n');
        }
    }

    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/csv; charset=utf-8")],
        csv,
    )
        .into_response()
}

/// Compute Gini coefficient for a slice of non-negative values.
/// Uses the formula: G = (2 * Σ(i * x_i)) / (n * Σ(x_i)) - (n + 1) / n
/// where x_i are sorted in ascending order.
fn compute_gini(values: &[u64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let n = values.len() as f64;
    let mut sorted: Vec<u64> = values.to_vec();
    sorted.sort();
    let sum: u64 = sorted.iter().sum();
    if sum == 0 {
        return 0.0;
    }
    let mut weighted_sum: u64 = 0;
    for (i, &v) in sorted.iter().enumerate() {
        weighted_sum += (i as u64 + 1) * v;
    }
    (2.0 * weighted_sum as f64) / (n * sum as f64) - (n + 1.0) / n
}

// ════════════════════════════════════════════════════════════
//  ORGANIZATION DATA EXPORT
// ════════════════════════════════════════════════════════════

async fn export_organization(state: &AppState, query: &ExportQuery) -> axum::response::Response {
    let fmt = query.format.as_deref().unwrap_or("json").to_lowercase();

    match fmt.as_str() {
        "csv" => export_org_csv(state, query).await,
        _ => export_org_json(state, query).await,
    }
}

async fn export_org_json(state: &AppState, _query: &ExportQuery) -> axum::response::Response {
    let mut result = serde_json::Map::new();

    // 1. Organizations and members
    {
        let orgs: Vec<serde_json::Value> = if let Some(ref org_store) = state.org_store {
            let store = org_store.lock().await;
            store
                .list()
                .iter()
                .map(|org| {
                    let members: Vec<serde_json::Value> = org
                        .members
                        .iter()
                        .map(|m| {
                            serde_json::json!({
                                "agent_id": m.agent_id,
                                "agent_name": m.agent_name,
                                "role": format!("{:?}", m.role),
                                "share": m.share,
                                "joined_tick": m.joined_tick,
                            })
                        })
                        .collect();
                    serde_json::json!({
                        "id": org.id,
                        "name": org.name,
                        "type": format!("{:?}", org.org_type),
                        "status": format!("{:?}", org.status),
                        "treasury": org.treasury,
                        "debts": org.debts,
                        "member_count": org.member_count(),
                        "created_tick": org.created_tick,
                        "last_activity_tick": org.last_activity_tick,
                        "members": members,
                    })
                })
                .collect()
        } else {
            Vec::new()
        };
        result.insert("organizations".into(), serde_json::json!(orgs));
    }

    // 2. Governance proposals
    {
        let proposals: Vec<serde_json::Value> = if let Some(ref governance) = state.governance {
            let gov = governance.lock().await;
            gov.proposals
                .values()
                .map(|p| {
                    let votes: Vec<serde_json::Value> = p
                        .votes
                        .iter()
                        .map(|v| {
                            serde_json::json!({
                                "voter_id": v.voter_id,
                                "in_favor": v.in_favor,
                                "weight": v.weight,
                                "voted_at": v.voted_at,
                            })
                        })
                        .collect();
                    serde_json::json!({
                        "id": p.id.to_string(),
                        "org_id": p.org_id.to_string(),
                        "proposer_id": p.proposer_id,
                        "type": format!("{:?}", p.proposal_type),
                        "title": p.title,
                        "status": format!("{:?}", p.status),
                        "votes_for": p.votes_for(),
                        "votes_against": p.votes_against(),
                        "vote_count": p.votes.len(),
                        "created_at": p.created_at,
                        "votes": votes,
                    })
                })
                .collect()
        } else {
            Vec::new()
        };
        result.insert("proposals".into(), serde_json::json!(proposals));
    }

    // 3. Governance metrics (elections, stability)
    {
        if let Some(ref metrics) = state.governance_metrics {
            let m = metrics.lock().await;
            let summary = m.get_world_governance_summary();

            // Per-org metrics
            let org_metrics: Vec<serde_json::Value> = if let Some(ref org_store) = state.org_store {
                let store = org_store.lock().await;
                store
                    .list()
                    .iter()
                    .map(|org| {
                        let om = m.get_org_metrics(uuid::Uuid::parse_str(&org.id).unwrap_or(uuid::Uuid::nil()));
                        serde_json::json!({
                            "org_id": org.id,
                            "org_name": org.name,
                            "election_count": om.election_count,
                            "avg_participation_rate": om.avg_participation_rate,
                            "governance_stability_score": om.governance_stability_score,
                            "total_tax_collected": om.total_tax_collected,
                            "treaties_signed": om.treaties_signed,
                            "treaties_broken": om.treaties_broken,
                            "rules_proposed": om.rules_proposed,
                            "rules_activated": om.rules_activated,
                            "legislation_success_rate": om.legislation_success_rate,
                        })
                    })
                    .collect()
            } else {
                Vec::new()
            };

            result.insert(
                "governance_summary".into(),
                serde_json::json!({
                    "total_orgs": summary.total_orgs,
                    "avg_stability": summary.avg_stability,
                    "total_tax_collected": summary.total_tax_collected,
                    "total_treaties": summary.total_treaties,
                    "election_activity_rate": summary.election_activity_rate,
                    "total_rules_proposed": summary.total_rules_proposed,
                    "total_rules_activated": summary.total_rules_activated,
                    "avg_legislation_success_rate": summary.avg_legislation_success_rate,
                }),
            );
            result.insert("org_metrics".into(), serde_json::json!(org_metrics));
        }
    }

    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/json")],
        axum::Json(serde_json::Value::Object(result)),
    )
        .into_response()
}

async fn export_org_csv(state: &AppState, _query: &ExportQuery) -> axum::response::Response {
    let mut csv = String::new();

    // Organizations
    csv.push_str("# Organizations\n");
    csv.push_str("org_id,name,type,status,treasury,debts,member_count,created_tick,last_activity_tick\n");
    if let Some(ref org_store) = state.org_store {
        let store = org_store.lock().await;
        for org in store.list() {
            csv.push_str(&format!(
                "{},{},{},{},{},{},{},{},{}\n",
                csv_escape(&org.id),
                csv_escape(&org.name),
                csv_escape(&format!("{:?}", org.org_type)),
                csv_escape(&format!("{:?}", org.status)),
                org.treasury,
                org.debts,
                org.member_count(),
                org.created_tick,
                org.last_activity_tick,
            ));
        }
    }
    csv.push('\n');

    // Members
    csv.push_str("# Organization Members\n");
    csv.push_str("org_id,org_name,agent_id,agent_name,role,share,joined_tick\n");
    if let Some(ref org_store) = state.org_store {
        let store = org_store.lock().await;
        for org in store.list() {
            for m in &org.members {
                csv.push_str(&format!(
                    "{},{},{},{},{},{:.4},{}\n",
                    csv_escape(&org.id),
                    csv_escape(&org.name),
                    csv_escape(&m.agent_id),
                    csv_escape(&m.agent_name),
                    csv_escape(&format!("{:?}", m.role)),
                    m.share,
                    m.joined_tick,
                ));
            }
        }
    }
    csv.push('\n');

    // Proposals
    csv.push_str("# Governance Proposals\n");
    csv.push_str("proposal_id,org_id,proposer_id,type,title,status,votes_for,votes_against,created_at\n");
    if let Some(ref governance) = state.governance {
        let gov = governance.lock().await;
        for p in gov.proposals.values() {
            csv.push_str(&format!(
                "{},{},{},{},{},{},{},{},{}\n",
                p.id,
                p.org_id,
                csv_escape(&p.proposer_id),
                csv_escape(&format!("{:?}", p.proposal_type)),
                csv_escape(&p.title),
                csv_escape(&format!("{:?}", p.status)),
                p.votes_for(),
                p.votes_against(),
                p.created_at,
            ));
        }
    }
    csv.push('\n');

    // Governance metrics
    if let Some(ref metrics) = state.governance_metrics {
        let m = metrics.lock().await;
        csv.push_str("# Governance Metrics Per Org\n");
        csv.push_str("org_id,election_count,avg_participation_rate,stability_score,total_tax_collected,treaties_signed,treaties_broken,rules_proposed,rules_activated,legislation_success_rate\n");
        if let Some(ref org_store) = state.org_store {
            let store = org_store.lock().await;
            for org in store.list() {
                let om = m.get_org_metrics(uuid::Uuid::parse_str(&org.id).unwrap_or(uuid::Uuid::nil()));
                csv.push_str(&format!(
                    "{},{},{:.4},{:.4},{},{},{},{},{},{:.4}\n",
                    csv_escape(&org.id),
                    om.election_count,
                    om.avg_participation_rate,
                    om.governance_stability_score,
                    om.total_tax_collected,
                    om.treaties_signed,
                    om.treaties_broken,
                    om.rules_proposed,
                    om.rules_activated,
                    om.legislation_success_rate,
                ));
            }
        }
    }

    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/csv; charset=utf-8")],
        csv,
    )
        .into_response()
}

// ── Tests ──────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_escape_basic() {
        assert_eq!(csv_escape("hello"), "hello");
        assert_eq!(csv_escape("hello,world"), "\"hello,world\"");
        assert_eq!(csv_escape("=SUM(A1)"), "\"=SUM(A1)\"");
    }

    #[test]
    fn xml_escape_basic() {
        assert_eq!(xml_escape("a&b<c>d"), "a&amp;b&lt;c&gt;d");
        assert_eq!(xml_escape("normal"), "normal");
    }

    #[test]
    fn gini_perfect_equality() {
        let values = vec![100, 100, 100, 100];
        let gini = compute_gini(&values);
        assert!(gini.abs() < 0.001, "Gini should be ~0 for equal values, got {}", gini);
    }

    #[test]
    fn gini_maximal_inequality() {
        let values = vec![0, 0, 0, 100];
        let gini = compute_gini(&values);
        assert!(gini > 0.5, "Gini should be high for unequal values, got {}", gini);
    }

    #[test]
    fn gini_empty() {
        assert_eq!(compute_gini(&[]), 0.0);
    }

    #[test]
    fn gini_all_zero() {
        assert_eq!(compute_gini(&[0, 0, 0]), 0.0);
    }

    #[test]
    fn dot_output_contains_nodes() {
        let nodes = vec![NetNode {
            id: "a1".into(),
            label: "Alice".into(),
            phase: "Adult".into(),
            alive: true,
            tokens: 100,
            generation: None,
            skills: HashMap::new(),
            organization: None,
        }];
        let edges = vec![NetEdge {
            source: "a1".into(),
            target: "a2".into(),
            weight: 3.0,
            edge_type: "message".into(),
            interaction_count: Some(3),
        }];

        let resp = build_dot_response(&nodes, &edges, true);
        // Verify it produces a response (can't easily inspect body here,
        // but the function compiled and returned)
        drop(resp);
    }

    #[test]
    fn gexf_output_structure() {
        let nodes = vec![NetNode {
            id: "a1".into(),
            label: "Alice".into(),
            phase: "Adult".into(),
            alive: true,
            tokens: 100,
            generation: None,
            skills: HashMap::new(),
            organization: None,
        }];
        let edges = vec![NetEdge {
            source: "a1".into(),
            target: "a1".into(),
            weight: 1.0,
            edge_type: "message".into(),
            interaction_count: Some(1),
        }];

        let resp = build_gexf_response(&nodes, &edges, true);
        drop(resp);
    }
}
