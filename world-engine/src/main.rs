//! # Binary Entry Point
//!
//! Bootstraps the agent-world server: loads genesis config, initializes
//! all subsystems (banking, stock market, marketplace, reputation, tasks,
//! organizations, governance, A2A federation, evolution, WAL), mounts the
//! Axum REST router, starts the tick loop, and listens for shutdown.
//!
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{Mutex, watch};
use tokio_util::sync::CancellationToken;

use agent_world_engine::a2a::registry::AgentRegistry;
use agent_world_engine::a2a::router::MessageRouter;
use agent_world_engine::a2a::service::A2aServiceImpl;
use agent_world_engine::a2a::world_message_router::WorldMessageRouter;
use agent_world_engine::a2a::federation::FederationEngine;
use agent_world_engine::agentworld::a2a::v1::a2a_service_server::A2aServiceServer;
use agent_world_engine::api::{self, AppState};
use agent_world_engine::api_auth::ApiKeyStore;
use agent_world_engine::config::{ConfigManager, GenesisConfig};
use agent_world_engine::economy::banking::{BankingSystem, CentralBankConfig};
use agent_world_engine::economy::escrow::EscrowManager;
use agent_world_engine::economy::inheritance::{InheritanceConfig, InheritanceSystem};
use agent_world_engine::economy::mentorship::{MentorshipConfig, MentorshipSystem};
use agent_world_engine::economy::trust::{TrustConfig, TrustNetwork};
use agent_world_engine::economy::investment::InvestmentSystem;
use agent_world_engine::economy::marketplace::Marketplace;
use agent_world_engine::economy::reputation::{ReputationConfig, ReputationSystem};
use agent_world_engine::economy::stock_market::StockMarket;
use agent_world_engine::economy::task::TaskBoard;
use agent_world_engine::economy::token_burn::TokenBurnEngine;
use agent_world_engine::organization::org::OrganizationStore;
use agent_world_engine::organization::governance::GovernanceSystem;
use agent_world_engine::organization::legislation_cycle::{LegislationCycleConfig, LegislationCycleEngine};
use agent_world_engine::time_capsule::SnapshotStore;
use agent_world_engine::wal::WAL;
use agent_world_engine::world::event::WorldEvent;
use agent_world_engine::world::state::EventBus;
use agent_world_engine::world::subsystem::SubsystemRegistry;
use agent_world_engine::world::subsystems::{
    DeathJudgmentSubsystem, EscrowExpirySubsystem, EventBroadcastSubsystem,
    LifecycleAgingSubsystem, MentorshipProgressSubsystem, ReputationDecaySubsystem,
    TokenBurnSubsystem, TrustDecaySubsystem,
};
use agent_world_engine::evolution::{EvolutionSubsystem, subsystem::EvolutionSubsystemConfig};
use agent_world_engine::evolution::mutation::OffspringMutationConfig;
use agent_world_engine::persistence::{SerializableWorldState, SqlitePersistence, StatePersistence};
use agent_world_engine::world::{Scheduler, WorldState};
use agent_world_engine::federation::{MigrationManager, MigrationPolicy, WorldRegistry};
use agent_world_engine::human_agent::{HumanActionQueue, HumanAgentRegistry, HumanAgentSubsystem};

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
    let event_bus = Arc::new(EventBus::new(genesis_config.world.event_bus_capacity));
    println!("   EventBus: created (capacity: {})", genesis_config.world.event_bus_capacity);

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

    // ── Phase 5.5: Human-as-Agent shared state ──────────────
    // Created before the SubsystemRegistry so the HumanAgentSubsystem can
    // share these handles with AppState.
    let human_action_queue = HumanActionQueue::shared();
    let human_agent_registry = HumanAgentRegistry::shared();

    let mut subsystem_registry = SubsystemRegistry::new();
    // CRITICAL: InterventionChecker runs FIRST — before any other subsystem.
    // This ensures all safety checks happen before token burn, death judgment, etc.
    let intervention_config = agent_world_engine::world::InterventionSubsystemConfig::default();
    subsystem_registry.register(Box::new(
        agent_world_engine::world::InterventionCheckerSubsystem::new(intervention_config),
    ));
    // Phase 5.5: HumanAgentSubsystem runs immediately after the InterventionChecker
    // so queued human actions land before token burn and death judgment evaluate.
    subsystem_registry.register(Box::new(HumanAgentSubsystem::new(
        human_action_queue.clone(),
        human_agent_registry.clone(),
    )));
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
        mutation_boost_xp: genesis_config.evolution.mutation_boost_xp,
        mutation_decay_xp: genesis_config.evolution.mutation_decay_xp,
        mutation_new_skill_xp: genesis_config.evolution.mutation_new_skill_xp,
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
        crossover_personality_blend: genesis_config.evolution.crossover_personality_blend,
    };
    subsystem_registry.register(Box::new(EvolutionSubsystem::new(evolution_config)));
    println!(
        "   SubsystemRegistry: {} subsystems registered",
        subsystem_registry.len()
    );

    // ── Initialize Plugin System ────────────────────────────
    let plugin_manager: agent_world_engine::plugin::SharedPluginManager =
        Arc::new(Mutex::new(agent_world_engine::plugin::PluginManager::new(event_bus.clone())));

    // ── Register Economy Tick Subsystems ──────────────────
    let trust_decay = TrustDecaySubsystem::new_with_event_bus(
        TrustConfig::default(),
        event_bus.as_ref().clone(),
    );
    subsystem_registry.register(Box::new(trust_decay));

    let mentorship_progress = MentorshipProgressSubsystem::new_with_event_bus(
        MentorshipConfig::default(),
        event_bus.as_ref().clone(),
    );
    subsystem_registry.register(Box::new(mentorship_progress));

    let escrow_expiry = EscrowExpirySubsystem::new_with_event_bus(
        event_bus.as_ref().clone(),
    );
    subsystem_registry.register(Box::new(escrow_expiry));
    println!(
        "   SubsystemRegistry: {} subsystems registered (with economy tick subsystems)",
        subsystem_registry.len()
    );

    // Register the plugin bridge as the last subsystem so plugin tick hooks
    // can observe state set by all built-in subsystems.
    subsystem_registry.register(Box::new(
        agent_world_engine::plugin::PluginSubsystemBridge::new(plugin_manager.clone()),
    ));
    println!(
        "   PluginManager: initialized (subsystem bridge registered, total subsystems: {})",
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

    // ── Initialize LegislationCycleEngine ──────────────────
    let legislation_cycle_engine = Arc::new(Mutex::new(
        LegislationCycleEngine::with_event_bus(
            LegislationCycleConfig::default(),
            event_bus.clone(),
        ),
    ));
    println!("   LegislationCycleEngine: initialized");

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

    // ── Initialize EscrowManager ────────────────────────
    let escrow_manager = Arc::new(Mutex::new(EscrowManager::with_event_bus(
        event_bus.as_ref().clone(),
    )));
    println!("   EscrowManager: initialized");

    // ── Initialize TrustNetwork ─────────────────────────
    let trust_network = Arc::new(Mutex::new(TrustNetwork::with_event_bus(
        TrustConfig::default(),
        event_bus.as_ref().clone(),
    )));
    println!("   TrustNetwork: initialized");

    // ── Initialize MentorshipSystem ─────────────────────
    let mentorship_system = Arc::new(Mutex::new(MentorshipSystem::with_event_bus(
        MentorshipConfig::default(),
        event_bus.as_ref().clone(),
    )));
    println!("   MentorshipSystem: initialized");

    // ── Initialize InheritanceSystem ─────────────────────
    let inheritance_system = Arc::new(Mutex::new(InheritanceSystem::with_event_bus(
        InheritanceConfig::default(),
        event_bus.as_ref().clone(),
    )));
    println!("   InheritanceSystem: initialized");

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
    let world_msg_router = Arc::new(WorldMessageRouter::new());
    let grpc_service = A2aServiceImpl::new(grpc_registry, grpc_router, world_msg_router.clone())
        .with_external_agent_resources(
            genesis_config.economy.external_agent_initial_tokens,
            genesis_config.economy.external_agent_initial_money,
        );
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
    let trade_manager = Arc::new(Mutex::new(
        agent_world_engine::federation::CrossWorldTradeManager::new(event_bus.clone()),
    ));
    // Phase 5.7: SubWorldManager shares the migration manager for inbound transfers.
    let subworld_manager = Arc::new(
        agent_world_engine::federation::SubWorldManager::new(
            event_bus.clone(),
            Arc::new(MigrationManager::new(
                agent_world_engine::federation::MigrationPolicy::default(),
                event_bus.clone(),
            )),
        ),
    );
    println!("   Federation: WorldRegistry + MigrationManager + CrossWorldTradeManager + SubWorldManager initialized");

    // ── Initialize A/B Experiment Store ─────────────────────
    let ab_experiment_store: api::SharedABExperimentStore =
        Arc::new(Mutex::new(Vec::new()));
    println!("   ABExperimentStore: initialized");

    let mut app_state = AppState::new(task_board, wal_writer.clone(), api::TestOverrides {
        event_bus: Some(event_bus.clone()),
        tick_tx: Some(tick_tx),
        tick_rx: Some(tick_rx),
        snapshot_store,
        marketplace: Some(marketplace),
        reputation_system: Some(reputation_system),
        org_store: Some(org_store),
        stock_market: Some(stock_market),
        governance: Some(governance),
        banking_system: Some(banking_system),
        trace_store: Some(Arc::new(Mutex::new(agent_world_engine::tracing::TraceStore::new()))),
        governance_metrics: Some(Arc::new(Mutex::new(governance_metrics))),
        investment_system: Some(investment_system),
        rule_engine: Some(Arc::new(Mutex::new(agent_world_engine::organization::rule_engine::RuleEngine::with_event_bus(event_bus.clone())))),
        tool_marketplace: Some(Arc::new(Mutex::new(agent_world_engine::economy::tool_marketplace::ToolMarketplace::with_shared_event_bus(event_bus.clone())))),
        federation: Some(federation),
        federation_registry: Some(federation_registry),
        migration_manager: Some(migration_manager),
        trade_manager: Some(trade_manager),
        subworld_manager: Some(subworld_manager),
        auth_store: Some(Arc::new(Mutex::new(agent_world_engine::auth::AuthStore::new(
            &std::env::var("JWT_SECRET").unwrap_or_else(|_| {
                use rand::Rng;
                let secret: String = rand::thread_rng()
                    .sample_iter(&rand::distributions::Alphanumeric)
                    .take(48)
                    .map(char::from)
                    .collect();
                tracing::warn!(
                    "JWT_SECRET env var not set — generated a random secret for this session. \
                     Set JWT_SECRET in your environment for production use."
                );
                secret
            })
        )))),
        api_key_store,
        ab_experiment_store: Some(ab_experiment_store),
        plugin_manager: Some(plugin_manager),
        providers: Some(std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new()))),
        agent_models: Some(std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new()))),
        diary_store: Some(std::sync::Arc::new(tokio::sync::Mutex::new(
            agent_world_engine::api_diary::DiaryStore::new(2000),
        ))),
        feed_store: Some(std::sync::Arc::new(tokio::sync::Mutex::new(
            agent_world_engine::api_feed::FeedStore::new(),
        ))),
        escrow_manager: Some(escrow_manager),
        trust_network: Some(trust_network),
        mentorship_system: Some(mentorship_system),
        inheritance_system: Some(inheritance_system),
        legislation_cycle_engine: Some(legislation_cycle_engine),
        world_state: Some(world_state.clone()),
        genesis_config: Some(Arc::new(genesis_config.clone())),
        human_action_queue: Some(human_action_queue),
        human_agent_registry: Some(human_agent_registry),
    });
    // Wire the WorldMessageRouter into the AppState for Oracle/Bounty delivery
    app_state.world_msg_router = Some(world_msg_router);

    // Clone external_agents before app_state is moved into build_full_router.
    // Used by the metrics sync task below to compute token_supply / money_supply.
    let metrics_external_agents = app_state.external_agents.clone();

    // ── Spawn EventBus → tick_tx bridge task ───────────────
    // Without this task, tick_tx stays at its initial value 0 forever, so
    // GET /api/v1/tick always returns {"tick": 0}. The bridge listens for
    // TickAdvanced events from the EventBus and forwards the tick value.
    let bridge_tick_tx = app_state.tick_tx.clone();
    let mut bridge_rx = event_bus.subscribe();
    let bridge_cancel = cancel_token.clone();
    let bridge_handle = tokio::spawn(async move {
        loop {
            tokio::select! {
                result = bridge_rx.recv() => {
                    match result {
                        Ok(event) => {
                            if let WorldEvent::TickAdvanced { tick } = event {
                                // send() is infallible as long as at least one Receiver exists.
                                // The watch channel always has the AppState's tick_rx, so this is safe.
                                let _ = bridge_tick_tx.send(tick);
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                            eprintln!("[TickBridge] Lagged {} events", n);
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                            break;
                        }
                    }
                }
                _ = bridge_cancel.cancelled() => {
                    break;
                }
            }
        }
    });
    // Keep the handle alive for graceful shutdown
    tokio::spawn(async move { let _ = bridge_handle.await; });
    println!("   TickBridge: EventBus → tick_tx bridge spawned");

    let app = api::build_full_router(app_state);

    let host = std::env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let port: u16 = std::env::var("PORT")
        .unwrap_or_else(|_| "8080".to_string())
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
                                    // Compute token_supply and money_supply from external agents
                                    let (token_supply, money_supply) = {
                                        let ea = metrics_external_agents.lock().await;
                                        let ts: i64 = ea.values().map(|a| a.tokens as i64).sum();
                                        let ms: i64 = ea.values().map(|a| a.money as i64).sum();
                                        (ts, ms)
                                    };
                                    agent_world_engine::observability::log_tick(
                                        *tick, alive as usize, token_supply, money_supply,
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

    let http_listener = tokio::net::TcpListener::bind(http_addr)
        .await
        .unwrap_or_else(|e| {
            panic!(
                "HTTP bind failed on {}: {}. Is another process using this port? \
                 Set PORT to a different value or stop the conflicting process.",
                http_addr, e
            )
        });
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
