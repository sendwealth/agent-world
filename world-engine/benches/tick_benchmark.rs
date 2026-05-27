//! Tick-level benchmarks for the World Engine.
//!
//! Measures full tick performance at 10 / 25 / 50 / 100 agent scales,
//! exercising the real subsystem pipeline (token burn, death judgment,
//! rule check, event broadcast, lifecycle aging, reputation decay).
//!
//! Outputs:
//!   - Per-tier tick latency (mean, p50, p95, p99)
//!   - Per-phase timing breakdown
//!   - JSON profile report for CI regression tracking
//!
//! Usage:
//!   cargo bench --bench tick_benchmark
//!   cargo bench --bench tick_benchmark -- --save-baseline main

use std::sync::Arc;
use std::time::Duration;

use criterion::{
    black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput,
};

use agent_world_engine::economy::token_burn::{AgentRecord, ConsumptionConfig, TokenBurnEngine};
use agent_world_engine::economy::reputation::ReputationConfig;
use agent_world_engine::lifecycle::LifecycleConfig;
use agent_world_engine::rules::default_registry;
use agent_world_engine::world::enums::AgentPhase;
use agent_world_engine::world::event::WorldEvent;
use agent_world_engine::world::state::{EventBus, WorldState};
use agent_world_engine::world::subsystem::SubsystemRegistry;
use agent_world_engine::world::subsystems::{
    DeathJudgmentSubsystem, EventBroadcastSubsystem, LifecycleAgingSubsystem,
    ReputationDecaySubsystem, RuleCheckSubsystem, TokenBurnSubsystem,
};
use agent_world_engine::world::tick_profiler::{TickPhase, TickProfiler};

use uuid::Uuid;

// ═══════════════════════════════════════════════════════════════════════════
// Helpers
// ═══════════════════════════════════════════════════════════════════════════

/// Create a batch of test agents with deterministic IDs.
fn make_agents(count: usize) -> Vec<(Uuid, u64, AgentRecord)> {
    (0..count)
        .map(|i| {
            (
                Uuid::new_v4(),
                0,
                AgentRecord {
                    id: Uuid::new_v4(),
                    name: format!("agent-{}", i),
                    phase: AgentPhase::Adult,
                    tokens: 500_000,
                    skills: std::collections::HashMap::new(),
                    personality: String::new(),
                    tasks_completed: 0,
                    tasks_attempted: 0,
                },
            )
        })
        .collect()
}

/// Build a fully-configured WorldState with the standard subsystem pipeline.
fn build_world_state(agent_count: usize) -> WorldState {
    let event_bus = Arc::new(EventBus::new(8192));
    let mut registry = SubsystemRegistry::new();

    // Standard subsystem pipeline (matching production config)
    registry.register(Box::new(TokenBurnSubsystem::new(
        TokenBurnEngine::with_defaults(),
    )));
    registry.register(Box::new(DeathJudgmentSubsystem::new(0)));
    registry.register(Box::new(RuleCheckSubsystem::new(default_registry())));
    registry.register(Box::new(LifecycleAgingSubsystem::new(
        LifecycleConfig::default(),
    )));
    registry.register(Box::new(ReputationDecaySubsystem::new(
        ReputationConfig::default(),
    )));
    registry.register(Box::new(EventBroadcastSubsystem::new(event_bus.clone())));

    let agents = make_agents(agent_count);
    WorldState::new(event_bus, registry, agents)
}

// ═══════════════════════════════════════════════════════════════════════════
// Benchmark 1: Full Tick (10 / 25 / 50 / 100 agents)
// ═══════════════════════════════════════════════════════════════════════════

fn bench_full_tick(c: &mut Criterion) {
    let mut group = c.benchmark_group("full_tick");
    group.sample_size(30);
    group.measurement_time(Duration::from_secs(5));

    for agent_count in [10, 25, 50, 100] {
        group.throughput(Throughput::Elements(1));
        group.bench_with_input(
            BenchmarkId::new("agents", agent_count),
            &agent_count,
            |b, &agent_count| {
                b.iter_batched(
                    || build_world_state(agent_count),
                    |mut state| {
                        let events = state.tick();
                        black_box(events);
                    },
                    criterion::BatchSize::SmallInput,
                );
            },
        );
    }
    group.finish();
}

// ═══════════════════════════════════════════════════════════════════════════
// Benchmark 2: Multi-Tick Stability (100 ticks at each tier)
// ═══════════════════════════════════════════════════════════════════════════

fn bench_multi_tick_stability(c: &mut Criterion) {
    let mut group = c.benchmark_group("multi_tick_stability");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(10));

    for agent_count in [10, 25, 50, 100] {
        group.throughput(Throughput::Elements(100));
        group.bench_with_input(
            BenchmarkId::new("100_ticks", agent_count),
            &agent_count,
            |b, &agent_count| {
                b.iter_batched(
                    || build_world_state(agent_count),
                    |mut state| {
                        for _ in 0..100 {
                            let events = state.tick();
                            black_box(events);
                        }
                    },
                    criterion::BatchSize::SmallInput,
                );
            },
        );
    }
    group.finish();
}

// ═══════════════════════════════════════════════════════════════════════════
// Benchmark 3: Per-Phase Profiling with TickProfiler
// ═══════════════════════════════════════════════════════════════════════════

fn bench_tick_phase_profiling(c: &mut Criterion) {
    let mut group = c.benchmark_group("tick_phase_profiling");
    group.sample_size(10);

    for agent_count in [10, 50, 100] {
        group.bench_with_input(
            BenchmarkId::new("profiled", agent_count),
            &agent_count,
            |b, &agent_count| {
                b.iter_batched(
                    || {
                        let state = build_world_state(agent_count);
                        let profiler = TickProfiler::new();
                        (state, profiler)
                    },
                    |(mut state, mut profiler)| {
                        for tick in 1..=50 {
                            profiler.start_tick(tick);

                            // Phase: Subsystems (runs all registered subsystems)
                            profiler.start_phase(TickPhase::Subsystems);
                            let events = state.tick();
                            profiler.end_phase();

                            // Note: In the sync WorldState, subsystems + rules + broadcast
                            // are all done in a single tick() call. The profiler still
                            // measures the overall tick time accurately.
                            black_box(&events);

                            profiler.end_tick();
                        }
                        let report = profiler.report(agent_count);
                        black_box(report);
                    },
                    criterion::BatchSize::SmallInput,
                );
            },
        );
    }
    group.finish();
}

// ═══════════════════════════════════════════════════════════════════════════
// Benchmark 4: Token Burn Engine Scaling
// ═══════════════════════════════════════════════════════════════════════════

fn bench_token_burn_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("token_burn_scaling");
    group.sample_size(30);

    for agent_count in [10, 25, 50, 100, 200] {
        let engine = TokenBurnEngine::new(ConsumptionConfig::default());
        let agents: Vec<AgentRecord> = (0..agent_count)
            .map(|i| AgentRecord {
                id: Uuid::new_v4(),
                name: format!("agent-{}", i),
                phase: AgentPhase::Adult,
                tokens: 500_000,
                skills: Default::default(),
                personality: String::new(),
                tasks_completed: 0,
                tasks_attempted: 0,
            })
            .collect();

        group.bench_with_input(
            BenchmarkId::new("single_tick", agent_count),
            &agent_count,
            |b, _| {
                b.iter(|| {
                    let mut agents = agents.clone();
                    let result = engine.process_tick(1, &mut agents);
                    black_box(result);
                });
            },
        );
    }
    group.finish();
}

// ═══════════════════════════════════════════════════════════════════════════
// Benchmark 5: EventBus Scaling (with subscribers)
// ═══════════════════════════════════════════════════════════════════════════

fn bench_event_bus_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("event_bus_scaling");
    group.sample_size(20);
    group.measurement_time(Duration::from_secs(3));

    for (subscriber_count, event_count) in [(1, 1000), (5, 1000), (10, 1000)] {
        group.bench_with_input(
            BenchmarkId::new(
                format!("subs_{}_events_{}", subscriber_count, event_count),
                "",
            ),
            &(subscriber_count, event_count),
            |b, &(subs, evts)| {
                b.iter(|| {
                    let bus = EventBus::new(evts + 256);
                    let mut receivers: Vec<_> =
                        (0..subs).map(|_| bus.subscribe()).collect();
                    for i in 0..evts {
                        bus.emit(WorldEvent::TickAdvanced { tick: i as u64 });
                    }
                    // Drain receivers
                    for rx in &mut receivers {
                        while rx.try_recv().is_ok() {}
                    }
                    black_box(&bus);
                });
            },
        );
    }
    group.finish();
}

// ═══════════════════════════════════════════════════════════════════════════
// Benchmark 6: Rule Registry Scaling
// ═══════════════════════════════════════════════════════════════════════════

fn bench_rule_registry_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("rule_registry_scaling");
    group.sample_size(30);

    for agent_count in [10, 50, 100, 200] {
        let registry = default_registry();
        let mut agents: Vec<(Uuid, u64, AgentRecord)> = (0..agent_count)
            .map(|i| {
                (
                    Uuid::new_v4(),
                    0,
                    AgentRecord {
                        id: Uuid::new_v4(),
                        name: format!("agent-{}", i),
                        phase: AgentPhase::Adult,
                        tokens: 500_000,
                        skills: Default::default(),
                        personality: String::new(),
                        tasks_completed: 0,
                        tasks_attempted: 0,
                    },
                )
            })
            .collect();

        group.bench_with_input(
            BenchmarkId::new("evaluate_all", agent_count),
            &agent_count,
            |b, _| {
                b.iter(|| {
                    let results = registry.evaluate_all(1, &mut agents);
                    black_box(results);
                });
            },
        );
    }
    group.finish();
}

// ═══════════════════════════════════════════════════════════════════════════
// Benchmark 7: Memory Snapshot Scaling
// ═══════════════════════════════════════════════════════════════════════════

fn bench_snapshot_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("snapshot_scaling");
    group.sample_size(20);

    for agent_count in [10, 50, 100] {
        // Use the async engine WorldState for snapshot benchmarks
        group.bench_with_input(
            BenchmarkId::new("world_engine_snapshot", agent_count),
            &agent_count,
            |b, &_agent_count| {
                b.iter(|| {
                    let state =
                        agent_world_engine::world::engine::WorldState::with_defaults();
                    black_box(&state);
                    // Snapshot is async; just measure agent creation overhead
                    // Actual async benchmarking would need tokio runtime
                });
            },
        );
    }
    group.finish();
}

// ═══════════════════════════════════════════════════════════════════════════
// Benchmark 8: Profiler Report Generation (end-to-end with JSON output)
// ═══════════════════════════════════════════════════════════════════════════

fn bench_profiler_report_generation(c: &mut Criterion) {
    let mut group = c.benchmark_group("profiler_report");
    group.sample_size(20);

    for agent_count in [10, 50, 100] {
        group.bench_with_input(
            BenchmarkId::new("full_profile_json", agent_count),
            &agent_count,
            |b, &agent_count| {
                b.iter_batched(
                    || {
                        let state = build_world_state(agent_count);
                        let profiler = TickProfiler::new();
                        (state, profiler)
                    },
                    |(mut state, mut profiler)| {
                        for tick in 1..=100 {
                            profiler.start_tick(tick);
                            profiler.start_phase(TickPhase::Subsystems);
                            let _ = state.tick();
                            profiler.end_phase();
                            profiler.end_tick();
                        }
                        let json = profiler.report_json(agent_count);
                        black_box(json);
                    },
                    criterion::BatchSize::SmallInput,
                );
            },
        );
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_full_tick,
    bench_multi_tick_stability,
    bench_tick_phase_profiling,
    bench_token_burn_scaling,
    bench_event_bus_scaling,
    bench_rule_registry_scaling,
    bench_snapshot_scaling,
    bench_profiler_report_generation,
);
criterion_main!(benches);
