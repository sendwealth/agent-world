//! Experiment management API — `/api/v2/experiments/*`.
//!
//! Provides experiment CRUD, lifecycle control (start/stop/pause/resume),
//! event injection (reusing the intervention subsystem), and result retrieval.
//! Experiments are recording sessions — no WorldState isolation/cloning.

use std::sync::Arc;

use axum::{
    Json,
    Router,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::api::{AppState, ErrorResponse};

// ── Types ─────────────────────────────────────────────────

/// In-memory experiment record.
#[derive(Debug, Clone, Serialize)]
pub struct Experiment {
    pub id: String,
    pub status: ExperimentStatus,
    pub config: ExperimentConfig,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub stopped_at: Option<DateTime<Utc>>,
    pub start_tick: Option<u64>,
    pub end_tick: Option<u64>,
    /// Injected events/commands recorded during the experiment.
    pub injections: Vec<InjectionRecord>,
    /// Per-tick metric snapshots collected during the experiment.
    pub tick_snapshots: Vec<TickSnapshot>,
}

/// Experiment lifecycle status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExperimentStatus {
    Created,
    Running,
    Paused,
    Stopped,
}

/// Configuration for creating a new experiment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExperimentConfig {
    /// Number of agents to track (informational — doesn't create agents).
    pub agent_count: Option<u64>,
    /// Target tick count for the experiment.
    pub target_ticks: Option<u64>,
    /// LLM model name (informational).
    pub llm_model: Option<String>,
    /// LLM temperature (informational).
    pub llm_temperature: Option<f64>,
    /// Free-form description.
    #[serde(default)]
    pub description: String,
}

/// Request body for `POST /api/v2/experiments`.
#[derive(Debug, Deserialize)]
pub struct CreateExperimentRequest {
    #[serde(default)]
    pub agent_count: Option<u64>,
    #[serde(default)]
    pub target_ticks: Option<u64>,
    #[serde(default)]
    pub llm_model: Option<String>,
    #[serde(default)]
    pub llm_temperature: Option<f64>,
    #[serde(default)]
    pub description: String,
}

/// Request body for `POST /api/v2/experiments/{id}/inject`.
#[derive(Debug, Deserialize)]
pub struct InjectRequest {
    /// What to inject — an event type name or an attribute modification.
    pub injection_type: String,
    /// Target agent ID (optional — some injections are global).
    pub agent_id: Option<String>,
    /// Free-form payload.
    #[serde(default)]
    pub payload: serde_json::Value,
}

/// Record of a single injection applied during an experiment.
#[derive(Debug, Clone, Serialize)]
pub struct InjectionRecord {
    pub tick: u64,
    pub injection_type: String,
    pub agent_id: Option<String>,
    pub payload: serde_json::Value,
    pub injected_at: DateTime<Utc>,
}

/// Per-tick metric snapshot collected during an experiment.
#[derive(Debug, Clone, Serialize)]
pub struct TickSnapshot {
    pub tick: u64,
    pub agent_count: usize,
    pub alive_count: usize,
    pub total_money: u64,
    pub total_tokens: u64,
}

/// Shared in-memory experiment store.
pub type SharedExperimentStore = Arc<Mutex<Vec<Experiment>>>;

// ── Router ────────────────────────────────────────────────

/// Build the experiment routes (without auth middleware).
pub fn experiment_routes() -> Router<AppState> {
    Router::new()
        .route("/api/v2/experiments", post(create_experiment))
        .route("/api/v2/experiments", get(list_experiments))
        .route("/api/v2/experiments/{id}/start", post(start_experiment))
        .route("/api/v2/experiments/{id}/stop", post(stop_experiment))
        .route("/api/v2/experiments/{id}/pause", post(pause_experiment))
        .route("/api/v2/experiments/{id}/resume", post(resume_experiment))
        .route("/api/v2/experiments/{id}/inject", post(inject_experiment))
        .route("/api/v2/experiments/{id}/results", get(get_experiment_results))
}

// ── Helpers ───────────────────────────────────────────────

/// Capture a tick snapshot from current world state.
async fn capture_tick_snapshot(state: &AppState) -> TickSnapshot {
    let agents = state.agents.lock().await;
    let tick = *state.tick_rx.borrow();
    TickSnapshot {
        tick,
        agent_count: agents.len(),
        alive_count: agents.iter().filter(|a| a.alive).count(),
        total_money: agents.iter().map(|a| a.money).sum(),
        total_tokens: agents.iter().map(|a| a.tokens).sum(),
    }
}

/// Build a 404 response for a missing experiment.
fn not_found() -> axum::response::Response {
    (
        StatusCode::NOT_FOUND,
        Json(ErrorResponse {
            error: "experiment not found".into(),
        }),
    )
        .into_response()
}

/// Summary of an experiment for the list endpoint.
#[derive(Debug, Serialize)]
struct ExperimentSummary {
    id: String,
    status: ExperimentStatus,
    description: String,
    created_at: DateTime<Utc>,
    start_tick: Option<u64>,
    end_tick: Option<u64>,
}

// ── Handlers ──────────────────────────────────────────────

/// `POST /api/v2/experiments` — create a new experiment.
async fn create_experiment(
    State(state): State<AppState>,
    Json(req): Json<CreateExperimentRequest>,
) -> impl IntoResponse {
    let id = Uuid::new_v4().to_string();

    let experiment = Experiment {
        id: id.clone(),
        status: ExperimentStatus::Created,
        config: ExperimentConfig {
            agent_count: req.agent_count,
            target_ticks: req.target_ticks,
            llm_model: req.llm_model,
            llm_temperature: req.llm_temperature,
            description: req.description,
        },
        created_at: Utc::now(),
        started_at: None,
        stopped_at: None,
        start_tick: None,
        end_tick: None,
        injections: Vec::new(),
        tick_snapshots: Vec::new(),
    };

    state.experiment_store.lock().await.push(experiment);

    (StatusCode::CREATED, Json(serde_json::json!({ "experiment_id": id }))).into_response()
}

/// `GET /api/v2/experiments` — list all experiments.
async fn list_experiments(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let experiments = state.experiment_store.lock().await;
    let summaries: Vec<ExperimentSummary> = experiments
        .iter()
        .map(|exp| ExperimentSummary {
            id: exp.id.clone(),
            status: exp.status,
            description: exp.config.description.clone(),
            created_at: exp.created_at,
            start_tick: exp.start_tick,
            end_tick: exp.end_tick,
        })
        .collect();
    Json(summaries).into_response()
}

/// `POST /api/v2/experiments/{id}/start` — start a created experiment.
///
/// Lock ordering: acquire store lock, validate + update status, drop store lock,
/// then capture snapshot (acquires agents lock), then re-acquire store lock to
/// append snapshot. This avoids holding both locks simultaneously.
async fn start_experiment(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    // Phase 1: Validate and update status under store lock.
    {
        let experiments = state.experiment_store.lock().await;
        let experiment = match experiments.iter().find(|exp| exp.id == id) {
            Some(exp) => exp,
            None => return not_found(),
        };

        if experiment.status != ExperimentStatus::Created {
            return (
                StatusCode::CONFLICT,
                Json(ErrorResponse {
                    error: format!(
                        "experiment is {:?}, expected created",
                        experiment.status
                    ),
                }),
            )
                .into_response();
        }
    }

    let tick = *state.tick_rx.borrow();

    // Update status and timestamps.
    {
        let mut experiments = state.experiment_store.lock().await;
        if let Some(experiment) = experiments.iter_mut().find(|exp| exp.id == id) {
            experiment.status = ExperimentStatus::Running;
            experiment.started_at = Some(Utc::now());
            experiment.start_tick = Some(tick);
        }
    }
    // Store lock is dropped here.

    // Phase 2: Capture snapshot (only agents lock held).
    let snapshot = capture_tick_snapshot(&state).await;

    // Phase 3: Append snapshot under store lock.
    {
        let mut experiments = state.experiment_store.lock().await;
        if let Some(experiment) = experiments.iter_mut().find(|exp| exp.id == id) {
            experiment.tick_snapshots.push(snapshot);
        }
    }

    Json(serde_json::json!({ "status": "running" })).into_response()
}

/// `POST /api/v2/experiments/{id}/stop` — stop a running experiment.
///
/// Same lock-ordering strategy as start: validate under store lock, drop,
/// capture snapshot, re-acquire to append.
async fn stop_experiment(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    // Phase 1: Validate.
    {
        let experiments = state.experiment_store.lock().await;
        let experiment = match experiments.iter().find(|exp| exp.id == id) {
            Some(exp) => exp,
            None => return not_found(),
        };

        if experiment.status != ExperimentStatus::Running
            && experiment.status != ExperimentStatus::Paused
        {
            return (
                StatusCode::CONFLICT,
                Json(ErrorResponse {
                    error: format!("experiment is {:?}, cannot stop", experiment.status),
                }),
            )
                .into_response();
        }
    }

    let tick = *state.tick_rx.borrow();

    // Update status and timestamps.
    {
        let mut experiments = state.experiment_store.lock().await;
        if let Some(experiment) = experiments.iter_mut().find(|exp| exp.id == id) {
            experiment.status = ExperimentStatus::Stopped;
            experiment.stopped_at = Some(Utc::now());
            experiment.end_tick = Some(tick);
        }
    }

    // Phase 2: Capture snapshot.
    let snapshot = capture_tick_snapshot(&state).await;

    // Phase 3: Append snapshot.
    {
        let mut experiments = state.experiment_store.lock().await;
        if let Some(experiment) = experiments.iter_mut().find(|exp| exp.id == id) {
            experiment.tick_snapshots.push(snapshot);
        }
    }

    Json(serde_json::json!({ "status": "stopped" })).into_response()
}

/// `POST /api/v2/experiments/{id}/pause` — pause a running experiment.
async fn pause_experiment(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    // Validate.
    {
        let experiments = state.experiment_store.lock().await;
        let experiment = match experiments.iter().find(|exp| exp.id == id) {
            Some(exp) => exp,
            None => return not_found(),
        };

        if experiment.status != ExperimentStatus::Running {
            return (
                StatusCode::CONFLICT,
                Json(ErrorResponse {
                    error: "experiment is not running".into(),
                }),
            )
                .into_response();
        }
    }

    // Capture snapshot (no store lock held).
    let snapshot = capture_tick_snapshot(&state).await;

    // Update status + append snapshot.
    {
        let mut experiments = state.experiment_store.lock().await;
        if let Some(experiment) = experiments.iter_mut().find(|exp| exp.id == id) {
            experiment.tick_snapshots.push(snapshot);
            experiment.status = ExperimentStatus::Paused;
        }
    }

    Json(serde_json::json!({ "status": "paused" })).into_response()
}

/// `POST /api/v2/experiments/{id}/resume` — resume a paused experiment.
async fn resume_experiment(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    // Validate.
    {
        let experiments = state.experiment_store.lock().await;
        let experiment = match experiments.iter().find(|exp| exp.id == id) {
            Some(exp) => exp,
            None => return not_found(),
        };

        if experiment.status != ExperimentStatus::Paused {
            return (
                StatusCode::CONFLICT,
                Json(ErrorResponse {
                    error: "experiment is not paused".into(),
                }),
            )
                .into_response();
        }
    }

    // Capture snapshot (no store lock held).
    let snapshot = capture_tick_snapshot(&state).await;

    // Update status + append snapshot.
    {
        let mut experiments = state.experiment_store.lock().await;
        if let Some(experiment) = experiments.iter_mut().find(|exp| exp.id == id) {
            experiment.tick_snapshots.push(snapshot);
            experiment.status = ExperimentStatus::Running;
        }
    }

    Json(serde_json::json!({ "status": "running" })).into_response()
}

/// `POST /api/v2/experiments/{id}/inject` — inject an external event or
/// modify an agent attribute during the experiment.
async fn inject_experiment(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<InjectRequest>,
) -> impl IntoResponse {
    let mut experiments = state.experiment_store.lock().await;

    let experiment = match experiments.iter_mut().find(|exp| exp.id == id) {
        Some(exp) => exp,
        None => return not_found(),
    };

    if experiment.status != ExperimentStatus::Running {
        return (
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                error: "experiment is not running".into(),
            }),
        )
            .into_response();
    }

    let tick = *state.tick_rx.borrow();
    let record = InjectionRecord {
        tick,
        injection_type: req.injection_type,
        agent_id: req.agent_id,
        payload: req.payload,
        injected_at: Utc::now(),
    };
    experiment.injections.push(record);

    Json(serde_json::json!({ "status": "injected" })).into_response()
}

/// `GET /api/v2/experiments/{id}/results` — retrieve full experiment results.
async fn get_experiment_results(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let experiments = state.experiment_store.lock().await;

    match experiments.iter().find(|exp| exp.id == id) {
        Some(experiment) => {
            let mut result = experiment.clone();
            if result.status == ExperimentStatus::Running {
                // Drop store lock before capturing snapshot.
                drop(experiments);
                let snapshot = capture_tick_snapshot(&state).await;
                result.tick_snapshots.push(snapshot);
                Json(result).into_response()
            } else {
                Json(result).into_response()
            }
        }
        None => not_found(),
    }
}

// ── Tests ──────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn experiment_status_serde_roundtrip() {
        let status = ExperimentStatus::Running;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"running\"");
        let back: ExperimentStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(back, ExperimentStatus::Running);
    }

    #[test]
    fn create_experiment_request_defaults() {
        let req: CreateExperimentRequest =
            serde_json::from_str("{}").unwrap();
        assert!(req.agent_count.is_none());
        assert!(req.target_ticks.is_none());
        assert!(req.llm_model.is_none());
        assert!(req.llm_temperature.is_none());
        assert!(req.description.is_empty());
    }

    #[test]
    fn create_experiment_request_full() {
        let req: CreateExperimentRequest = serde_json::from_str(
            r#"{"agent_count":10,"target_ticks":1000,"llm_model":"gpt-4","llm_temperature":0.7,"description":"test"}"#,
        )
        .unwrap();
        assert_eq!(req.agent_count, Some(10));
        assert_eq!(req.target_ticks, Some(1000));
        assert_eq!(req.llm_model.as_deref(), Some("gpt-4"));
        assert_eq!(req.llm_temperature, Some(0.7));
        assert_eq!(req.description, "test");
    }

    #[test]
    fn inject_request_parse() {
        let req: InjectRequest = serde_json::from_str(
            r#"{"injection_type":"add_resource","agent_id":"a1","payload":{"amount":100}}"#,
        )
        .unwrap();
        assert_eq!(req.injection_type, "add_resource");
        assert_eq!(req.agent_id.as_deref(), Some("a1"));
    }

    #[test]
    fn experiment_lifecycle_transitions() {
        let mut exp = Experiment {
            id: "test".into(),
            status: ExperimentStatus::Created,
            config: ExperimentConfig {
                agent_count: None,
                target_ticks: None,
                llm_model: None,
                llm_temperature: None,
                description: String::new(),
            },
            created_at: Utc::now(),
            started_at: None,
            stopped_at: None,
            start_tick: None,
            end_tick: None,
            injections: Vec::new(),
            tick_snapshots: Vec::new(),
        };

        assert_eq!(exp.status, ExperimentStatus::Created);
        exp.status = ExperimentStatus::Running;
        assert_eq!(exp.status, ExperimentStatus::Running);

        exp.status = ExperimentStatus::Paused;
        assert_eq!(exp.status, ExperimentStatus::Paused);

        exp.status = ExperimentStatus::Running;
        assert_eq!(exp.status, ExperimentStatus::Running);

        exp.status = ExperimentStatus::Stopped;
        assert_eq!(exp.status, ExperimentStatus::Stopped);
    }
}
