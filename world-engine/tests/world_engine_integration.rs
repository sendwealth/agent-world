//! World Engine End-to-End Integration Tests (SEN-25).
//!
//! Validates the complete World Engine stack per acceptance criteria:
//!   1. Full startup flow: config loading → subsystem init → scheduler tick loop
//!   2. Agent lifecycle: spawn → token consumption → death → snapshot
//!   3. 100-tick stability: no panics, all subsystems consistent
//!   4. Ledger consistency: conservation of money in == money out

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::Mutex;
use uuid::Uuid;

use agent_world_engine::economy::token_burn::{
    AgentRecord, ConsumptionConfig, SkillRecord, TokenBurnEngine,
};
use agent_world_engine::economy::{
    EscrowManager, RewardConfig, RewardDistributor, TaskBoard, TaskStatus, TransactionType,
};
use agent_world_engine::world::enums::{AgentPhase, Currency, DeathReason};
use agent_world_engine::world::event::{EventType, WorldEvent};
use agent_world_engine::world::state::EventBus;

// ══════════════════════════════════════════════════════════════════════════
// HELPERS
// ══════════════════════════════════════════════════════════════════════════

/// World state container that wires all subsystems together,
/// simulating the production startup path from `main.rs`.
struct WorldState {
    event_bus: Arc<EventBus>,
    task_board: Arc<Mutex<TaskBoard>>,
    token_engine: TokenBurnEngine,
    reward_distributor: RewardDistributor,
    escrow_manager: EscrowManager,
    /// Agents stored as a Vec so we can pass &mut [AgentRecord] to process_tick.
    agents: Vec<(String, AgentRecord)>,
    tick: u64,
    config: ConsumptionConfig,
}

impl WorldState {
    /// Bootstrap the world from a genesis YAML config string.
    fn from_yaml(yaml: &str) -> Self {
        let value: serde_yaml::Value = serde_yaml::from_str(yaml).unwrap();
        let config = ConsumptionConfig::from_yaml_value(&value);

        let event_bus = Arc::new(EventBus::new(4096));
        let task_board = Arc::new(Mutex::new(TaskBoard::new()));

        Self {
            event_bus,
            task_board,
            token_engine: TokenBurnEngine::new(config.clone()),
            reward_distributor: RewardDistributor::new(RewardConfig::default()),
            escrow_manager: EscrowManager::new(),
            agents: Vec::new(),
            tick: 0,
            config,
        }
    }

    /// Bootstrap with default config (no YAML).
    fn new_default() -> Self {
        let event_bus = Arc::new(EventBus::new(4096));
        let task_board = Arc::new(Mutex::new(TaskBoard::new()));

        Self {
            event_bus,
            task_board,
            token_engine: TokenBurnEngine::with_defaults(),
            reward_distributor: RewardDistributor::new(RewardConfig::default()),
            escrow_manager: EscrowManager::new(),
            agents: Vec::new(),
            tick: 0,
            config: ConsumptionConfig::default(),
        }
    }

    /// Spawn a new agent into the world.
    fn spawn_agent(&mut self, name: &str, tokens: u64) -> String {
        let id = Uuid::new_v4();
        let id_str = id.to_string();

        let record = AgentRecord {
            id,
            name: name.to_string(),
            phase: AgentPhase::Adult,
            tokens,
            skills: HashMap::new(),
        };

        self.agents.push((id_str.clone(), record));
        self.event_bus.emit(WorldEvent::AgentSpawned {
            agent_id: id_str.clone(),
            name: name.to_string(),
        });

        id_str
    }

    /// Advance the world by one tick: burn tokens for all living agents.
    async fn tick(&mut self) -> Vec<String> {
        self.tick += 1;
        let mut dead_agents = Vec::new();

        // Extract just the AgentRecords for batch processing
        let mut agent_records: Vec<AgentRecord> =
            self.agents.iter().map(|(_, r)| r.clone()).collect();

        // Process token burn
        let _burn_result = self
            .token_engine
            .process_tick(self.tick, &mut agent_records);

        // Write back updated token counts
        for (i, (_, ref mut record)) in self.agents.iter_mut().enumerate() {
            record.tokens = agent_records[i].tokens;
        }

        // Check for dead agents
        for (id, agent) in &self.agents {
            if agent.phase != AgentPhase::Dead && agent.tokens == 0 {
                self.event_bus.emit(WorldEvent::AgentDying {
                    agent_id: id.clone(),
                    reason: DeathReason::TokenDepleted,
                    grace_ticks: 0,
                });
                self.event_bus.emit(WorldEvent::AgentDied {
                    agent_id: id.clone(),
                    reason: DeathReason::TokenDepleted,
                });
                dead_agents.push(id.clone());
            }
        }

        // Mark dead agents
        for id in &dead_agents {
            for (agent_id, agent) in &mut self.agents {
                if agent_id == id {
                    agent.phase = AgentPhase::Dead;
                }
            }
        }

        // Process task expiry
        {
            let mut board = self.task_board.lock().await;
            board.process_expiry(self.tick);
        }

        self.event_bus
            .emit(WorldEvent::TickAdvanced { tick: self.tick });
        dead_agents
    }

    /// Take a snapshot of current world state.
    fn snapshot(&self) -> WorldSnapshot {
        let agents_json: HashMap<String, serde_json::Value> = self
            .agents
            .iter()
            .map(|(id, record)| (id.clone(), serde_json::to_value(record).unwrap()))
            .collect();

        WorldSnapshot {
            tick: self.tick,
            agents_json,
            config: self.config.clone(),
        }
    }

    /// Count living agents.
    fn living_agents(&self) -> usize {
        self.agents
            .iter()
            .filter(|(_, a)| a.phase != AgentPhase::Dead)
            .count()
    }

    /// Get agent record by ID.
    fn get_agent(&self, id: &str) -> &AgentRecord {
        self.agents
            .iter()
            .find(|(aid, _)| aid == id)
            .map(|(_, r)| r)
            .unwrap()
    }

    /// Get mutable agent record by ID.
    fn get_agent_mut(&mut self, id: &str) -> &mut AgentRecord {
        self.agents
            .iter_mut()
            .find(|(aid, _)| aid == id)
            .map(|(_, r)| r)
            .unwrap()
    }
}

/// Serializable world snapshot.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct WorldSnapshot {
    tick: u64,
    agents_json: HashMap<String, serde_json::Value>,
    config: ConsumptionConfig,
}

// ══════════════════════════════════════════════════════════════════════════
// TEST 1: Full Startup Flow — Config → Scheduler → Subsystem
// ══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_full_startup_flow_config_to_scheduler_to_subsystem() {
    // ── Step 1: Load config from genesis.yaml ─────────────────────────
    let genesis_yaml = r#"
economy:
  base_burn_per_tick: 10
  skill_cost_per_level: 0.5
  phase_multipliers:
    childhood: 0.5
    adult: 1.0
    elder: 0.7
"#;

    let mut world = WorldState::from_yaml(genesis_yaml);

    // Verify config loaded correctly
    assert_eq!(world.config.base_burn_per_tick, 10.0);
    assert_eq!(world.config.skill_cost_per_level, 0.5);
    assert_eq!(world.config.phase_multipliers.childhood, 0.5);
    assert_eq!(world.config.phase_multipliers.adult, 1.0);
    assert_eq!(world.config.phase_multipliers.elder, 0.7);

    // ── Step 2: Verify subsystem initialization ───────────────────────

    // EventBus should be ready
    let mut rx = world.event_bus.subscribe();

    // TaskBoard should be ready
    {
        let board = world.task_board.lock().await;
        assert!(board.list().is_empty(), "TaskBoard should start empty");
    }

    // TokenBurnEngine should use config
    assert_eq!(
        world.token_engine.calculate_tick_burn(&AgentRecord {
            id: Uuid::new_v4(),
            name: "test".to_string(),
            phase: AgentPhase::Adult,
            tokens: 1000,
            skills: HashMap::new(),
        }),
        10
    );

    // RewardDistributor should be ready
    assert_eq!(world.reward_distributor.ledger().list().len(), 0);

    // EscrowManager should be ready
    assert!(world.escrow_manager.list().is_empty());

    // ── Step 3: Spawn agents ──────────────────────────────────────────

    let alice = world.spawn_agent("Alice", 500_000);
    let bob = world.spawn_agent("Bob", 500_000);

    // Verify spawn events emitted
    let spawn1 = rx.try_recv().unwrap();
    assert_eq!(spawn1.event_type(), EventType::AgentSpawned);
    let spawn2 = rx.try_recv().unwrap();
    assert_eq!(spawn2.event_type(), EventType::AgentSpawned);

    assert_eq!(world.living_agents(), 2);
    assert_eq!(world.agents.len(), 2);

    // ── Step 4: Run scheduler for 20 ticks ────────────────────────────

    for _ in 1..=20 {
        let dead = world.tick().await;
        assert!(dead.is_empty(), "No agents should die in 20 ticks");
    }

    assert_eq!(world.tick, 20);
    assert_eq!(world.living_agents(), 2);

    // Verify tokens have been consumed
    let alice_tokens = world.get_agent(&alice).tokens;
    let bob_tokens = world.get_agent(&bob).tokens;
    assert!(alice_tokens < 500_000, "Alice should have burned tokens");
    assert!(bob_tokens < 500_000, "Bob should have burned tokens");
    assert!(alice_tokens > 0, "Alice should still have tokens");
    assert!(bob_tokens > 0, "Bob should still have tokens");

    // Conservation: remaining + burned = initial
    // 500_000 - 10 * 20 = 499_800
    assert_eq!(alice_tokens, 500_000 - 10 * 20);
    assert_eq!(bob_tokens, 500_000 - 10 * 20);

    // ── Step 5: Verify tick events accumulated ────────────────────────

    let mut tick_events = 0;
    while let Ok(event) = rx.try_recv() {
        if event.event_type() == EventType::TickAdvanced {
            tick_events += 1;
        }
    }
    assert_eq!(tick_events, 20, "Should have 20 tick events");
}

// ══════════════════════════════════════════════════════════════════════════
// TEST 2: Agent Lifecycle — Spawn → Token Consume → Death → Snapshot
// ══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_agent_spawn_consume_die_snapshot() {
    let mut world = WorldState::new_default();
    let mut event_rx = world.event_bus.subscribe();

    // ── Phase 1: Spawn agent with limited tokens ──────────────────────

    // Adult burns 10/tick → will die at tick 100
    let agent_id = world.spawn_agent("ShortLived", 1000);
    let initial_tokens = world.get_agent(&agent_id).tokens;
    assert_eq!(initial_tokens, 1000);

    // Verify spawn event
    let spawn_evt = event_rx.try_recv().unwrap();
    assert_eq!(spawn_evt.event_type(), EventType::AgentSpawned);

    // ── Phase 2: Consume tokens through ticks ─────────────────────────

    for expected_tick in 1..=99u64 {
        let dead = world.tick().await;
        assert!(
            dead.is_empty(),
            "Agent should not die at tick {}",
            expected_tick
        );
        assert_eq!(world.tick, expected_tick);

        let agent = world.get_agent(&agent_id);
        assert_eq!(agent.tokens, 1000 - 10 * expected_tick);
        assert_eq!(agent.phase, AgentPhase::Adult);
    }

    // At tick 99: tokens = 1000 - 990 = 10
    let agent = world.get_agent(&agent_id);
    assert_eq!(agent.tokens, 10);

    // ── Phase 3: Agent runs out of tokens → death ─────────────────────

    let dead = world.tick().await; // tick 100
    assert_eq!(dead.len(), 1, "Agent should die at tick 100");
    assert_eq!(dead[0], agent_id);

    let agent = world.get_agent(&agent_id);
    assert_eq!(agent.phase, AgentPhase::Dead);
    assert_eq!(agent.tokens, 0);

    // Collect events from the death tick — drain all and check for dying/died
    let mut found_dying = false;
    let mut found_died = false;
    while let Ok(event) = event_rx.try_recv() {
        match event.event_type() {
            EventType::AgentDying => found_dying = true,
            EventType::AgentDied => found_died = true,
            _ => {} // TickAdvanced or other events
        }
    }
    assert!(found_dying, "Should have received AgentDying event");
    assert!(found_died, "Should have received AgentDied event");

    // ── Phase 4: Dead agent stops burning ─────────────────────────────

    for _ in 1..=10 {
        let dead = world.tick().await;
        assert!(
            dead.is_empty(),
            "Already dead agent should not trigger death again"
        );
    }

    let agent = world.get_agent(&agent_id);
    assert_eq!(agent.tokens, 0, "Dead agent tokens should stay at 0");
    assert_eq!(agent.phase, AgentPhase::Dead);

    // ── Phase 5: Take snapshot and verify serialization ───────────────

    let snapshot = world.snapshot();
    assert_eq!(snapshot.tick, 110);

    let snapshot_json = serde_json::to_string(&snapshot).unwrap();
    assert!(!snapshot_json.is_empty());

    let restored: WorldSnapshot = serde_json::from_str(&snapshot_json).unwrap();
    assert_eq!(restored.tick, snapshot.tick);
    assert_eq!(restored.agents_json.len(), 1);
    assert_eq!(restored.config.base_burn_per_tick, 10.0);

    // ── Phase 6: Spawn a second agent after snapshot ──────────────────

    let survivor_id = world.spawn_agent("Survivor", 50_000);
    assert_eq!(world.living_agents(), 1); // Only survivor alive
    let survivor = world.get_agent(&survivor_id);
    assert_eq!(survivor.tokens, 50_000);
    assert_eq!(survivor.phase, AgentPhase::Adult);
}

// ══════════════════════════════════════════════════════════════════════════
// TEST 3: 100-Tick Stability — No Panics, All Subsystems Consistent
// ══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_100_tick_stability_no_panics_consistent() {
    let mut world = WorldState::new_default();
    let mut event_rx = world.event_bus.subscribe();

    // ── Setup: spawn 3 agents with varying profiles ───────────────────

    let alice_id = world.spawn_agent("Alice", 500_000);
    world.get_agent_mut(&alice_id).skills.insert(
        "mining".to_string(),
        SkillRecord {
            name: "mining".to_string(),
            level: 5,
            experience: 0.0,
        },
    );

    let bob_id = world.spawn_agent("Bob", 300_000);
    let carol_id = world.spawn_agent("Carol", 200_000);
    world.get_agent_mut(&carol_id).skills.insert(
        "trading".to_string(),
        SkillRecord {
            name: "trading".to_string(),
            level: 3,
            experience: 0.0,
        },
    );

    // Drain spawn events
    let _ = event_rx.try_recv().unwrap();
    let _ = event_rx.try_recv().unwrap();
    let _ = event_rx.try_recv().unwrap();

    // Track task creation for tasks completed
    let mut tasks_created: u32 = 0;
    let mut tasks_completed: u32 = 0;

    // Set up task board balances
    {
        let mut board = world.task_board.lock().await;
        board.set_balance(&alice_id, 50_000);
        board.set_balance(&bob_id, 50_000);
        board.set_balance(&carol_id, 50_000);
    }

    // ── Run 100 ticks ─────────────────────────────────────────────────

    for tick_num in 1..=100u64 {
        // 1. Tick all agents (token burn)
        let dead = world.tick().await;
        assert!(
            dead.is_empty(),
            "No agents should die in 100 ticks with ample tokens"
        );

        // 2. Create tasks periodically (every 20 ticks)
        if tick_num % 20 == 0 {
            let publisher = if tick_num % 40 == 0 {
                &alice_id
            } else {
                &bob_id
            };
            let task_id = {
                let mut board = world.task_board.lock().await;
                board
                    .create_task(
                        format!("Task @ tick {}", tick_num),
                        format!("Integration task created at tick {}", tick_num),
                        500,
                        publisher.clone(),
                        tick_num,
                        Some(tick_num + 200),
                    )
                    .unwrap()
            };
            tasks_created += 1;

            // Complete the task within the same tick
            let assignee = if publisher == &alice_id {
                &bob_id
            } else {
                &carol_id
            };
            let mut board = world.task_board.lock().await;
            board.claim_task(task_id, assignee.clone()).unwrap();
            board.start_task(task_id).unwrap();
            board
                .submit_result(task_id, format!("Done at tick {}", tick_num))
                .unwrap();
            board.review_task(task_id, publisher, true).unwrap();
            board.complete_task(task_id, tick_num).unwrap();
            tasks_completed += 1;
        }

        // 3. Verify agents are alive and consistent at checkpoints
        if tick_num % 25 == 0 {
            assert!(
                world.living_agents() == 3,
                "All 3 agents should be alive at tick {}",
                tick_num
            );

            for (_, agent) in &world.agents {
                assert!(
                    agent.tokens > 0,
                    "Agent should have tokens at tick {}",
                    tick_num
                );
                assert!(
                    agent.phase != AgentPhase::Dead,
                    "Agent should not be dead at tick {}",
                    tick_num
                );
            }
        }
    }

    // ── Final verification ─────────────────────────────────────────────

    // All agents still alive
    assert_eq!(world.living_agents(), 3);

    // Token conservation per agent
    let alice = world.get_agent(&alice_id);
    let bob = world.get_agent(&bob_id);
    let carol = world.get_agent(&carol_id);

    // Alice: base=10, skills: mining(5)*0.5=2.5 → 12/tick → 1200 burned
    assert_eq!(alice.tokens, 500_000 - 12 * 100);
    // Bob: base=10, no skills → 10/tick → 1000 burned
    assert_eq!(bob.tokens, 300_000 - 10 * 100);
    // Carol: base=10, skills: trading(3)*0.5=1.5 → 11/tick → 1100 burned
    assert_eq!(carol.tokens, 200_000 - 11 * 100);

    // All tasks should be completed
    assert_eq!(tasks_created, 5); // ticks 20, 40, 60, 80, 100
    assert_eq!(tasks_completed, 5);

    // Verify task board state
    {
        let board = world.task_board.lock().await;
        for task in board.list() {
            assert_eq!(
                task.status,
                TaskStatus::Completed,
                "All tasks should be completed, found {:?}",
                task.status
            );
        }
    }

    // Verify events accumulated (3 spawns + 100 ticks + task events)
    let mut total_events = 0;
    while event_rx.try_recv().is_ok() {
        total_events += 1;
    }
    // At minimum: 100 tick events + task events
    assert!(
        total_events >= 100,
        "Should have at least 100 events, got {}",
        total_events
    );
}

// ══════════════════════════════════════════════════════════════════════════
// TEST 4: Ledger Consistency — Conservation of Value
// ══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_ledger_consistency_100_ticks() {
    let mut world = WorldState::new_default();

    // ── Setup: 2 agents with initial balances ─────────────────────────

    let alice_id = world.spawn_agent("Alice", 500_000);
    let bob_id = world.spawn_agent("Bob", 500_000);

    // Set up reward distributor balances
    world.reward_distributor.set_balance(&alice_id, 100_000);
    world.reward_distributor.set_balance(&bob_id, 100_000);

    // Track initial state
    let alice_initial_tokens = 500_000u64;
    let bob_initial_tokens = 500_000u64;

    // ── Run 100 ticks with task completions ───────────────────────────

    for tick_num in 1..=100u64 {
        let _ = world.tick().await;

        // Every 25 ticks, Alice creates a task and Bob completes it
        if tick_num % 25 == 0 {
            let reward = 1000u64;
            let result = world.reward_distributor.distribute_reward(
                &format!("task-{}", tick_num),
                &bob_id,
                reward,
                Currency::Money,
                tick_num,
            );

            // Verify conservation: gross = net + fee
            assert_eq!(
                result.gross_reward,
                result.net_reward + result.platform_fee,
                "Conservation violated at tick {}: gross={}, net={}, fee={}",
                tick_num,
                result.gross_reward,
                result.net_reward,
                result.platform_fee
            );
        }
    }

    // ── Verify token conservation ─────────────────────────────────────

    let alice = world.get_agent(&alice_id);
    let bob = world.get_agent(&bob_id);

    // Token burn: both are adults with no skills → 10/tick → 1000 total
    assert_eq!(alice.tokens, alice_initial_tokens - 10 * 100);
    assert_eq!(bob.tokens, bob_initial_tokens - 10 * 100);
    assert!(alice.tokens > 0);
    assert!(bob.tokens > 0);

    // ── Verify ledger entries ──────────────────────────────────────────

    let ledger = world.reward_distributor.ledger();
    let all_entries = ledger.list();

    // 4 tasks × 2 entries each (reward + fee) = 8 entries
    assert_eq!(
        all_entries.len(),
        8,
        "Expected 8 ledger entries, got {}",
        all_entries.len()
    );

    // Verify all entries have positive amounts
    for entry in all_entries {
        assert!(
            entry.amount > 0 || entry.tx_type == TransactionType::PlatformFee,
            "Ledger entry amount should be positive: {:?}",
            entry
        );
    }

    // ── Conservation: total money in == total money out ───────────────

    let reward_entries = ledger.list_by_type(TransactionType::TaskReward);
    let fee_entries = ledger.list_by_type(TransactionType::PlatformFee);

    let total_rewards_paid: u64 = reward_entries.iter().map(|e| e.amount).sum();
    let total_fees_collected: u64 = fee_entries.iter().map(|e| e.amount).sum();

    // Sum gross from reward+fee for each task = total value through system
    let total_through_system: u64 = total_rewards_paid + total_fees_collected;

    // 4 tasks × 1000 gross reward = 4000 total gross
    // Each: fee = 20, net = 980
    assert_eq!(total_rewards_paid, 980 * 4);
    assert_eq!(total_fees_collected, 20 * 4);
    assert_eq!(total_through_system, 4000);

    // ── Verify central bank ────────────────────────────────────────────

    let cb = world.reward_distributor.central_bank();
    assert_eq!(cb.total_fees(Currency::Money), 20 * 4);

    // ── Verify agent balances ──────────────────────────────────────────

    // distribute_reward adds net_reward to assignee and records fee to central bank.
    // Bob's balance: 100_000 + 4 × 980 = 103_920
    assert_eq!(
        world.reward_distributor.get_balance(&bob_id),
        100_000 + 980 * 4
    );

    // ── Verify per-task entries ────────────────────────────────────────

    for tick_num in [25u64, 50, 75, 100] {
        let task_ref = format!("task-{}", tick_num);
        let task_entries = ledger.list_by_reference(&task_ref);
        assert_eq!(
            task_entries.len(),
            2,
            "Expected 2 entries for task at tick {}",
            tick_num
        );

        let reward_entry = task_entries
            .iter()
            .find(|e| e.tx_type == TransactionType::TaskReward)
            .unwrap();
        let fee_entry = task_entries
            .iter()
            .find(|e| e.tx_type == TransactionType::PlatformFee)
            .unwrap();

        assert_eq!(
            reward_entry.amount + fee_entry.amount,
            1000,
            "Task at tick {} doesn't add up to gross reward",
            tick_num
        );
        assert_eq!(reward_entry.tick, tick_num);
        assert_eq!(fee_entry.tick, tick_num);
    }

    // ── Verify agents alive and consistent ─────────────────────────────

    assert_eq!(world.living_agents(), 2);
    assert_eq!(world.tick, 100);
}

// ══════════════════════════════════════════════════════════════════════════
// TEST 5: Combined — Full Lifecycle with Multiple Deaths and Snapshots
// ══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_combined_lifecycle_with_deaths_and_snapshots() {
    let mut world = WorldState::new_default();
    let mut event_rx = world.event_bus.subscribe();

    // ── Spawn agents with different token budgets ─────────────────────

    // rich_agent: 500_000 tokens → 50_000 ticks to live (won't die in 100)
    // mid_agent: 2_000 tokens → 200 ticks to live (won't die in 100)
    // poor_agent: 500 tokens → 50 ticks to live (WILL die)
    let rich_id = world.spawn_agent("Rich", 500_000);
    let mid_id = world.spawn_agent("Mid", 2_000);
    let poor_id = world.spawn_agent("Poor", 500);

    // Drain spawn events
    let _ = event_rx.try_recv().unwrap();
    let _ = event_rx.try_recv().unwrap();
    let _ = event_rx.try_recv().unwrap();

    // ── Phase 1: Run until poor_agent dies (tick 50) ──────────────────

    for tick in 1..=49 {
        let dead = world.tick().await;
        assert!(
            dead.is_empty(),
            "No deaths expected before tick 50, got deaths at tick {}",
            tick
        );
    }

    // Take pre-death snapshot
    let pre_death_snapshot = world.snapshot();
    assert_eq!(pre_death_snapshot.tick, 49);
    assert_eq!(pre_death_snapshot.agents_json.len(), 3);

    // Tick 50: poor_agent should die
    let dead = world.tick().await;
    assert_eq!(dead.len(), 1);
    assert_eq!(dead[0], poor_id);

    // Verify poor_agent is dead
    let poor = world.get_agent(&poor_id);
    assert_eq!(poor.phase, AgentPhase::Dead);
    assert_eq!(poor.tokens, 0);

    // ── Phase 2: Continue running with 2 agents ──────────────────────

    for tick in 51..=100 {
        let dead = world.tick().await;
        assert!(
            dead.is_empty(),
            "No more deaths expected after tick 50, got at tick {}",
            tick
        );
    }

    // Verify final state
    assert_eq!(world.tick, 100);
    assert_eq!(world.living_agents(), 2);

    let rich = world.get_agent(&rich_id);
    let mid = world.get_agent(&mid_id);

    // Rich: 500_000 - 10*100 = 499_000
    assert_eq!(rich.tokens, 500_000 - 10 * 100);
    assert_eq!(rich.phase, AgentPhase::Adult);

    // Mid: 2_000 - 10*100 = 1_000
    assert_eq!(mid.tokens, 2_000 - 10 * 100);
    assert_eq!(mid.phase, AgentPhase::Adult);

    // Poor: dead, 0 tokens
    let poor = world.get_agent(&poor_id);
    assert_eq!(poor.phase, AgentPhase::Dead);
    assert_eq!(poor.tokens, 0);

    // ── Phase 3: Take final snapshot and verify ───────────────────────

    let final_snapshot = world.snapshot();
    let snapshot_json = serde_json::to_string(&final_snapshot).unwrap();
    let restored: WorldSnapshot = serde_json::from_str(&snapshot_json).unwrap();

    assert_eq!(restored.tick, 100);
    assert_eq!(restored.agents_json.len(), 3);

    // Verify pre-death snapshot is different from final
    assert_ne!(pre_death_snapshot.tick, final_snapshot.tick);
    assert_eq!(
        pre_death_snapshot.agents_json.len(),
        final_snapshot.agents_json.len()
    );
}
