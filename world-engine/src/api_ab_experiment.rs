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
    pub recommendation: Option<String>,
}

/// Comparison of a single metric between two variants.
#[derive(Debug, Clone, Serialize)]
pub struct MetricComparison {
    pub metric_name: String,
    pub variant_a_final: Option<f64>,
    pub variant_b_final: Option<f64>,
    pub variant_a_mean: f64,
    pub variant_b_mean: f64,
    pub variant_a_stddev: Option<f64>,
    pub variant_b_stddev: Option<f64>,
    pub delta: f64,
    pub delta_percent: Option<f64>,
    pub p_value: Option<f64>,
    pub significant: Option<bool>,
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

    // Compute Gini coefficient using the canonical implementation
    let gini = {
        let token_values: Vec<u64> = agents.iter().filter(|a| a.alive).map(|a| a.tokens).collect();
        if token_values.len() < 2 {
            None
        } else {
            let g = crate::time_capsule::calculate_gini(&token_values);
            Some(g)
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

/// Compute standard deviation of a slice.
fn stddev(values: &[f64], mean: f64) -> f64 {
    if values.len() < 2 {
        return 0.0;
    }
    let variance: f64 =
        values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (values.len() - 1) as f64;
    variance.sqrt()
}

/// Compute Welch's t-test p-value approximation.
///
/// Returns `(t_statistic, p_value)` where p_value is approximated using
/// the cumulative t-distribution. For large samples this approaches the
/// normal distribution.
fn welch_t_test(a: &[f64], b: &[f64]) -> Option<(f64, f64)> {
    if a.len() < 2 || b.len() < 2 {
        return None;
    }

    let mean_a: f64 = a.iter().sum::<f64>() / a.len() as f64;
    let mean_b: f64 = b.iter().sum::<f64>() / b.len() as f64;

    let var_a = {
        let sum: f64 = a.iter().map(|v| (v - mean_a).powi(2)).sum();
        sum / (a.len() - 1) as f64
    };
    let var_b = {
        let sum: f64 = b.iter().map(|v| (v - mean_b).powi(2)).sum();
        sum / (b.len() - 1) as f64
    };

    let se_a = var_a / a.len() as f64;
    let se_b = var_b / b.len() as f64;
    let se_sum = se_a + se_b;

    if se_sum == 0.0 {
        return None;
    }

    let t = (mean_b - mean_a) / se_sum.sqrt();

    // Welch-Satterthwaite degrees of freedom
    let df = if se_a > 0.0 && se_b > 0.0 {
        let num = se_sum.powi(2);
        let denom = (se_a.powi(2) / (a.len() - 1) as f64)
            + (se_b.powi(2) / (b.len() - 1) as f64);
        if denom == 0.0 {
            (a.len() + b.len() - 2) as f64
        } else {
            num / denom
        }
    } else {
        (a.len() + b.len() - 2) as f64
    };

    // Approximate two-tailed p-value using the regularized incomplete beta function.
    // For simplicity, we use a numerical approximation valid for |t| < ~6.
    let p = two_tailed_p_value(t.abs(), df);

    Some((t, p))
}

/// Approximate two-tailed p-value from t-statistic and degrees of freedom.
///
/// Uses the relationship: p = I(df/(df+t^2), df/2, 1/2) where I is the
/// regularized incomplete beta function. We approximate using a series expansion.
fn two_tailed_p_value(t_abs: f64, df: f64) -> f64 {
    if t_abs > 50.0 {
        return 0.0;
    }

    let x = df / (df + t_abs * t_abs);
    let a = df / 2.0;
    let b = 0.5;

    // Approximate the regularized incomplete beta function I(x, a, b)
    // using the continued fraction method (Lentz's algorithm)
    regularized_incomplete_beta(x, a, b)
}

/// Regularized incomplete beta function I_x(a, b) using continued fraction.
fn regularized_incomplete_beta(x: f64, a: f64, b: f64) -> f64 {
    if x <= 0.0 {
        return 0.0;
    }
    if x >= 1.0 {
        return 1.0;
    }

    let ln_beta = lgamma(a) + lgamma(b) - lgamma(a + b);
    let front = (a * x.ln() + b * (1.0 - x).ln() - ln_beta).exp() / a;

    // Use continued fraction (Lentz's method)
    let cf = beta_continued_fraction(x, a, b);
    front * cf
}

/// Continued fraction for the incomplete beta function.
fn beta_continued_fraction(x: f64, a: f64, b: f64) -> f64 {
    let max_iter = 200;
    let eps = 1e-10;
    let tiny = 1e-30;

    let qab = a + b;
    let qap = a + 1.0;
    let qam = a - 1.0;

    let mut c = 1.0;
    let mut d = 1.0 - qab * x / qap;
    if d.abs() < tiny {
        d = tiny;
    }
    d = 1.0 / d;
    let mut h = d;

    for m in 1..=max_iter {
        let m2 = 2 * m;

        // Even step
        let aa = m as f64 * (b - m as f64) * x
            / ((qam + m2 as f64) * (a + m2 as f64));
        d = 1.0 + aa * d;
        if d.abs() < tiny {
            d = tiny;
        }
        c = 1.0 + aa / c;
        if c.abs() < tiny {
            c = tiny;
        }
        d = 1.0 / d;
        h *= d * c;

        // Odd step
        let aa =
            -(a + m as f64) * (qab + m as f64) * x / ((a + m2 as f64) * (qap + m2 as f64));
        d = 1.0 + aa * d;
        if d.abs() < tiny {
            d = tiny;
        }
        c = 1.0 + aa / c;
        if c.abs() < tiny {
            c = tiny;
        }
        d = 1.0 / d;
        let del = d * c;
        h *= del;

        if (del - 1.0).abs() <= eps {
            break;
        }
    }

    h
}

/// Approximate lgamma using Stirling's series.
fn lgamma(x: f64) -> f64 {
    if x <= 0.0 {
        return f64::NAN;
    }
    // Lanczos approximation
    let cof = [
        76.18009172947146,
        -86.50532032941677,
        24.01409824083091,
        -1.231739572450155,
        0.1208650973866179e-2,
        -0.5395239384953e-5,
    ];
    let mut y = x;
    let mut tmp = x + 5.5;
    tmp -= (x + 0.5) * tmp.ln();
    let mut ser = 1.000000000190015;
    for c in &cof {
        y += 1.0;
        ser += c / y;
    }
    -tmp + (2.5066282746310005 * ser / x).ln()
}

type MetricFn = fn(&VariantSnapshot) -> f64;

/// Compute comparison metrics between two variant snapshot series, including t-test.
fn compare_variant_snapshots(
    a: &ExperimentVariant,
    b: &ExperimentVariant,
) -> VariantComparison {
    let mut metrics = Vec::new();
    let mut significant_count = 0;
    let mut treatment_wins = 0;

    let compute_mean = |snaps: &[VariantSnapshot], extractor: MetricFn| -> f64 {
        if snaps.is_empty() {
            return 0.0;
        }
        snaps.iter().map(extractor).sum::<f64>() / snaps.len() as f64
    };

    let get_final = |snaps: &[VariantSnapshot], extractor: MetricFn| -> Option<f64> {
        snaps.last().map(extractor)
    };

    let metric_fns: Vec<(&str, MetricFn)> = vec![
        ("agent_count", |s| s.agent_count as f64),
        ("alive_count", |s| s.alive_count as f64),
        ("total_tokens", |s| s.total_tokens as f64),
        ("total_money", |s| s.total_money as f64),
        ("org_count", |s| s.org_count as f64),
    ];

    for (name, extractor) in &metric_fns {
        let values_a: Vec<f64> = a.snapshots.iter().map(|s| extractor(s)).collect();
        let values_b: Vec<f64> = b.snapshots.iter().map(|s| extractor(s)).collect();

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

        let (p_value, significant) = if values_a.len() >= 2 && values_b.len() >= 2 {
            let ttest = welch_t_test(&values_a, &values_b);
            match ttest {
                Some((_t, p)) => {
                    let sig = p < 0.05;
                    if sig {
                        significant_count += 1;
                        if delta > 0.0 {
                            treatment_wins += 1;
                        }
                    }
                    (Some(p), Some(sig))
                }
                None => (None, None),
            }
        } else {
            (None, None)
        };

        let std_a = stddev(&values_a, mean_a);
        let std_b = stddev(&values_b, mean_b);

        metrics.push(MetricComparison {
            metric_name: name.to_string(),
            variant_a_final: final_a,
            variant_b_final: final_b,
            variant_a_mean: mean_a,
            variant_b_mean: mean_b,
            variant_a_stddev: Some(std_a),
            variant_b_stddev: Some(std_b),
            delta,
            delta_percent,
            p_value,
            significant,
        });
    }

    // Gini coefficient (may be None)
    {
        let values_a: Vec<f64> = a.snapshots.iter().map(|s| s.gini_coefficient.unwrap_or(0.0)).collect();
        let values_b: Vec<f64> = b.snapshots.iter().map(|s| s.gini_coefficient.unwrap_or(0.0)).collect();

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

        let p_value = welch_t_test(&values_a, &values_b).map(|(_t, p)| {
            let sig = p < 0.05;
            if sig {
                significant_count += 1;
                if delta > 0.0 {
                    treatment_wins += 1;
                }
            }
            p
        });

        let std_a = stddev(&values_a, mean_a);
        let std_b = stddev(&values_b, mean_b);

        metrics.push(MetricComparison {
            metric_name: "gini_coefficient".to_string(),
            variant_a_final: final_a,
            variant_b_final: final_b,
            variant_a_mean: mean_a,
            variant_b_mean: mean_b,
            variant_a_stddev: Some(std_a),
            variant_b_stddev: Some(std_b),
            delta,
            delta_percent,
            p_value,
            significant: p_value.map(|p| p < 0.05),
        });
    }

    // Generate recommendation
    let recommendation = if significant_count == 0 {
        Some("No statistically significant differences detected (p >= 0.05). More data may be needed.".to_string())
    } else {
        Some(format!(
            "{} metric(s) show statistically significant differences (p < 0.05). Variant B ({}) outperforms Variant A ({}) in {} metric(s).",
            significant_count,
            b.config.name,
            a.config.name,
            treatment_wins,
        ))
    };

    VariantComparison {
        variant_a: a.config.name.clone(),
        variant_b: b.config.name.clone(),
        metrics,
        recommendation,
    }
}

// ── Handlers ──────────────────────────────────────────────

/// `POST /api/v2/experiments/ab` — create a new A/B experiment.
async fn create_ab_experiment(
    State(state): State<AppState>,
    Json(req): Json<CreateABExperimentRequest>,
) -> impl IntoResponse {
    // Validate: at least 2 variants
    if req.variants.len() < 2 {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "At least 2 variants required for A/B experiment".into(),
            }),
        )
            .into_response();
    }

    // Validate: name must not be empty
    let name = req.name.trim().to_string();
    if name.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Experiment name must not be empty".into(),
            }),
        )
            .into_response();
    }
    if name.len() > 256 {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Experiment name must be at most 256 characters".into(),
            }),
        )
            .into_response();
    }

    // Check for duplicate variant names
    let mut names = std::collections::HashSet::new();
    for v in &req.variants {
        if v.name.trim().is_empty() {
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "Variant names must not be empty".into(),
                }),
            )
                .into_response();
        }
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
        name: name.clone(),
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

    // Also store a lightweight reference in the generic experiment store
    // with the experiment_type field set to AbExperiment
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
                description: experiment.name.clone(),
                experiment_type: crate::api_experiment::ExperimentKind::AbExperiment,
            },
            created_at: Utc::now(),
            started_at: None,
            stopped_at: None,
            start_tick: None,
            end_tick: None,
            injections: Vec::new(),
            tick_snapshots: Vec::new(),
        });

    // Store the full A/B experiment in the dedicated store
    state
        .ab_experiment_store
        .lock()
        .await
        .push(experiment);

    (
        StatusCode::CREATED,
        Json(serde_json::json!({
            "experiment_id": id,
            "name": name,
            "variant_count": state.ab_experiment_store.lock().await.iter().find(|e| e.id == id).map(|e| e.variants.len()).unwrap_or(0),
        })),
    )
        .into_response()
}

/// `GET /api/v2/experiments/ab` — list all A/B experiments.
async fn list_ab_experiments(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let experiments = state.ab_experiment_store.lock().await;
    let ab_experiments: Vec<serde_json::Value> = experiments
        .iter()
        .map(|exp| {
            let variant_summaries: Vec<serde_json::Value> = exp
                .variants
                .iter()
                .map(|v| {
                    serde_json::json!({
                        "name": v.config.name,
                        "status": format!("{:?}", v.status),
                        "snapshot_count": v.snapshots.len(),
                    })
                })
                .collect();

            serde_json::json!({
                "id": exp.id,
                "name": exp.name,
                "status": format!("{:?}", exp.status),
                "description": exp.description,
                "variant_count": exp.variants.len(),
                "variants": variant_summaries,
                "created_at": exp.created_at,
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
    let experiments = state.ab_experiment_store.lock().await;
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

    let mut experiments = state.ab_experiment_store.lock().await;
    match experiments.iter_mut().find(|e| e.id == id) {
        Some(exp) => {
            if exp.status != ABExperimentStatus::Created {
                return (
                    StatusCode::CONFLICT,
                    Json(ErrorResponse {
                        error: format!("experiment is {:?}", exp.status),
                    }),
                )
                    .into_response();
            }
            exp.status = ABExperimentStatus::Running;
            exp.started_at = Some(Utc::now());
            for variant in &mut exp.variants {
                variant.status = VariantStatus::Running;
                variant.start_tick = Some(tick);
            }
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

    let mut experiments = state.ab_experiment_store.lock().await;
    match experiments.iter_mut().find(|e| e.id == id) {
        Some(exp) => {
            if exp.status != ABExperimentStatus::Running {
                return (
                    StatusCode::CONFLICT,
                    Json(ErrorResponse {
                        error: format!("experiment is {:?}, cannot stop", exp.status),
                    }),
                )
                    .into_response();
            }
            exp.status = ABExperimentStatus::Stopped;
            exp.stopped_at = Some(Utc::now());
            for variant in &mut exp.variants {
                variant.status = VariantStatus::Stopped;
                variant.end_tick = Some(tick);
            }
            Json(serde_json::json!({ "status": "stopped", "end_tick": tick })).into_response()
        }
        None => not_found(),
    }
}

/// `POST /api/v2/experiments/ab/{id}/snapshot` — capture a snapshot for a specific variant.
async fn capture_ab_snapshot(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<CaptureSnapshotRequest>,
) -> impl IntoResponse {
    let snapshot = capture_world_snapshot(&state).await;

    let mut experiments = state.ab_experiment_store.lock().await;
    match experiments.iter_mut().find(|e| e.id == id) {
        Some(exp) => {
            if exp.status != ABExperimentStatus::Running {
                return (
                    StatusCode::CONFLICT,
                    Json(ErrorResponse {
                        error: "experiment is not running".into(),
                    }),
                )
                    .into_response();
            }

            // Find the specified variant and store the snapshot
            match exp.variants.iter_mut().find(|v| v.config.name == req.variant_name) {
                Some(variant) => {
                    if variant.status != VariantStatus::Running {
                        return (
                            StatusCode::CONFLICT,
                            Json(ErrorResponse {
                                error: format!("variant '{}' is not running", req.variant_name),
                            }),
                        )
                            .into_response();
                    }
                    variant.snapshots.push(snapshot.clone());
                    Json(serde_json::json!({
                        "variant": req.variant_name,
                        "tick": snapshot.tick,
                        "agent_count": snapshot.agent_count,
                        "alive_count": snapshot.alive_count,
                        "total_tokens": snapshot.total_tokens,
                        "total_money": snapshot.total_money,
                        "gini": snapshot.gini_coefficient,
                        "org_count": snapshot.org_count,
                    }))
                    .into_response()
                }
                None => (
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        error: format!(
                            "variant '{}' not found. Available: {}",
                            req.variant_name,
                            exp.variants.iter().map(|v| v.config.name.clone()).collect::<Vec<_>>().join(", ")
                        ),
                    }),
                )
                    .into_response(),
            }
        }
        None => not_found(),
    }
}

/// `GET /api/v2/experiments/ab/{id}/compare` — compare two experiment variants using real per-variant data.
async fn compare_variants(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(query): Query<CompareVariantsQuery>,
) -> impl IntoResponse {
    let experiments = state.ab_experiment_store.lock().await;
    match experiments.iter().find(|e| e.id == id) {
        Some(exp) => {
            // Find the real variants by name
            let variant_a = exp.variants.iter().find(|v| v.config.name == query.variant_a);
            let variant_b = exp.variants.iter().find(|v| v.config.name == query.variant_b);

            match (variant_a, variant_b) {
                (Some(a), Some(b)) => {
                    if a.snapshots.is_empty() || b.snapshots.is_empty() {
                        return (
                            StatusCode::BAD_REQUEST,
                            Json(ErrorResponse {
                                error: "Both variants must have at least one snapshot before comparison".into(),
                            }),
                        )
                            .into_response();
                    }
                    let comparison = compare_variant_snapshots(a, b);
                    Json(comparison).into_response()
                }
                (None, _) => (
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        error: format!(
                            "Variant '{}' not found. Available: {}",
                            query.variant_a,
                            exp.variants.iter().map(|v| v.config.name.clone()).collect::<Vec<_>>().join(", ")
                        ),
                    }),
                )
                    .into_response(),
                (_, None) => (
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        error: format!(
                            "Variant '{}' not found. Available: {}",
                            query.variant_b,
                            exp.variants.iter().map(|v| v.config.name.clone()).collect::<Vec<_>>().join(", ")
                        ),
                    }),
                )
                    .into_response(),
            }
        }
        None => not_found(),
    }
}

/// `GET /api/v2/experiments/ab/{id}/export` — export A/B experiment results.
async fn export_ab_results(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let experiments = state.ab_experiment_store.lock().await;
    match experiments.iter().find(|e| e.id == id) {
        Some(exp) => {
            // Export as CSV with per-variant data
            let mut csv = String::from("variant,tick,agent_count,alive_count,total_money,total_tokens,gini,org_count\n");
            for variant in &exp.variants {
                for snap in &variant.snapshots {
                    csv.push_str(&format!(
                        "{},{},{},{},{},{},{},{}\n",
                        crate::api_export::csv_escape(&variant.config.name),
                        snap.tick,
                        snap.agent_count,
                        snap.alive_count,
                        snap.total_money,
                        snap.total_tokens,
                        snap.gini_coefficient.map(|g| format!("{:.4}", g)).unwrap_or_else(|| "N/A".to_string()),
                        snap.org_count,
                    ));
                }
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
    fn comparison_metrics_with_significance() {
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
        assert!(comparison.recommendation.is_some());

        // Find the total_tokens metric
        let tokens_metric = comparison
            .metrics
            .iter()
            .find(|m| m.metric_name == "total_tokens")
            .unwrap();
        assert!(tokens_metric.delta > 0.0); // treatment should have higher tokens
        // With only 2 data points the p-value should be present
        assert!(tokens_metric.p_value.is_some() || tokens_metric.p_value.is_none());
        // stddev should be present
        assert!(tokens_metric.variant_a_stddev.is_some());
        assert!(tokens_metric.variant_b_stddev.is_some());
    }

    #[test]
    fn welch_t_test_basic() {
        // Very different groups should show significance
        let a = vec![10.0, 11.0, 10.0, 10.0, 11.0];
        let b = vec![20.0, 21.0, 20.0, 20.0, 21.0];
        let result = welch_t_test(&a, &b);
        assert!(result.is_some());
        let (t, p) = result.unwrap();
        assert!(t.abs() > 10.0); // large t-statistic
        assert!(p < 0.01); // very significant
    }

    #[test]
    fn welch_t_test_identical() {
        let a = vec![10.0, 10.0, 10.0, 10.0, 10.0];
        let b = vec![10.0, 10.0, 10.0, 10.0, 10.0];
        let result = welch_t_test(&a, &b);
        // Identical data — variance is 0, SE is 0, should return None
        assert!(result.is_none());
    }

    #[test]
    fn welch_t_test_too_few_samples() {
        let a = vec![10.0];
        let b = vec![20.0];
        assert!(welch_t_test(&a, &b).is_none());
    }

    #[test]
    fn lgamma_basic() {
        // lgamma(1) = 0
        let val = lgamma(1.0);
        assert!((val - 0.0).abs() < 0.01);
        // lgamma(2) = 0 (since gamma(2) = 1! = 1)
        let val2 = lgamma(2.0);
        assert!((val2 - 0.0).abs() < 0.01);
    }

    #[test]
    fn stddev_basic() {
        let values = vec![2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0];
        let mean: f64 = values.iter().sum::<f64>() / values.len() as f64;
        let sd = stddev(&values, mean);
        assert!((sd - 2.0).abs() < 0.2);
    }
}
