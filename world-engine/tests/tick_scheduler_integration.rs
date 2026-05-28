//! Tick Scheduler Integration Tests (SEN-73).
//!
//! Validates the tick pipeline: Subsystem trait → Scheduler → WorldState.
//!
//! Acceptance criteria:
//! - WorldState.tick() can be called by the Scheduler at a configurable frequency
//! - Each tick executes: Token burn → Death judgment → Rule check → Event broadcast
//! - Tick interval is configurable via genesis.yaml
//! - Integration test verifies 100 ticks without panic

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;

use agent_world_engine::economy::token_burn::{AgentRecord, SkillRecord, TokenBurnEngine};
use agent_world_engine::rules::default_registry;
use agent_world_engine::world::enums::AgentPhase;
use agent_world_engine::world::event::WorldEvent;
use agent_world_engine::world::genesis::GenesisConfig;
use agent_world_engine::world::scheduler::Scheduler;
use agent_world_engine::world::state::EventBus;
use agent_world_engine::world::state::WorldState;
use agent_world_engine::world::subsystem::SubsystemRegistry;
use agent_world_engine::world::subsystems::{
    DeathJudgmentSubsystem, EventBroadcastSubsystem, RuleCheckSubsystem, TokenBurnSubsystem,
};

// ══════════════════════════════════════════════════════════════════════════
// HELPERS
// ══════════════════════════════════════════════════════════════════════════

/// Build a WorldState with the standard pipeline:
/// TokenBurn → DeathJudgment → EventBroadcast
fn make_world_with_individual_subsystems() -> WorldState {
    let bus = Arc::new(EventBus::new(4096));
    let mut registry = SubsystemRegistry::new();

    registry.register(Box::new(TokenBurnSubsystem::new(
        TokenBurnEngine::with_defaults(),
    )));
    registry.register(Box::new(DeathJudgmentSubsystem::new(0)));
    registry.register(Box::new(EventBroadcastSubsystem::new(bus.clone())));

    WorldState::new(bus, registry, vec![])
}

/// Build a WorldState using the rule-engine-based subsystem.
fn make_world_with_rule_subsystem() -> WorldState {
    let bus = Arc::new(EventBus::new(4096));
    let mut registry = SubsystemRegistry::new();

    let rules = default_registry();
    registry.register(Box::new(RuleCheckSubsystem::new(rules)));
    registry.register(Box::new(EventBroadcastSubsystem::new(bus.clone())));

    WorldState::new(bus, registry, vec![])
}

#[allow(dead_code)]
fn make_agent(phase: AgentPhase, tokens: u64) -> (uuid::Uuid, u64, AgentRecord) {
    let id = uuid::Uuid::new_v4();
    (
        id,
        0,
        AgentRecord {
            id,
            name: "test-agent".to_string(),
            phase,
            tokens,
            skills: HashMap::new(),
            personality: String::new(),
            tasks_completed: 0,
            tasks_attempted: 0,
        },
    )
}

fn make_agent_named(name: &str, phase: AgentPhase, tokens: u64) -> (uuid::Uuid, u64, AgentRecord) {
    let id = uuid::Uuid::new_v4();
    (
        id,
        0,
        AgentRecord {
            id,
            name: name.to_string(),
            phase,
            tokens,
            skills: HashMap::new(),
            personality: String::new(),
            tasks_completed: 0,
            tasks_attempted: 0,
        },
    )
}

// ══════════════════════════════════════════════════════════════════════════
// TEST 1: 100 Ticks — No Panic (Individual Subsystems)
// ══════════════════════════════════════════════════════════════════════════

#[test]
fn test_100_ticks_no_panic_individual_subsystems() {
    let mut world = make_world_with_individual_subsystems();

    // Spawn 3 agents with varying token amounts
    let alice = world.spawn_agent("Alice", 500_000, 0);
    let bob = world.spawn_agent("Bob", 300_000, 0);
    let carol = world.spawn_agent("Carol", 200_000, 0);

    // Run 100 ticks
    for tick in 1..=100u64 {
        let events = world.tick();
        assert!(!events.is_empty(), "Tick {} should produce events", tick);
    }

    // Verify final state
    assert_eq!(world.current_tick(), 100);
    assert_eq!(world.living_agent_count(), 3);

    // Verify token deductions
    // Alice: 500_000 - 10*100 = 499_000
    let alice_agent = world.agents.iter().find(|(id, _, _)| *id == alice).unwrap();
    assert_eq!(alice_agent.2.tokens, 500_000 - 10 * 100);

    // Bob: 300_000 - 10*100 = 299_000
    let bob_agent = world.agents.iter().find(|(id, _, _)| *id == bob).unwrap();
    assert_eq!(bob_agent.2.tokens, 300_000 - 10 * 100);

    // Carol: 200_000 - 10*100 = 199_000
    let carol_agent = world.agents.iter().find(|(id, _, _)| *id == carol).unwrap();
    assert_eq!(carol_agent.2.tokens, 200_000 - 10 * 100);
}

// ══════════════════════════════════════════════════════════════════════════
// TEST 2: 100 Ticks — No Panic (Rule-based Subsystem)
// ══════════════════════════════════════════════════════════════════════════

#[test]
fn test_100_ticks_no_panic_rule_subsystem() {
    let mut world = make_world_with_rule_subsystem();

    let alice = world.spawn_agent("Alice", 500_000, 0);
    let bob = world.spawn_agent("Bob", 300_000, 0);

    for tick in 1..=100u64 {
        let events = world.tick();
        assert!(!events.is_empty(), "Tick {} should produce events", tick);
    }

    assert_eq!(world.current_tick(), 100);
    assert_eq!(world.living_agent_count(), 2);

    let alice_agent = world.agents.iter().find(|(id, _, _)| *id == alice).unwrap();
    assert_eq!(alice_agent.2.tokens, 500_000 - 10 * 100);

    let bob_agent = world.agents.iter().find(|(id, _, _)| *id == bob).unwrap();
    assert_eq!(bob_agent.2.tokens, 300_000 - 10 * 100);
}

// ══════════════════════════════════════════════════════════════════════════
// TEST 3: Tick Pipeline Order — Token Burn → Death → Event Broadcast
// ══════════════════════════════════════════════════════════════════════════

#[test]
fn test_tick_pipeline_order_burn_then_death_then_broadcast() {
    let mut world = make_world_with_individual_subsystems();
    let _rx = world.event_bus.subscribe();

    // Spawn agent with exactly 10 tokens (1 tick of burn)
    let agent_id = world.spawn_agent("ShortLived", 10, 0);

    // Tick 1: burn 10 tokens → tokens=0 → death judgment kills → broadcast TickAdvanced
    let events = world.tick();

    // Verify agent is dead
    let agent = world
        .agents
        .iter()
        .find(|(id, _, _)| *id == agent_id)
        .unwrap();
    assert_eq!(agent.2.tokens, 0);
    assert_eq!(agent.2.phase, AgentPhase::Dead);

    // Verify event types are present
    let has_balance_changed = events
        .iter()
        .any(|e| matches!(e, WorldEvent::BalanceChanged { .. }));
    let has_dying = events
        .iter()
        .any(|e| matches!(e, WorldEvent::AgentDying { .. }));
    let has_died = events
        .iter()
        .any(|e| matches!(e, WorldEvent::AgentDied { .. }));
    let has_tick = events
        .iter()
        .any(|e| matches!(e, WorldEvent::TickAdvanced { .. }));

    assert!(
        has_balance_changed,
        "Should have BalanceChanged from token burn"
    );
    assert!(has_dying, "Should have AgentDying from death judgment");
    assert!(has_died, "Should have AgentDied from death judgment");
    assert!(has_tick, "Should have TickAdvanced from event broadcast");

    // Events should be in order: burn, death, tick
    let burn_idx = events
        .iter()
        .position(|e| matches!(e, WorldEvent::BalanceChanged { .. }))
        .unwrap();
    let died_idx = events
        .iter()
        .position(|e| matches!(e, WorldEvent::AgentDied { .. }))
        .unwrap();
    let tick_idx = events
        .iter()
        .position(|e| matches!(e, WorldEvent::TickAdvanced { .. }))
        .unwrap();

    assert!(burn_idx < died_idx, "Burn should come before death");
    assert!(
        died_idx < tick_idx,
        "Death should come before tick broadcast"
    );
}

// ══════════════════════════════════════════════════════════════════════════
// TEST 4: Scheduler Runs at Configured Interval via run_n_ticks
// ══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_scheduler_run_n_ticks_100() {
    let bus = Arc::new(EventBus::new(4096));
    let mut registry = SubsystemRegistry::new();
    registry.register(Box::new(TokenBurnSubsystem::new(
        TokenBurnEngine::with_defaults(),
    )));
    registry.register(Box::new(DeathJudgmentSubsystem::new(0)));
    registry.register(Box::new(EventBroadcastSubsystem::new(bus.clone())));

    let agents = vec![
        make_agent_named("Alice", AgentPhase::Adult, 500_000),
        make_agent_named("Bob", AgentPhase::Adult, 300_000),
    ];

    let state = Arc::new(Mutex::new(WorldState::new(bus, registry, agents)));

    // Run 100 ticks through the Scheduler
    Scheduler::run_n_ticks(&state, 100).await;

    let s = state.lock().await;
    assert_eq!(s.current_tick(), 100);
    assert_eq!(s.living_agent_count(), 2);
}

// ══════════════════════════════════════════════════════════════════════════
// TEST 5: Scheduler Real-Time with Cancellation
// ══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_scheduler_realtime_with_cancel() {
    let bus = Arc::new(EventBus::new(4096));
    let mut registry = SubsystemRegistry::new();
    registry.register(Box::new(TokenBurnSubsystem::new(
        TokenBurnEngine::with_defaults(),
    )));
    registry.register(Box::new(DeathJudgmentSubsystem::new(0)));
    registry.register(Box::new(EventBroadcastSubsystem::new(bus.clone())));

    let agents = vec![make_agent_named("Alice", AgentPhase::Adult, 1_000_000)];
    let state = Arc::new(Mutex::new(WorldState::new(bus, registry, agents)));

    // 50ms interval
    let scheduler = Scheduler::new(Duration::from_millis(50), state.clone());
    let cancel = scheduler.cancel_token();

    let handle = tokio::spawn(scheduler.run());

    // Let it run for ~250ms (should get ~5 ticks)
    tokio::time::sleep(Duration::from_millis(250)).await;
    cancel.cancel();

    handle.await.unwrap();

    let s = state.lock().await;
    let ticks = s.current_tick();
    assert!(ticks >= 3, "Expected at least 3 ticks, got {}", ticks);
    assert!(ticks <= 8, "Expected at most 8 ticks, got {}", ticks);
}

// ══════════════════════════════════════════════════════════════════════════
// TEST 6: Genesis Config Tick Interval
// ══════════════════════════════════════════════════════════════════════════

#[test]
fn test_genesis_config_tick_interval() {
    let yaml = r#"
world:
  tick_interval_ms: 500
"#;
    let config = GenesisConfig::from_yaml(yaml).unwrap();
    assert_eq!(config.tick_interval(), Duration::from_millis(500));
    assert_eq!(config.world.tick_interval_ms, 500);
}

#[test]
fn test_genesis_config_default_tick_interval() {
    let config = GenesisConfig::default();
    assert_eq!(config.tick_interval(), Duration::from_millis(1000));
}

#[test]
fn test_genesis_config_from_actual_file_format() {
    // Simulates the actual genesis.yaml format
    let yaml = r#"
world:
  name: "agent-world-v1"
  tick_interval_ms: 1000
  max_agents: 10
economy:
  initial_tokens: 100000
  think_cost_per_token: 1
  memory_cost_per_kb: 0.1
  communicate_cost: 10
  initial_money: 0
  token_price: 100
  interest_rate: 0.001
lifecycle:
  birth_tokens: 100000
  childhood_ticks: 100
  adult_ticks: 1000
  elder_ticks: 200
  death_grace_ticks: 10
safety:
  max_agents_per_org: 5
  anti_monopoly_threshold: 0.3
  new_agent_protection_ticks: 50
"#;
    let config = GenesisConfig::from_yaml(yaml).unwrap();
    assert_eq!(config.tick_interval(), Duration::from_millis(1000));
    assert_eq!(config.world.name, "agent-world-v1");
    assert_eq!(config.economy.initial_tokens, 100_000);
    assert_eq!(config.lifecycle.death_grace_ticks, 10);
    assert_eq!(config.safety.new_agent_protection_ticks, 50);
}

// ══════════════════════════════════════════════════════════════════════════
// TEST 7: 100 Ticks with Agent Death and Mixed Profiles
// ══════════════════════════════════════════════════════════════════════════

#[test]
fn test_100_ticks_with_death_and_mixed_profiles() {
    let mut world = make_world_with_individual_subsystems();
    let mut rx = world.event_bus.subscribe();

    // Rich: won't die in 100 ticks
    let _rich = world.spawn_agent("Rich", 500_000, 0);
    // Poor: 500 tokens → 50 ticks then dies
    let poor = world.spawn_agent("Poor", 500, 0);
    // Skilled: costs more per tick
    let skilled = world.spawn_agent("Skilled", 300_000, 0);
    // Add skills to skilled agent
    {
        let skilled_agent = world
            .agents
            .iter_mut()
            .find(|(id, _, _)| *id == skilled)
            .unwrap();
        skilled_agent.2.skills.insert(
            "mining".to_string(),
            SkillRecord {
                name: "mining".to_string(),
                level: 5,
                experience: 0.0,
            },
        );
    }

    // Drain spawn events
    let _ = rx.try_recv().unwrap();
    let _ = rx.try_recv().unwrap();
    let _ = rx.try_recv().unwrap();

    // Run 100 ticks
    let mut tick_count = 0u64;
    let mut death_tick: Option<u64> = None;

    for _ in 1..=100u64 {
        tick_count += 1;
        world.tick();

        // Check if poor agent died
        let poor_agent = world.agents.iter().find(|(id, _, _)| *id == poor).unwrap();
        if poor_agent.2.phase == AgentPhase::Dead && death_tick.is_none() {
            death_tick = Some(tick_count);
        }
    }

    // Poor agent should have died at tick 50 (500 tokens / 10 per tick)
    assert_eq!(death_tick, Some(50), "Poor agent should die at tick 50");

    // Rich and skilled should survive
    assert_eq!(world.living_agent_count(), 2);

    // Rich: 500_000 - 10*100 = 499_000
    let rich_agent = world.agents.iter().find(|(id, _, _)| *id == _rich).unwrap();
    assert_eq!(rich_agent.2.tokens, 500_000 - 10 * 100);

    // Skilled: 300_000 - (10 + 5*0.5)*100 = 300_000 - 12*100 = 298_800
    let skilled_agent = world
        .agents
        .iter()
        .find(|(id, _, _)| *id == skilled)
        .unwrap();
    assert_eq!(skilled_agent.2.tokens, 300_000 - 12 * 100);
}

// ══════════════════════════════════════════════════════════════════════════
// TEST 8: Scheduler with GenesisConfig Interval
// ══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_scheduler_with_genesis_config_interval() {
    let config = GenesisConfig::default();

    let bus = Arc::new(EventBus::new(4096));
    let mut registry = SubsystemRegistry::new();
    registry.register(Box::new(TokenBurnSubsystem::new(
        TokenBurnEngine::with_defaults(),
    )));
    registry.register(Box::new(DeathJudgmentSubsystem::new(0)));
    registry.register(Box::new(EventBroadcastSubsystem::new(bus.clone())));

    let agents = vec![make_agent_named("Alice", AgentPhase::Adult, 1_000_000)];
    let state = Arc::new(Mutex::new(WorldState::new(bus, registry, agents)));

    // Create scheduler with interval from genesis config
    let scheduler = Scheduler::new(config.tick_interval(), state.clone());
    let cancel = scheduler.cancel_token();

    let handle = tokio::spawn(scheduler.run());

    // Let it run for ~1.5 seconds (should get ~1-2 ticks at 1000ms interval)
    tokio::time::sleep(Duration::from_millis(1500)).await;
    cancel.cancel();

    handle.await.unwrap();

    let s = state.lock().await;
    let ticks = s.current_tick();
    assert!(ticks >= 1, "Expected at least 1 tick, got {}", ticks);
    assert!(ticks <= 3, "Expected at most 3 ticks, got {}", ticks);
}

// ══════════════════════════════════════════════════════════════════════════
// TEST 9: Events are Broadcast to EventBus on Each Tick
// ══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_events_broadcast_to_bus_per_tick() {
    let bus = Arc::new(EventBus::new(4096));
    let mut rx = bus.subscribe();

    let mut registry = SubsystemRegistry::new();
    registry.register(Box::new(TokenBurnSubsystem::new(
        TokenBurnEngine::with_defaults(),
    )));
    registry.register(Box::new(DeathJudgmentSubsystem::new(0)));
    registry.register(Box::new(EventBroadcastSubsystem::new(bus.clone())));

    let agents = vec![make_agent_named("Alice", AgentPhase::Adult, 1_000)];

    let state = Arc::new(Mutex::new(WorldState::new(bus, registry, agents)));

    Scheduler::run_n_ticks(&state, 5).await;

    // Should receive events from each tick: BalanceChanged + TickAdvanced
    let mut tick_events = 0;
    let mut balance_events = 0;

    while let Ok(event) = rx.try_recv() {
        match event {
            WorldEvent::TickAdvanced { .. } => tick_events += 1,
            WorldEvent::BalanceChanged { .. } => balance_events += 1,
            _ => {}
        }
    }

    assert_eq!(tick_events, 5, "Should have 5 tick events");
    assert_eq!(balance_events, 5, "Should have 5 balance change events");
}

// ══════════════════════════════════════════════════════════════════════════
// TEST 10: Empty World — 100 Ticks No Panic
// ══════════════════════════════════════════════════════════════════════════

#[test]
fn test_100_ticks_empty_world_no_panic() {
    let mut world = make_world_with_individual_subsystems();

    for tick in 1..=100u64 {
        let events = world.tick();
        // Should still get TickAdvanced even with no agents
        assert!(
            !events.is_empty(),
            "Tick {} should produce TickAdvanced",
            tick
        );
    }

    assert_eq!(world.current_tick(), 100);
    assert_eq!(world.living_agent_count(), 0);
}
