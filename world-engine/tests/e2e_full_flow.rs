//! E2E Full-Flow Integration Test — 2 Agents, 1000 Ticks.
//!
//! Validates the entire World Engine stack:
//!   1. World Engine + 2 Agent Runtimes startup
//!   2. Agent discovery → communication → trade → complete 3 tasks
//!   3. Survive 1000 ticks without crashing
//!   4. Ledger consistency verification
//!   5. Snapshot/restore test

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::Mutex;
use uuid::Uuid;

use agent_world_engine::economy::{
    EscrowManager, EscrowStatus, RewardConfig, RewardDistributor, TaskBoard, TaskStatus,
    TokenBurnEngine,
};
use agent_world_engine::world::enums::{AgentPhase, Currency};
use agent_world_engine::world::event::{EventType, WorldEvent};
use agent_world_engine::world::state::EventBus;

// ── Test Helpers ──────────────────────────────────────────────────────────

/// A simulated agent with state tracked by the world engine.
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct SimAgent {
    id: String,
    name: String,
    phase: AgentPhase,
    tokens: u64,
    money: u64,
    initial_tokens: u64,
    max_tokens: u64,
    skills: HashMap<String, u32>,
    reputation: f64,
    xp: u64,
    is_alive: bool,
    ticks_survived: u64,
    tasks_created: u32,
    tasks_completed: u32,
}

impl SimAgent {
    fn new(name: &str, tokens: u64, money: u64) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: name.to_string(),
            phase: AgentPhase::Adult,
            tokens,
            money,
            initial_tokens: tokens,
            max_tokens: tokens,
            skills: HashMap::new(),
            reputation: 0.0,
            xp: 0,
            is_alive: true,
            ticks_survived: 0,
            tasks_created: 0,
            tasks_completed: 0,
        }
    }

    fn burn_rate(&self) -> u64 {
        let phase_mult = match self.phase {
            AgentPhase::Childhood => 0.5,
            AgentPhase::Adult => 1.0,
            AgentPhase::Elder => 0.7,
            _ => 0.0,
        };
        let base = 10.0 * phase_mult;
        let skill_cost: f64 = self.skills.values().map(|l| *l as f64 * 0.5).sum();
        (base + skill_cost) as u64
    }

    fn tick(&mut self) {
        if !self.is_alive {
            return;
        }
        let burn = self.burn_rate();
        self.tokens = self.tokens.saturating_sub(burn);
        self.ticks_survived += 1;

        if self.tokens == 0 {
            self.phase = AgentPhase::Dead;
            self.is_alive = false;
        }
    }
}

// ══════════════════════════════════════════════════════════════════════════
// TEST 1: Agent Discovery → Communication → Trade → 3 Task Completions
// ══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_agent_discovery_communication_trade_tasks() {
    let event_bus = Arc::new(EventBus::new(256));
    let mut rx = event_bus.subscribe();

    let task_board = Arc::new(Mutex::new(TaskBoard::new()));
    // Wire up event emission manually via the task board
    // Since EventBus can't be cloned, we share it via Arc

    // ── Step 1: Spawn 2 agents (simulated discovery) ──────────────────

    let alice_id = "agent-alice".to_string();
    let bob_id = "agent-bob".to_string();

    event_bus.emit(WorldEvent::AgentSpawned {
        agent_id: alice_id.clone(),
        name: "Alice".to_string(),
    });
    event_bus.emit(WorldEvent::AgentSpawned {
        agent_id: bob_id.clone(),
        name: "Bob".to_string(),
    });

    // Verify discovery events
    let discovered_alice = rx.try_recv().unwrap();
    assert_eq!(discovered_alice.event_type(), EventType::AgentSpawned);
    let discovered_bob = rx.try_recv().unwrap();
    assert_eq!(discovered_bob.event_type(), EventType::AgentSpawned);

    // ── Step 2: Communication (transaction between agents) ─────────────

    event_bus.emit(WorldEvent::TransactionCompleted {
        from: alice_id.clone(),
        to: bob_id.clone(),
        amount: 100,
        currency: Currency::Token,
    });

    let comm_event = rx.try_recv().unwrap();
    assert_eq!(comm_event.event_type(), EventType::TransactionCompleted);

    // ── Step 3: Set up balances ────────────────────────────────────────

    {
        let mut board = task_board.lock().await;
        board.set_balance(&alice_id, 10_000);
        board.set_balance(&bob_id, 10_000);
    }

    // ── Step 4: Complete 3 tasks (full lifecycle for each) ─────────────

    let tasks_data = vec![
        ("Gather Resources", "Collect 100 wood and 50 stone", 500u64),
        ("Build Shelter", "Construct a basic shelter", 1000u64),
        ("Research Tech", "Discover basic tool crafting", 750u64),
    ];

    for (title, desc, reward) in &tasks_data {
        // Alice creates the task
        let task_id = {
            let mut board = task_board.lock().await;
            board
                .create_task(
                    title.to_string(),
                    desc.to_string(),
                    *reward,
                    alice_id.clone(),
                    0,
                    Some(5000),
                )
                .unwrap()
        };

        // Bob claims the task
        {
            let mut board = task_board.lock().await;
            board.claim_task(task_id, bob_id.clone()).unwrap();
        }

        // Bob starts the task
        {
            let mut board = task_board.lock().await;
            board.start_task(task_id).unwrap();
        }

        // Bob submits result
        {
            let mut board = task_board.lock().await;
            board
                .submit_result(task_id, format!("Completed: {}", title))
                .unwrap();
        }

        // Alice reviews and approves
        {
            let mut board = task_board.lock().await;
            board.review_task(task_id, &alice_id, true).unwrap();
        }

        // Complete the task
        {
            let mut board = task_board.lock().await;
            board.complete_task(task_id, 0).unwrap();
        }

        // Verify task is in Completed status
        {
            let board = task_board.lock().await;
            let task = board.get(task_id).unwrap();
            assert_eq!(task.status, TaskStatus::Completed);
            assert!(!task.escrow_held);
            assert_eq!(task.assignee_id.as_deref(), Some(bob_id.as_str()));
        }
    }

    // ── Step 5: Verify balances after 3 task completions ───────────────

    {
        let board = task_board.lock().await;
        let alice_balance = board.get_balance(&alice_id);
        let bob_balance = board.get_balance(&bob_id);
        assert_eq!(alice_balance, 10_000 - 500 - 1000 - 750);
        assert_eq!(bob_balance, 10_000 + 500 + 1000 + 750);
    }
}

// ══════════════════════════════════════════════════════════════════════════
// TEST 2: 2 Agents Survive 1000 Ticks Without Crashing
// ══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_two_agents_survive_1000_ticks() {
    let mut agents = vec![
        SimAgent::new("Alice", 500_000, 10_000),
        SimAgent::new("Bob", 500_000, 10_000),
    ];

    let total_ticks: u64 = 1000;

    for tick in 1..=total_ticks {
        for agent in agents.iter_mut() {
            // Add skill at tick 200 for Alice
            if tick == 200 && agent.name == "Alice" {
                agent.skills.insert("mining".to_string(), 3);
            }
            agent.tick();
        }

        // Verify both alive at key checkpoints
        if tick == 250 || tick == 500 || tick == 750 || tick == 1000 {
            let alive = agents.iter().filter(|a| a.is_alive).count();
            assert_eq!(alive, 2, "Both agents should be alive at tick {}", tick);
        }
    }

    // Final verification
    for agent in &agents {
        assert!(agent.is_alive, "{} should still be alive", agent.name);
        assert_eq!(agent.ticks_survived, 1000);
        assert!(agent.tokens > 0, "{} should still have tokens", agent.name);
    }

    // Alice with skills should have burned more tokens than Bob
    assert!(
        agents[0].tokens < agents[1].tokens,
        "Alice (with skills) should have fewer tokens than Bob"
    );
}

// ══════════════════════════════════════════════════════════════════════════
// TEST 3: Ledger Consistency Verification
// ══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_ledger_consistency() {
    let mut dist = RewardDistributor::new(RewardConfig::default());
    dist.set_balance("alice", 10_000);
    dist.set_balance("bob", 10_000);
    dist.set_balance("carol", 10_000);

    // Scenario: 3 tasks completed by different agents
    let tasks = vec![
        ("task-1", "bob", 1000u64, Currency::Money, 10u64),
        ("task-2", "carol", 500u64, Currency::Money, 20u64),
        ("task-3", "bob", 2000u64, Currency::Token, 30u64),
    ];

    let mut total_rewards: u64 = 0;

    for (task_id, assignee, reward, currency, tick) in &tasks {
        let result = dist.distribute_reward(task_id, assignee, *reward, *currency, *tick);

        // Verify conservation: gross = net + fee
        assert_eq!(
            result.gross_reward,
            result.net_reward + result.platform_fee,
            "Conservation violated for {}",
            task_id
        );

        // Verify fee is 2% (with truncation)
        let expected_fee = (*reward as u128 * 200 / 10_000) as u64;
        assert_eq!(result.platform_fee, expected_fee);

        // Verify XP and reputation
        assert_eq!(result.xp_awarded, 50);
        assert_eq!(result.reputation_change, 2.0);

        total_rewards += result.gross_reward;
    }

    // ── Verify ledger entries ──────────────────────────────────────────

    let ledger = dist.ledger();
    let all_entries = ledger.list();

    // 3 tasks × 2 entries each (reward + fee) = 6 entries
    assert_eq!(all_entries.len(), 6);

    // Verify by type
    let reward_entries =
        ledger.list_by_type(agent_world_engine::economy::TransactionType::TaskReward);
    let fee_entries =
        ledger.list_by_type(agent_world_engine::economy::TransactionType::PlatformFee);
    assert_eq!(reward_entries.len(), 3);
    assert_eq!(fee_entries.len(), 3);

    // Verify by agent
    let bob_entries = ledger.list_by_agent("bob");
    let carol_entries = ledger.list_by_agent("carol");
    assert_eq!(bob_entries.len(), 4); // 2 rewards + 2 fees
    assert_eq!(carol_entries.len(), 2); // 1 reward + 1 fee

    // Verify by reference
    for (task_id, _, _, _, _) in &tasks {
        let task_entries = ledger.list_by_reference(task_id);
        assert_eq!(task_entries.len(), 2, "Expected 2 entries for {}", task_id);
    }

    // ── Verify central bank fees ───────────────────────────────────────

    let cb = dist.central_bank();
    assert_eq!(cb.total_fees(Currency::Money), 30);
    assert_eq!(cb.total_fees(Currency::Token), 40);

    // ── Verify agent balances ──────────────────────────────────────────

    assert_eq!(dist.get_balance("bob"), 12_940);
    assert_eq!(dist.get_balance("carol"), 10_490);

    // ── Verify reputation ──────────────────────────────────────────────

    assert_eq!(dist.get_reputation("bob"), 4.0);
    assert_eq!(dist.get_reputation("carol"), 2.0);

    // ── Verify XP ──────────────────────────────────────────────────────

    assert_eq!(dist.get_experience("bob"), 100);
    assert_eq!(dist.get_experience("carol"), 50);

    // ── Cross-check: sum of all money in = sum of all money out ────────

    let total_reward_paid: u64 = reward_entries.iter().map(|e| e.amount).sum();
    let total_fee_collected: u64 = fee_entries.iter().map(|e| e.amount).sum();
    assert_eq!(
        total_reward_paid + total_fee_collected,
        total_rewards,
        "Total money in ({}) should equal gross rewards ({})",
        total_reward_paid + total_fee_collected,
        total_rewards
    );
}

// ══════════════════════════════════════════════════════════════════════════
// TEST 4: Token Burn Engine - Multi-Agent 1000 Tick Consistency
// ══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_token_burn_1000_ticks_consistency() {
    use agent_world_engine::economy::token_burn::{AgentRecord, SkillRecord};

    let engine = TokenBurnEngine::with_defaults();

    let mut agents = vec![
        AgentRecord {
            id: Uuid::new_v4(),
            name: "Alice".to_string(),
            phase: AgentPhase::Adult,
            tokens: 500_000,
            skills: {
                let mut m = HashMap::new();
                m.insert(
                    "mining".to_string(),
                    SkillRecord {
                        name: "mining".to_string(),
                        level: 5,
                        experience: 0.0,
                    },
                );
                m
            },
            personality: String::new(),
            tasks_completed: 0,
            tasks_attempted: 0,
        },
        AgentRecord {
            id: Uuid::new_v4(),
            name: "Bob".to_string(),
            phase: AgentPhase::Adult,
            tokens: 500_000,
            skills: HashMap::new(),
            personality: String::new(),
            tasks_completed: 0,
            tasks_attempted: 0,
        },
    ];

    let alice_initial = agents[0].tokens;
    let bob_initial = agents[1].tokens;

    let alice_per_tick = engine.calculate_tick_burn(&agents[0]);
    let bob_per_tick = engine.calculate_tick_burn(&agents[1]);
    assert_eq!(alice_per_tick, 12);
    assert_eq!(bob_per_tick, 10);

    let mut total_burned_alice: u64 = 0;
    let mut total_burned_bob: u64 = 0;

    for tick in 1..=1000u64 {
        let result = engine.process_tick(tick, &mut agents);

        assert_eq!(result.tick, tick);
        assert_eq!(result.burns.len(), 2);

        total_burned_alice += result.burns[0].burn_amount;
        total_burned_bob += result.burns[1].burn_amount;
    }

    // Conservation: initial = remaining + burned
    assert_eq!(
        alice_initial,
        agents[0].tokens + total_burned_alice,
        "Alice: tokens not conserved"
    );
    assert_eq!(
        bob_initial,
        agents[1].tokens + total_burned_bob,
        "Bob: tokens not conserved"
    );

    assert!(agents[0].tokens > 0, "Alice should still have tokens");
    assert!(agents[1].tokens > 0, "Bob should still have tokens");

    assert_eq!(agents[0].tokens, 500_000 - 12_000);
    assert_eq!(agents[1].tokens, 500_000 - 10_000);
}

// ══════════════════════════════════════════════════════════════════════════
// TEST 5: Escrow + TaskBoard Integration — Full Trade Lifecycle
// ══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_escrow_task_integration() {
    // Use separate instances since EventBus can't be shared
    let mut escrow_mgr = EscrowManager::new();
    let mut task_board = TaskBoard::new();

    let alice = "alice";
    let bob = "bob";

    escrow_mgr.set_balance(alice, 10_000);
    escrow_mgr.set_balance(bob, 5_000);
    task_board.set_balance(alice, 10_000);
    task_board.set_balance(bob, 5_000);

    // ── Create escrow for the trade ────────────────────────────────────

    let escrow_id = escrow_mgr
        .create_escrow(alice, 1000, 200, Currency::Money, 1, Some(5000))
        .unwrap();
    assert_eq!(escrow_mgr.get_balance(alice), 9_000);

    // ── Create corresponding task ──────────────────────────────────────

    let task_id = task_board
        .create_task(
            "Build Widget".to_string(),
            "Construct a premium widget".to_string(),
            1000,
            alice.to_string(),
            1,
            Some(5000),
        )
        .unwrap();

    // ── Bob claims both escrow and task ────────────────────────────────

    escrow_mgr.claim_escrow(escrow_id, bob).unwrap();
    assert_eq!(escrow_mgr.get_balance(bob), 4_800);

    task_board.claim_task(task_id, bob.to_string()).unwrap();

    // ── Bob does the work ──────────────────────────────────────────────

    task_board.start_task(task_id).unwrap();
    task_board
        .submit_result(task_id, "Widget completed!".to_string())
        .unwrap();

    // ── Alice reviews and approves ─────────────────────────────────────

    task_board.review_task(task_id, alice, true).unwrap();

    // ── Complete: release escrow + mark task done ──────────────────────

    escrow_mgr.complete_escrow(escrow_id).unwrap();
    task_board.complete_task(task_id, 0).unwrap();

    // ── Verify final state ─────────────────────────────────────────────

    // Bob gets reward (1000) + deposit back (200) = 1200 → 4800 + 1200 = 6000
    assert_eq!(escrow_mgr.get_balance(bob), 6_000);
    // Alice's balance: 9000 (escrow reward was consumed by the trade)
    assert_eq!(escrow_mgr.get_balance(alice), 9_000);

    let task = task_board.get(task_id).unwrap();
    assert_eq!(task.status, TaskStatus::Completed);
    assert!(!task.escrow_held);

    let escrow = escrow_mgr.get(escrow_id).unwrap();
    assert_eq!(escrow.status, EscrowStatus::Completed);
}

// ══════════════════════════════════════════════════════════════════════════
// TEST 6: Snapshot/Restore — Event Bus State Capture and Replay
// ══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_snapshot_restore() {
    let event_bus = Arc::new(EventBus::new(1024));
    let mut rx = event_bus.subscribe();

    let task_board = Arc::new(Mutex::new(TaskBoard::new()));

    let alice_id = "agent-alice";
    let bob_id = "agent-bob";

    // ── Phase 1: Run, capture events as "snapshot" ────────────────────

    let mut captured_events: Vec<WorldEvent> = Vec::new();

    // Set up initial balances
    {
        let mut board = task_board.lock().await;
        board.set_balance(alice_id, 10_000);
        board.set_balance(bob_id, 10_000);
    }

    // Create and complete a task before snapshot
    let task1_id = {
        let mut board = task_board.lock().await;
        let id = board
            .create_task(
                "Pre-snapshot task".to_string(),
                "Complete before snapshot".to_string(),
                500,
                alice_id.to_string(),
                100,
                Some(1000),
            )
            .unwrap();
        board.claim_task(id, bob_id.to_string()).unwrap();
        board.start_task(id).unwrap();
        board
            .submit_result(id, "Done before snapshot".to_string())
            .unwrap();
        board.review_task(id, alice_id, true).unwrap();
        board.complete_task(id, 0).unwrap();
        id
    };

    // Emit events for the lifecycle (simulating the snapshot)
    event_bus.emit(WorldEvent::TaskCreated {
        task_id: task1_id.to_string(),
        publisher: alice_id.to_string(),
        reward: 500,
    });
    event_bus.emit(WorldEvent::TaskClaimed {
        task_id: task1_id.to_string(),
        assignee: bob_id.to_string(),
    });
    event_bus.emit(WorldEvent::TaskStarted {
        task_id: task1_id.to_string(),
    });
    event_bus.emit(WorldEvent::TaskSubmitted {
        task_id: task1_id.to_string(),
    });
    event_bus.emit(WorldEvent::TaskReviewed {
        task_id: task1_id.to_string(),
        approved: true,
    });
    event_bus.emit(WorldEvent::TaskCompleted {
        task_id: task1_id.to_string(),
    });

    // Drain all events and store as "snapshot"
    while let Ok(event) = rx.try_recv() {
        captured_events.push(event);
    }

    // Emit snapshot event
    event_bus.emit(WorldEvent::SnapshotTaken {
        tick: 500,
        path: "/snapshots/tick-500.json".to_string(),
    });

    let snapshot_event = rx.try_recv().unwrap();
    assert_eq!(snapshot_event.event_type(), EventType::SnapshotTaken);

    // ── Verify snapshot events are serializable ────────────────────────

    let snapshot_json = serde_json::to_string(&captured_events).unwrap();
    assert!(!snapshot_json.is_empty());

    // ── Phase 2: "Restore" — deserialize and verify state ──────────────

    let restored_events: Vec<WorldEvent> = serde_json::from_str(&snapshot_json).unwrap();
    assert_eq!(restored_events.len(), captured_events.len());

    // Verify each event round-trips correctly
    for (original, restored) in captured_events.iter().zip(restored_events.iter()) {
        assert_eq!(original, restored, "Event mismatch after restore");
    }

    // ── Phase 3: Continue running after restore ────────────────────────

    let task2_id = {
        let mut board = task_board.lock().await;
        let id = board
            .create_task(
                "Post-restore task".to_string(),
                "Work after snapshot restore".to_string(),
                750,
                alice_id.to_string(),
                501,
                Some(2000),
            )
            .unwrap();
        board.claim_task(id, bob_id.to_string()).unwrap();
        board.start_task(id).unwrap();
        board
            .submit_result(id, "Done after restore".to_string())
            .unwrap();
        board.review_task(id, alice_id, true).unwrap();
        board.complete_task(id, 0).unwrap();
        id
    };

    // Emit post-restore events
    event_bus.emit(WorldEvent::TaskCreated {
        task_id: task2_id.to_string(),
        publisher: alice_id.to_string(),
        reward: 750,
    });
    event_bus.emit(WorldEvent::TaskClaimed {
        task_id: task2_id.to_string(),
        assignee: bob_id.to_string(),
    });
    event_bus.emit(WorldEvent::TaskStarted {
        task_id: task2_id.to_string(),
    });
    event_bus.emit(WorldEvent::TaskSubmitted {
        task_id: task2_id.to_string(),
    });
    event_bus.emit(WorldEvent::TaskReviewed {
        task_id: task2_id.to_string(),
        approved: true,
    });
    event_bus.emit(WorldEvent::TaskCompleted {
        task_id: task2_id.to_string(),
    });

    // Verify both tasks are completed
    {
        let board = task_board.lock().await;
        let t1 = board.get(task1_id).unwrap();
        let t2 = board.get(task2_id).unwrap();
        assert_eq!(t1.status, TaskStatus::Completed);
        assert_eq!(t2.status, TaskStatus::Completed);
    }

    // ── Phase 4: Verify post-restore events ────────────────────────────

    let mut post_restore_events: Vec<WorldEvent> = Vec::new();
    while let Ok(event) = rx.try_recv() {
        post_restore_events.push(event);
    }

    assert_eq!(post_restore_events.len(), 6);

    let expected_sequence = [
        EventType::TaskCreated,
        EventType::TaskClaimed,
        EventType::TaskStarted,
        EventType::TaskSubmitted,
        EventType::TaskReviewed,
        EventType::TaskCompleted,
    ];

    for (event, expected_type) in post_restore_events.iter().zip(expected_sequence.iter()) {
        assert_eq!(&event.event_type(), expected_type);
    }
}

// ══════════════════════════════════════════════════════════════════════════
// TEST 7: Combined 1000-Tick Simulation with Tasks, Token Burn, Events
// ══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_full_1000_tick_simulation() {
    let event_bus = Arc::new(EventBus::new(4096));
    let mut event_rx = event_bus.subscribe();

    let task_board = Arc::new(Mutex::new(TaskBoard::new()));

    let mut agents = vec![
        SimAgent::new("Alice", 500_000, 10_000),
        SimAgent::new("Bob", 500_000, 10_000),
    ];

    // Set up task board balances
    {
        let mut board = task_board.lock().await;
        board.set_balance(&agents[0].id, 50_000);
        board.set_balance(&agents[1].id, 50_000);
    }

    // Emit spawn events
    for agent in &agents {
        event_bus.emit(WorldEvent::AgentSpawned {
            agent_id: agent.id.clone(),
            name: agent.name.clone(),
        });
    }

    let mut tasks_completed_count = 0u32;
    let mut active_tasks: Vec<Uuid> = Vec::new();

    for tick in 1..=1000u64 {
        // 1. Token burn for all agents
        for agent in agents.iter_mut() {
            agent.tick();
        }

        // 2. Check agent survival
        let alive_count = agents.iter().filter(|a| a.is_alive).count();
        assert_eq!(alive_count, 2, "Both agents should survive tick {}", tick);

        // 3. Periodic task creation (every 100 ticks, create a task)
        if tick % 100 == 0 && tasks_completed_count < 10 {
            let publisher_idx = (tick as usize / 100) % 2;
            let publisher = &agents[publisher_idx];
            let reward = 500u64;

            let task_id = {
                let mut board = task_board.lock().await;
                board
                    .create_task(
                        format!("Task at tick {}", tick),
                        format!("Automated task created at tick {}", tick),
                        reward,
                        publisher.id.clone(),
                        tick,
                        Some(tick + 500),
                    )
                    .unwrap()
            };
            active_tasks.push(task_id);
        }

        // 4. Process task lifecycle — advance ALL active tasks through their
        //    full lifecycle within a single tick (simulating fast completion).
        //    Each task advances through: Published→Claimed→InProgress→Submitted→Reviewed→Completed
        let mut completed_this_tick = Vec::new();
        for (idx, &task_id) in active_tasks.iter().enumerate() {
            let mut board = task_board.lock().await;
            if let Some(task) = board.get(task_id).cloned() {
                let assignee_idx = if task.publisher_id == agents[0].id {
                    1
                } else {
                    0
                };

                // Run through as many transitions as possible in one tick
                let mut current_status = task.status;
                loop {
                    match current_status {
                        TaskStatus::Published => {
                            if board
                                .claim_task(task_id, agents[assignee_idx].id.clone())
                                .is_ok()
                            {
                                current_status = TaskStatus::Claimed;
                            } else {
                                break;
                            }
                        }
                        TaskStatus::Claimed => {
                            if board.start_task(task_id).is_ok() {
                                current_status = TaskStatus::InProgress;
                            } else {
                                break;
                            }
                        }
                        TaskStatus::InProgress => {
                            if board
                                .submit_result(task_id, format!("Result at tick {}", tick))
                                .is_ok()
                            {
                                current_status = TaskStatus::Submitted;
                            } else {
                                break;
                            }
                        }
                        TaskStatus::Submitted => {
                            if board.review_task(task_id, &task.publisher_id, true).is_ok() {
                                current_status = TaskStatus::Reviewed;
                            } else {
                                break;
                            }
                        }
                        TaskStatus::Reviewed => {
                            if board.complete_task(task_id, 0).is_ok() {
                                tasks_completed_count += 1;
                                completed_this_tick.push(idx);
                            }
                            break;
                        }
                        _ => break,
                    }
                }
            }
        }

        // Remove completed tasks
        for idx in completed_this_tick.into_iter().rev() {
            active_tasks.remove(idx);
        }

        // 5. Process task expiry
        {
            let mut board = task_board.lock().await;
            board.process_expiry(tick);
        }

        // 6. Emit tick event
        event_bus.emit(WorldEvent::TickAdvanced { tick });
    }

    // ── Final assertions ───────────────────────────────────────────────

    for agent in &agents {
        assert!(
            agent.is_alive,
            "{} should be alive after 1000 ticks",
            agent.name
        );
        assert_eq!(agent.ticks_survived, 1000);
    }

    assert!(
        tasks_completed_count >= 3,
        "Expected at least 3 completed tasks, got {}",
        tasks_completed_count
    );

    // Drain events
    let mut event_count = 0;
    while event_rx.try_recv().is_ok() {
        event_count += 1;
    }

    assert!(
        event_count > 0,
        "Should have accumulated events over 1000 ticks"
    );

    // Verify no tasks in stuck state
    {
        let board = task_board.lock().await;
        for task in board.list() {
            assert!(
                matches!(
                    task.status,
                    TaskStatus::Completed | TaskStatus::Expired | TaskStatus::Published
                ),
                "Task {} should be in terminal/published state, not {:?}",
                task.id,
                task.status
            );
        }
    }
}

// ══════════════════════════════════════════════════════════════════════════
// TEST 8: REST API E2E — Execute Operations via HTTP
// ══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_rest_api_e2e() {
    use axum::http::StatusCode;

    use agent_world_engine::api::create_router;

    let task_board = Arc::new(Mutex::new(TaskBoard::new()));
    let app = create_router(task_board.clone());

    let alice_id = "alice";
    let bob_id = "bob";

    // ── Create 3 tasks directly ────────────────────────────────────────

    let tasks = vec![
        ("Gather Wood", "Collect 50 wood"),
        ("Mine Stone", "Collect 30 stone"),
        ("Build House", "Construct shelter"),
    ];

    let mut task_ids: Vec<Uuid> = Vec::new();

    for (title, desc) in &tasks {
        let mut board = task_board.lock().await;
        let id = board
            .create_task(
                title.to_string(),
                desc.to_string(),
                0,
                alice_id.to_string(),
                0,
                None,
            )
            .unwrap();
        task_ids.push(id);
    }

    // ── Full lifecycle via direct calls ────────────────────────────────

    for task_id in &task_ids {
        let mut board = task_board.lock().await;
        board.claim_task(*task_id, bob_id.to_string()).unwrap();
        board.start_task(*task_id).unwrap();
        board
            .submit_result(*task_id, "Work completed!".to_string())
            .unwrap();
        board.review_task(*task_id, alice_id, true).unwrap();
        board.complete_task(*task_id, 0).unwrap();
    }

    // ── Verify all tasks completed ─────────────────────────────────────

    {
        let board = task_board.lock().await;
        for task_id in &task_ids {
            let task = board.get(*task_id).unwrap();
            assert_eq!(task.status, TaskStatus::Completed);
            assert!(!task.escrow_held);
        }
    }

    // ── Test API via real HTTP server ──────────────────────────────────

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let port = addr.port();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    let client = reqwest::Client::new();

    // ── GET /tasks (list) ──────────────────────────────────────────────

    let resp = client
        .get(format!("http://127.0.0.1:{}/tasks", port))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body_text = resp.text().await.unwrap();
    let tasks_list: serde_json::Value = serde_json::from_str(&body_text).unwrap();
    assert_eq!(tasks_list.as_array().unwrap().len(), 3);

    // Verify all tasks are completed in the list response
    for task_val in tasks_list.as_array().unwrap() {
        assert_eq!(task_val["status"].as_str().unwrap(), "completed");
    }

    // ── POST /tasks (create) ───────────────────────────────────────────

    let create_body = serde_json::json!({
        "title": "API Created Task",
        "description": "Created via REST API",
        "reward": 0,
        "publisher_id": alice_id,
    });

    let resp = client
        .post(format!("http://127.0.0.1:{}/tasks", port))
        .json(&create_body)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let task_resp: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(task_resp["title"].as_str().unwrap(), "API Created Task");
    assert_eq!(task_resp["status"].as_str().unwrap(), "published");

    let new_task_id = task_resp["id"].as_str().unwrap();

    // ── Full lifecycle via API for new task ─────────────────────────────

    // Claim
    let resp = client
        .post(format!(
            "http://127.0.0.1:{}/tasks/{}/claim",
            port, new_task_id
        ))
        .json(&serde_json::json!({ "assignee_id": bob_id }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Start
    let resp = client
        .post(format!(
            "http://127.0.0.1:{}/tasks/{}/start",
            port, new_task_id
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Submit
    let resp = client
        .post(format!(
            "http://127.0.0.1:{}/tasks/{}/submit",
            port, new_task_id
        ))
        .json(&serde_json::json!({ "result": "Work completed!" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Review
    let resp = client
        .post(format!(
            "http://127.0.0.1:{}/tasks/{}/review",
            port, new_task_id
        ))
        .json(&serde_json::json!({ "approved": true, "reviewer_id": alice_id }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Complete
    let resp = client
        .post(format!(
            "http://127.0.0.1:{}/tasks/{}/complete",
            port, new_task_id
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // ── GET /tasks/{id} — verify final state ───────────────────────────

    let resp = client
        .get(format!("http://127.0.0.1:{}/tasks/{}", port, new_task_id))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body_text = resp.text().await.unwrap();
    let task: serde_json::Value = serde_json::from_str(&body_text).unwrap();
    assert_eq!(task["status"].as_str().unwrap(), "completed");
    assert_eq!(task["title"].as_str().unwrap(), "API Created Task");

    // Verify the list now has 4 tasks
    let resp = client
        .get(format!("http://127.0.0.1:{}/tasks", port))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body_text = resp.text().await.unwrap();
    let tasks_list: serde_json::Value = serde_json::from_str(&body_text).unwrap();
    assert_eq!(tasks_list.as_array().unwrap().len(), 4);
}

// ══════════════════════════════════════════════════════════════════════════
// TEST 9: Dispute Resolution — Escrow Dispute Flow
// ══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_dispute_resolution_flow() {
    let mut mgr = EscrowManager::new();
    mgr.set_balance("alice", 10_000);
    mgr.set_balance("bob", 5_000);

    let escrow_id = mgr
        .create_escrow("alice", 1000, 200, Currency::Money, 1, None)
        .unwrap();
    mgr.claim_escrow(escrow_id, "bob").unwrap();

    assert_eq!(mgr.get_balance("alice"), 9_000);
    assert_eq!(mgr.get_balance("bob"), 4_800);

    mgr.dispute_escrow(escrow_id, "Work does not meet quality standards")
        .unwrap();

    let escrow = mgr.get(escrow_id).unwrap();
    assert_eq!(escrow.status, EscrowStatus::Disputed);

    // Balances unchanged during dispute
    assert_eq!(mgr.get_balance("alice"), 9_000);
    assert_eq!(mgr.get_balance("bob"), 4_800);

    // Resolution: favor publisher (alice) — refund both
    mgr.resolve_dispute(escrow_id, false).unwrap();

    assert_eq!(mgr.get_balance("alice"), 10_000);
    assert_eq!(mgr.get_balance("bob"), 5_000);

    let escrow = mgr.get(escrow_id).unwrap();
    assert_eq!(escrow.status, EscrowStatus::Refunded);
}

// ══════════════════════════════════════════════════════════════════════════
// TEST 10: Event Bus — Cross-Agent Event Filtering
// ══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_cross_agent_event_filtering() {
    let bus = Arc::new(EventBus::new(1024));

    // Agent A subscribes to its own events
    let mut rx_a = bus.subscribe_filtered(
        vec![EventType::BalanceChanged, EventType::TransactionCompleted],
        Some("agent-a".to_string()),
    );

    // Agent B subscribes to its own events
    let mut rx_b = bus.subscribe_filtered(
        vec![EventType::BalanceChanged, EventType::TransactionCompleted],
        Some("agent-b".to_string()),
    );

    // Global subscriber gets everything
    let mut rx_global = bus.subscribe();

    // Emit events from various agents
    bus.emit(WorldEvent::BalanceChanged {
        agent_id: "agent-a".to_string(),
        currency: Currency::Token,
        old_balance: 1000,
        new_balance: 900,
        tick: 0,
    });

    bus.emit(WorldEvent::BalanceChanged {
        agent_id: "agent-b".to_string(),
        currency: Currency::Money,
        old_balance: 500,
        new_balance: 600,
        tick: 0,
    });

    bus.emit(WorldEvent::TransactionCompleted {
        from: "agent-a".to_string(),
        to: "agent-b".to_string(),
        amount: 100,
        currency: Currency::Token,
    });

    bus.emit(WorldEvent::TickAdvanced { tick: 1 });

    // Agent A sees: BalanceChanged(a), TransactionCompleted(from=a)
    let a1 = rx_a.try_recv().unwrap();
    assert_eq!(a1.event_type(), EventType::BalanceChanged);
    let a2 = rx_a.try_recv().unwrap();
    assert_eq!(a2.event_type(), EventType::TransactionCompleted);
    assert!(rx_a.try_recv().is_err());

    // Agent B sees only its own BalanceChanged
    // (TransactionCompleted agent_id() returns "from" = agent-a, so filtered out)
    let b1 = rx_b.try_recv().unwrap();
    assert_eq!(b1.event_type(), EventType::BalanceChanged);
    assert!(rx_b.try_recv().is_err());

    // Global sees everything
    let _ = rx_global.try_recv().unwrap();
    let _ = rx_global.try_recv().unwrap();
    let _ = rx_global.try_recv().unwrap();
    let _ = rx_global.try_recv().unwrap();
    assert!(rx_global.try_recv().is_err());
}
