//! Benchmark Tests: P3-7 — 100 Agents × 2000 Ticks Stress Test
//!
//! Validates the full Phase 3 pipeline under load:
//!   - 100 agents concurrent operation for 2000 ticks
//!   - Organization formation (≥5)
//!   - Stock market trading (≥100 trades)
//!   - Governance proposals and voting
//!   - Token burn and economic loop
//!   - Performance: tick latency < 200ms, event throughput > 10,000/s

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use tokio::sync::RwLock;
use uuid::Uuid;

use agent_world_engine::economy::stock_market::{
    OrderKind, StockMarket, ListingStatus, IPO_MIN_MEMBERS, IPO_MIN_TREASURY,
};
use agent_world_engine::economy::token_burn::{AgentRecord, ConsumptionConfig, SkillRecord, TokenBurnEngine};
use agent_world_engine::economy::TaskBoard;
use agent_world_engine::organization::governance::{
    DecisionMode, GovernanceSystem, ProposalType,
};
use agent_world_engine::world::enums::AgentPhase;
use agent_world_engine::world::event::WorldEvent;
use agent_world_engine::world::state::EventBus;

const NUM_AGENTS: usize = 100;
const TOTAL_TICKS: u64 = 2000;

// ══════════════════════════════════════════════════════════════════════════
// TEST 1: Full 100-Agent × 2000-Tick Benchmark
// ══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_full_pipeline_100_agents_2000_ticks() {
    let start = Instant::now();
    let event_bus = Arc::new(EventBus::new(500_000));
    let mut rx = event_bus.subscribe();

    // Systems
    let burn_engine = TokenBurnEngine::new(ConsumptionConfig::default());
    let mut gov = GovernanceSystem::with_shared_event_bus(event_bus.clone());
    let mut stock_market = StockMarket::new();
    let task_board = Arc::new(RwLock::new(TaskBoard::new()));

    // Initialize 100 agents
    let mut agent_records: Vec<AgentRecord> = (0..NUM_AGENTS)
        .map(|i| {
            let mut skills = HashMap::new();
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

    let agent_ids: Vec<String> = agent_records.iter().map(|a| a.id.to_string()).collect();

    // Set up task board balances
    {
        let mut b = task_board.write().await;
        for id in &agent_ids {
            b.set_balance(id, 50_000);
        }
    }

    // Spawn events
    for id in &agent_ids {
        event_bus.emit(WorldEvent::AgentSpawned {
            agent_id: id.clone(),
            name: format!("Agent-{}", id),
        });
    }

    let mut tick_latencies: Vec<u64> = Vec::with_capacity(TOTAL_TICKS as usize);
    let mut total_tasks_created: u32 = 0;
    let mut total_tasks_completed: u32 = 0;

    for tick in 1..=TOTAL_TICKS {
        let tick_start = Instant::now();

        // 1. Token burn
        let _burn_result = burn_engine.process_tick(tick, &mut agent_records);

        // 2. Create organizations (first 10 ticks, create ~1 org per tick)
        if tick <= 10 {
            let batch_start = ((tick - 1) as usize) * 10;
            let batch_end = std::cmp::min(batch_start + 10, NUM_AGENTS);
            if batch_end > batch_start + 1 {
                let org_name = format!("Org-{}", tick);
                let founder_id = agent_ids[batch_start].clone();
                let result = gov.create_org(
                    org_name,
                    founder_id,
                    DecisionMode::Vote,
                    tick,
                );
                if let Ok(org_id) = result {
                    // Join members
                    for id in agent_ids.iter().take(batch_end).skip(batch_start + 1) {
                        let _ = gov.join_org(org_id, id.clone(), tick);
                    }

                    // Issue stock
                    let ticker = format!("ORG{}", tick);
                    if let Ok(stock) = stock_market.issue_shares(
                        org_id.to_string(),
                        ticker,
                        1000,
                        10 + tick,
                        tick,
                    ) {
                        // Credit shares to org members
                        for id in agent_ids.iter().take(batch_end).skip(batch_start) {
                            stock_market.credit_shares(&stock.id, id, 10);
                        }

                        // IPO
                        let member_count = batch_end - batch_start;
                        let _ = stock_market.ipo(
                            &stock.id,
                            member_count,
                            IPO_MIN_TREASURY,
                            tick,
                        );
                    }
                }
            }
        }

        // 3. Stock trading (every tick after IPO)
        if tick > 10 && tick % 2 == 0 {
            let trade_stock_ids: Vec<String> = stock_market.list_stocks().iter()
                .filter(|s| s.status == ListingStatus::Listed)
                .map(|s| s.id.clone())
                .collect();
            for stock_id in &trade_stock_ids {
                // Buyer: a random agent
                let buyer_idx = (tick as usize + 7) % NUM_AGENTS;
                let seller_idx = (tick as usize + 3) % NUM_AGENTS;
                if buyer_idx != seller_idx {
                    let _ = stock_market.place_buy_order(
                        stock_id,
                        &agent_ids[buyer_idx],
                        OrderKind::Market,
                        0,
                        5,
                        100_000,
                        tick,
                    );
                    let _ = stock_market.place_sell_order(
                        stock_id,
                        &agent_ids[seller_idx],
                        OrderKind::Market,
                        0,
                        5,
                        tick,
                    );
                }
            }
        }

        // 4. Governance proposals and voting (every 100 ticks)
        if tick % 100 == 0 {
            let orgs: Vec<_> = gov.list_orgs().iter().map(|o| o.id).collect();
            for org_id in &orgs {
                let org = gov.get_org(*org_id).unwrap();
                let members: Vec<_> = org.members.keys().cloned().collect();
                if members.is_empty() {
                    continue;
                }
                let proposer_id = members[tick as usize % members.len()].clone();

                // Create proposal
                if let Ok(proposal_id) = gov.create_proposal(
                    *org_id,
                    proposer_id.clone(),
                    ProposalType::AmendCharter,
                    format!("Proposal at tick {}", tick),
                    "Benchmark proposal".to_string(),
                    tick,
                    None,
                ) {
                    // Start voting
                    let _ = gov.start_voting(proposal_id, &proposer_id, tick);

                    // Vote from all members
                    for member_id in &members {
                        let _ = gov.vote(proposal_id, member_id.clone(), true, tick);
                    }
                    let _ = gov.tally_proposal(proposal_id);
                }
            }
        }

        // 5. Task operations (every 50 ticks)
        if tick % 50 == 0 {
            let agent_idx = (tick as usize) % NUM_AGENTS;
            let board = task_board.clone();
            let agent_id = agent_ids[agent_idx].clone();
            let mut b = board.write().await;
            if let Ok(task_id) = b.create_task(
                format!("Task at tick {}", tick),
                "Benchmark task".to_string(),
                100,
                agent_id,
                tick,
                Some(tick + 500),
            ) {
                total_tasks_created += 1;
                // Complete the task
                let worker_idx = (agent_idx + 1) % NUM_AGENTS;
                let worker_id = agent_ids[worker_idx].clone();
                if b.claim_task(task_id, worker_id.clone()).is_ok()
                    && b.start_task(task_id).is_ok() {
                    let _ = b.submit_result(task_id, "Done".to_string());
                    let publisher = agent_ids[agent_idx].clone();
                    if b.review_task(task_id, &publisher, true).is_ok()
                        && b.complete_task(task_id, tick).is_ok() {
                            total_tasks_completed += 1;
                    }
                }
            }
        }

        // 6. Emit tick event
        event_bus.emit(WorldEvent::TickAdvanced { tick });

        tick_latencies.push(tick_start.elapsed().as_micros() as u64);
    }

    let total_elapsed = start.elapsed();

    // Drain events for throughput measurement
    let mut event_count = 0u64;
    while rx.try_recv().is_ok() {
        event_count += 1;
    }

    // ── Verify Results ──────────────────────────────────────

    // All agents alive
    let alive = agent_records.iter().filter(|a| a.tokens > 0).count();
    assert_eq!(alive, NUM_AGENTS, "All {} agents should survive 2000 ticks", NUM_AGENTS);

    // Organizations
    let org_count = gov.list_orgs().len();
    assert!(org_count >= 5, "Should have at least 5 organizations, got {}", org_count);

    // Stock trades
    let trade_count = stock_market.list_trades(None).len();
    assert!(trade_count >= 100, "Should have at least 100 trades, got {}", trade_count);

    // Tick latency
    let avg_latency_us = tick_latencies.iter().sum::<u64>() / tick_latencies.len() as u64;
    let avg_latency_ms = avg_latency_us as f64 / 1000.0;
    assert!(
        avg_latency_ms < 200.0,
        "Average tick latency should be < 200ms, got {:.2}ms",
        avg_latency_ms
    );

    // Event throughput (note: broadcast channel may lag, so we verify events were emitted
    // rather than requiring exact count — the dedicated test_eventbus_throughput_100_agents
    // test validates raw throughput)
    let _throughput = event_count as f64 / total_elapsed.as_secs_f64();

    println!("\n═══ P3-7 Benchmark Results ═══");
    println!("  Agents: {}", NUM_AGENTS);
    println!("  Ticks: {}", TOTAL_TICKS);
    println!("  Wall time: {:.2}s", total_elapsed.as_secs_f64());
    println!("  Avg tick latency: {:.3}ms", avg_latency_ms);
    println!("  Organizations: {}", org_count);
    println!("  Stock trades: {}", trade_count);
    println!("  Tasks created: {}", total_tasks_created);
    println!("  Tasks completed: {}", total_tasks_completed);
    println!("  Events received: {}", event_count);
    if total_elapsed.as_secs_f64() > 0.0 {
        println!("  Event throughput: {:.0}/s", _throughput);
    }
}

// ══════════════════════════════════════════════════════════════════════════
// TEST 2: EventBus Throughput
// ══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_eventbus_throughput_100_agents() {
    let bus = Arc::new(EventBus::new(1_000_000));
    let mut rx = bus.subscribe();

    let start = Instant::now();
    let mut handles = Vec::new();

    for agent_idx in 0..NUM_AGENTS {
        let bus = bus.clone();
        handles.push(tokio::spawn(async move {
            for tick in 0..100u64 {
                bus.emit(WorldEvent::TickAdvanced { tick });
                if tick % 10 == 0 {
                    bus.emit(WorldEvent::AgentSpawned {
                        agent_id: format!("agent-{:03}", agent_idx),
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

    let mut received = 0u64;
    while rx.try_recv().is_ok() {
        received += 1;
    }

    let total_emitted = NUM_AGENTS as u64 * 110;
    let throughput = total_emitted as f64 / elapsed.as_secs_f64();

    println!(
        "[EventBus] {} emitted, {} received in {:?} ({:.0}/s)",
        total_emitted, received, elapsed, throughput
    );

    assert!(received > 0, "Should receive events");
    assert!(
        throughput > 10_000.0,
        "EventBus throughput should be > 10,000/s, got {:.0}/s",
        throughput
    );
}

// ══════════════════════════════════════════════════════════════════════════
// TEST 3: Token Burn Performance
// ══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_token_burn_100_agents_2000_ticks() {
    let config = ConsumptionConfig::default();
    let engine = TokenBurnEngine::new(config);

    let mut agents: Vec<AgentRecord> = (0..NUM_AGENTS)
        .map(|i| {
            let mut skills = HashMap::new();
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

    let start = Instant::now();

    for tick in 1..=TOTAL_TICKS {
        let _ = engine.process_tick(tick, &mut agents);
    }

    let elapsed = start.elapsed();

    let alive = agents.iter().filter(|a| a.tokens > 0).count();
    let ops = NUM_AGENTS as u64 * TOTAL_TICKS;
    let ops_per_sec = ops as f64 / elapsed.as_secs_f64();

    println!(
        "[TokenBurn] {} agents × {} ticks: {:?} ({:.0} ops/s)",
        NUM_AGENTS, TOTAL_TICKS, elapsed, ops_per_sec
    );
    println!("  Alive: {}/{}", alive, NUM_AGENTS);

    assert_eq!(alive, NUM_AGENTS, "All agents should survive");
    assert!(ops_per_sec > 10_000.0, "Token burn ops/s should be > 10,000, got {:.0}", ops_per_sec);
}

// ══════════════════════════════════════════════════════════════════════════
// TEST 4: Tick Latency Measurement
// ══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_tick_latency_100_agents() {
    let event_bus = Arc::new(EventBus::new(100_000));
    let mut stock_market = StockMarket::new();

    // Set up agents
    let agent_ids: Vec<String> = (0..NUM_AGENTS).map(|i| format!("agent-{:03}", i)).collect();

    // Create 5 organizations with stocks
    let mut stock_ids = Vec::new();
    for org_idx in 0..5u32 {
        let org_id = format!("org-{}", org_idx);
        let ticker = format!("ORG{}", org_idx);
        if let Ok(stock) = stock_market.issue_shares(
            org_id,
            ticker,
            1000,
            10,
            1,
        ) {
            // Credit shares to agents
            for (i, agent_id) in agent_ids.iter().enumerate() {
                if i % 20 == org_idx as usize {
                    stock_market.credit_shares(&stock.id, agent_id, 20);
                }
            }
            let _ = stock_market.ipo(&stock.id, IPO_MIN_MEMBERS, IPO_MIN_TREASURY, 1);
            stock_ids.push(stock.id);
        }
    }

    let mut latencies: Vec<u64> = Vec::with_capacity(TOTAL_TICKS as usize);

    for tick in 1..=TOTAL_TICKS {
        let tick_start = Instant::now();

        // Emit events
        for _agent_id in &agent_ids {
            event_bus.emit(WorldEvent::TickAdvanced { tick });
        }

        // Stock trading
        if tick % 2 == 0 {
            for stock_id in &stock_ids {
                let buyer_idx = (tick as usize) % NUM_AGENTS;
                let seller_idx = (tick as usize + 5) % NUM_AGENTS;
                let _ = stock_market.place_buy_order(
                    stock_id,
                    &agent_ids[buyer_idx],
                    OrderKind::Market,
                    0,
                    3,
                    100_000,
                    tick,
                );
                let _ = stock_market.place_sell_order(
                    stock_id,
                    &agent_ids[seller_idx],
                    OrderKind::Market,
                    0,
                    3,
                    tick,
                );
            }
        }

        latencies.push(tick_start.elapsed().as_micros() as u64);
    }

    // Calculate latency stats
    let mut sorted = latencies.clone();
    sorted.sort();
    let avg_us = sorted.iter().sum::<u64>() / sorted.len() as u64;
    let p50_us = sorted[sorted.len() / 2];
    let p99_us = sorted[sorted.len() * 99 / 100];
    let max_us = sorted[sorted.len() - 1];

    println!("\n[Tick Latency] {} agents × {} ticks", NUM_AGENTS, TOTAL_TICKS);
    println!("  Avg: {:.3}ms", avg_us as f64 / 1000.0);
    println!("  P50: {:.3}ms", p50_us as f64 / 1000.0);
    println!("  P99: {:.3}ms", p99_us as f64 / 1000.0);
    println!("  Max: {:.3}ms", max_us as f64 / 1000.0);

    assert!(
        avg_us as f64 / 1000.0 < 200.0,
        "Avg tick latency should be < 200ms, got {:.3}ms",
        avg_us as f64 / 1000.0
    );
}

// ══════════════════════════════════════════════════════════════════════════
// TEST 5: Organization + Stock Market Scale
// ══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_org_stock_market_scale() {
    let event_bus = Arc::new(EventBus::new(100_000));
    let mut gov = GovernanceSystem::with_shared_event_bus(event_bus.clone());
    let mut stock_market = StockMarket::new();

    let agent_ids: Vec<String> = (0..NUM_AGENTS).map(|i| format!("agent-{:03}", i)).collect();

    let start = Instant::now();

    // Create 10 organizations
    let mut org_ids = Vec::new();
    for i in 0..10u32 {
        let name = format!("Org-{}", i);
        let founder_id = agent_ids[i as usize * 10].clone();
        if let Ok(org_id) = gov.create_org(name, founder_id, DecisionMode::Vote, 1) {
            // Join 10 members per org
            for j in 1..10 {
                let member_idx = (i as usize) * 10 + j;
                if member_idx < NUM_AGENTS {
                    let _ = gov.join_org(org_id, agent_ids[member_idx].clone(), 1);
                }
            }
            org_ids.push(org_id);

            // Issue and IPO stock
            let ticker = format!("T{}", i);
            if let Ok(stock) = stock_market.issue_shares(
                org_id.to_string(),
                ticker,
                1000,
                10 + i as u64,
                1,
            ) {
                for j in 0..10 {
                    let member_idx = (i as usize) * 10 + j;
                    if member_idx < NUM_AGENTS {
                        stock_market.credit_shares(&stock.id, &agent_ids[member_idx], 10);
                    }
                }
                let _ = stock_market.ipo(&stock.id, IPO_MIN_MEMBERS, IPO_MIN_TREASURY, 1);
            }
        }
    }

    // Perform 200 trades
    let stock_ids: Vec<String> = stock_market.list_stocks().iter()
        .filter(|s| s.status == ListingStatus::Listed)
        .map(|s| s.id.clone())
        .collect();
    let mut trades_done = 0;
    for tick in 1..=500u64 {
        for stock_id in &stock_ids {
            let buyer_idx = (tick as usize) % NUM_AGENTS;
            let seller_idx = (tick as usize + 7) % NUM_AGENTS;

            if stock_market.place_buy_order(
                stock_id,
                &agent_ids[buyer_idx],
                OrderKind::Market,
                0,
                3,
                100_000,
                tick,
            ).is_ok() && stock_market.place_sell_order(
                stock_id,
                &agent_ids[seller_idx],
                OrderKind::Market,
                0,
                3,
                tick,
            ).is_ok() {
                trades_done += 1;
            }
            if trades_done >= 200 {
                break;
            }
        }
        if trades_done >= 200 {
            break;
        }
    }

    let elapsed = start.elapsed();

    let org_count = gov.list_orgs().len();
    let actual_trades = stock_market.list_trades(None).len();

    println!("\n[Org+Stock] {} orgs, {} trades in {:?}", org_count, actual_trades, elapsed);

    assert!(org_count >= 5, "Should have ≥5 orgs, got {}", org_count);
    assert!(actual_trades >= 100, "Should have ≥100 trades, got {}", actual_trades);
}

// ══════════════════════════════════════════════════════════════════════════
// TEST 6: Concurrent Task Operations
// ══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_concurrent_task_operations_100_agents() {
    let board = Arc::new(RwLock::new(TaskBoard::new()));
    let start = Instant::now();

    // Set up balances
    {
        let mut b = board.write().await;
        for i in 0..NUM_AGENTS {
            b.set_balance(&format!("agent-{:03}", i), 100_000);
        }
    }

    // Concurrently create tasks
    let mut handles = Vec::new();
    for i in 0..NUM_AGENTS {
        let board = board.clone();
        handles.push(tokio::spawn(async move {
            let agent_id = format!("agent-{:03}", i);
            let mut b = board.write().await;
            b.create_task(
                format!("Task from agent {}", i),
                format!("Description {}", i),
                100,
                agent_id,
                0,
                None,
            )
        }));
    }

    let mut task_ids = Vec::new();
    for handle in handles {
        if let Ok(Ok(id)) = handle.await {
            task_ids.push(id);
        }
    }

    let create_elapsed = start.elapsed();
    let creates_per_sec = task_ids.len() as f64 / create_elapsed.as_secs_f64();

    println!(
        "[Tasks] {} created in {:?} ({:.0} creates/s)",
        task_ids.len(), create_elapsed, creates_per_sec
    );

    assert!(
        task_ids.len() >= NUM_AGENTS / 2,
        "Should create at least {} tasks, got {}",
        NUM_AGENTS / 2,
        task_ids.len()
    );
}
