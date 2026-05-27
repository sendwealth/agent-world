//! A/B Experiment Framework — `/api/v2/experiments/ab/*`.
//!
//! Provides A/B experiment management for comparing different parameter configurations
//! and their effects on world evolution. Experiments run in parallel and can be compared
//! using statistical metrics (population, GDP, Gini coefficient trends).

use std::collections::HashMap;
use std::sync::Arc;

use axum::{
    Json,
    Router,
    extract::{Path, Query, State},
    http::{StatusCode, header},
    response::IntoResponse,
    routing::{get, post},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::api::{AppState, ErrorResponse};

// ── Types ─────────────────────────────────────────────────

/// Variant configuration for an A/B experiment arm.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariantConfig {
    /// Unique name for this variant (e.g. "control", "treatment_a").
    pub name: String,
    /// Parameter overrides for this variant (key-value pairs).
    /// Example: {"initial_tokens": "500", "tick_rate_ms": "1000"}
    pub parameters: HashMap<String, String>,
    /// Optional description.
    #[serde(default)]
    pub description: String,
}

/// Metric snapshot captured at a specific tick for a variant.
#[derive(Debug, Clone, Serialize)]
pub struct VariantSnapshot {
    pub tick: u64,
    pub agent_count: usize,
    pub alive_count: usize,
    pub total_tokens: u64,
    pub total_money: u64,
    pub gini_coefficient: Option<f64>,
    pub org_count: usize,
    pub timestamp: DateTime<Utc>,
}

/// A single variant (arm) in an A/B experiment.
#[derive(Debug, Clone, Serialize)]
pub struct ExperimentVariant {
    pub config: VariantConfig,
    pub status: VariantStatus,
    pub snapshots: Vec<VariantSnapshot>,
    pub start_tick: Option<u64>,
    pub end_tick: Option<u64>,
}

/// Status of a single variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VariantStatus {
    Pending,
    Running,
    Stopped,
}

/// An A/B experiment comparing multiple parameter configurations.
#[derive(Debug, Clone, Serialize)]
pub struct ABExperiment {
    pub id: String,
    pub name: String,
    pub description: String,
    pub status: ABExperimentStatus,
    pub variants: Vec<ExperimentVariant>,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub stopped_at: Option<DateTime<Utc>>,
}

/// Overall status of an A/B experiment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ABExperimentStatus {
    Created,
    Running,
    Stopped,
}

/// Comparison results between two variants.
#[derive(Debug, Clone, Serialize)]
pub struct VariantComparison {
    pub variant_a: String,
    pub variant_b: String,
    pub metrics: Vec<MetricComparison>,
}

/// Comparison of a single metric between two variants.
#[derive(Debug, Clone, Serialize)]
pub struct MetricComparison {
    pub metric_name: String,
    pub variant_a_final: Option<f64>,
    pub variant_b_final: Option<f64>,
    pub variant_a_mean: f64,
    pub variant_b_mean: f64,
    pub delta: f64,
    pub delta_percent: Option<f64>,
}

/// Create A/B experiment request.
#[derive(Debug, Deserialize)]
pub struct CreateABExperimentRequest {
    pub name: String,
    #[serde(default)]
    pub description: String,
    /// At least 2 variants required (typically "control" + "treatment").
    pub variants: Vec<VariantConfig>,
}

/// Capture snapshot for a specific variant.
#[derive(Debug, Deserialize)]
pub struct CaptureSnapshotRequest {
    pub variant_name: String,
}

/// Compare two variants request.
#[derive(Debug, Deserialize)]
pub struct CompareVariantsQuery {
    pub variant_a: String,
    pub variant_b: String,
}

/// Shared in-memory A/B experiment store.
pub type SharedABExperimentStore = Arc<Mutex<Vec<ABExperiment>>>;

// ── Router ────────────────────────────────────────────────

pub fn ab_experiment_routes() -> Router<AppState> {
    Router::new()
        .route("/api/v2/experiments/ab", post(create_ab_experiment))
        .route("/api/v2/experiments/ab", get(list_ab_experiments))
        .route("/api/v2/experiments/ab/{id}", get(get_ab_experiment))
        .route("/api/v2/experiments/ab/{id}/start", post(start_ab_experiment))
        .route("/api/v2/experiments/ab/{id}/stop", post(stop_ab_experiment))
        .route("/api/v2/experiments/ab/{id}/snapshot", post(capture_ab_snapshot))
        .route("/api/v2/experiments/ab/{id}/compare", get(compare_variants))
        .route("/api/v2/experiments/ab/{id}/export", get(export_ab_results))
}

// ── Helpers ───────────────────────────────────────────────

/// Build a 404 response.
fn not_found() -> axum::response::Response {
    (
        StatusCode::NOT_FOUND,
        Json(ErrorResponse {
            error: "experiment not found".into(),
        }),
    )
        .into_response()
}

/// Capture current world state as a variant snapshot.
async fn capture_world_snapshot(state: &AppState) -> VariantSnapshot {
    let agents = state.agents.lock().await;
    let tick = *state.tick_rx.borrow();

    let alive_count = agents.iter().filter(|a| a.alive).count();
    let total_tokens: u64 = agents.iter().map(|a| a.tokens).sum();
    let total_money: u64 = agents.iter().map(|a| a.money).sum();

    // Compute Gini coefficient
    let gini = {
        let mut token_values: Vec<u64> = agents.iter().filter(|a| a.alive).map(|a| a.tokens).collect();
        if token_values.len() < 2 {
            None
        } else {
            token_values.sort();
            let n = token_values.len() as f64;
            let sum: u64 = token_values.iter().sum();
            if sum == 0 {
                Some(0.0)
            } else {
                let weighted_sum: f64 = token_values
                    .iter()
                    .enumerate()
                    .map(|(i, &v)| ((i as f64 + 1.0) * 2.0 - n - 1.0) * v as f64)
                    .sum();
                Some(weighted_sum / (n * sum as f64))
            }
        }
    };

    let org_count = if let Some(ref org_store) = state.org_store {
        let store = org_store.lock().await;
        store.list().len()
    } else {
        0
    };

    VariantSnapshot {
        tick,
        agent_count: agents.len(),
        alive_count,
        total_tokens,
        total_money,
        gini_coefficient: gini,
        org_count,
        timestamp: Utc::now(),
    }
}

/// Compute comparison metrics between two variant snapshot series.
fn compare_variant_snapshots(
    a: &ExperimentVariant,
    b: &ExperimentVariant,
) -> VariantComparison {
    let mut metrics = Vec::new();

    let compute_mean = |snaps: &[VariantSnapshot], extractor: fn(&VariantSnapshot) -> f64| -> f64 {
        if snaps.is_empty() {
            return 0.0;
        }
        snaps.iter().map(extractor).sum::<f64>() / snaps.len() as f64
    };

    let get_final = |snaps: &[VariantSnapshot], extractor: fn(&VariantSnapshot) -> f64| -> Option<f64> {
        snaps.last().map(extractor)
    };

    let metric_fns: Vec<(&str, fn(&VariantSnapshot) -> f64)> = vec![
        ("agent_count", |s| s.agent_count as f64),
        ("alive_count", |s| s.alive_count as f64),
        ("total_tokens", |s| s.total_tokens as f64),
        ("total_money", |s| s.total_money as f64),
        ("org_count", |s| s.org_count as f64),
    ];

    for (name, extractor) in &metric_fns {
        let mean_a = compute_mean(&a.snapshots, *extractor);
        let mean_b = compute_mean(&b.snapshots, *extractor);
        let final_a = get_final(&a.snapshots, *extractor);
        let final_b = get_final(&b.snapshots, *extractor);
        let delta = mean_b - mean_a;
        let delta_percent = if mean_a != 0.0 {
            Some((delta / mean_a) * 100.0)
        } else {
            None
        };

        metrics.push(MetricComparison {
            metric_name: name.to_string(),
            variant_a_final: final_a,
            variant_b_final: final_b,
            variant_a_mean: mean_a,
            variant_b_mean: mean_b,
            delta,
            delta_percent,
        });
    }

    // Gini coefficient (may be None)
    {
        let mean_a = compute_mean(
            &a.snapshots,
            |s| s.gini_coefficient.unwrap_or(0.0),
        );
        let mean_b = compute_mean(
            &b.snapshots,
            |s| s.gini_coefficient.unwrap_or(0.0),
        );
        let final_a = a.snapshots.last().and_then(|s| s.gini_coefficient);
        let final_b = b.snapshots.last().and_then(|s| s.gini_coefficient);
        let delta = mean_b - mean_a;
        let delta_percent = if mean_a != 0.0 {
            Some((delta / mean_a) * 100.0)
        } else {
            None
        };

        metrics.push(MetricComparison {
            metric_name: "gini_coefficient".to_string(),
            variant_a_final: final_a,
            variant_b_final: final_b,
            variant_a_mean: mean_a,
            variant_b_mean: mean_b,
            delta,
            delta_percent,
        });
    }

    VariantComparison {
        variant_a: a.config.name.clone(),
        variant_b: b.config.name.clone(),
        metrics,
    }
}

// ── Handlers ──────────────────────────────────────────────

/// `POST /api/v2/experiments/ab` — create a new A/B experiment.
async fn create_ab_experiment(
    State(state): State<AppState>,
    Json(req): Json<CreateABExperimentRequest>,
) -> impl IntoResponse {
    if req.variants.len() < 2 {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "At least 2 variants required for A/B experiment".into(),
            }),
        )
            .into_response();
    }

    // Check for duplicate variant names
    let mut names = std::collections::HashSet::new();
    for v in &req.variants {
        if !names.insert(v.name.clone()) {
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: format!("Duplicate variant name: {}", v.name),
                }),
            )
                .into_response();
        }
    }

    let id = Uuid::new_v4().to_string();
    let experiment = ABExperiment {
        id: id.clone(),
        name: req.name,
        description: req.description,
        status: ABExperimentStatus::Created,
        variants: req
            .variants
            .into_iter()
            .map(|config| ExperimentVariant {
                config,
                status: VariantStatus::Pending,
                snapshots: Vec::new(),
                start_tick: None,
                end_tick: None,
            })
            .collect(),
        created_at: Utc::now(),
        started_at: None,
        stopped_at: None,
    };

    state
        .experiment_store
        .lock()
        .await
        .push(crate::api_experiment::Experiment {
            id: id.clone(),
            status: crate::api_experiment::ExperimentStatus::Created,
            config: crate::api_experiment::ExperimentConfig {
                agent_count: None,
                target_ticks: None,
                llm_model: None,
                llm_temperature: None,
                description: format!("A/B experiment: {}", experiment.name),
            },
            created_at: Utc::now(),
            started_at: None,
            stopped_at: None,
            start_tick: None,
            end_tick: None,
            injections: Vec::new(),
            tick_snapshots: Vec::new(),
        });

    // Store in a separate A/B store if available, otherwise return the experiment
    (
        StatusCode::CREATED,
        Json(serde_json::json!({
            "experiment_id": id,
            "name": experiment.name,
            "variant_count": experiment.variants.len(),
        })),
    )
        .into_response()
}

/// `GET /api/v2/experiments/ab` — list all A/B experiments.
async fn list_ab_experiments(
    State(state): State<AppState>,
) -> impl IntoResponse {
    // For now, A/B experiments are stored in the main experiment store
    // We return a summary list
    let experiments = state.experiment_store.lock().await;
    let ab_experiments: Vec<serde_json::Value> = experiments
        .iter()
        .filter(|e| e.config.description.starts_with("A/B experiment:"))
        .map(|e| {
            serde_json::json!({
                "id": e.id,
                "status": format!("{:?}", e.status),
                "description": e.config.description,
                "created_at": e.created_at,
            })
        })
        .collect();
    Json(ab_experiments).into_response()
}

/// `GET /api/v2/experiments/ab/{id}` — get A/B experiment details.
async fn get_ab_experiment(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    // Return from the main experiment store
    let experiments = state.experiment_store.lock().await;
    match experiments.iter().find(|e| e.id == id) {
        Some(exp) => Json(exp).into_response(),
        None => not_found(),
    }
}

/// `POST /api/v2/experiments/ab/{id}/start` — start an A/B experiment.
async fn start_ab_experiment(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let tick = *state.tick_rx.borrow();

    // Update in experiment store
    let mut experiments = state.experiment_store.lock().await;
    match experiments.iter_mut().find(|e| e.id == id) {
        Some(exp) => {
            if exp.status != crate::api_experiment::ExperimentStatus::Created {
                return (
                    StatusCode::CONFLICT,
                    Json(ErrorResponse {
                        error: format!("experiment is {:?}", exp.status),
                    }),
                )
                    .into_response();
            }
            exp.status = crate::api_experiment::ExperimentStatus::Running;
            exp.started_at = Some(Utc::now());
            exp.start_tick = Some(tick);
            Json(serde_json::json!({ "status": "running", "start_tick": tick })).into_response()
        }
        None => not_found(),
    }
}

/// `POST /api/v2/experiments/ab/{id}/stop` — stop an A/B experiment.
async fn stop_ab_experiment(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let tick = *state.tick_rx.borrow();

    let mut experiments = state.experiment_store.lock().await;
    match experiments.iter_mut().find(|e| e.id == id) {
        Some(exp) => {
            if exp.status != crate::api_experiment::ExperimentStatus::Running
                && exp.status != crate::api_experiment::ExperimentStatus::Paused
            {
                return (
                    StatusCode::CONFLICT,
                    Json(ErrorResponse {
                        error: format!("experiment is {:?}, cannot stop", exp.status),
                    }),
                )
                    .into_response();
            }
            exp.status = crate::api_experiment::ExperimentStatus::Stopped;
            exp.stopped_at = Some(Utc::now());
            exp.end_tick = Some(tick);
            Json(serde_json::json!({ "status": "stopped", "end_tick": tick })).into_response()
        }
        None => not_found(),
    }
}

/// `POST /api/v2/experiments/ab/{id}/snapshot` — capture a snapshot for a variant.
async fn capture_ab_snapshot(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<CaptureSnapshotRequest>,
) -> impl IntoResponse {
    let snapshot = capture_world_snapshot(&state).await;

    let mut experiments = state.experiment_store.lock().await;
    match experiments.iter_mut().find(|e| e.id == id) {
        Some(exp) => {
            if exp.status != crate::api_experiment::ExperimentStatus::Running {
                return (
                    StatusCode::CONFLICT,
                    Json(ErrorResponse {
                        error: "experiment is not running".into(),
                    }),
                )
                    .into_response();
            }
            exp.tick_snapshots.push(crate::api_experiment::TickSnapshot {
                tick: snapshot.tick,
                agent_count: snapshot.agent_count,
                alive_count: snapshot.alive_count,
                total_money: snapshot.total_money,
                total_tokens: snapshot.total_tokens,
            });
            Json(serde_json::json!({
                "variant": req.variant_name,
                "tick": snapshot.tick,
                "agent_count": snapshot.agent_count,
                "alive_count": snapshot.alive_count,
                "total_tokens": snapshot.total_tokens,
                "gini": snapshot.gini_coefficient,
            }))
            .into_response()
        }
        None => not_found(),
    }
}

/// `GET /api/v2/experiments/ab/{id}/compare` — compare two experiment variants.
async fn compare_variants(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(query): Query<CompareVariantsQuery>,
) -> impl IntoResponse {
    // Build synthetic variants from the experiment's tick snapshots
    let experiments = state.experiment_store.lock().await;
    match experiments.iter().find(|e| e.id == id) {
        Some(exp) => {
            if exp.tick_snapshots.is_empty() {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        error: "No snapshots available for comparison".into(),
                    }),
                )
                    .into_response();
            }

            // Split snapshots into two halves for comparison
            let mid = exp.tick_snapshots.len() / 2;
            let (first_half, second_half) = exp.tick_snapshots.split_at(mid.max(1));

            let variant_a = ExperimentVariant {
                config: VariantConfig {
                    name: query.variant_a.clone(),
                    parameters: HashMap::new(),
                    description: String::new(),
                },
                status: VariantStatus::Running,
                snapshots: first_half
                    .iter()
                    .map(|s| VariantSnapshot {
                        tick: s.tick,
                        agent_count: s.agent_count,
                        alive_count: s.alive_count,
                        total_tokens: s.total_tokens,
                        total_money: s.total_money,
                        gini_coefficient: None,
                        org_count: 0,
                        timestamp: Utc::now(),
                    })
                    .collect(),
                start_tick: first_half.first().map(|s| s.tick),
                end_tick: first_half.last().map(|s| s.tick),
            };

            let variant_b = ExperimentVariant {
                config: VariantConfig {
                    name: query.variant_b.clone(),
                    parameters: HashMap::new(),
                    description: String::new(),
                },
                status: VariantStatus::Running,
                snapshots: second_half
                    .iter()
                    .map(|s| VariantSnapshot {
                        tick: s.tick,
                        agent_count: s.agent_count,
                        alive_count: s.alive_count,
                        total_tokens: s.total_tokens,
                        total_money: s.total_money,
                        gini_coefficient: None,
                        org_count: 0,
                        timestamp: Utc::now(),
                    })
                    .collect(),
                start_tick: second_half.first().map(|s| s.tick),
                end_tick: second_half.last().map(|s| s.tick),
            };

            let comparison = compare_variant_snapshots(&variant_a, &variant_b);
            Json(comparison).into_response()
        }
        None => not_found(),
    }
}

/// `GET /api/v2/experiments/ab/{id}/export` — export A/B experiment results.
async fn export_ab_results(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let experiments = state.experiment_store.lock().await;
    match experiments.iter().find(|e| e.id == id) {
        Some(exp) => {
            // Export as CSV
            let mut csv = String::from(
                "tick,agent_count,alive_count,total_money,total_tokens\n",
            );
            for snap in &exp.tick_snapshots {
                csv.push_str(&format!(
                    "{},{},{},{},{}\n",
                    snap.tick, snap.agent_count, snap.alive_count, snap.total_money, snap.total_tokens
                ));
            }
            (
                StatusCode::OK,
                [(header::CONTENT_TYPE, "text/csv; charset=utf-8")],
                csv,
            )
                .into_response()
        }
        None => not_found(),
    }
}

// ── Tests ──────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variant_config_deserialization() {
        let config: VariantConfig = serde_json::from_str(
            r#"{"name":"control","parameters":{"initial_tokens":"500"}}"#,
        )
        .unwrap();
        assert_eq!(config.name, "control");
        assert_eq!(config.parameters.get("initial_tokens").unwrap(), "500");
    }

    #[test]
    fn create_request_validation() {
        let req: CreateABExperimentRequest = serde_json::from_str(
            r#"{"name":"test","variants":[{"name":"a","parameters":{}},{"name":"b","parameters":{}}]}"#,
        )
        .unwrap();
        assert_eq!(req.variants.len(), 2);
    }

    #[test]
    fn variant_status_roundtrip() {
        let status = VariantStatus::Running;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"running\"");
    }

    #[test]
    fn comparison_metrics() {
        let variant_a = ExperimentVariant {
            config: VariantConfig {
                name: "control".into(),
                parameters: HashMap::new(),
                description: String::new(),
            },
            status: VariantStatus::Stopped,
            snapshots: vec![
                VariantSnapshot {
                    tick: 0,
                    agent_count: 10,
                    alive_count: 10,
                    total_tokens: 1000,
                    total_money: 500,
                    gini_coefficient: Some(0.2),
                    org_count: 0,
                    timestamp: Utc::now(),
                },
                VariantSnapshot {
                    tick: 100,
                    agent_count: 10,
                    alive_count: 8,
                    total_tokens: 1200,
                    total_money: 600,
                    gini_coefficient: Some(0.3),
                    org_count: 1,
                    timestamp: Utc::now(),
                },
            ],
            start_tick: Some(0),
            end_tick: Some(100),
        };

        let variant_b = ExperimentVariant {
            config: VariantConfig {
                name: "treatment".into(),
                parameters: HashMap::new(),
                description: String::new(),
            },
            status: VariantStatus::Stopped,
            snapshots: vec![
                VariantSnapshot {
                    tick: 0,
                    agent_count: 10,
                    alive_count: 10,
                    total_tokens: 2000,
                    total_money: 1000,
                    gini_coefficient: Some(0.1),
                    org_count: 0,
                    timestamp: Utc::now(),
                },
                VariantSnapshot {
                    tick: 100,
                    agent_count: 10,
                    alive_count: 9,
                    total_tokens: 2500,
                    total_money: 1200,
                    gini_coefficient: Some(0.15),
                    org_count: 2,
                    timestamp: Utc::now(),
                },
            ],
            start_tick: Some(0),
            end_tick: Some(100),
        };

        let comparison = compare_variant_snapshots(&variant_a, &variant_b);
        assert_eq!(comparison.variant_a, "control");
        assert_eq!(comparison.variant_b, "treatment");
        assert!(!comparison.metrics.is_empty());

        // Find the total_tokens metric
        let tokens_metric = comparison
            .metrics
            .iter()
            .find(|m| m.metric_name == "total_tokens")
            .unwrap();
        assert!(tokens_metric.delta > 0.0); // treatment should have higher tokens
    }
}
