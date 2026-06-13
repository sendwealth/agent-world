//! # API Routes (Axum)
//!
//! 6764-line REST API serving all simulation operations: agents, organizations,
//! economy (banking/stocks/marketplace), tasks, snapshots, world state, and traces.
//!
//! Key types: AppState, AgentDto, A2AMessage, SpawnAgentRequest,
//!            TaskResponse, WorldStatsResponse, ErrorResponse
//! Depends on: world, economy, organization, auth, federation, tracing, snapshot
//!
//! ~197 route handlers / ~201 handler functions
//!
use std::collections::HashMap;
use std::sync::Arc;

use axum::{http::StatusCode, response::IntoResponse, routing::get, Router};
use serde::{Deserialize, Serialize};
use tokio::sync::{watch, Mutex};
use tower_http::cors::CorsLayer;
use uuid::Uuid;

use crate::api_auth::SharedApiKeyStore;

/// Hardcoded JWT secret for test/dev helpers only. Production reads `JWT_SECRET` from env.
#[cfg(test)]
const _TEST_JWT_SECRET: &str = "test-only-jwt-secret";

use crate::api_experiment::SharedExperimentStore;
use crate::auth::AuthStore;
use crate::economy::banking::BankingSystem;
use crate::economy::investment::InvestmentSystem;
use crate::economy::marketplace::Marketplace;
use crate::economy::reputation::{ReputationConfig, ReputationSystem};
use crate::economy::stock_market::StockMarket;
use crate::economy::task::TaskBoard;
use crate::economy::tool_marketplace::ToolMarketplace;
use crate::economy::escrow::EscrowManager;
use crate::economy::inheritance::InheritanceSystem;
use crate::economy::mentorship::MentorshipSystem;
use crate::economy::trust::TrustNetwork;
use crate::federation::{MigrationManager, MigrationPolicy, WorldRegistry};
use crate::a2a::world_message_router::WorldMessageRouter;
use crate::human::store::HumanParticipationStore;
use crate::organization::governance::GovernanceSystem;
use crate::organization::governance_metrics::GovernanceMetricsCollector;
use crate::organization::legislation_cycle::LegislationCycleEngine;
use crate::organization::org::OrganizationStore;
use crate::organization::rule_engine::RuleEngine;
use crate::time_capsule::SnapshotStore;
use crate::wal::WAL;
use crate::world::map::building::BuildingManager;
use crate::world::state::EventBus;
use crate::world::WorldState;

// ── Shared Type Aliases ──────────────────────────────────

pub type SharedTaskBoard = Arc<Mutex<TaskBoard>>;
pub type SharedWAL = Arc<Mutex<WAL>>;
pub type SharedSnapshotStore = Arc<Mutex<SnapshotStore>>;
pub type SharedMarketplace = Arc<Mutex<Marketplace>>;
pub type SharedReputationSystem = Arc<Mutex<ReputationSystem>>;
pub type SharedOrganizationStore = Arc<Mutex<OrganizationStore>>;
pub type SharedStockMarket = Arc<Mutex<StockMarket>>;
pub type SharedGovernanceSystem = Arc<Mutex<GovernanceSystem>>;
pub type SharedBankingSystem = Arc<Mutex<BankingSystem>>;
pub type SharedTraceStore = Arc<Mutex<crate::tracing::TraceStore>>;
pub type SharedGovernanceMetricsCollector = Arc<Mutex<GovernanceMetricsCollector>>;
pub type SharedInvestmentSystem = Arc<Mutex<InvestmentSystem>>;
pub type SharedRuleEngine = Arc<Mutex<RuleEngine>>;
pub type SharedToolMarketplace = Arc<Mutex<ToolMarketplace>>;
pub type SharedEscrowManager = Arc<Mutex<EscrowManager>>;
pub type SharedTrustNetwork = Arc<Mutex<TrustNetwork>>;
pub type SharedMentorshipSystem = Arc<Mutex<MentorshipSystem>>;
pub type SharedInheritanceSystem = Arc<Mutex<InheritanceSystem>>;
pub type SharedLegislationCycleEngine = Arc<Mutex<LegislationCycleEngine>>;
pub type SharedAuthStore = Arc<Mutex<AuthStore>>;
pub type SharedABExperimentStore = Arc<Mutex<Vec<crate::api_ab_experiment::ABExperiment>>>;

// ── Shared Data Types ───────────────────────────────────

/// Agent DTO for API responses (maps from world::agent::AgentRecord).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentDto {
    pub id: String,
    pub name: String,
    pub phase: String,
    pub tokens: u64,
    pub money: u64,
    pub alive: bool,
    pub ticks_survived: u64,
    #[serde(default)]
    pub personality: String,
    #[serde(default)]
    pub parent_ids: Vec<String>,
    #[serde(default)]
    pub generation: u32,
    #[serde(default)]
    pub skills: HashMap<String, u32>,
    /// ISO 8601 timestamp when this agent was created / registered.
    #[serde(default)]
    pub created_at: String,
}

impl From<crate::world::agent::AgentRecord> for AgentDto {
    fn from(rec: crate::world::agent::AgentRecord) -> Self {
        Self {
            id: rec.id.to_string(),
            name: rec.name,
            phase: format!("{:?}", rec.phase).to_lowercase(),
            tokens: rec.tokens,
            money: 0, // Money is tracked in ExternalAgent / BankingSystem, not AgentRecord
            alive: !matches!(rec.phase, crate::world::enums::AgentPhase::Dead),
            ticks_survived: rec.tasks_attempted as u64, // Best proxy until tick-based tracking is added
            personality: rec.personality,
            parent_ids: Vec::new(),
            generation: 0,
            skills: rec.skills.values().map(|s| (s.name.clone(), s.level)).collect(),
            created_at: String::new(), // AgentRecord doesn't carry a timestamp
        }
    }
}

/// A2A message record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A2AMessage {
    pub id: String,
    pub from_agent: String,
    pub to_agent: String,
    pub message_type: String,
    pub payload: String,
    pub tick: u64,
}

/// Error response used across all API handlers.
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

/// Valid action types for external agents.
pub const ALLOWED_ACTIONS: &[&str] = &[
    "move",
    "gather",
    "trade",
    "communicate",
    "explore",
    "rest",
    "build",
    "claim_task",
    "submit_task",
];

/// Position in the world grid.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub x: i64,
    pub y: i64,
}

/// An externally registered agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalAgent {
    pub agent_id: String,
    pub name: String,
    pub api_key: String,
    pub capabilities: Vec<String>,
    pub config: serde_json::Value,
    pub alive: bool,
    pub phase: String,
    pub tokens: u64,
    pub money: u64,
    pub position: Position,
    pub registered_tick: u64,
    /// ISO 8601 timestamp when this agent was created / registered.
    #[serde(default)]
    pub created_at: String,
}

pub type SharedExternalAgents = Arc<Mutex<HashMap<String, ExternalAgent>>>;

// ── Re-exports from sub-modules ──────────────────────────
pub use crate::api_world::{parse_event_types, SseQuery};

// ── Shared Helpers ──────────────────────────────────────

/// Helper: wrap success response in { data, error: null, request_id } format.
pub fn api_ok(data: impl serde::Serialize) -> axum::response::Response {
    use axum::Json;
    let request_id = Uuid::new_v4().to_string();
    Json(serde_json::json!({
        "data": data,
        "error": null,
        "request_id": request_id,
    }))
    .into_response()
}

/// Helper: wrap error response in { data: null, error, request_id } format.
pub fn api_err(status: StatusCode, error: impl Into<String>) -> axum::response::Response {
    use axum::Json;
    let request_id = Uuid::new_v4().to_string();
    (
        status,
        Json(serde_json::json!({
            "data": null,
            "error": error.into(),
            "request_id": request_id,
        })),
    )
        .into_response()
}

// ── AppState ────────────────────────────────────────────

#[derive(Clone)]
pub struct AppState {
    pub board: SharedTaskBoard,
    pub wal: SharedWAL,
    pub event_bus: Arc<EventBus>,
    pub agents: Arc<Mutex<Vec<AgentDto>>>,
    pub messages: Arc<Mutex<Vec<A2AMessage>>>,
    pub tick_tx: watch::Sender<u64>,
    pub tick_rx: watch::Receiver<u64>,
    pub snapshot_store: Option<SharedSnapshotStore>,
    pub marketplace: Option<SharedMarketplace>,
    pub reputation_system: Option<SharedReputationSystem>,
    pub org_store: Option<SharedOrganizationStore>,
    pub stock_market: Option<SharedStockMarket>,
    pub governance: Option<SharedGovernanceSystem>,
    pub banking_system: Option<SharedBankingSystem>,
    pub trace_store: Option<SharedTraceStore>,
    pub external_agents: SharedExternalAgents,
    pub governance_metrics: Option<SharedGovernanceMetricsCollector>,
    pub building_manager: Arc<Mutex<BuildingManager>>,
    pub human_store: Arc<Mutex<HumanParticipationStore>>,
    pub auth_store: SharedAuthStore,
    pub investment_system: Option<SharedInvestmentSystem>,
    pub rule_engine: Option<SharedRuleEngine>,
    pub tool_marketplace: Option<SharedToolMarketplace>,
    pub federation: Option<Arc<Mutex<crate::a2a::federation::FederationEngine>>>,
    pub federation_registry: Option<Arc<Mutex<crate::federation::WorldRegistry>>>,
    pub migration_manager: Option<Arc<Mutex<crate::federation::MigrationManager>>>,
    pub api_key_store: Option<SharedApiKeyStore>,
    pub experiment_store: SharedExperimentStore,
    pub ab_experiment_store: SharedABExperimentStore,
    pub plugin_manager: Option<crate::plugin::SharedPluginManager>,
    pub world_msg_router: Option<Arc<WorldMessageRouter>>,
    pub providers: crate::api_providers::SharedProviderStore,
    pub agent_models: crate::api_providers::SharedAgentModelStore,
    pub diary_store: Option<crate::api_diary::SharedDiaryStore>,
    pub feed_store: Option<crate::api_feed::SharedFeedStore>,
    pub escrow_manager: Option<SharedEscrowManager>,
    pub trust_network: Option<SharedTrustNetwork>,
    pub mentorship_system: Option<SharedMentorshipSystem>,
    pub inheritance_system: Option<SharedInheritanceSystem>,
    pub legislation_cycle_engine: Option<SharedLegislationCycleEngine>,
    /// Shared reference to the WorldState used by the scheduler and metrics.
    /// External agent registration must also insert here so that
    /// `agents_alive` metrics reflect all registered agents.
    pub world_state: Option<Arc<Mutex<WorldState>>>,
}

/// Optional subsystem overrides for test AppState construction.
///
/// All fields default to `None`; only set the subsystems your test needs.
#[derive(Default)]
pub struct TestOverrides {
    pub event_bus: Option<Arc<EventBus>>,
    pub tick_tx: Option<watch::Sender<u64>>,
    pub tick_rx: Option<watch::Receiver<u64>>,
    pub snapshot_store: Option<SharedSnapshotStore>,
    pub marketplace: Option<SharedMarketplace>,
    pub reputation_system: Option<SharedReputationSystem>,
    pub org_store: Option<SharedOrganizationStore>,
    pub stock_market: Option<SharedStockMarket>,
    pub governance: Option<SharedGovernanceSystem>,
    pub banking_system: Option<SharedBankingSystem>,
    pub trace_store: Option<SharedTraceStore>,
    pub governance_metrics: Option<SharedGovernanceMetricsCollector>,
    pub investment_system: Option<SharedInvestmentSystem>,
    pub rule_engine: Option<SharedRuleEngine>,
    pub tool_marketplace: Option<SharedToolMarketplace>,
    pub federation: Option<Arc<Mutex<crate::a2a::federation::FederationEngine>>>,
    pub federation_registry: Option<Arc<Mutex<crate::federation::WorldRegistry>>>,
    pub migration_manager: Option<Arc<Mutex<crate::federation::MigrationManager>>>,
    pub api_key_store: Option<SharedApiKeyStore>,
    pub auth_store: Option<SharedAuthStore>,
    pub ab_experiment_store: Option<SharedABExperimentStore>,
    pub plugin_manager: Option<crate::plugin::SharedPluginManager>,
    pub providers: Option<crate::api_providers::SharedProviderStore>,
    pub agent_models: Option<crate::api_providers::SharedAgentModelStore>,
    pub diary_store: Option<crate::api_diary::SharedDiaryStore>,
    pub feed_store: Option<crate::api_feed::SharedFeedStore>,
    pub escrow_manager: Option<SharedEscrowManager>,
    pub trust_network: Option<SharedTrustNetwork>,
    pub mentorship_system: Option<SharedMentorshipSystem>,
    pub inheritance_system: Option<SharedInheritanceSystem>,
    pub legislation_cycle_engine: Option<SharedLegislationCycleEngine>,
    pub world_state: Option<Arc<Mutex<WorldState>>>,
}

impl AppState {
    /// Build an AppState with selected subsystem overrides.
    ///
    /// Used by both production (`main.rs`) and test code. Fields left as `None`
    /// in the overrides are initialised with safe defaults.
    pub fn new(board: SharedTaskBoard, wal: SharedWAL, overrides: TestOverrides) -> Self {
        let event_bus = overrides
            .event_bus
            .unwrap_or_else(|| Arc::new(EventBus::new(256)));
        let (tick_tx, tick_rx) = match (overrides.tick_tx, overrides.tick_rx) {
            (Some(tx), Some(rx)) => (tx, rx),
            _ => watch::channel(0u64),
        };

        Self {
            board,
            wal,
            event_bus: event_bus.clone(),
            agents: Arc::new(Mutex::new(Vec::new())),
            messages: Arc::new(Mutex::new(Vec::new())),
            tick_tx,
            tick_rx,
            snapshot_store: overrides.snapshot_store,
            marketplace: overrides.marketplace,
            reputation_system: overrides.reputation_system,
            org_store: overrides.org_store,
            stock_market: overrides.stock_market,
            governance: overrides.governance,
            banking_system: overrides.banking_system,
            trace_store: overrides.trace_store,
            external_agents: Arc::new(Mutex::new(HashMap::new())),
            governance_metrics: overrides.governance_metrics,
            building_manager: Arc::new(Mutex::new(BuildingManager::new())),
            human_store: Arc::new(Mutex::new(HumanParticipationStore::new())),
            auth_store: overrides
                .auth_store
                .unwrap_or_else(|| {
                    #[cfg(test)]
                    { Arc::new(Mutex::new(AuthStore::new(_TEST_JWT_SECRET))) }
                    #[cfg(not(test))]
                    {
                        use rand::Rng;
                        let secret: String = rand::thread_rng()
                            .sample_iter(&rand::distributions::Alphanumeric)
                            .take(48)
                            .map(char::from)
                            .collect();
                        Arc::new(Mutex::new(AuthStore::new(&secret)))
                    }
                }),
            investment_system: overrides.investment_system,
            rule_engine: overrides.rule_engine,
            tool_marketplace: overrides.tool_marketplace,
            federation: overrides.federation,
            federation_registry: overrides.federation_registry,
            migration_manager: overrides.migration_manager,
            api_key_store: overrides.api_key_store,
            experiment_store: Arc::new(Mutex::new(Vec::new())),
            ab_experiment_store: overrides
                .ab_experiment_store
                .unwrap_or_else(|| Arc::new(Mutex::new(Vec::new()))),
            plugin_manager: overrides.plugin_manager,
            world_msg_router: None,
            providers: overrides.providers.unwrap_or_else(|| Arc::new(Mutex::new(HashMap::new()))),
            agent_models: overrides.agent_models.unwrap_or_else(|| Arc::new(Mutex::new(HashMap::new()))),
            diary_store: overrides.diary_store,
            feed_store: overrides.feed_store,
            escrow_manager: overrides.escrow_manager,
            trust_network: overrides.trust_network,
            mentorship_system: overrides.mentorship_system,
            inheritance_system: overrides.inheritance_system,
            legislation_cycle_engine: overrides.legislation_cycle_engine,
            world_state: overrides.world_state,
        }
    }

    /// Build a minimal test AppState (test-only).
    #[cfg(test)]
    pub fn for_test(board: SharedTaskBoard, wal: SharedWAL) -> Self {
        Self::new(board, wal, TestOverrides::default())
    }
}

// ── Router Factories ────────────────────────────────────

pub fn create_router(board: SharedTaskBoard) -> Router {
    use axum::routing::{delete, get, post};
    Router::new()
        .route("/tasks", post(crate::api_tasks::create_task))
        .route("/tasks", get(crate::api_tasks::list_tasks))
        .route("/tasks/:id", get(crate::api_tasks::get_task))
        .route("/tasks/:id/claim", post(crate::api_tasks::claim_task))
        .route("/tasks/:id/start", post(crate::api_tasks::start_task))
        .route("/tasks/:id/submit", post(crate::api_tasks::submit_task))
        .route("/tasks/:id/review", post(crate::api_tasks::review_task))
        .route("/tasks/:id/complete", post(crate::api_tasks::complete_task))
        .route("/tasks/:id/expire", post(crate::api_tasks::expire_task))
        .route("/tasks/:id", delete(crate::api_tasks::delete_task))
        .with_state(board)
}

fn make_test_state(
    board: SharedTaskBoard,
    wal: SharedWAL,
    event_bus: Arc<EventBus>,
    tick_tx: watch::Sender<u64>,
    tick_rx: watch::Receiver<u64>,
    snapshot_store: Option<SharedSnapshotStore>,
) -> AppState {
    AppState {
        board,
        wal,
        event_bus: event_bus.clone(),
        agents: Arc::new(Mutex::new(Vec::new())),
        messages: Arc::new(Mutex::new(Vec::new())),
        tick_tx,
        tick_rx,
        snapshot_store,
        marketplace: Some(Arc::new(Mutex::new(Marketplace::with_event_bus(
            event_bus.as_ref().clone(),
        )))),
        reputation_system: Some(Arc::new(Mutex::new(ReputationSystem::with_event_bus(
            ReputationConfig::default(),
            event_bus.as_ref().clone(),
        )))),
        org_store: None,
        stock_market: None,
        governance: None,
        banking_system: None,
        trace_store: Some(Arc::new(Mutex::new(crate::tracing::TraceStore::new()))),
        external_agents: Arc::new(Mutex::new(HashMap::new())),
        governance_metrics: None,
        building_manager: Arc::new(Mutex::new(BuildingManager::new())),
        human_store: Arc::new(Mutex::new(HumanParticipationStore::new())),
        auth_store: {
            #[cfg(test)]
            { Arc::new(Mutex::new(AuthStore::new(_TEST_JWT_SECRET))) }
            #[cfg(not(test))]
            {
                use rand::Rng;
                let secret: String = rand::thread_rng()
                    .sample_iter(&rand::distributions::Alphanumeric)
                    .take(48)
                    .map(char::from)
                    .collect();
                Arc::new(Mutex::new(AuthStore::new(&secret)))
            }
        },
        investment_system: None,
        rule_engine: None,
        tool_marketplace: None,
        federation: Some(Arc::new(Mutex::new(
            crate::a2a::federation::FederationEngine::with_shared_event_bus(event_bus.clone()),
        ))),
        federation_registry: Some(Arc::new(Mutex::new(WorldRegistry::new(event_bus.clone())))),
        migration_manager: Some(Arc::new(Mutex::new(MigrationManager::new(
            MigrationPolicy::default(),
            event_bus,
        )))),
        api_key_store: None,
        experiment_store: Arc::new(Mutex::new(Vec::new())),
        ab_experiment_store: Arc::new(Mutex::new(Vec::new())),
        plugin_manager: None,
        world_msg_router: None,
        providers: Arc::new(Mutex::new(HashMap::new())),
        agent_models: Arc::new(Mutex::new(HashMap::new())),
        diary_store: None,
        feed_store: None,
        escrow_manager: None,
        trust_network: None,
        mentorship_system: None,
        inheritance_system: None,
        legislation_cycle_engine: None,
        world_state: None,
    }
}

pub fn create_router_with_wal(board: SharedTaskBoard, wal: SharedWAL) -> Router {
    let event_bus = Arc::new(EventBus::new(256));
    let (tick_tx, tick_rx) = watch::channel(0);
    let snapshot_store = SnapshotStore::new("./data/snapshots.db")
        .ok()
        .map(|s| Arc::new(Mutex::new(s)));
    let state = make_test_state(board, wal, event_bus, tick_tx, tick_rx, snapshot_store);
    build_full_router(state)
}

pub fn create_router_with_wal_and_snapshots(
    board: SharedTaskBoard,
    wal: SharedWAL,
    snapshot_path: &str,
) -> Router {
    let event_bus = Arc::new(EventBus::new(256));
    let (tick_tx, tick_rx) = watch::channel(0);
    let snapshot_store = SnapshotStore::new(snapshot_path)
        .ok()
        .map(|s| Arc::new(Mutex::new(s)));
    let state = make_test_state(board, wal, event_bus, tick_tx, tick_rx, snapshot_store);
    build_full_router(state)
}

/// Build a CORS layer based on the `CORS_ORIGINS` environment variable.
///
/// - If `CORS_ORIGINS` is unset or empty → permissive (allow any origin, dev-friendly).
/// - If set → only the listed origins are allowed (production-safe).
///   Multiple origins can be separated by commas, e.g. `CORS_ORIGINS=https://app.example.com,https://admin.example.com`
fn build_cors_layer() -> CorsLayer {
    use axum::http::header::{AUTHORIZATION, CONTENT_TYPE};
    use axum::http::Method;
    use tower_http::cors::AllowOrigin;

    let origins_env = std::env::var("CORS_ORIGINS").unwrap_or_default();
    if origins_env.is_empty() {
        CorsLayer::permissive()
    } else {
        let origins: Vec<_> = origins_env
            .split(',')
            .filter_map(|s| s.trim().parse().ok())
            .collect();
        CorsLayer::new()
            .allow_origin(AllowOrigin::list(origins))
            .allow_methods([
                Method::GET,
                Method::POST,
                Method::PUT,
                Method::DELETE,
                Method::OPTIONS,
            ])
            .allow_headers([CONTENT_TYPE, AUTHORIZATION])
    }
}

/// Build the full router by merging all domain sub-routers.
///
/// All v1 route modules define bare paths (e.g. `/tasks`, `/world/stats`).
/// They are nested under `/api/v1` here so that the final paths are
/// `/api/v1/tasks`, `/api/v1/world/stats`, etc.
pub fn build_full_router(state: AppState) -> Router {
    let cors = build_cors_layer();

    let v1_routes = Router::new()
        .merge(crate::api_tasks::task_routes())
        .merge(crate::api_coordination_tasks::coordination_task_routes())
        .merge(crate::api_world::world_routes())
        .merge(crate::api_org::org_routes())
        .merge(crate::api_governance::governance_routes())
        .merge(crate::api_stocks::stock_routes())
        .merge(crate::api_bank::bank_routes())
        .merge(crate::api_traces::trace_routes())
        .merge(crate::api_agents_ext::agents_ext_routes())
        .merge(crate::api_population::population_routes())
        .merge(crate::api_buildings::building_routes())
        .merge(crate::api_auth_handlers::auth_routes())
        .merge(crate::api_human::human_routes())
        .merge(crate::api_dsl::dsl_routes())
        .merge(crate::api_federation::federation_routes())
        .merge(crate::api_investment::investment_routes())
        .merge(crate::api_diplomacy::diplomacy_routes())
        .merge(crate::api_marketplace::marketplace_routes())
        .merge(crate::api_tool_marketplace::tool_marketplace_routes())
        .merge(crate::api_reputation::reputation_routes())
        .merge(crate::api_escrow::escrow_routes())
        .merge(crate::api_trust::trust_routes())
        .merge(crate::api_mentorship::mentorship_routes())
        .merge(crate::api_inheritance::inheritance_routes())
        .merge(crate::api_legislation::legislation_routes())
        .merge(crate::api_export_v1::export_v1_routes())
        .merge(crate::api_plugins::plugin_routes())
        .merge(crate::api_providers::provider_routes())
        .merge(crate::api_diary::diary_routes())
        .merge(crate::api_feed::feed_routes())
        .merge(crate::api_dashboard::dashboard_routes());

    Router::new()
        // All v1 routes nested under /api/v1
        .nest("/api/v1", v1_routes)
        // Prometheus metrics endpoint (outside /api/v1 — scraped directly by Prometheus)
        .route("/metrics", get(crate::observability::metrics_handler))
        // Research API v2 (with optional auth middleware)
        .merge(v2_router(&state))
        .layer(cors)
        .with_state(state)
}

/// Build the `/api/v2/*` research router, optionally wrapped in auth middleware.
fn v2_router(state: &AppState) -> Router<AppState> {
    use axum::middleware;

    let v2_routes = crate::api_research::research_routes()
        .merge(crate::api_experiment::experiment_routes())
        .merge(crate::api_export::export_routes())
        .merge(crate::api_behavior_log::behavior_log_routes())
        .merge(crate::api_network_graph::network_graph_routes())
        .merge(crate::api_ab_experiment::ab_experiment_routes())
        .merge(crate::api_report::report_routes());
    match &state.api_key_store {
        Some(store) => v2_routes.layer(middleware::from_fn_with_state(
            store.clone(),
            crate::api_auth::auth_middleware,
        )),
        None => v2_routes,
    }
}

/// Create a router for testing with a provided EventBus and tick channel.
pub fn create_router_for_test(
    board: SharedTaskBoard,
    wal: SharedWAL,
    event_bus: Arc<EventBus>,
    tick_tx: watch::Sender<u64>,
    tick_rx: watch::Receiver<u64>,
) -> Router {
    let snapshot_store = SnapshotStore::new_in_memory()
        .ok()
        .map(|s| Arc::new(Mutex::new(s)));
    let state = make_test_state(board, wal, event_bus, tick_tx, tick_rx, snapshot_store);
    build_full_router(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::event::WorldEvent;
    use axum::body::Body;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    /// Build a test AppState with governance_metrics wired up.
    fn build_test_state() -> (AppState, tempfile::TempDir) {
        let bus = Arc::new(EventBus::new(256));
        let collector = GovernanceMetricsCollector::new(&bus);
        // Allow background task to subscribe
        std::thread::sleep(std::time::Duration::from_millis(10));

        let board = Arc::new(Mutex::new(TaskBoard::new()));
        let tmp = tempfile::tempdir().expect("tempdir");
        let wal = Arc::new(Mutex::new(WAL::new(tmp.path())));

        let state = AppState::new(
            board,
            wal,
            TestOverrides {
                event_bus: Some(bus),
                governance_metrics: Some(Arc::new(Mutex::new(collector))),
                ..TestOverrides::default()
            },
        );
        (state, tmp)
    }

    /// Helper to extract JSON body from a response.
    async fn body_to_json(body: Body) -> serde_json::Value {
        let bytes = body
            .collect()
            .await
            .expect("failed to read body")
            .to_bytes();
        serde_json::from_slice(&bytes).expect("failed to parse JSON")
    }

    #[tokio::test]
    async fn test_governance_summary_returns_world_summary() {
        let (state, _tmp) = build_test_state();
        let bus = state.event_bus.clone();

        // Emit some governance events
        let org_a = Uuid::new_v4();
        let org_b = Uuid::new_v4();
        bus.emit(WorldEvent::TaxCollected {
            org_id: org_a.to_string(),
            payer_id: "p1".into(),
            tax_kind: "IncomeTax".into(),
            rate: 0.1,
            gross_amount: 1000,
            tax_amount: 100,
            tick: 5,
        });
        bus.emit(WorldEvent::TreatySigned {
            treaty_id: "t-1".into(),
            org_a: org_a.to_string(),
            org_b: org_b.to_string(),
        });

        tokio::time::sleep(std::time::Duration::from_millis(30)).await;

        let app = build_full_router(state);
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/api/v1/governance/summary")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let json = body_to_json(response.into_body()).await;
        assert_eq!(json["total_orgs"], 2);
        assert_eq!(json["total_tax_collected"], 100);
        assert_eq!(json["total_treaties"], 2); // Each org counts as 1 signing
    }

    #[tokio::test]
    async fn test_governance_org_metrics_returns_per_org_data() {
        let (state, _tmp) = build_test_state();
        let bus = state.event_bus.clone();
        let org_id = Uuid::new_v4();

        bus.emit(WorldEvent::OrganizationMemberJoined {
            org_id,
            agent_id: "a1".into(),
            role: "Member".into(),
        });
        bus.emit(WorldEvent::TaxCollected {
            org_id: org_id.to_string(),
            payer_id: "a1".into(),
            tax_kind: "IncomeTax".into(),
            rate: 0.1,
            gross_amount: 500,
            tax_amount: 50,
            tick: 10,
        });

        tokio::time::sleep(std::time::Duration::from_millis(30)).await;

        let app = build_full_router(state);
        let uri = format!("/api/v1/governance/orgs/{}", org_id);
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri(&uri)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let json = body_to_json(response.into_body()).await;
        assert_eq!(json["org_id"], org_id.to_string());
        assert_eq!(json["total_tax_collected"], 50);
        assert_eq!(json["member_count"], 1);
        assert_eq!(json["tax_collection_count"], 1);
    }

    #[tokio::test]
    async fn test_governance_org_metrics_returns_404_for_unknown_org() {
        let (state, _tmp) = build_test_state();
        let unknown_id = Uuid::new_v4();

        let app = build_full_router(state);
        let uri = format!("/api/v1/governance/orgs/{}", unknown_id);
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri(&uri)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_governance_timeline_returns_filtered_events() {
        let (state, _tmp) = build_test_state();
        let bus = state.event_bus.clone();
        let org_id = Uuid::new_v4();

        bus.emit(WorldEvent::TaxCollected {
            org_id: org_id.to_string(),
            payer_id: "p1".into(),
            tax_kind: "IncomeTax".into(),
            rate: 0.1,
            gross_amount: 100,
            tax_amount: 10,
            tick: 5,
        });
        bus.emit(WorldEvent::TaxCollected {
            org_id: org_id.to_string(),
            payer_id: "p2".into(),
            tax_kind: "IncomeTax".into(),
            rate: 0.1,
            gross_amount: 200,
            tax_amount: 20,
            tick: 15,
        });
        bus.emit(WorldEvent::LeadershipElectionStarted {
            org_id,
            candidates: vec!["c1".into()],
            voting_method: "SimpleMajority".into(),
        });

        tokio::time::sleep(std::time::Duration::from_millis(30)).await;

        let app = build_full_router(state);

        // Query with tick range filter [0, 10]
        let uri = format!(
            "/api/v1/governance/orgs/{}/timeline?from_tick=0&to_tick=10",
            org_id
        );
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri(&uri)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let json = body_to_json(response.into_body()).await;
        let events = json.as_array().expect("expected array");
        // tick 5 TaxCollected + tick 0 LeadershipElectionStarted = 2 events
        assert_eq!(events.len(), 2);
    }

    #[tokio::test]
    async fn test_governance_comparison_returns_multiple_orgs() {
        let (state, _tmp) = build_test_state();
        let bus = state.event_bus.clone();
        let org_a = Uuid::new_v4();
        let org_b = Uuid::new_v4();

        // org_a: tax + election
        bus.emit(WorldEvent::TaxCollected {
            org_id: org_a.to_string(),
            payer_id: "p1".into(),
            tax_kind: "IncomeTax".into(),
            rate: 0.1,
            gross_amount: 100,
            tax_amount: 10,
            tick: 1,
        });
        bus.emit(WorldEvent::LeadershipElectionStarted {
            org_id: org_a,
            candidates: vec!["c1".into(), "c2".into()],
            voting_method: "SimpleMajority".into(),
        });

        // org_b: treaty only
        bus.emit(WorldEvent::TreatySigned {
            treaty_id: "t-1".into(),
            org_a: org_a.to_string(),
            org_b: org_b.to_string(),
        });

        tokio::time::sleep(std::time::Duration::from_millis(30)).await;

        let app = build_full_router(state);
        let uri = format!("/api/v1/governance/comparison?org_ids={},{}", org_a, org_b);
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri(&uri)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let json = body_to_json(response.into_body()).await;
        let comparison = json.as_array().expect("expected array");
        assert_eq!(comparison.len(), 2);

        // org_a should have tax and election data
        let metrics_a = &comparison[0];
        assert_eq!(metrics_a["total_tax_collected"], 10);
        assert_eq!(metrics_a["election_count"], 1);

        // org_b should have treaty data
        let metrics_b = &comparison[1];
        assert_eq!(metrics_b["treaties_signed"], 1);
    }

    #[tokio::test]
    async fn test_governance_comparison_returns_400_without_org_ids() {
        let (state, _tmp) = build_test_state();
        let app = build_full_router(state);

        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/api/v1/governance/comparison")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_governance_summary_returns_503_when_not_configured() {
        let board = Arc::new(Mutex::new(TaskBoard::new()));
        let tmp = tempfile::tempdir().expect("tempdir");
        let wal = Arc::new(Mutex::new(WAL::new(tmp.path())));

        // State without governance_metrics — all optional subsystems None
        let state = AppState::for_test(board, wal);

        let app = build_full_router(state);
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/api/v1/governance/summary")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    // ── Legislation History API Tests ─────────────────────────

    /// Build a test AppState with rule_engine wired up.
    fn build_test_state_with_rules() -> (AppState, tempfile::TempDir) {
        use crate::organization::rule_engine::{
            RuleCondition, RuleEngine, RuleEffect, RuleType,
        };
        use serde_json::json;

        let bus = Arc::new(EventBus::new(256));
        let board = Arc::new(Mutex::new(TaskBoard::new()));
        let tmp = tempfile::tempdir().expect("tempdir");
        let wal = Arc::new(Mutex::new(WAL::new(tmp.path())));

        let mut engine = RuleEngine::new();
        let org_id = "org-legislature";

        // Proposed tax rule
        engine.propose_rule(
            "leader-1".to_string(),
            org_id.to_string(),
            "Tax on wealthy".to_string(),
            "Extra tax for high-resource agents".to_string(),
            RuleType::Tax,
            vec![RuleCondition {
                field: "agent.resources".to_string(),
                operator: ">".to_string(),
                value: json!(200),
            }],
            vec![RuleEffect {
                target: "agent.tax_bonus".to_string(),
                action: "set".to_string(),
                value: json!(0.1),
            }],
            100,
            None,
        );

        // Proposed behavior rule — then activate it
        let rule_id_2 = engine.propose_rule(
            "leader-1".to_string(),
            org_id.to_string(),
            "Safety regulation".to_string(),
            "Limit attacks per tick".to_string(),
            RuleType::Behavior,
            vec![],
            vec![RuleEffect {
                target: "agent.attack_blocked".to_string(),
                action: "set".to_string(),
                value: json!(true),
            }],
            110,
            Some(500),
        );
        engine.vote_on_rule(&rule_id_2, "v1".to_string(), true).unwrap();
        engine.vote_on_rule(&rule_id_2, "v2".to_string(), true).unwrap();
        engine.vote_on_rule(&rule_id_2, "v3".to_string(), false).unwrap();
        engine.activate_rule(&rule_id_2).unwrap();

        // Rule for a different org
        engine.propose_rule(
            "other-leader".to_string(),
            "org-other".to_string(),
            "Other rule".to_string(),
            "Not relevant".to_string(),
            RuleType::Trade,
            vec![],
            vec![],
            50,
            None,
        );

        let state = AppState::new(
            board,
            wal,
            TestOverrides {
                event_bus: Some(bus),
                rule_engine: Some(Arc::new(Mutex::new(engine))),
                ..TestOverrides::default()
            },
        );
        (state, tmp)
    }

    #[tokio::test]
    async fn test_legislation_history_returns_rules_for_org() {
        let (state, _tmp) = build_test_state_with_rules();
        let app = build_full_router(state);

        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/api/v1/governance/orgs/org-legislature/legislation")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let json = body_to_json(response.into_body()).await;
        let entries = json.as_array().expect("expected array");
        // Two rules for org-legislature, one for org-other
        assert_eq!(entries.len(), 2);

        // Verify entry structure
        let entry = &entries[0];
        assert!(entry["rule_id"].is_string());
        assert_eq!(entry["proposer_id"], "leader-1");
        assert_eq!(entry["org_id"], "org-legislature");
        assert!(entry["title"].is_string());
        assert!(entry["rule_type"].is_string());
        assert!(entry["status"].is_string());
        assert!(entry["votes_for"].is_number());
        assert!(entry["votes_against"].is_number());
        assert!(entry["created_tick"].is_number());
    }

    #[tokio::test]
    async fn test_legislation_history_filters_by_status() {
        let (state, _tmp) = build_test_state_with_rules();
        let app = build_full_router(state);

        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/api/v1/governance/orgs/org-legislature/legislation?status=active")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let json = body_to_json(response.into_body()).await;
        let entries = json.as_array().expect("expected array");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0]["status"], "active");
        assert_eq!(entries[0]["title"], "Safety regulation");
    }

    #[tokio::test]
    async fn test_legislation_history_filters_by_rule_type() {
        let (state, _tmp) = build_test_state_with_rules();
        let app = build_full_router(state);

        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/api/v1/governance/orgs/org-legislature/legislation?rule_type=tax")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let json = body_to_json(response.into_body()).await;
        let entries = json.as_array().expect("expected array");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0]["rule_type"], "tax");
        assert_eq!(entries[0]["status"], "proposed");
    }

    #[tokio::test]
    async fn test_legislation_history_returns_empty_for_unknown_org() {
        let (state, _tmp) = build_test_state_with_rules();
        let app = build_full_router(state);

        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/api/v1/governance/orgs/nonexistent-org/legislation")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let json = body_to_json(response.into_body()).await;
        let entries = json.as_array().expect("expected array");
        assert!(entries.is_empty());
    }

    #[tokio::test]
    async fn test_legislation_history_returns_503_when_rule_engine_not_configured() {
        let board = Arc::new(Mutex::new(TaskBoard::new()));
        let tmp = tempfile::tempdir().expect("tempdir");
        let wal = Arc::new(Mutex::new(WAL::new(tmp.path())));

        let state = AppState::for_test(board, wal);
        let app = build_full_router(state);

        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/api/v1/governance/orgs/some-org/legislation")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn test_legislation_history_combined_filters() {
        let (state, _tmp) = build_test_state_with_rules();
        let app = build_full_router(state);

        // Filter by status=proposed AND rule_type=tax
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/api/v1/governance/orgs/org-legislature/legislation?status=proposed&rule_type=tax")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let json = body_to_json(response.into_body()).await;
        let entries = json.as_array().expect("expected array");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0]["rule_type"], "tax");
        assert_eq!(entries[0]["status"], "proposed");
    }
}
