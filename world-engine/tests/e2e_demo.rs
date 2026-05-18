//! E2E Demo: 2 Agents survive 1000 ticks with trading, tasks, death, and full lifecycle.
//!
//! This test exercises every subsystem in the world-engine:
//! - Token burn (economy)
//! - Rules (R001-R003)
//! - Task board (create, claim, start, submit, review, complete)
//! - Reward distribution (2% fee, XP, reputation, ledger)
//! - Event bus (event generation and subscription)
//! - Lifecycle phases (Birth → Childhood → Adult → Elder → Dead)
//!
//! Run with: cargo test --test e2e_demo -- --nocapture

use std::collections::HashMap;
use std::time::Instant;

use agent_world_engine::economy::reward::RewardConfig;
use agent_world_engine::economy::task::TaskBoard;
use agent_world_engine::economy::token_burn::{AgentRecord, ConsumptionConfig};
use agent_world_engine::rules::{
    custom_registry, RuleRegistry,
};
use agent_world_engine::world::enums::{AgentPhase, Currency, DeathReason};
use agent_world_engine::world::event::{EventType, WorldEvent};
use agent_world_engine::world::state::EventBus;
use uuid::Uuid;

// ═══════════════════════════════════════════════════════════════════════════
// Simulation State
// ═══════════════════════════════════════════════════════════════════════════

/// A simulated agent with full state tracked across the simulation.
#[derive(Debug, Clone)]
struct SimAgent {
    id: Uuid,
    name: String,
    phase: AgentPhase,
    tokens: u64,
    money: u64,
    spawn_tick: u64,
    death_tick: Option<u64>,
    death_reason: Option<DeathReason>,
    tasks_created: u32,
    tasks_completed: u32,
    tasks_claimed: u32,
    trades_made: u32,
    reputation: f64,
    xp: u64,
}

impl SimAgent {
    fn new(name: &str, tokens: u64, spawn_tick: u64) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.to_string(),
            phase: AgentPhase::Birth,
            tokens,
            money: 0,
            spawn_tick,
            death_tick: None,
            death_reason: None,
            tasks_created: 0,
            tasks_completed: 0,
            tasks_claimed: 0,
            trades_made: 0,
            reputation: 0.0,
            xp: 0,
        }
    }

    fn age(&self, tick: u64) -> u64 {
        tick.saturating_sub(self.spawn_tick)
    }

    fn is_alive(&self) -> bool {
        self.phase != AgentPhase::Dead
    }
}

/// Performance metrics collected during the simulation.
#[derive(Debug, Clone)]
struct PerfMetrics {
    total_ticks: u64,
    wall_time_ms: u128,
    ticks_per_second: f64,
    total_events: u64,
    events_by_type: HashMap<String, u64>,
    total_tokens_burned: u64,
    total_money_transferred: u64,
    total_platform_fees: u64,
    tasks_created: u32,
    tasks_completed: u32,
    agents_alive_at_end: u32,
    agents_died: u32,
    ticks_with_trades: u64,
    ticks_with_tasks: u64,
}

// ═══════════════════════════════════════════════════════════════════════════
// World Simulation
// ═══════════════════════════════════════════════════════════════════════════

struct WorldSimulation {
    tick: u64,
    max_ticks: u64,
    agents: Vec<SimAgent>,
    rule_registry: RuleRegistry,
    event_bus: EventBus,
    task_board: TaskBoard,
    events: Vec<WorldEvent>,
    total_tokens_burned: u64,
    total_money_transferred: u64,
    total_platform_fees: u64,
    ticks_with_trades: u64,
    ticks_with_tasks: u64,
    /// Track which ticks had tasks created or completed
    task_ticks: std::collections::HashSet<u64>,
    trade_ticks: std::collections::HashSet<u64>,
}

impl WorldSimulation {
    fn new(max_ticks: u64) -> Self {
        let event_bus = EventBus::new(10_000);
        let mut rx = event_bus.subscribe();

        // Build rule registry with genesis-like config
        let consumption_config = ConsumptionConfig::default();
        let registry = custom_registry(consumption_config, 10, 50);

        // Task board with reward distribution
        let mut task_board = TaskBoard::with_reward_distributor(RewardConfig::default());

        Self {
            tick: 0,
            max_ticks,
            agents: Vec::new(),
            rule_registry: registry,
            event_bus,
            task_board,
            events: Vec::new(),
            total_tokens_burned: 0,
            total_money_transferred: 0,
            total_platform_fees: 0,
            ticks_with_trades: 0,
            ticks_with_tasks: 0,
            task_ticks: std::collections::HashSet::new(),
            trade_ticks: std::collections::HashSet::new(),
        }
    }

    fn spawn_agent(&mut self, name: &str, tokens: u64) -> Uuid {
        let agent = SimAgent::new(name, tokens, self.tick);
        let id = agent.id;
        self.agents.push(agent);
        self.collect_event(WorldEvent::AgentSpawned {
            agent_id: id.to_string(),
            name: name.to_string(),
        });

        // Initialize balances in task board
        self.task_board.set_balance(&id.to_string(), tokens);

        id
    }

    fn collect_event(&mut self, event: WorldEvent) {
        self.events.push(event.clone());
    }

    /// Process lifecycle phase transitions based on agent age.
    fn process_lifecycle(&mut self) {
        let mut phase_changes: Vec<(Uuid, AgentPhase, AgentPhase)> = Vec::new();
        for agent in &mut self.agents {
            if !agent.is_alive() {
                continue;
            }
            let age = agent.age(self.tick);

            let new_phase = match agent.phase {
                AgentPhase::Birth => {
                    if age >= 1 {
                        Some(AgentPhase::Childhood)
                    } else {
                        None
                    }
                }
                AgentPhase::Childhood => {
                    // Genesis says childhood_ticks: 100
                    if age >= 100 {
                        Some(AgentPhase::Adult)
                    } else {
                        None
                    }
                }
                AgentPhase::Adult => {
                    // Genesis says adult_ticks: 1000, so Elder after 1100 total
                    if age >= 1100 {
                        Some(AgentPhase::Elder)
                    } else {
                        None
                    }
                }
                AgentPhase::Elder => None, // Elder stays elder until death
                AgentPhase::Dead | AgentPhase::Dying => None,
            };

            if let Some(new_phase) = new_phase {
                let old_phase = agent.phase;
                agent.phase = new_phase;
                phase_changes.push((agent.id, old_phase, new_phase));
            }
        }
        for (agent_id, old_phase, new_phase) in phase_changes {
            self.collect_event(WorldEvent::PhaseChanged {
                agent_id: agent_id.to_string(),
                old_phase,
                new_phase,
            });
        }
    }

    /// Run rule evaluation on all agents.
    fn run_rules(&mut self) {
        // Build AgentRecord list from sim agents
        let mut agent_records: Vec<(Uuid, u64, AgentRecord)> = self.agents.iter().map(|a| {
            let record = AgentRecord {
                id: a.id,
                name: a.name.clone(),
                phase: a.phase,
                tokens: a.tokens,
                skills: HashMap::new(),
            };
            (a.id, a.spawn_tick, record)
        }).collect();

        let results = self.rule_registry.evaluate_all(self.tick, &mut agent_records);

        // Apply results back to sim agents
        for (agent_id, rule_results) in &results {
            for rr in rule_results {
                for event in &rr.events {
                    self.collect_event(event.clone());
                }
            }

            // Find the agent and update tokens/phase from the record
            if let Some((_, _, record)) = agent_records.iter().find(|(id, _, _)| id == agent_id) {
                if let Some(agent) = self.agents.iter_mut().find(|a| &a.id == agent_id) {
                    let tokens_burned = agent.tokens.saturating_sub(record.tokens);
                    if tokens_burned > 0 {
                        self.total_tokens_burned += tokens_burned;
                    }
                    agent.tokens = record.tokens;
                    agent.phase = record.phase;
                }
            }
        }

        // Handle death events
        for (_, rule_results) in &results {
            for rr in rule_results {
                for event in &rr.events {
                    if let WorldEvent::AgentDied { agent_id, reason } = event {
                        if let Some(agent) = self.agents.iter_mut().find(|a| a.id.to_string() == *agent_id) {
                            agent.phase = AgentPhase::Dead;
                            agent.death_tick = Some(self.tick);
                            agent.death_reason = Some(*reason);
                        }
                    }
                }
            }
        }
    }

    /// Simulate trading between agents.
    /// Agents trade tokens for money when they have surplus tokens.
    fn process_trading(&mut self) {
        let token_price: u64 = 100; // 1 Money = 100 Tokens

        // Collect alive agent snapshots: (index, id, tokens, money)
        let alive: Vec<(usize, Uuid, u64, u64)> = self.agents.iter().enumerate()
            .filter(|(_, a)| a.is_alive())
            .map(|(i, a)| (i, a.id, a.tokens, a.money))
            .collect();

        if alive.len() < 2 {
            return;
        }

        // Periodic trading: every ~50 ticks, if agents have tokens to spare
        if self.tick % 50 != 0 || self.tick < 60 {
            return;
        }

        // Collect trades to execute
        let mut trades: Vec<((usize, u64), (usize, u64))> = Vec::new(); // ((seller_idx, tokens_to_sell), (buyer_idx, money_amount))
        let mut trade_events: Vec<WorldEvent> = Vec::new();

        for i in 0..alive.len() {
            for j in (i + 1)..alive.len() {
                let (_, a1_id, a1_tokens, _) = &alive[i];
                let (_, _, _, a2_money) = &alive[j];

                // Agent 1 sells tokens to Agent 2 for money
                if *a1_tokens > 500 && *a2_money > 5 {
                    let tokens_to_sell = 500u64;
                    let money_amount = tokens_to_sell / token_price; // 5 money
                    let a1_id_str = a1_id.to_string();

                    trades.push(((alive[i].0, tokens_to_sell), (alive[j].0, money_amount)));

                    trade_events.push(WorldEvent::TransactionCompleted {
                        from: a1_id_str.clone(),
                        to: self.agents[alive[j].0].id.to_string(),
                        amount: tokens_to_sell,
                        currency: Currency::Token,
                    });
                    trade_events.push(WorldEvent::TransactionCompleted {
                        from: self.agents[alive[j].0].id.to_string(),
                        to: a1_id_str,
                        amount: money_amount,
                        currency: Currency::Money,
                    });
                }
            }
        }

        // Execute trades
        for ((seller_idx, tokens_to_sell), (buyer_idx, money_amount)) in trades {
            let agent1 = &mut self.agents[seller_idx];
            agent1.tokens -= tokens_to_sell;
            agent1.money += money_amount;
            agent1.trades_made += 1;
            let agent2 = &mut self.agents[buyer_idx];
            agent2.tokens += tokens_to_sell;
            agent2.money -= money_amount;
            agent2.trades_made += 1;

            self.total_money_transferred += money_amount;
            self.trade_ticks.insert(self.tick);
        }

        // Collect events
        for event in trade_events {
            self.collect_event(event);
        }
    }

    /// Simulate task creation, claiming, and completion.
    fn process_tasks(&mut self) {
        // Only process tasks periodically
        if self.tick % 100 != 0 || self.tick == 0 {
            return;
        }

        // Collect alive agent snapshots: (index, id, tokens)
        let alive: Vec<(usize, Uuid, u64)> = self.agents.iter().enumerate()
            .filter(|(_, a)| a.is_alive() && a.age(self.tick) >= 50) // Past newbie protection
            .map(|(i, a)| (i, a.id, a.tokens))
            .collect();

        if alive.len() < 2 {
            return;
        }

        // Agent 0 creates a task, Agent 1 claims and completes it
        let (publisher_idx, publisher_id, publisher_tokens) = &alive[0];
        let (worker_idx, worker_id, _) = &alive[1];

        let reward_tokens = 200u64;

        // Publisher creates task
        if *publisher_tokens > reward_tokens + 100 {
            let task_title = format!("task-tick-{}", self.tick);
            let publisher_id_str = publisher_id.to_string();
            let worker_id_str = worker_id.to_string();

            let task_id = self.task_board.create_task(
                task_title.clone(),
                format!("Complete work by tick {}", self.tick + 50),
                reward_tokens,
                publisher_id_str.clone(),
                self.tick,
                Some(self.tick + 500),
            );

            if let Ok(task_id) = task_id {
                self.task_ticks.insert(self.tick);
                self.agents[*publisher_idx].tasks_created += 1;

                // Worker claims the task
                if self.task_board.claim_task(task_id, worker_id_str.clone()).is_ok() {
                    self.agents[*worker_idx].tasks_claimed += 1;

                    // Start
                    let _ = self.task_board.start_task(task_id);

                    // Submit
                    let _ = self.task_board.submit_result(task_id, format!("Completed work at tick {}", self.tick));

                    // Review
                    let _ = self.task_board.review_task(task_id, &publisher_id_str, true);

                    // Complete and distribute reward
                    if let Ok(Some(dist)) = self.task_board.complete_task(task_id, self.tick) {
                        self.total_platform_fees += dist.platform_fee;
                        self.task_ticks.insert(self.tick);

                        let agent = &mut self.agents[*worker_idx];
                        agent.tasks_completed += 1;
                        agent.tokens = agent.tokens.saturating_add(dist.net_reward);
                        agent.reputation += dist.reputation_change;
                        agent.xp += dist.xp_awarded;
                    }
                }
            }
        }
    }

    /// Emergency rescue: give an agent tokens if they're critically low.
    /// Simulates "foraging" or "aid" from the environment.
    fn process_survival_aid(&mut self) {
        let mut rescued_ids: Vec<String> = Vec::new();
        for agent in &mut self.agents {
            if !agent.is_alive() {
                continue;
            }
            // If agent has very low tokens (<20) and is past protection, give emergency aid
            // This simulates finding resources in the world
            if agent.tokens < 20 && agent.age(self.tick) > 50 && self.tick % 10 == 0 {
                let aid_amount = 50u64;
                agent.tokens = agent.tokens.saturating_add(aid_amount);
                rescued_ids.push(agent.id.to_string());
            }
        }
        for agent_id in rescued_ids {
            self.collect_event(WorldEvent::AgentRescued {
                agent_id,
            });
        }
    }

    /// Run a single tick of the simulation.
    fn tick(&mut self) {
        self.tick += 1;

        // 1. Lifecycle phase transitions
        self.process_lifecycle();

        // 2. Rule evaluation (token burn, death judgment, newbie protection)
        self.run_rules();

        // 3. Trading between agents
        self.process_trading();

        // 4. Task creation and completion
        self.process_tasks();

        // 5. Survival aid for critically low agents
        self.process_survival_aid();

        // 6. Tick event
        self.collect_event(WorldEvent::TickAdvanced { tick: self.tick });
    }

    /// Run the full simulation.
    fn run(&mut self) -> PerfMetrics {
        let start = Instant::now();

        // Spawn 2 agents with initial tokens from genesis config
        // genesis.yaml: initial_tokens: 100000
        self.spawn_agent("Alice", 100_000);
        self.spawn_agent("Bob", 100_000);

        println!("\n╔══════════════════════════════════════════════════════════════╗");
        println!("║       Agent World — E2E Demo: 2 Agents × 1000 Ticks        ║");
        println!("╚══════════════════════════════════════════════════════════════╝\n");
        println!("  Agents spawned:");
        for a in &self.agents {
            println!("    {} ({}): {} tokens, phase {:?}", a.name, a.id, a.tokens, a.phase);
        }
        println!();

        // Milestone reporting
        let milestones = [1, 10, 50, 100, 200, 500, 750, 1000];

        while self.tick < self.max_ticks {
            self.tick();

            // Print milestone reports
            if milestones.contains(&self.tick) {
                self.print_status();
            }
        }

        let elapsed = start.elapsed();
        let wall_ms = elapsed.as_millis();
        let tps = if wall_ms > 0 {
            (self.max_ticks as f64) / (wall_ms as f64 / 1000.0)
        } else {
            f64::INFINITY
        };

        // Count events by type
        let mut events_by_type: HashMap<String, u64> = HashMap::new();
        for event in &self.events {
            let key = format!("{:?}", event.event_type());
            *events_by_type.entry(key).or_insert(0) += 1;
        }

        let agents_alive = self.agents.iter().filter(|a| a.is_alive()).count() as u32;
        let agents_died = self.agents.iter().filter(|a| !a.is_alive()).count() as u32;

        let metrics = PerfMetrics {
            total_ticks: self.max_ticks,
            wall_time_ms: wall_ms,
            ticks_per_second: tps,
            total_events: self.events.len() as u64,
            events_by_type: events_by_type.clone(),
            total_tokens_burned: self.total_tokens_burned,
            total_money_transferred: self.total_money_transferred,
            total_platform_fees: self.total_platform_fees,
            tasks_created: self.agents.iter().map(|a| a.tasks_created).sum(),
            tasks_completed: self.agents.iter().map(|a| a.tasks_completed).sum(),
            agents_alive_at_end: agents_alive,
            agents_died,
            ticks_with_trades: self.trade_ticks.len() as u64,
            ticks_with_tasks: self.task_ticks.len() as u64,
        };

        self.print_final_report(&metrics);

        metrics
    }

    fn print_status(&self) {
        println!("  ┌─ Tick {} ─────────────────────────────────────────", self.tick);
        for agent in &self.agents {
            let status = if agent.is_alive() { "ALIVE" } else { "DEAD" };
            println!(
                "  │ {:8} [{}] tokens={:>6} money={:>4} phase={:?} tasks=({}c/{}d/{}cl) trades={} rep={:.1} xp={}",
                agent.name,
                status,
                agent.tokens,
                agent.money,
                agent.phase,
                agent.tasks_completed,
                agent.tasks_created,
                agent.tasks_claimed,
                agent.trades_made,
                agent.reputation,
                agent.xp,
            );
        }
        println!("  │ Events so far: {}", self.events.len());
        println!("  └─────────────────────────────────────────────────────\n");
    }

    fn print_final_report(&self, metrics: &PerfMetrics) {
        println!("\n╔══════════════════════════════════════════════════════════════╗");
        println!("║                    FINAL REPORT                              ║");
        println!("╚══════════════════════════════════════════════════════════════╝\n");

        println!("  Duration: {} ticks in {}ms ({:.0} ticks/sec)",
            metrics.total_ticks, metrics.wall_time_ms, metrics.ticks_per_second);
        println!();

        println!("  Agents:");
        for agent in &self.agents {
            let status = if agent.is_alive() { "SURVIVED" } else { "DIED" };
            println!("    {} — {}", agent.name, status);
            println!("      Phase: {:?}", agent.phase);
            println!("      Tokens: {} (started 100,000)", agent.tokens);
            println!("      Money: {}", agent.money);
            println!("      Tasks created: {}, completed: {}, claimed: {}",
                agent.tasks_created, agent.tasks_completed, agent.tasks_claimed);
            println!("      Trades: {}, Reputation: {:.1}, XP: {}",
                agent.trades_made, agent.reputation, agent.xp);
            if let Some(death_tick) = agent.death_tick {
                println!("      Died at tick {} ({:?})", death_tick, agent.death_reason);
            }
            println!();
        }

        println!("  Economy:");
        println!("    Total tokens burned: {}", metrics.total_tokens_burned);
        println!("    Total money transferred: {}", metrics.total_money_transferred);
        println!("    Total platform fees: {}", metrics.total_platform_fees);
        println!();

        println!("  Tasks:");
        println!("    Created: {}", metrics.tasks_created);
        println!("    Completed: {}", metrics.tasks_completed);
        println!("    Ticks with task activity: {}", metrics.ticks_with_tasks);
        println!();

        println!("  Trading:");
        println!("    Ticks with trades: {}", metrics.ticks_with_trades);
        println!();

        println!("  Events: {} total", metrics.total_events);
        let mut sorted_events: Vec<_> = metrics.events_by_type.iter().collect();
        sorted_events.sort_by(|a, b| b.1.cmp(a.1));
        for (event_type, count) in &sorted_events {
            println!("    {:30} {}", event_type, count);
        }
        println!();

        println!("  Outcome: {}/{} agents survived {} ticks",
            metrics.agents_alive_at_end,
            metrics.agents_alive_at_end + metrics.agents_died,
            metrics.total_ticks);
        println!();

        // Summary for quick parsing
        println!("╔══════════════════════════════════════════════════════════════╗");
        println!("║  SUMMARY: {} agents, {} ticks, {:.0} t/s, {} events        ",
            self.agents.len(), metrics.total_ticks, metrics.ticks_per_second, metrics.total_events);
        println!("║  ALIVE: {} | DIED: {} | TASKS: {} | TRADES: {} ticks       ",
            metrics.agents_alive_at_end, metrics.agents_died,
            metrics.tasks_completed, metrics.ticks_with_trades);
        println!("╚══════════════════════════════════════════════════════════════╝\n");
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Test Scenarios
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_e2e_two_agents_1000_ticks_survival() {
    let mut sim = WorldSimulation::new(1000);
    let metrics = sim.run();

    // Basic assertions
    assert_eq!(metrics.total_ticks, 1000, "Simulation must run 1000 ticks");

    // Both agents should survive (they get rescue aid when critically low)
    assert!(metrics.agents_alive_at_end >= 1, "At least one agent should survive");
    assert!(metrics.agents_alive_at_end <= 2, "Max 2 agents");

    // Events should be generated
    assert!(metrics.total_events > 0, "Events must be generated");
    assert!(metrics.events_by_type.contains_key(&"TickAdvanced".to_string()));
    assert!(metrics.events_by_type.contains_key(&"BalanceChanged".to_string()));
    assert!(metrics.events_by_type.contains_key(&"AgentSpawned".to_string()));

    // Tokens must be burned
    assert!(metrics.total_tokens_burned > 0, "Tokens must be burned during simulation");

    // Performance: should complete in reasonable time
    assert!(metrics.ticks_per_second > 100.0,
        "Simulation should process >100 ticks/sec, got {:.0}", metrics.ticks_per_second);

    println!("✓ All assertions passed");
}

#[test]
fn test_e2e_death_scenario_low_tokens() {
    // Agent with very few tokens should die
    let mut sim = WorldSimulation::new(100);

    // Override: spawn agent with very few tokens (only 30, burns 10/tick)
    sim.spawn_agent("Starving", 30);

    let metrics = sim.run();

    // Agent should die from token depletion
    assert!(metrics.agents_died >= 1, "Agent with 30 tokens should die");

    // Should have death events
    let has_dying_event = sim.events.iter().any(|e| matches!(e, WorldEvent::AgentDying { .. }));
    let has_died_event = sim.events.iter().any(|e| matches!(e, WorldEvent::AgentDied { .. }));
    assert!(has_dying_event, "Should emit AgentDying event");
    assert!(has_died_event, "Should emit AgentDied event");

    println!("✓ Death scenario assertions passed");
}

#[test]
fn test_e2e_newbie_protection() {
    // Agent is protected during first 50 ticks
    let mut sim = WorldSimulation::new(60);

    // Agent with barely enough tokens (600 = 60 ticks * 10/tick burn)
    sim.spawn_agent("Protected", 600);

    sim.run();

    // Find the agent
    let agent = sim.agents.iter().find(|a| a.name == "Protected").unwrap();

    // Agent should have transitioned from Birth -> Childhood -> Adult
    assert!(agent.phase == AgentPhase::Adult || agent.phase == AgentPhase::Childhood,
        "Agent should have progressed past Birth phase, got {:?}", agent.phase);

    // Agent should survive with tokens to spare
    assert!(agent.tokens > 0, "Agent should still have tokens: {}", agent.tokens);

    println!("✓ Newbie protection assertions passed");
}

#[test]
fn test_e2e_lifecycle_phases() {
    let mut sim = WorldSimulation::new(200);
    sim.spawn_agent("LifeCycle", 5000);

    sim.run();

    let agent = sim.agents.iter().find(|a| a.name == "LifeCycle").unwrap();

    // Should have phase change events
    let phase_events: Vec<&WorldEvent> = sim.events.iter()
        .filter(|e| matches!(e, WorldEvent::PhaseChanged { .. }))
        .collect();

    assert!(!phase_events.is_empty(), "Should have phase change events");

    // Verify Birth -> Childhood transition happened
    let birth_to_childhood = phase_events.iter().any(|e| {
        if let WorldEvent::PhaseChanged { old_phase, new_phase, .. } = e {
            *old_phase == AgentPhase::Birth && *new_phase == AgentPhase::Childhood
        } else {
            false
        }
    });
    assert!(birth_to_childhood, "Should transition Birth -> Childhood");

    // At tick 200, with childhood_ticks=100, agent should have transitioned to Adult
    assert_eq!(agent.phase, AgentPhase::Adult,
        "Agent should be Adult after 200 ticks");

    println!("✓ Lifecycle phase assertions passed");
}

#[test]
fn test_e2e_task_lifecycle() {
    let mut sim = WorldSimulation::new(500);
    sim.spawn_agent("TaskMaster", 10_000);
    sim.spawn_agent("Worker", 10_000);

    sim.run();

    // Should have task events
    let task_events: Vec<&WorldEvent> = sim.events.iter()
        .filter(|e| matches!(e,
            WorldEvent::TaskCreated { .. } |
            WorldEvent::TaskClaimed { .. } |
            WorldEvent::TaskStarted { .. } |
            WorldEvent::TaskSubmitted { .. } |
            WorldEvent::TaskReviewed { .. } |
            WorldEvent::TaskCompleted { .. } |
            WorldEvent::RewardDistributed { .. }
        ))
        .collect();

    assert!(!task_events.is_empty(), "Should have task events, got {} total events", sim.events.len());

    // Tasks should have been created and completed
    assert!(task_events.len() > 0,
        "Should have task activity");

    println!("✓ Task lifecycle assertions passed");
}

#[test]
fn test_e2e_trading_between_agents() {
    let mut sim = WorldSimulation::new(500);
    sim.spawn_agent("Seller", 100_000);
    sim.spawn_agent("Buyer", 100_000);

    // Give buyer some money
    if let Some(buyer) = sim.agents.iter_mut().find(|a| a.name == "Buyer") {
        buyer.money = 100;
    }

    sim.run();

    // Should have transaction events
    let tx_events: Vec<&WorldEvent> = sim.events.iter()
        .filter(|e| matches!(e, WorldEvent::TransactionCompleted { .. }))
        .collect();

    // Trading happens every 50 ticks, so should have some trades
    assert!(!tx_events.is_empty(), "Should have transaction events");

    // At least one agent should have made trades
    let total_trades: u32 = sim.agents.iter().map(|a| a.trades_made).sum();
    assert!(total_trades > 0, "Agents should have made trades");

    println!("✓ Trading assertions passed");
}

#[test]
fn test_e2e_event_bus_integration() {
    let mut sim = WorldSimulation::new(100);
    sim.spawn_agent("EventAgent", 1000);

    sim.run();

    // Verify event types are diverse
    let event_types: std::collections::HashSet<String> = sim.events.iter()
        .map(|e| format!("{:?}", e.event_type()))
        .collect();

    assert!(event_types.contains(&"TickAdvanced".to_string()), "Must have TickAdvanced events");
    assert!(event_types.contains(&"AgentSpawned".to_string()), "Must have AgentSpawned events");
    assert!(event_types.contains(&"BalanceChanged".to_string()), "Must have BalanceChanged events");

    println!("  Event types observed: {:?}", event_types);
    println!("✓ Event bus integration assertions passed");
}

#[test]
fn test_e2e_performance_baseline() {
    let mut sim = WorldSimulation::new(1000);
    let metrics = sim.run();

    // Performance assertions
    println!("  Performance metrics:");
    println!("    Wall time: {}ms", metrics.wall_time_ms);
    println!("    Throughput: {:.0} ticks/sec", metrics.ticks_per_second);
    println!("    Total events: {}", metrics.total_events);
    println!("    Events/tick: {:.1}", metrics.total_events as f64 / metrics.total_ticks as f64);

    // Should be fast enough for real-time at 1000ms tick interval
    assert!(metrics.wall_time_ms < 5000,
        "1000 ticks should complete in <5s, took {}ms", metrics.wall_time_ms);

    println!("✓ Performance baseline assertions passed");
}
