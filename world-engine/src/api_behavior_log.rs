//! Behavior Log Export API — `/api/v2/export/behavior/*`.
//!
//! Provides per-agent action history export in CSV and JSON formats.
//! Behavior logs are derived from the EventBus event history, filtered by
//! agent_id and time range.

use axum::{
    extract::{Path, Query, State},
    http::{header, StatusCode},
    response::IntoResponse,
    routing::get,
    Router,
};
use serde::{Deserialize, Serialize};

use crate::api::AppState;

// ── Query Types ───────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct BehaviorLogQuery {
    /// Filter by agent_id (optional — returns all agents if omitted).
    pub agent_id: Option<String>,
    /// Start tick (inclusive). Defaults to 0.
    pub from_tick: Option<u64>,
    /// End tick (inclusive). Defaults to latest tick.
    pub to_tick: Option<u64>,
    /// Output format: "json" or "csv". Defaults to "json".
    pub format: Option<String>,
    /// Maximum number of log entries to return. Defaults to 1000.
    pub limit: Option<u64>,
    /// Event types to include (comma-separated). Empty = all.
    pub event_types: Option<String>,
}

/// A single behavior log entry derived from a WorldEvent.
#[derive(Debug, Clone, Serialize)]
pub struct BehaviorLogEntry {
    /// Tick when the action occurred.
    pub tick: u64,
    /// Event type (e.g. "transaction_completed", "agent_died").
    pub event_type: String,
    /// Primary agent involved.
    pub agent_id: String,
    /// Optional secondary agent (e.g. transaction counterparty).
    pub target_agent_id: Option<String>,
    /// Human-readable description.
    pub description: String,
    /// Structured event details.
    pub details: serde_json::Value,
}

// ── Router ────────────────────────────────────────────────

pub fn behavior_log_routes() -> Router<AppState> {
    Router::new()
        .route("/api/v2/export/behavior", get(export_behavior_log))
        .route(
            "/api/v2/export/behavior/{agent_id}",
            get(export_agent_behavior_log),
        )
}

// ── Helpers ───────────────────────────────────────────────

/// Extract behavior log entries from the message (A2A) history and agent state.
///
/// Since the world engine stores events in-memory via EventBus (broadcast, no replay),
/// we derive behavior logs from:
/// 1. A2A messages (inter-agent communication)
/// 2. Agent state changes (phase transitions inferred from agent records)
/// 3. Snapshot key events (from the time capsule store)
async fn collect_behavior_entries(
    state: &AppState,
    agent_filter: Option<&str>,
    from_tick: Option<u64>,
    to_tick: Option<u64>,
    event_type_filter: Option<&str>,
    limit: u64,
) -> Vec<BehaviorLogEntry> {
    let mut entries: Vec<BehaviorLogEntry> = Vec::new();
    let current_tick = *state.tick_rx.borrow();

    let from = from_tick.unwrap_or(0);
    let to = to_tick.unwrap_or(current_tick);

    // Parse event type filter
    let type_filter: Option<std::collections::HashSet<String>> =
        event_type_filter.map(|s| s.split(',').map(|t| t.trim().to_lowercase()).collect());

    // 1. Collect from A2A messages
    let messages = state.messages.lock().await;
    for msg in messages.iter() {
        if msg.tick < from || msg.tick > to {
            continue;
        }

        let matches_agent = match agent_filter {
            Some(aid) => msg.from_agent == aid || msg.to_agent == aid,
            None => true,
        };
        if !matches_agent {
            continue;
        }

        let etype = format!("message_{}", msg.message_type);
        if let Some(ref tf) = type_filter {
            if !tf.contains(&etype) && !tf.contains(&msg.message_type.to_lowercase()) {
                continue;
            }
        }

        let description = format!(
            "{} sent {} to {} (tick {})",
            msg.from_agent, msg.message_type, msg.to_agent, msg.tick
        );

        entries.push(BehaviorLogEntry {
            tick: msg.tick,
            event_type: etype,
            agent_id: msg.from_agent.clone(),
            target_agent_id: Some(msg.to_agent.clone()),
            description,
            details: serde_json::json!({
                "message_id": msg.id,
                "message_type": msg.message_type,
                "payload": msg.payload,
            }),
        });

        // Also create a "received" entry for the target
        if agent_filter.is_none() || Some(msg.to_agent.as_str()) == agent_filter {
            entries.push(BehaviorLogEntry {
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

    // 2. Collect from snapshot key events (time capsule)
    if let Some(ref store) = state.snapshot_store {
        let store = store.lock().await;
        if let Ok(snapshots) = store.list(Some(from), Some(to), None) {
            for snap in &snapshots {
                for ke in &snap.key_events {
                    if ke.tick < from || ke.tick > to {
                        continue;
                    }
                    let matches_agent = match agent_filter {
                        Some(aid) => ke.agent_id.as_deref() == Some(aid),
                        None => true,
                    };
                    if !matches_agent {
                        continue;
                    }
                    if let Some(ref tf) = type_filter {
                        if !tf.contains(&ke.event_type.to_lowercase()) {
                            continue;
                        }
                    }

                    entries.push(BehaviorLogEntry {
                        tick: ke.tick,
                        event_type: ke.event_type.clone(),
                        agent_id: ke.agent_id.clone().unwrap_or_default(),
                        target_agent_id: None,
                        description: ke.description.clone(),
                        details: serde_json::json!({
                            "event_type": ke.event_type,
                        }),
                    });
                }
            }
        }
    }

    // 3. Collect from agent state (current snapshot)
    let agents = state.agents.lock().await;
    for agent in agents.iter() {
        if let Some(aid) = agent_filter {
            if agent.id != aid {
                continue;
            }
        }

        if let Some(ref tf) = type_filter {
            if !tf.contains("agent_state") && !tf.contains("state") {
                continue;
            }
        }

        entries.push(BehaviorLogEntry {
            tick: current_tick,
            event_type: "agent_state".to_string(),
            agent_id: agent.id.clone(),
            target_agent_id: None,
            description: format!(
                "Agent {} (phase: {}, tokens: {}, money: {}, alive: {}, ticks_survived: {})",
                agent.name,
                agent.phase,
                agent.tokens,
                agent.money,
                agent.alive,
                agent.ticks_survived
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

    // Sort by tick, then by event_type
    entries.sort_by(|a, b| {
        a.tick
            .cmp(&b.tick)
            .then_with(|| a.event_type.cmp(&b.event_type))
    });

    // Apply limit
    entries.truncate(limit as usize);

    entries
}

/// Escape a CSV field per RFC 4180.
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

fn entries_to_csv(entries: &[BehaviorLogEntry]) -> String {
    let mut csv = String::from("tick,event_type,agent_id,target_agent_id,description,details\n");
    for e in entries {
        let details_str = serde_json::to_string(&e.details).unwrap_or_default();
        csv.push_str(&format!(
            "{},{},{},{},{},{}\n",
            e.tick,
            csv_escape(&e.event_type),
            csv_escape(&e.agent_id),
            match &e.target_agent_id {
                Some(t) => csv_escape(t),
                None => String::new(),
            },
            csv_escape(&e.description),
            csv_escape(&details_str),
        ));
    }
    csv
}

fn resolve_format(query: &BehaviorLogQuery, default: &str) -> String {
    query.format.as_deref().unwrap_or(default).to_lowercase()
}

// ── Handlers ──────────────────────────────────────────────

/// `GET /api/v2/export/behavior` — export behavior logs for all or filtered agents.
async fn export_behavior_log(
    State(state): State<AppState>,
    Query(query): Query<BehaviorLogQuery>,
) -> impl IntoResponse {
    let limit = query.limit.unwrap_or(1000);
    let fmt = resolve_format(&query, "json");

    let entries = collect_behavior_entries(
        &state,
        query.agent_id.as_deref(),
        query.from_tick,
        query.to_tick,
        query.event_types.as_deref(),
        limit,
    )
    .await;

    match fmt.as_str() {
        "csv" => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "text/csv; charset=utf-8")],
            entries_to_csv(&entries),
        )
            .into_response(),
        _ => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "application/json")],
            axum::Json(serde_json::json!({
                "count": entries.len(),
                "entries": entries,
            })),
        )
            .into_response(),
    }
}

/// `GET /api/v2/export/behavior/{agent_id}` — export behavior log for a specific agent.
async fn export_agent_behavior_log(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
    Query(query): Query<BehaviorLogQuery>,
) -> impl IntoResponse {
    let limit = query.limit.unwrap_or(1000);
    let fmt = resolve_format(&query, "json");

    let entries = collect_behavior_entries(
        &state,
        Some(&agent_id),
        query.from_tick,
        query.to_tick,
        query.event_types.as_deref(),
        limit,
    )
    .await;

    if entries.is_empty() {
        return (
            StatusCode::NOT_FOUND,
            [(header::CONTENT_TYPE, "application/json")],
            axum::Json(serde_json::json!({
                "error": format!("No behavior log entries found for agent {}", agent_id)
            })),
        )
            .into_response();
    }

    match fmt.as_str() {
        "csv" => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "text/csv; charset=utf-8")],
            entries_to_csv(&entries),
        )
            .into_response(),
        _ => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "application/json")],
            axum::Json(serde_json::json!({
                "agent_id": agent_id,
                "count": entries.len(),
                "entries": entries,
            })),
        )
            .into_response(),
    }
}

// ── Tests ──────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_escape_simple() {
        assert_eq!(csv_escape("hello"), "hello");
    }

    #[test]
    fn csv_escape_comma() {
        assert_eq!(csv_escape("hello,world"), "\"hello,world\"");
    }

    #[test]
    fn csv_escape_formula_injection() {
        assert_eq!(csv_escape("=SUM(A1)"), "\"=SUM(A1)\"");
    }

    #[test]
    fn entries_to_csv_basic() {
        let entries = vec![BehaviorLogEntry {
            tick: 100,
            event_type: "transaction_completed".into(),
            agent_id: "agent-1".into(),
            target_agent_id: Some("agent-2".into()),
            description: "Sent 50 tokens".into(),
            details: serde_json::json!({"amount": 50}),
        }];
        let csv = entries_to_csv(&entries);
        assert!(csv.starts_with("tick,event_type"));
        assert!(csv.contains("100,transaction_completed,agent-1,agent-2"));
    }

    #[test]
    fn resolve_format_defaults() {
        let q = BehaviorLogQuery {
            agent_id: None,
            from_tick: None,
            to_tick: None,
            format: None,
            limit: None,
            event_types: None,
        };
        assert_eq!(resolve_format(&q, "json"), "json");
        assert_eq!(resolve_format(&q, "csv"), "csv");
    }

    #[test]
    fn resolve_format_explicit() {
        let q = BehaviorLogQuery {
            agent_id: None,
            from_tick: None,
            to_tick: None,
            format: Some("csv".into()),
            limit: None,
            event_types: None,
        };
        assert_eq!(resolve_format(&q, "json"), "csv");
    }
}
