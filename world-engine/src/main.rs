use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{Mutex, watch};
use tokio_util::sync::CancellationToken;

use agent_world_engine::a2a::registry::AgentRegistry;
use agent_world_engine::a2a::router::MessageRouter;
use agent_world_engine::a2a::service::A2aServiceImpl;
use agent_world_engine::a2a::federation::FederationEngine;
use agent_world_engine::agentworld::a2a::v1::a2a_service_server::A2aServiceServer;
use agent_world_engine::api::{self, AppState};
use agent_world_engine::api_auth::ApiKeyStore;
use agent_world_engine::config::{ConfigManager, GenesisConfig};
use agent_world_engine::economy::banking::{BankingSystem, CentralBankConfig};
use agent_world_engine::economy::investment::InvestmentSystem;
use agent_world_engine::economy::marketplace::Marketplace;
use agent_world_engine::economy::reputation::{ReputationConfig, ReputationSystem};
use agent_world_engine::economy::stock_market::StockMarket;
use agent_world_engine::economy::task::TaskBoard;
use agent_world_engine::economy::token_burn::TokenBurnEngine;
use agent_world_engine::organization::org::OrganizationStore;
use agent_world_engine::organization::governance::GovernanceSystem;
use agent_world_engine::time_capsule::SnapshotStore;
use agent_world_engine::wal::WAL;
use agent_world_engine::world::event::WorldEvent;
use agent_world_engine::world::state::EventBus;
use agent_world_engine::world::subsystem::SubsystemRegistry;
use agent_world_engine::world::subsystems::{
    DeathJudgmentSubsystem, EventBroadcastSubsystem, LifecycleAgingSubsystem,
    ReputationDecaySubsystem, TokenBurnSubsystem,
};
use agent_world_engine::evolution::{EvolutionSubsystem, subsystem::EvolutionSubsystemConfig};
use agent_world_engine::evolution::mutation::OffspringMutationConfig;
use agent_world_engine::persistence::{SerializableWorldState, SqlitePersistence, StatePersistence};
use agent_world_engine::world::{Scheduler, WorldState};
use agent_world_engine::federation::{MigrationManager, MigrationPolicy, WorldRegistry};

#[tokio::main]
async fn main() {
    // ── Initialize tracing ──────────────────────────────────
    tracing_subscriber::fmt().init();

    // ── Initialize Prometheus metrics ────────────────────────
    agent_world_engine::observability::init();

    println!("Agent World Engine v1.0.0");
    println!("   Status: initializing...");

    // ── Shared cancellation token for all background tasks ──
    let cancel_token = CancellationToken::new();

    // ── Load genesis.yaml ───────────────────────────────────
    let genesis_path = std::env::var("GENESIS_PATH")
        .unwrap_or_else(|_| "config/genesis.yaml".to_string());

    let genesis_config: GenesisConfig = if std::path::Path::new(&genesis_path).exists() {
        match GenesisConfig::load_from_file(std::path::Path::new(&genesis_path)) {
            Ok(config) => {
                let errors = config.validate();
                if !errors.is_empty() {
                    for e in &errors {
                        eprintln!("   Config validation error: {}", e);
                    }
                    println!("   Using default config due to validation errors");
                    GenesisConfig::default()
                } else {
                    println!("   GenesisConfig: loaded from {}", genesis_path);
                    config
                }
            }
            Err(e) => {
                eprintln!("   Failed to load genesis.yaml: {} — using defaults", e);
                GenesisConfig::default()
            }
        }
    } else {
        println!("   GenesisConfig: file not found, using defaults");
        GenesisConfig::default()
    };

    let tick_interval = Duration::from_millis(genesis_config.world.tick_interval_ms);

    // ── Initialize EventBus ─────────────────────────────────
    let event_bus = Arc::new(EventBus::new(256));
    println!("   EventBus: created (capacity: 256)");

    // ── Initialize WAL ──────────────────────────────────────
    let wal_dir = std::env::var("WAL_DIR").unwrap_or_else(|_| "./data".to_string());
    let mut wal = WAL::new(&wal_dir);
    match wal.open() {
        Ok(()) => {
            match wal.recover() {
                Ok(result) => {
                    if result.recovered_from_snapshot || result.wal_entries_replayed > 0 {
                        println!(
                            "   WAL recovery: snapshot={}, replayed={}, corrupted={}",
                            result.recovered_from_snapshot,
                            result.wal_entries_replayed,
                            result.corrupted_records
                        );
                    }
                    println!("   WAL: opened ({} events recovered)", result.event_counter);
                }
                Err(e) => {
                    eprintln!("   WAL recovery failed: {}", e);
                }
            }
        }
        Err(e) => {
            eprintln!("   WAL open failed: {}", e);
        }
    }

    let wal_writer = Arc::new(Mutex::new(wal));

    // Spawn WAL writer task with cancellation
    let wal_cancel = cancel_token.clone();
    let wal_subscriber = wal_writer.clone();
    let mut wal_rx = event_bus.subscribe();
    let wal_handle = tokio::spawn(async move {
        loop {
            tokio::select! {
                result = wal_rx.recv() => {
                    match result {
                        Ok(event) => {
                            let mut w = wal_subscriber.lock().await;
                            if let Err(e) = w.append_event(&event) {
                                eprintln!("[WAL] Failed to write event: {}", e);
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                            eprintln!("[WAL] Lagged {} events", n);
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                            break;
                        }
                    }
                }
                _ = wal_cancel.cancelled() => {
                    break;
                }
            }
        }
    });

    // ── Initialize Subsystems ───────────────────────────────
    let lifecycle_config = agent_world_engine::lifecycle::LifecycleConfig {
        childhood_ticks: genesis_config.lifecycle.childhood_ticks,
        adult_ticks: genesis_config.lifecycle.adult_ticks,
        elder_ticks: genesis_config.lifecycle.elder_ticks,
        death_grace_ticks: genesis_config.lifecycle.death_grace_ticks,
    };

    let mut subsystem_registry = SubsystemRegistry::new();
    // CRITICAL: InterventionChecker runs FIRST — before any other subsystem.
    // This ensures all safety checks happen before token burn, death judgment, etc.
    let intervention_config = agent_world_engine::world::InterventionSubsystemConfig::default();
    subsystem_registry.register(Box::new(
        agent_world_engine::world::InterventionCheckerSubsystem::new(intervention_config),
    ));
    subsystem_registry.register(Box::new(TokenBurnSubsystem::new(
        TokenBurnEngine::with_defaults(),
    )));
    subsystem_registry.register(Box::new(DeathJudgmentSubsystem::new(
        genesis_config.lifecycle.death_grace_ticks,
    )));
    // CRITICAL: LifecycleMachine is now wired into the tick loop
    subsystem_registry.register(Box::new(LifecycleAgingSubsystem::new(lifecycle_config)));
    // CRITICAL: ReputationSystem is now wired into the tick loop for decay
    let reputation_decay = ReputationDecaySubsystem::new_with_event_bus(
        ReputationConfig::default(),
        event_bus.as_ref().clone(),
    );
    subsystem_registry.register(Box::new(reputation_decay));
    subsystem_registry.register(Box::new(EventBroadcastSubsystem::new(event_bus.clone())));
    // CRITICAL: EvolutionSubsystem for skill trees, mutations, and natural selection
    let evolution_config = EvolutionSubsystemConfig {
        skill_max_level: genesis_config.evolution.skill_max_level,
        mutation_rate: genesis_config.evolution.mutation_rate,
        evaluation_interval: genesis_config.evolution.evaluation_interval,
        max_agents: genesis_config.world.max_agents,
        inactivity_threshold: genesis_config.evolution.inactivity_threshold,
        initial_tokens: genesis_config.economy.initial_tokens,
        passive_xp_per_tick: genesis_config.evolution.passive_xp_per_tick,
        mutation_boost_xp: 75.0,
        mutation_decay_xp: 30.0,
        mutation_new_skill_xp: 50.0,
        offspring_mutation: OffspringMutationConfig {
            base_offspring_mutation_rate: genesis_config.evolution.offspring_mutation_rate,
            max_offspring_mutations: genesis_config.evolution.max_offspring_mutations,
            personality_dimensions: genesis_config.evolution.personality_dimensions,
            personality_shift_magnitude: genesis_config.evolution.personality_shift_magnitude,
            skill_level_jump_range: genesis_config.evolution.skill_level_jump_range,
            skill_level_drop_range: genesis_config.evolution.skill_level_drop_range,
            env_pressure_multiplier: genesis_config.evolution.env_pressure_multiplier,
            heritable_strengthen_chance: genesis_config.evolution.heritable_strengthen_chance,
            heritable_disappear_chance: genesis_config.evolution.heritable_disappear_chance,
        },
        crossover_personality_blend: 0.5,
    };
    subsystem_registry.register(Box::new(EvolutionSubsystem::new(evolution_config)));
    println!(
        "   SubsystemRegistry: {} subsystems registered",
        subsystem_registry.len()
    );

    // ── Initialize WorldState ───────────────────────────────
    // Try to restore from SQLite persistence first
    let persistence_db_path = std::env::var("PERSISTENCE_DB")
        .unwrap_or_else(|_| "./data/world.db".to_string());
    let persistence: Option<Arc<SqlitePersistence>> = match SqlitePersistence::open(std::path::Path::new(&persistence_db_path)) {
        Ok(db) => {
            println!("   Persistence: opened {}", persistence_db_path);
            Some(Arc::new(db))
        }
        Err(e) => {
            eprintln!("   Persistence: failed to open {}: {}", persistence_db_path, e);
            None
        }
    };

    let (initial_tick, initial_agents) = match &persistence {
        Some(db) => match db.load_latest_snapshot() {
            Ok(Some(snapshot)) => {
                let (tick, agents) = snapshot.to_world_state_parts();
                println!(
                    "   Persistence: restored tick={}, agents={}",
                    tick,
                    agents.len()
                );
                (tick, agents)
            }
            Ok(None) => {
                println!("   Persistence: no previous snapshot found, starting fresh");
                (0u64, vec![])
            }
            Err(e) => {
                eprintln!("   Persistence: failed to load snapshot: {}", e);
                (0u64, vec![])
            }
        },
        None => (0u64, vec![]),
    };

    let world_state = Arc::new(Mutex::new(WorldState::new(
        event_bus.clone(),
        subsystem_registry,
        initial_agents,
    )));
    // Restore tick counter if recovering from a snapshot
    if initial_tick > 0 {
        let mut state = world_state.lock().await;
        state.set_tick(initial_tick);
    }
    println!("   WorldState: initialized (tick={})", initial_tick);

    // ── Initialize Scheduler ────────────────────────────────
    let scheduler = Scheduler::new(tick_interval, world_state.clone());
    let scheduler_cancel = scheduler.cancel_token();
    let scheduler_handle = tokio::spawn(scheduler.run());
    println!(
        "   Scheduler: tick interval {}ms",
        genesis_config.world.tick_interval_ms
    );

    // ── Initialize TimeCapsule ──────────────────────────────
    let snapshot_store = SnapshotStore::new("./data/snapshots.db")
        .ok()
        .map(|s| Arc::new(Mutex::new(s)));

    let snapshot_interval: u64 = std::env::var("SNAPSHOT_INTERVAL")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(500);

    // Spawn periodic snapshot task with cancellation and non-blocking I/O
    if let Some(ref store) = snapshot_store {
        let store_clone = store.clone();
        let state_clone = world_state.clone();
        let mut snapshot_rx = event_bus.subscribe();
        let snapshot_cancel = cancel_token.clone();
        let snapshot_handle = tokio::spawn(async move {
            loop {
                tokio::select! {
                    result = snapshot_rx.recv() => {
                        match result {
                            Ok(event) => {
                                if let WorldEvent::TickAdvanced { tick } = event {
                                    if tick > 0 && tick % snapshot_interval == 0 {
                                        // Clone the snapshot data under the lock,
                                        // then release the lock before doing I/O
                                        let snapshot = {
                                            let state = state_clone.lock().await;
                                            agent_world_engine::time_capsule::build_snapshot(
                                                tick,
                                                &state.agents,
                                                &[],
                                            )
                                        };
                                        // Lock the store separately for I/O
                                        let s = store_clone.lock().await;
                                        if let Err(e) = s.save(&snapshot) {
                                            eprintln!("[TimeCapsule] Failed to save snapshot: {}", e);
                                        }
                                    }
                                }
                            }
                            Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                                eprintln!("[TimeCapsule] Lagged {} events", n);
                            }
                            Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                                break;
                            }
                        }
                    }
                    _ = snapshot_cancel.cancelled() => {
                        break;
                    }
                }
            }
        });
        println!("   TimeCapsule: snapshot every {} ticks", snapshot_interval);
        // Keep the handle alive for graceful shutdown
        tokio::spawn(async move {
            let _ = snapshot_handle.await;
        });
    }

    // ── Initialize Persistence Snapshot Task ─────────────
    let persistence_interval: u64 = std::env::var("PERSISTENCE_INTERVAL")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1000);
    let persistence_keep: usize = std::env::var("PERSISTENCE_KEEP")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(5);

    if let Some(ref db) = persistence {
        let persistence_db = Arc::clone(db);
        let persist_state = world_state.clone();
        let mut persist_rx = event_bus.subscribe();
        let persist_cancel = cancel_token.clone();

        let persistence_handle = tokio::spawn(async move {
            loop {
                tokio::select! {
                    result = persist_rx.recv() => {
                        match result {
                            Ok(event) => {
                                if let WorldEvent::TickAdvanced { tick } = event {
                                    if tick > 0 && tick % persistence_interval == 0 {
                                        // Clone snapshot data under lock, release before I/O
                                        let snapshot_data = {
                                            let state = persist_state.lock().await;
                                            SerializableWorldState::from_world_state(
                                                tick,
                                                &state.agents,
                                            )
                                        };
                                        // Background save via spawn_blocking
                                        let db = persistence_db.clone();
                                        let keep = persistence_keep;
                                        tokio::task::spawn_blocking(move || {
                                            if let Err(e) = db.save_snapshot(&snapshot_data) {
                                                eprintln!("[Persistence] Failed to save snapshot: {}", e);
                                            }
                                            if let Err(e) = db.prune_snapshots(keep) {
                                                eprintln!("[Persistence] Failed to prune old snapshots: {}", e);
                                            }
                                        }).await.ok();
                                    }
                                }
                            }
                            Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                                eprintln!("[Persistence] Lagged {} events", n);
                            }
                            Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                                break;
                            }
                        }
                    }
                    _ = persist_cancel.cancelled() => {
                        break;
                    }
                }
            }
        });
        println!("   Persistence: snapshot every {} ticks", persistence_interval);
        tokio::spawn(async move {
            let _ = persistence_handle.await;
        });
    }

    // ── Initialize ReputationSystem (standalone for API access) ──
    // This is kept alive in Arc<Mutex<>> and exposed via AppState
    // The tick-driven decay is handled by ReputationDecaySubsystem above
    let reputation_system = Arc::new(Mutex::new(ReputationSystem::with_event_bus(
        ReputationConfig::default(),
        event_bus.as_ref().clone(),
    )));
    println!("   ReputationSystem: initialized");

    // ── Initialize Marketplace ──────────────────────────────
    // CRITICAL: Marketplace is kept alive in Arc<Mutex<>> for the full process lifetime
    let marketplace = Arc::new(Mutex::new(Marketplace::with_event_bus(
        event_bus.as_ref().clone(),
    )));
    println!("   Marketplace: initialized");

    // ── Initialize TaskBoard ────────────────────────────────
    let task_board = Arc::new(Mutex::new(TaskBoard::with_event_bus(
        event_bus.as_ref().clone(),
    )));
    println!("   TaskBoard: initialized");

    // ── Initialize OrganizationStore ────────────────────────
    let org_store = Arc::new(Mutex::new(OrganizationStore::with_event_bus(
        event_bus.as_ref().clone(),
    )));
    println!("   OrganizationStore: initialized");

    // ── Initialize StockMarket ────────────────────────────
    let stock_market = Arc::new(Mutex::new(StockMarket::with_event_bus(
        event_bus.as_ref().clone(),
    )));
    println!("   StockMarket: initialized");

    // ── Initialize GovernanceSystem ────────────────────────
    let governance = Arc::new(Mutex::new(GovernanceSystem::with_shared_event_bus(
        event_bus.clone(),
    )));
    println!("   GovernanceSystem: initialized");

    // ── Initialize Banking System ──────────────────────────
    let banking_system = Arc::new(Mutex::new(BankingSystem::with_event_bus(
        CentralBankConfig::default(),
        event_bus.as_ref().clone(),
    )));
    println!("   BankingSystem: initialized");

    // ── Initialize Investment System ───────────────────────
    let investment_system = Arc::new(Mutex::new(InvestmentSystem::with_event_bus(
        event_bus.as_ref().clone(),
    )));
    println!("   InvestmentSystem: initialized");

    // ── Initialize Federation Engine ──────────────────────
    let federation = Arc::new(Mutex::new(FederationEngine::with_shared_event_bus(
        event_bus.clone(),
    )));
    println!("   FederationEngine: initialized");

    // ── Initialize ConfigManager (hot-reload) ───────────────
    if std::path::Path::new(&genesis_path).exists() {
        match ConfigManager::new(&genesis_path, Some(event_bus.clone())) {
            Ok(config_mgr) => match agent_world_engine::config::spawn_config_watcher(Arc::new(config_mgr)) {
                Ok((_watcher_handle, _cancel_tx)) => {
                    println!("   ConfigManager: watching {} for changes", genesis_path);
                }
                Err(e) => {
                    eprintln!("   ConfigManager: watcher failed: {}", e);
                }
            },
            Err(e) => {
                eprintln!("   ConfigManager: failed to initialize: {}", e);
            }
        }
    }

    // ── Initialize gRPC Server ──────────────────────────────
    let grpc_registry = Arc::new(AgentRegistry::new(event_bus.clone()));
    let grpc_router = Arc::new(MessageRouter::new(grpc_registry.clone()));
    let grpc_service = A2aServiceImpl::new(grpc_registry, grpc_router);
    let grpc_addr_str =
        std::env::var("GRPC_ADDR").unwrap_or_else(|_| "0.0.0.0:50051".to_string());
    let grpc_addr: std::net::SocketAddr = grpc_addr_str
        .parse()
        .expect("Invalid GRPC_ADDR");
    println!("   gRPC server: {}", grpc_addr);

    // ── Initialize API Key Store ───────────────────────────
    let api_key_store = match ApiKeyStore::from_env() {
        Some(store) => {
            println!("   API Auth: enabled ({} keys loaded)", store.key_count());
            Some(Arc::new(store))
        }
        None => {
            let require_auth = std::env::var("REQUIRE_AUTH")
                .ok()
                .map(|v| v.to_lowercase() == "true")
                .unwrap_or(false);
            if require_auth {
                panic!("REQUIRE_AUTH=true but API_KEYS is not set or empty — refusing to start with unauthenticated v2 endpoints");
            }
            tracing::warn!("API_KEYS not set, v2 endpoints are unauthenticated");
            println!("   API Auth: DISABLED (set API_KEYS env var to enable)");
            None
        }
    };

    // ── Initialize HTTP/SSE Server ──────────────────────────
    let (tick_tx, tick_rx) = watch::channel(0u64);
    let governance_metrics = agent_world_engine::organization::GovernanceMetricsCollector::new(&event_bus);

    // ── Initialize Federation Subsystem ─────────────────────
    let migration_policy = MigrationPolicy {
        enabled: genesis_config.migration.enabled,
        daily_quota: genesis_config.migration.daily_quota,
        weekly_quota: genesis_config.migration.weekly_quota,
        min_reputation: genesis_config.migration.min_reputation,
        token_cost: genesis_config.migration.token_cost,
        resource_tax_rate: genesis_config.migration.resource_tax_rate,
        require_skill_certification: genesis_config.migration.require_skill_certification,
        blocked_skills: genesis_config.migration.blocked_skills.clone(),
        cooldown_ticks: genesis_config.migration.cooldown_ticks,
    };
    let federation_registry = Arc::new(Mutex::new(
        WorldRegistry::new(event_bus.clone())
            .with_heartbeat_timeout(genesis_config.federation.heartbeat_timeout_secs),
    ));
    let migration_manager = Arc::new(Mutex::new(
        MigrationManager::new(migration_policy, event_bus.clone()),
    ));
    println!("   Federation: WorldRegistry + MigrationManager initialized");

    let app_state = AppState {
        board: task_board,
        wal: wal_writer.clone(),
        event_bus: event_bus.clone(),
        agents: Arc::new(Mutex::new(Vec::new())),
        messages: Arc::new(Mutex::new(Vec::new())),
        tick_tx,
        tick_rx,
        snapshot_store,
        marketplace: Some(marketplace),
        reputation_system: Some(reputation_system),
        org_store: Some(org_store),
        stock_market: Some(stock_market),
        governance: Some(governance),
        banking_system: Some(banking_system),
        trace_store: Some(Arc::new(Mutex::new(agent_world_engine::tracing::TraceStore::new()))),
        external_agents: Arc::new(Mutex::new(std::collections::HashMap::new())),
        governance_metrics: Some(Arc::new(Mutex::new(governance_metrics))),
        building_manager: Arc::new(Mutex::new(agent_world_engine::world::map::building::BuildingManager::new())),
        human_store: Arc::new(Mutex::new(agent_world_engine::human::store::HumanParticipationStore::new())),
        auth_store: Arc::new(Mutex::new(agent_world_engine::auth::AuthStore::new(
            &std::env::var("JWT_SECRET").unwrap_or_else(|_| "change-me-in-production".to_string())
        ))),
        investment_system: Some(investment_system),
        rule_engine: Some(Arc::new(Mutex::new(agent_world_engine::organization::rule_engine::RuleEngine::with_event_bus(event_bus.clone())))),
        tool_marketplace: Some(Arc::new(Mutex::new(agent_world_engine::economy::tool_marketplace::ToolMarketplace::with_shared_event_bus(event_bus.clone())))),
        federation: Some(federation),        federation_registry: Some(federation_registry),
        migration_manager: Some(migration_manager),
        api_key_store,
        experiment_store: Arc::new(Mutex::new(Vec::new())),
    };
    let app = api::build_full_router(app_state);

    let host = std::env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let port: u16 = std::env::var("PORT")
        .unwrap_or_else(|_| "3000".to_string())
        .parse()
        .expect("PORT must be a valid u16");
    let http_addr: std::net::SocketAddr = format!("{}:{}", host, port)
        .parse()
        .expect("Invalid HOST:PORT");

    println!("   HTTP API: http://{}", http_addr);
    println!("   SSE endpoint: http://{}/api/v1/world/events", http_addr);
    println!("   Metrics: http://{}/metrics", http_addr);
    println!("   Status: ready");

    // ── Spawn metrics sync task ────────────────────────────
    // Periodically sync Prometheus gauges from world state.
    let metrics_state = world_state.clone();
    let mut metrics_rx = event_bus.subscribe();
    let metrics_cancel = cancel_token.clone();
    let metrics_handle = tokio::spawn(async move {
        loop {
            tokio::select! {
                result = metrics_rx.recv() => {
                    match result {
                        Ok(event) => {
                            match &event {
                                WorldEvent::TickAdvanced { tick } => {
                                    agent_world_engine::observability::TICK_TOTAL.inc();
                                    let state = metrics_state.lock().await;
                                    let alive = state.agents.len() as i64;
                                    agent_world_engine::observability::AGENTS_ALIVE.set(alive);
                                    agent_world_engine::observability::log_tick(
                                        *tick, alive as usize, 0, 0,
                                    );
                                }
                                WorldEvent::AgentDied { agent_id, .. } => {
                                    agent_world_engine::observability::log_agent_death(
                                        agent_id, "natural",
                                    );
                                }
                                WorldEvent::TransactionCompleted { .. } => {
                                    agent_world_engine::observability::TRANSACTIONS_TOTAL.inc();
                                }
                                _ => {}
                            }
                            agent_world_engine::observability::EVENTS_PUBLISHED_TOTAL.inc();
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                            eprintln!("[Metrics] Lagged {} events", n);
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                            break;
                        }
                    }
                }
                _ = metrics_cancel.cancelled() => {
                    break;
                }
            }
        }
    });
    // Keep the handle alive
    tokio::spawn(async move { let _ = metrics_handle.await; });

    let http_listener = tokio::net::TcpListener::bind(http_addr).await.unwrap();
    let http_server = axum::serve(http_listener, app);

    // Spawn gRPC server as a separate task
    let grpc_handle = tokio::spawn(async move {
        tonic::transport::Server::builder()
            .add_service(A2aServiceServer::new(grpc_service))
            .serve(grpc_addr)
            .await
    });

    // ── Run all servers ─────────────────────────────────────
    let shutdown_wal = wal_writer.clone();
    let shutdown_state = world_state.clone();
    tokio::select! {
        result = http_server => {
            if let Err(e) = result {
                eprintln!("HTTP server error: {}", e);
            }
        }
        result = grpc_handle => {
            match result {
                Ok(Ok(())) => {}
                Ok(Err(e)) => eprintln!("gRPC server error: {}", e),
                Err(e) => eprintln!("gRPC task error: {}", e),
            }
        }
        _ = tokio::signal::ctrl_c() => {
            println!("\n[Server] SIGINT received, shutting down gracefully...");
        }
    }

    // ── Graceful shutdown ───────────────────────────────────
    // Cancel all background tasks
    cancel_token.cancel();
    scheduler_cancel.cancel();

    // Wait for scheduler to stop
    let _ = scheduler_handle.await;

    // Wait for WAL writer to stop (it will exit via cancel_token)
    let _ = wal_handle.await;

    // Take final WAL snapshot with actual state
    {
        let state = shutdown_state.lock().await;
        let mut w = shutdown_wal.lock().await;
        if let Err(e) = w.take_snapshot(&[], state.current_tick()) {
            eprintln!("[WAL] Final snapshot failed: {}", e);
        }
        w.close();
    }

    // Save final persistence snapshot (outside the lock)
    if let Some(ref db) = persistence {
        let snapshot = {
            let state = shutdown_state.lock().await;
            SerializableWorldState::from_world_state(
                state.current_tick(),
                &state.agents,
            )
        };
        if let Err(e) = db.save_snapshot(&snapshot) {
            eprintln!("[Persistence] Final snapshot failed: {}", e);
        } else {
            println!("[Persistence] Final snapshot saved at tick {}", snapshot.tick);
        }
    }

    println!("[Server] Shutdown complete.");
}
