use std::collections::HashMap;
use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::*,
};
use serde::{Deserialize, Serialize};

use crate::api::{AppState, ErrorResponse, AgentRecord, A2AMessage};

pub fn default_format() -> String { "json".to_string() }

#[derive(Debug, Deserialize)]
pub struct ExportSnapshotQuery {
    #[serde(default = "default_format")]
    pub format: String,
}

#[derive(Debug, Serialize)]
pub struct WorldSnapshotExport {
    pub tick: u64,
    pub agents: Vec<AgentRecord>,
    pub messages: Vec<A2AMessage>,
    pub task_count: usize,
    pub exported_at: String,
}

#[derive(Debug, Deserialize)]
pub struct ExportEconomyQuery {
    #[serde(default)]
    pub from_tick: Option<u64>,
    #[serde(default)]
    pub to_tick: Option<u64>,
    #[serde(default = "default_format")]
    pub format: String,
}

#[derive(Debug, Serialize)]
pub struct EconomyTickData {
    pub tick: u64,
    pub total_money: u64,
    pub total_tokens: u64,
    pub alive_count: usize,
    pub gini_coefficient: f64,
    pub task_count: usize,
}

#[derive(Debug, Deserialize)]
pub struct ExportQueryRequest {
    pub filters: ExportFilters,
}

#[derive(Debug, Deserialize)]
pub struct ExportFilters {
    #[serde(default)]
    pub agent_ids: Option<Vec<String>>,
    #[serde(default)]
    pub event_types: Option<Vec<String>>,
    pub tick_range: Option<(u64, u64)>,
    #[serde(default = "default_format")]
    pub format: String,
}

// ── Trace Handlers ────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct ListTracesQuery {
    limit: Option<usize>,
    offset: Option<usize>,
}

pub async fn list_agent_traces(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
    Query(query): Query<ListTracesQuery>,
) -> impl IntoResponse {
    let store = match &state.trace_store {
        Some(s) => s.clone(),
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(ErrorResponse { error: "trace store not configured".into() })).into_response(),
    };
    let store = store.lock().await;
    let limit = query.limit.unwrap_or(100);
    let offset = query.offset.unwrap_or(0);
    let timeline = store.get_timeline(&agent_id, limit, offset);
    Json(&timeline).into_response()
}

pub async fn get_latest_trace(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
) -> impl IntoResponse {
    let store = match &state.trace_store {
        Some(s) => s.clone(),
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(ErrorResponse { error: "trace store not configured".into() })).into_response(),
    };
    let store = store.lock().await;
    match store.get_latest(&agent_id) {
        Some(trace) => Json(trace).into_response(),
        None => (StatusCode::NOT_FOUND, Json(ErrorResponse { error: "no traces for agent".into() })).into_response(),
    }
}

pub async fn get_trace_by_tick(
    State(state): State<AppState>,
    Path((agent_id, tick)): Path<(String, u64)>,
) -> impl IntoResponse {
    let store = match &state.trace_store {
        Some(s) => s.clone(),
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(ErrorResponse { error: "trace store not configured".into() })).into_response(),
    };
    let store = store.lock().await;
    match store.get_tick(&agent_id, tick) {
        Some(trace) => Json(trace).into_response(),
        None => (StatusCode::NOT_FOUND, Json(ErrorResponse { error: format!("no trace at tick {} for agent", tick) })).into_response(),
    }
}

pub async fn submit_trace(
    State(state): State<AppState>,
    Json(trace): Json<crate::tracing::TickTraceData>,
) -> impl IntoResponse {
    let store = match &state.trace_store {
        Some(s) => s.clone(),
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(ErrorResponse { error: "trace store not configured".into() })).into_response(),
    };
    let mut store = store.lock().await;
    store.save(trace);
    (StatusCode::OK, Json(serde_json::json!({"status": "ok"}))).into_response()
}

// ── Data Export Handlers ──────────────────────────────────

pub fn compute_gini(values: &[u64]) -> f64 {
    if values.is_empty() { return 0.0; }
    let n = values.len() as f64;
    let mean: f64 = values.iter().sum::<u64>() as f64 / n;
    if mean == 0.0 { return 0.0; }
    let mut sorted: Vec<u64> = values.to_vec();
    sorted.sort();
    let sum_diff: f64 = sorted.iter()
        .flat_map(|&xi| sorted.iter().map(move |&xj| (xi as f64 - xj as f64).abs()))
        .sum();
    sum_diff / (2.0 * n * n * mean)
}

pub fn agents_to_csv(agents: &[AgentRecord]) -> String {
    let mut csv = String::from("id,name,phase,tokens,money,alive,ticks_survived,personality\n");
    for a in agents {
        csv.push_str(&format!(
            "{},{},{},{},{},{},{},{}\n",
            a.id, a.name, a.phase, a.tokens, a.money, a.alive, a.ticks_survived, a.personality
        ));
    }
    csv
}

pub async fn export_snapshot(
    State(state): State<AppState>,
    Query(query): Query<ExportSnapshotQuery>,
) -> impl IntoResponse {
    let agents = state.agents.lock().await;
    let messages = state.messages.lock().await;
    let board = state.board.lock().await;
    let tick = *state.tick_rx.borrow();
    let task_count = board.list().len();

    let exported_at = chrono::Utc::now().to_rfc3339();
    let snapshot = WorldSnapshotExport {
        tick,
        agents: agents.clone(),
        messages: messages.clone(),
        task_count,
        exported_at: exported_at.clone(),
    };

    match query.format.as_str() {
        "csv" => {
            let csv = agents_to_csv(&agents);
            let body = axum::body::Body::from(csv);
            let mut resp = axum::response::Response::new(body);
            *resp.status_mut() = StatusCode::OK;
            resp.headers_mut().insert("content-type", "text/csv".parse().unwrap());
            resp.headers_mut().insert("content-disposition", "attachment; filename=\"world_snapshot.csv\"".parse().unwrap());
            resp
        }
        _ => {
            match serde_json::to_string_pretty(&snapshot) {
                Ok(json) => {
                    let body = axum::body::Body::from(json);
                    let mut resp = axum::response::Response::new(body);
                    *resp.status_mut() = StatusCode::OK;
                    resp.headers_mut().insert("content-type", "application/json".parse().unwrap());
                    resp.headers_mut().insert("content-disposition", "attachment; filename=\"world_snapshot.json\"".parse().unwrap());
                    resp
                }
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: e.to_string() })).into_response(),
            }
        }
    }
}

pub async fn export_snapshot_by_tick(
    State(state): State<AppState>,
    Path(tick): Path<u64>,
    Query(query): Query<ExportSnapshotQuery>,
) -> impl IntoResponse {
    let agents = state.agents.lock().await;
    let messages = state.messages.lock().await;
    let board = state.board.lock().await;
    let task_count = board.list().len();

    // Filter messages to the specific tick
    let filtered_messages: Vec<A2AMessage> = messages.iter()
        .filter(|m| m.tick == tick)
        .cloned()
        .collect();

    let exported_at = chrono::Utc::now().to_rfc3339();
    let snapshot = WorldSnapshotExport {
        tick,
        agents: agents.clone(),
        messages: filtered_messages,
        task_count,
        exported_at: exported_at.clone(),
    };

    match query.format.as_str() {
        "csv" => {
            let csv = agents_to_csv(&agents);
            let body = axum::body::Body::from(csv);
            let mut resp = axum::response::Response::new(body);
            *resp.status_mut() = StatusCode::OK;
            resp.headers_mut().insert("content-type", "text/csv".parse().unwrap());
            resp.headers_mut().insert("content-disposition", "attachment; filename=\"world_snapshot.csv\"".parse().unwrap());
            resp
        }
        _ => {
            match serde_json::to_string_pretty(&snapshot) {
                Ok(json) => {
                    let body = axum::body::Body::from(json);
                    let mut resp = axum::response::Response::new(body);
                    *resp.status_mut() = StatusCode::OK;
                    resp.headers_mut().insert("content-type", "application/json".parse().unwrap());
                    resp.headers_mut().insert("content-disposition", "attachment; filename=\"world_snapshot.json\"".parse().unwrap());
                    resp
                }
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: e.to_string() })).into_response(),
            }
        }
    }
}

pub async fn export_economy(
    State(state): State<AppState>,
    Query(query): Query<ExportEconomyQuery>,
) -> impl IntoResponse {
    let agents = state.agents.lock().await;
    let board = state.board.lock().await;
    let current_tick = *state.tick_rx.borrow();

    let _from_tick = query.from_tick.unwrap_or(0);
    let _to_tick = query.to_tick.unwrap_or(current_tick);

    // For now we compute a single data point for the current state
    // (since we don't store per-tick history in memory)
    let tick = current_tick;
    let total_money: u64 = agents.iter().map(|a| a.money).sum();
    let total_tokens: u64 = agents.iter().map(|a| a.tokens).sum();
    let alive_count = agents.iter().filter(|a| a.alive).count();
    let task_count = board.list().len();
    let money_values: Vec<u64> = agents.iter().map(|a| a.money).collect();
    let gini = compute_gini(&money_values);

    // Build data points — single point for current state
    let data = vec![EconomyTickData {
        tick,
        total_money,
        total_tokens,
        alive_count,
        gini_coefficient: gini,
        task_count,
    }];

    match query.format.as_str() {
        "csv" => {
            let mut csv = String::from("tick,total_money,total_tokens,alive_count,gini,task_count\n");
            for d in &data {
                csv.push_str(&format!(
                    "{},{},{},{},{},{}\n",
                    d.tick, d.total_money, d.total_tokens, d.alive_count, d.gini_coefficient, d.task_count
                ));
            }
            let body = axum::body::Body::from(csv);
            let mut resp = axum::response::Response::new(body);
            *resp.status_mut() = StatusCode::OK;
            resp.headers_mut().insert("content-type", "text/csv".parse().unwrap());
            resp.headers_mut().insert("content-disposition", "attachment; filename=\"economy_export.csv\"".parse().unwrap());
            resp
        }
        _ => {
            match serde_json::to_string_pretty(&data) {
                Ok(json) => {
                    let body = axum::body::Body::from(json);
                    let mut resp = axum::response::Response::new(body);
                    *resp.status_mut() = StatusCode::OK;
                    resp.headers_mut().insert("content-type", "application/json".parse().unwrap());
                    resp.headers_mut().insert("content-disposition", "attachment; filename=\"economy_export.json\"".parse().unwrap());
                    resp
                }
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: e.to_string() })).into_response(),
            }
        }
    }
}

pub async fn export_query(
    State(state): State<AppState>,
    Json(body): Json<ExportQueryRequest>,
) -> impl IntoResponse {
    let filters = body.filters;

    // Validate tick_range
    let tick_range = match filters.tick_range {
        Some((from, to)) if from <= to => Some((from, to)),
        Some((from, to)) => {
            return (StatusCode::BAD_REQUEST, Json(ErrorResponse {
                error: format!("invalid tick_range: start ({}) must be <= end ({})", from, to),
            })).into_response();
        }
        None => {
            return (StatusCode::BAD_REQUEST, Json(ErrorResponse {
                error: "tick_range is required".into(),
            })).into_response();
        }
    };

    // Filter agents
    let agents = state.agents.lock().await;
    let filtered_agents: Vec<AgentRecord> = match &filters.agent_ids {
        Some(ids) => agents.iter()
            .filter(|a| ids.contains(&a.id))
            .cloned()
            .collect(),
        None => agents.clone(),
    };

    // Filter traces
    let trace_data = match &state.trace_store {
        Some(store) => {
            let store = store.lock().await;
            let agent_ids_ref = filters.agent_ids.as_deref();
            let all_traces = store.get_all_traces(agent_ids_ref, tick_range);

            // Filter by event_types if provided
            if let Some(ref event_types) = filters.event_types {
                all_traces.into_iter()
                    .filter(|trace| {
                        trace.phases.iter().any(|p| event_types.contains(&p.phase))
                    })
                    .cloned()
                    .collect::<Vec<_>>()
            } else {
                all_traces.into_iter().cloned().collect::<Vec<_>>()
            }
        }
        None => Vec::new(),
    };

    let result = serde_json::json!({
        "agents": filtered_agents,
        "traces": trace_data,
        "tick_range": tick_range,
    });

    match filters.format.as_str() {
        "csv" => {
            let csv = agents_to_csv(&filtered_agents);
            let body = axum::body::Body::from(csv);
            let mut resp = axum::response::Response::new(body);
            *resp.status_mut() = StatusCode::OK;
            resp.headers_mut().insert("content-type", "text/csv".parse().unwrap());
            resp.headers_mut().insert("content-disposition", "attachment; filename=\"query_export.csv\"".parse().unwrap());
            resp
        }
        _ => {
            match serde_json::to_string_pretty(&result) {
                Ok(json) => {
                    let body = axum::body::Body::from(json);
                    let mut resp = axum::response::Response::new(body);
                    *resp.status_mut() = StatusCode::OK;
                    resp.headers_mut().insert("content-type", "application/json".parse().unwrap());
                    resp.headers_mut().insert("content-disposition", "attachment; filename=\"query_export.json\"".parse().unwrap());
                    resp
                }
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: e.to_string() })).into_response(),
            }
        }
    }
}

/// Trace + data export routes.
pub fn trace_routes() -> axum::Router<AppState> {
    axum::Router::new()
        .route("/api/v1/agents/:id/traces", get(list_agent_traces))
        .route("/api/v1/agents/:id/traces/latest", get(get_latest_trace))
        .route("/api/v1/agents/:id/traces/:tick", get(get_trace_by_tick))
        .route("/api/v1/agents/:id/traces", post(submit_trace))
        .route("/api/v1/export/snapshot", get(export_snapshot))
        .route("/api/v1/export/snapshot/:tick", get(export_snapshot_by_tick))
        .route("/api/v1/export/economy", get(export_economy))
        .route("/api/v1/export/query", post(export_query))
}
