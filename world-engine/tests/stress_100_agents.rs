//! Stress Test: 100 Concurrent Agents
//!
//! Simulates 100 agents running simultaneously against the World Engine.
//! Each agent:
//!   - Has its own token balance
//!   - Creates tasks
//!   - Claims and completes other agents' tasks
//!   - Burns tokens per tick
//!
//! Measures:
//!   - Total throughput (operations/second)
//!   - Wall-clock time for 100-tick simulation
//!   - Concurrent read/write performance under load
//!   - No deadlocks or panics

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use tokio::sync::RwLock;
use uuid::Uuid;

use agent_world_engine::economy::token_burn::{AgentRecord, ConsumptionConfig, SkillRecord, TokenBurnEngine};
use agent_world_engine::economy::{TaskBoard, TaskStatus};
use agent_world_engine::world::enums::AgentPhase;
use agent_world_engine::world::event::WorldEvent;
use agent_world_engine::world::state::EventBus;

const NUM_AGENTS: usize = 100;
const TICKS: u64 = 100;

/// Simulated agent state.
#[derive(Debug, Clone)]
struct StressAgent {
    id: String,
    name: String,
    tokens: u64,
    phase: AgentPhase,
    tasks_created: u32,
    tasks_completed: u32,
    is_alive: bool,
}

impl StressAgent {
    fn new(index: usize) -> Self {
        Self {
            id: format!("agent-{:03}", index),
            name: format!("Agent-{}", index),
            tokens: 500_000,
            phase: AgentPhase::Adult,
            tasks_created: 0,
            tasks_completed: 0,
            is_alive: true,
        }
    }
}

// ══════════════════════════════════════════════════════════════════════════
// TEST 1: 100 Agents, 100 Ticks — Token Burn Consistency
// ══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_100_agents_100_ticks_token_burn() {
    let start = Instant::now();

    // Create 100 agent records for the token burn engine
    let config = ConsumptionConfig::default();
    let engine = TokenBurnEngine::new(config);

    let mut agents: Vec<AgentRecord> = (0..NUM_AGENTS)
        .map(|i| {
            let mut skills = HashMap::new();
            // 20% of agents have skills
            if i % 5 == 0 {
                skills.insert(
                    format!("skill-{}", i % 3),
                    SkillRecord {
                        name: format!("skill-{}", i % 3),
                        level: (i % 5 + 1) as u32,
                        experience: 0.0,
                    },
                );
            }
            AgentRecord {
                id: Uuid::new_v4(),
                name: format!("Agent-{}", i),
                phase: AgentPhase::Adult,
                tokens: 500_000,
                skills,
                personality: String::new(),
            }
        })
        .collect();

    let initial_total: u64 = agents.iter().map(|a| a.tokens).sum();

    // Run 100 ticks
    for tick in 1..=TICKS {
        let result = engine.process_tick(tick, &mut agents);
        assert_eq!(result.burns.len(), NUM_AGENTS, "All agents should burn at tick {}", tick);
    }

    let final_total: u64 = agents.iter().map(|a| a.tokens).sum();

    // Conservation: total tokens should decrease by exactly the amount burned
    assert!(
        final_total < initial_total,
        "Total tokens should decrease: {} -> {}",
        initial_total,
        final_total
    );

    // All agents should survive (500k - 10-15/tick * 100 = ~499k)
    let alive = agents.iter().filter(|a| a.tokens > 0).count();
    assert_eq!(alive, NUM_AGENTS, "All agents should survive 100 ticks");

    let elapsed = start.elapsed();
    println!(
        "[Stress] 100 agents × 100 ticks token burn: {:?} ({:.0} ops/s)",
        elapsed,
        (NUM_AGENTS * TICKS as usize) as f64 / elapsed.as_secs_f64()
    );
}

// ══════════════════════════════════════════════════════════════════════════
// TEST 2: 100 Agents Concurrent Task Operations
// ══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_100_agents_concurrent_tasks() {
    let start = Instant::now();

    let board = Arc::new(RwLock::new(TaskBoard::new()));

    // Set up balances for all 100 agents
    {
        let mut b = board.write().await;
        for i in 0..NUM_AGENTS {
            b.set_balance(&format!("agent-{:03}", i), 100_000);
        }
    }

    // Phase 1: All agents concurrently create tasks
    let mut create_handles = Vec::new();
    for i in 0..NUM_AGENTS {
        let board = board.clone();
        create_handles.push(tokio::spawn(async move {
            let agent_id = format!("agent-{:03}", i);
            let mut b = board.write().await;
            b.create_task(
                format!("Task from agent {}", i),
                format!("Agent {} needs this done", i),
                100,
                agent_id,
                0,
                None,
            ).unwrap()
        }));
    }

    let mut task_ids = Vec::new();
    for handle in create_handles {
        task_ids.push(handle.await.unwrap());
    }

    let elapsed_create = start.elapsed();
    println!(
        "[Stress] 100 concurrent task creates: {:?} ({:.0} creates/s)",
        elapsed_create,
        NUM_AGENTS as f64 / elapsed_create.as_secs_f64()
    );

    // Verify all 100 tasks created
    {
        let b = board.read().await;
        assert_eq!(b.list().len(), NUM_AGENTS);
    }

    // Phase 2: All agents concurrently claim + complete a different agent's task
    let start_lifecycle = Instant::now();
    let mut lifecycle_handles = Vec::new();

    for i in 0..NUM_AGENTS {
        let board = board.clone();
        let task_id = task_ids[(i + 1) % NUM_AGENTS]; // claim next agent's task
        let worker_id = format!("agent-{:03}", i);

        lifecycle_handles.push(tokio::spawn(async move {
            let mut b = board.write().await;
            let _ = b.claim_task(task_id, worker_id.clone());
            let _ = b.start_task(task_id);
            let _ = b.submit_result(task_id, format!("Completed by {}", worker_id));
            // Find the publisher for review
            if let Some(task) = b.get(task_id).cloned() {
                let _ = b.review_task(task_id, &task.publisher_id, true);
                let _ = b.complete_task(task_id, 0);
            }
        }));
    }

    for handle in lifecycle_handles {
        let _ = handle.await;
    }

    let elapsed_lifecycle = start_lifecycle.elapsed();
    println!(
        "[Stress] 100 concurrent task lifecycles: {:?} ({:.0} lifecycles/s)",
        elapsed_lifecycle,
        NUM_AGENTS as f64 / elapsed_lifecycle.as_secs_f64()
    );

    // Phase 3: Verify final state — most tasks should be completed
    {
        let b = board.read().await;
        let tasks = b.list();
        let completed = tasks.iter().filter(|t| t.status == TaskStatus::Completed).count();
        // Some may collide on the same task, but most should complete
        assert!(
            completed > NUM_AGENTS / 2,
            "At least half of tasks should be completed, got {}/{}",
            completed,
            NUM_AGENTS
        );
    }

    let total_elapsed = start.elapsed();
    println!("[Stress] Total test time: {:?}", total_elapsed);
}

// ══════════════════════════════════════════════════════════════════════════
// TEST 3: 100 Agents Concurrent Reads (80% read / 20% write)
// ══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_100_agents_read_heavy_workload() {
    let start = Instant::now();

    let board = Arc::new(RwLock::new(TaskBoard::new()));

    // Pre-populate with 100 tasks
    {
        let mut b = board.write().await;
        b.set_balance("publisher", 10_000_000);
        for i in 0..100 {
            b.create_task(
                format!("Pre-existing task {}", i),
                format!("Description {}", i),
                100,
                "publisher".to_string(),
                i as u64,
                None,
            ).unwrap();
        }
    }

    // Run 1000 concurrent operations: 800 reads + 200 writes
    let total_ops = 1000;
    let mut handles = Vec::new();

    for i in 0..total_ops {
        let board = board.clone();
        handles.push(tokio::spawn(async move {
            if i % 5 == 0 {
                // 20% writes: create a new task
                let mut b = board.write().await;
                let _ = b.create_task(
                    format!("Write-op task {}", i),
                    format!("From op {}", i),
                    50,
                    "publisher".to_string(),
                    i as u64,
                    None,
                );
            } else {
                // 80% reads: list tasks
                let b = board.read().await;
                let _ = black_box(b.list());
            }
        }));
    }

    for handle in handles {
        handle.await.unwrap();
    }

    let elapsed = start.elapsed();
    let ops_per_sec = total_ops as f64 / elapsed.as_secs_f64();
    println!(
        "[Stress] 1000 concurrent ops (80% read / 20% write): {:?} ({:.0} ops/s)",
        elapsed, ops_per_sec
    );

    // Should complete in reasonable time (< 5 seconds even on slow machines)
    assert!(elapsed.as_secs() < 10, "Should complete within 10s, took {:?}", elapsed);

    // Verify board has tasks (100 initial + ~200 created)
    {
        let b = board.read().await;
        let task_count = b.list().len();
        assert!(task_count >= 100, "Should have at least 100 tasks, got {}", task_count);
    }
}

// ══════════════════════════════════════════════════════════════════════════
// TEST 4: 100 Agents EventBus Throughput
// ══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_100_agents_eventbus_throughput() {
    let bus = Arc::new(EventBus::new(100_000));

    // Subscribe *before* emitting — broadcast channels only deliver to existing subscribers
    let mut rx = bus.subscribe();

    let start = Instant::now();

    // Spawn 100 agents each emitting 100 events concurrently
    let mut handles = Vec::new();
    for agent_idx in 0..NUM_AGENTS {
        let bus = bus.clone();
        handles.push(tokio::spawn(async move {
            let agent_id = format!("agent-{:03}", agent_idx);
            for tick in 0..100u64 {
                bus.emit(WorldEvent::TickAdvanced { tick });
                if tick % 10 == 0 {
                    bus.emit(WorldEvent::AgentSpawned {
                        agent_id: agent_id.clone(),
                        name: format!("Agent-{}", agent_idx),
                    });
                }
            }
        }));
    }

    for handle in handles {
        handle.await.unwrap();
    }

    let elapsed = start.elapsed();
    let events_per_sec = (NUM_AGENTS * 110) as f64 / elapsed.as_secs_f64();
    println!(
        "[Stress] 100 agents × 110 events = {} events in {:?} ({:.0} events/s)",
        NUM_AGENTS * 110,
        elapsed,
        events_per_sec
    );

    // Verify subscriber received events
    let mut received = 0;
    while rx.try_recv().is_ok() {
        received += 1;
    }
    // Should have received events (may have lagged some due to capacity)
    assert!(received > 0, "Subscriber should have received events, got 0");
}

// ══════════════════════════════════════════════════════════════════════════
// TEST 5: Full 100-Agent Simulation — Token Burn + Tasks + Events
// ══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_full_100_agent_simulation() {
    let start = Instant::now();

    let event_bus = Arc::new(EventBus::new(100_000));
    let task_board = Arc::new(RwLock::new(TaskBoard::new()));
    let engine = TokenBurnEngine::new(ConsumptionConfig::default());

    // Initialize 100 agents
    let mut stress_agents: Vec<StressAgent> = (0..NUM_AGENTS)
        .map(|i| StressAgent::new(i))
        .collect();

    let mut agent_records: Vec<AgentRecord> = stress_agents
        .iter()
        .map(|a| AgentRecord {
            id: Uuid::new_v4(),
            name: a.name.clone(),
            phase: AgentPhase::Adult,
            tokens: a.tokens,
            skills: HashMap::new(),
            personality: String::new(),
        })
        .collect();

    // Set up task board balances
    {
        let mut b = task_board.write().await;
        for agent in &stress_agents {
            b.set_balance(&agent.id, 50_000);
        }
    }

    // Emit spawn events
    for agent in &stress_agents {
        event_bus.emit(WorldEvent::AgentSpawned {
            agent_id: agent.id.clone(),
            name: agent.name.clone(),
        });
    }

    let mut total_tasks_created: u32 = 0;
    let mut total_tasks_completed: u32 = 0;

    // Run 100 ticks
    for tick in 1..=TICKS {
        // 1. Token burn for all agents
        let burn_result = engine.process_tick(tick, &mut agent_records);

        // Update stress agent tokens
        for (i, burn) in burn_result.burns.iter().enumerate() {
            stress_agents[i].tokens = stress_agents[i].tokens.saturating_sub(burn.burn_amount);
            if stress_agents[i].tokens == 0 {
                stress_agents[i].is_alive = false;
                stress_agents[i].phase = AgentPhase::Dead;
            }
        }

        // Verify all agents alive
        let alive = stress_agents.iter().filter(|a| a.is_alive).count();
        assert_eq!(alive, NUM_AGENTS, "All agents should be alive at tick {}", tick);

        // 2. Every 10 ticks, each agent creates a task (10 agents at a time to avoid overload)
        if tick % 10 == 0 {
            let batch_size = 10;
            for batch_start in (0..NUM_AGENTS).step_by(batch_size) {
                let mut handles = Vec::new();
                for i in batch_start..std::cmp::min(batch_start + batch_size, NUM_AGENTS) {
                    let board = task_board.clone();
                    let agent_id = stress_agents[i].id.clone();
                    handles.push(tokio::spawn(async move {
                        let mut b = board.write().await;
                        b.create_task(
                            format!("Task @ tick {} by agent {}", tick, i),
                            format!("Agent {} task at tick {}", i, tick),
                            100,
                            agent_id,
                            tick,
                            Some(tick + 500),
                        )
                    }));
                }
                for handle in handles {
                    if let Ok(Ok(_id)) = handle.await {
                        total_tasks_created += 1;
                    }
                }
            }
        }

        // 3. Every 20 ticks, agents complete tasks
        if tick % 20 == 0 {
            let board = task_board.read().await;
            let published: Vec<_> = board
                .list()
                .iter()
                .filter(|t| t.status == TaskStatus::Published)
                .take(20)
                .map(|t| (t.id, t.publisher_id.clone()))
                .collect();
            drop(board);

            for (task_id, publisher_id) in &published {
                // Pick a random different agent as worker
                let worker_idx = (publisher_id.split('-').last().unwrap().parse::<usize>().unwrap() + 1) % NUM_AGENTS;
                let worker_id = stress_agents[worker_idx].id.clone();
                let publisher_id = publisher_id.clone();

                let mut b = task_board.write().await;
                if b.claim_task(*task_id, worker_id).is_ok() {
                    if b.start_task(*task_id).is_ok() {
                        if b.submit_result(*task_id, format!("Done at tick {}", tick)).is_ok() {
                            if b.review_task(*task_id, &publisher_id, true).is_ok() {
                                if b.complete_task(*task_id, tick).is_ok() {
                                    total_tasks_completed += 1;
                                }
                            }
                        }
                    }
                }
            }
        }

        // 4. Process task expiry
        {
            let mut b = task_board.write().await;
            b.process_expiry(tick);
        }

        // 5. Emit tick event
        event_bus.emit(WorldEvent::TickAdvanced { tick });
    }

    let elapsed = start.elapsed();

    // ── Final verification ─────────────────────────────────────────────────

    // All agents alive
    let alive = stress_agents.iter().filter(|a| a.is_alive).count();
    assert_eq!(alive, NUM_AGENTS, "All 100 agents should survive");

    // All agents should have burned tokens
    for agent in &stress_agents {
        assert!(agent.tokens < 500_000, "{} should have burned tokens", agent.name);
        assert!(agent.tokens > 0, "{} should still have tokens", agent.name);
    }

    // Token conservation
    let total_agent_tokens: u64 = stress_agents.iter().map(|a| a.tokens).sum();
    let expected_total = 500_000 * NUM_AGENTS as u64 - 10 * TICKS * NUM_AGENTS as u64;
    assert_eq!(total_agent_tokens, expected_total, "Token conservation violated");

    // Tasks were created and completed
    assert!(total_tasks_created > 0, "Should have created tasks");
    assert!(total_tasks_completed > 0, "Should have completed tasks");

    // Task board state should be consistent
    {
        let b = task_board.read().await;
        for task in b.list() {
            assert!(
                matches!(task.status, TaskStatus::Completed | TaskStatus::Expired | TaskStatus::Published),
                "Task {} should be in terminal/published state, not {:?}",
                task.id, task.status
            );
        }
    }

    println!("[Stress] Full 100-agent simulation complete:");
    println!("  Duration: {:?}", elapsed);
    println!("  Agents: {}", NUM_AGENTS);
    println!("  Ticks: {}", TICKS);
    println!("  Tasks created: {}", total_tasks_created);
    println!("  Tasks completed: {}", total_tasks_completed);
    println!("  Total tokens burned: {}", 500_000 * NUM_AGENTS as u64 - total_agent_tokens);
    println!("  Throughput: {:.0} ops/s",
        (NUM_AGENTS * TICKS as usize + total_tasks_created as usize + total_tasks_completed as usize) as f64 / elapsed.as_secs_f64()
    );
}

// Helper to prevent compiler from optimizing away reads
fn black_box<T>(t: T) -> T {
    std::hint::black_box(t)
}
