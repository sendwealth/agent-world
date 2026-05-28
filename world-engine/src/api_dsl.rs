use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::*,
};
use serde::{Deserialize, Serialize};

use crate::api::{AppState, ErrorResponse};

pub fn default_dsl_format() -> String { "yaml".to_string() }

#[derive(Debug, Deserialize)]
struct DslParseRequest {
    /// Rule document in YAML or JSON format.
    document: String,
    /// Format hint: "yaml" (default) or "json".
    #[serde(default = "default_dsl_format")]
    format: String,
}

#[derive(Debug, Serialize)]
struct DslParseResponse {
    valid: bool,
    rule: Option<crate::dsl::DslRule>,
    errors: Vec<String>,
    warnings: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct DslSubmitRequest {
    /// Agent proposing the rule.
    proposer_id: String,
    /// Organization ID (required for organization-scoped rules).
    #[serde(default)]
    org_id: String,
    /// Rule document in YAML or JSON format.
    document: String,
    /// Format hint: "yaml" (default) or "json".
    #[serde(default = "default_dsl_format")]
    format: String,
}

#[derive(Debug, Serialize)]
struct DslSubmitResponse {
    rule_id: String,
    status: String,
    title: String,
    message: String,
}

#[derive(Debug, Serialize)]
struct DslTemplateListResponse {
    templates: Vec<DslTemplateEntry>,
}

#[derive(Debug, Serialize)]
struct DslTemplateEntry {
    name: String,
    description: String,
    category: String,
}

#[derive(Debug, Serialize)]
struct DslTemplateDetailResponse {
    name: String,
    description: String,
    category: String,
    yaml: String,
    parsed: Option<crate::dsl::DslRule>,
}

#[derive(Debug, Serialize)]
struct DslRuleResponse {
    id: String,
    proposer_id: String,
    org_id: String,
    title: String,
    description: String,
    rule_type: String,
    status: String,
    conditions: Vec<crate::organization::rule_engine::RuleCondition>,
    effects: Vec<crate::organization::rule_engine::RuleEffect>,
    votes_for: u32,
    votes_against: u32,
    created_tick: u64,
    expires_tick: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct DslVoteRequest {
    voter_id: String,
    support: bool,
}

pub fn dsl_rule_to_response(rule: &crate::organization::rule_engine::SoftRule) -> DslRuleResponse {
    DslRuleResponse {
        id: rule.id.clone(),
        proposer_id: rule.proposer_id.clone(),
        org_id: rule.org_id.clone(),
        title: rule.title.clone(),
        description: rule.description.clone(),
        rule_type: rule.rule_type.to_string(),
        status: rule.status.to_string(),
        conditions: rule.conditions.clone(),
        effects: rule.effects.clone(),
        votes_for: rule.votes_for,
        votes_against: rule.votes_against,
        created_tick: rule.created_tick,
        expires_tick: rule.expires_tick,
    }
}

pub fn rule_engine_error_status(e: &crate::organization::rule_engine::RuleEngineError) -> StatusCode {
    use crate::organization::rule_engine::RuleEngineError;
    match e {
        RuleEngineError::NotFound(_) => StatusCode::NOT_FOUND,
        RuleEngineError::AlreadyActive(_) => StatusCode::CONFLICT,
        RuleEngineError::NotProposed(_) => StatusCode::CONFLICT,
        RuleEngineError::AlreadyVoted { .. } => StatusCode::CONFLICT,
        RuleEngineError::Expired(_) => StatusCode::GONE,
        RuleEngineError::Repealed(_) => StatusCode::GONE,
    }
}

// ── DSL Handlers ───────────────────────────────────────────

/// POST /api/v1/rules/dsl/parse — Parse and validate a DSL rule document.
pub async fn dsl_parse_rule(
    State(_state): State<AppState>,
    Json(body): Json<DslParseRequest>,
) -> impl IntoResponse {
    let result = if body.format == "json" {
        crate::dsl::parse_json(&body.document)
    } else {
        crate::dsl::parse_yaml(&body.document)
    };

    let status = if result.valid { StatusCode::OK } else { StatusCode::UNPROCESSABLE_ENTITY };
    let response = DslParseResponse {
        valid: result.valid,
        rule: result.rule,
        errors: result.errors,
        warnings: result.warnings,
    };
    (status, Json(response)).into_response()
}

/// POST /api/v1/rules/dsl/submit — Parse, validate, and submit a DSL rule into the legislation flow.
///
/// This is the key endpoint: Agent submits a DSL rule → it gets parsed → validated →
/// converted to SoftRule conditions/effects → injected into the RuleEngine as a proposed rule.
pub async fn dsl_submit_rule(
    State(state): State<AppState>,
    Json(body): Json<DslSubmitRequest>,
) -> impl IntoResponse {
    let rule_engine = match &state.rule_engine {
        Some(re) => re.clone(),
        None => return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorResponse { error: "rule engine not configured".to_string() }),
        ).into_response(),
    };

    // 1. Parse the DSL document
    let parse_result = if body.format == "json" {
        crate::dsl::parse_json(&body.document)
    } else {
        crate::dsl::parse_yaml(&body.document)
    };

    if !parse_result.valid {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(ErrorResponse {
                error: format!("DSL validation failed: {}", parse_result.errors.join("; ")),
            }),
        ).into_response();
    }

    let dsl_rule = match parse_result.rule {
        Some(r) => r,
        None => return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(ErrorResponse { error: "DSL parse returned no rule".to_string() }),
        ).into_response(),
    };

    // 2. Determine org_id
    let org_id = if body.org_id.is_empty() {
        dsl_rule.org_id.clone().unwrap_or_default()
    } else {
        body.org_id.clone()
    };

    // 3. Convert DSL → SoftRule types
    let conditions = crate::dsl::to_rule_conditions(&dsl_rule.conditions);
    let effects = crate::dsl::to_rule_effects(&dsl_rule.actions);
    let rule_type = crate::dsl::to_rule_type(&dsl_rule.category);
    let tick = *state.tick_rx.borrow();

    // 4. Compute expires_tick from ttl_ticks
    let expires_tick = dsl_rule.ttl_ticks.map(|ttl| tick + ttl);

    // 5. Inject into RuleEngine as proposed rule
    let mut engine = rule_engine.lock().await;
    let rule_id = engine.propose_rule(
        body.proposer_id.clone(),
        org_id.clone(),
        dsl_rule.name.clone(),
        format!("DSL rule {} (scope: {}, trigger: {})", dsl_rule.id, dsl_rule.scope, dsl_rule.trigger.event),
        rule_type,
        conditions,
        effects,
        tick,
        expires_tick,
    );

    let response = DslSubmitResponse {
        rule_id: rule_id.clone(),
        status: "proposed".to_string(),
        title: dsl_rule.name,
        message: "Rule submitted to legislation flow. Use vote and activate endpoints to advance.".to_string(),
    };

    (StatusCode::CREATED, Json(response)).into_response()
}

/// GET /api/v1/rules/dsl/templates — List built-in rule templates.
pub async fn dsl_list_templates() -> impl IntoResponse {
    let templates = crate::dsl::builtin_templates()
        .into_iter()
        .map(|t| DslTemplateEntry {
            name: t.name,
            description: t.description,
            category: t.category,
        })
        .collect();
    Json(DslTemplateListResponse { templates })
}

/// GET /api/v1/rules/dsl/templates/:name — Get a specific template with its parsed rule.
pub async fn dsl_get_template(
    Path(name): Path<String>,
) -> impl IntoResponse {
    match crate::dsl::get_template(&name) {
        Some(t) => {
            let parsed = crate::dsl::parse_yaml(&t.yaml).rule;
            Json(DslTemplateDetailResponse {
                name: t.name,
                description: t.description,
                category: t.category,
                yaml: t.yaml,
                parsed,
            }).into_response()
        }
        None => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse { error: format!("template '{}' not found", name) }),
        ).into_response(),
    }
}

/// GET /api/v1/rules/dsl/rules — List all rules in the engine.
pub async fn dsl_list_rules(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let rule_engine = match &state.rule_engine {
        Some(re) => re.clone(),
        None => return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorResponse { error: "rule engine not configured".to_string() }),
        ).into_response(),
    };

    let engine = rule_engine.lock().await;
    let rules: Vec<DslRuleResponse> = engine.list_rules()
        .into_iter()
        .map(dsl_rule_to_response)
        .collect();
    Json(rules).into_response()
}

/// GET /api/v1/rules/dsl/rules/:id — Get a specific rule.
pub async fn dsl_get_rule(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let rule_engine = match &state.rule_engine {
        Some(re) => re.clone(),
        None => return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorResponse { error: "rule engine not configured".to_string() }),
        ).into_response(),
    };

    let engine = rule_engine.lock().await;
    match engine.get_rule(&id) {
        Some(rule) => Json(dsl_rule_to_response(rule)).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse { error: format!("rule '{}' not found", id) }),
        ).into_response(),
    }
}

/// POST /api/v1/rules/dsl/rules/:id/vote — Vote on a proposed rule.
pub async fn dsl_vote_rule(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<DslVoteRequest>,
) -> impl IntoResponse {
    let rule_engine = match &state.rule_engine {
        Some(re) => re.clone(),
        None => return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorResponse { error: "rule engine not configured".to_string() }),
        ).into_response(),
    };

    let mut engine = rule_engine.lock().await;
    match engine.vote_on_rule(&id, body.voter_id, body.support) {
        Ok(()) => {
            let rule = engine.get_rule(&id).unwrap();
            (StatusCode::OK, Json(dsl_rule_to_response(rule))).into_response()
        }
        Err(e) => (rule_engine_error_status(&e), Json(ErrorResponse { error: e.to_string() })).into_response(),
    }
}

/// POST /api/v1/rules/dsl/rules/:id/activate — Activate a proposed rule.
pub async fn dsl_activate_rule(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let rule_engine = match &state.rule_engine {
        Some(re) => re.clone(),
        None => return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorResponse { error: "rule engine not configured".to_string() }),
        ).into_response(),
    };

    let mut engine = rule_engine.lock().await;
    match engine.activate_rule(&id) {
        Ok(()) => {
            let rule = engine.get_rule(&id).unwrap();
            (StatusCode::OK, Json(dsl_rule_to_response(rule))).into_response()
        }
        Err(e) => (rule_engine_error_status(&e), Json(ErrorResponse { error: e.to_string() })).into_response(),
    }
}

/// POST /api/v1/rules/dsl/rules/:id/suspend — Suspend an active rule.
pub async fn dsl_suspend_rule(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let rule_engine = match &state.rule_engine {
        Some(re) => re.clone(),
        None => return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorResponse { error: "rule engine not configured".to_string() }),
        ).into_response(),
    };

    let mut engine = rule_engine.lock().await;
    match engine.suspend_rule(&id) {
        Ok(()) => {
            let rule = engine.get_rule(&id).unwrap();
            (StatusCode::OK, Json(dsl_rule_to_response(rule))).into_response()
        }
        Err(e) => (rule_engine_error_status(&e), Json(ErrorResponse { error: e.to_string() })).into_response(),
    }
}

/// POST /api/v1/rules/dsl/rules/:id/repeal — Repeal a rule permanently.
pub async fn dsl_repeal_rule(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let rule_engine = match &state.rule_engine {
        Some(re) => re.clone(),
        None => return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorResponse { error: "rule engine not configured".to_string() }),
        ).into_response(),
    };

    let mut engine = rule_engine.lock().await;
    let tick = *state.tick_rx.borrow();
    match engine.repeal_rule(&id, tick) {
        Ok(()) => {
            let rule = engine.get_rule(&id).unwrap();
            (StatusCode::OK, Json(dsl_rule_to_response(rule))).into_response()
        }
        Err(e) => (rule_engine_error_status(&e), Json(ErrorResponse { error: e.to_string() })).into_response(),
    }
}

/// DSL rule engine routes.
pub fn dsl_routes() -> axum::Router<AppState> {
    axum::Router::new()
        .route("/api/v1/rules/dsl/parse", post(dsl_parse_rule))
        .route("/api/v1/rules/dsl/submit", post(dsl_submit_rule))
        .route("/api/v1/rules/dsl/templates", get(dsl_list_templates))
        .route("/api/v1/rules/dsl/templates/:name", get(dsl_get_template))
        .route("/api/v1/rules/dsl/rules", get(dsl_list_rules))
        .route("/api/v1/rules/dsl/rules/:id", get(dsl_get_rule))
        .route("/api/v1/rules/dsl/rules/:id/vote", post(dsl_vote_rule))
        .route("/api/v1/rules/dsl/rules/:id/activate", post(dsl_activate_rule))
        .route("/api/v1/rules/dsl/rules/:id/suspend", post(dsl_suspend_rule))
        .route("/api/v1/rules/dsl/rules/:id/repeal", post(dsl_repeal_rule))
}
