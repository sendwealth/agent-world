//! Sustained-run performance baseline harness.
//!
//! Runs the production subsystem pipeline (token burn -> death judgment ->
//! rule check -> lifecycle aging -> reputation decay -> event broadcast)
//! for a fixed wall-clock duration and emits a JSON report with per-tick
//! p50/p95/p99 latency plus the top-3 phase bottlenecks.
//!
//! This complements `benches/tick_benchmark.rs` (criterion, short samples)
//! with a long-running steady-state measurement suitable for capturing the
//! sustained performance curve and memory behaviour documented in
//! `reports/performance-baseline.md`.
//!
//! Usage:
//!   cargo run --release --example perf_baseline -- --agents 10 --duration-secs 120
//!   cargo run --release --example perf_baseline -- --agents 10 --ticks 6000

use std::sync::Arc;
use std::time::{Duration, Instant};

use agent_world_engine::economy::reputation::ReputationConfig;
use agent_world_engine::economy::token_burn::{AgentRecord, TokenBurnEngine};
use agent_world_engine::lifecycle::LifecycleConfig;
use agent_world_engine::rules::default_registry;
use agent_world_engine::world::enums::AgentPhase;
use agent_world_engine::world::state::{EventBus, WorldState};
use agent_world_engine::world::subsystem::SubsystemRegistry;
use agent_world_engine::world::subsystems::{
    DeathJudgmentSubsystem, EventBroadcastSubsystem, LifecycleAgingSubsystem,
    ReputationDecaySubsystem, RuleCheckSubsystem, TokenBurnSubsystem,
};
use agent_world_engine::world::tick_profiler::{TickPhase, TickProfiler};

use uuid::Uuid;

// ═══════════════════════════════════════════════════════════════════════════
// World construction -- mirrors benches/tick_benchmark.rs::build_world_state
// ═══════════════════════════════════════════════════════════════════════════

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
                    tasks_attempted: 0,
                    tasks_completed: 0,
                },
            )
        })
        .collect()
}

fn build_world_state(agent_count: usize) -> WorldState {
    let event_bus = Arc::new(EventBus::new(8192));
    let mut registry = SubsystemRegistry::new();

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
// Arg parsing
// ═══════════════════════════════════════════════════════════════════════════

struct Args {
    agents: usize,
    duration_secs: Option<u64>,
    ticks: Option<u64>,
}

fn parse_args() -> Args {
    let mut agents: usize = 10;
    let mut duration_secs: Option<u64> = None;
    let mut ticks: Option<u64> = None;

    let mut iter = std::env::args().skip(1);
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--agents" => agents = iter.next().and_then(|v| v.parse().ok()).unwrap_or(10),
            "--duration-secs" => duration_secs = iter.next().and_then(|v| v.parse().ok()),
            "--ticks" => ticks = iter.next().and_then(|v| v.parse().ok()),
            "--help" | "-h" => {
                eprintln!(
                    "Usage: perf_baseline --agents N --duration-secs S | --ticks N\n\
                     --agents N          Number of agents (default 10)\n\
                     --duration-secs S   Run for S seconds of wall-clock time\n\
                     --ticks N           Run exactly N ticks (overrides duration)"
                );
                std::process::exit(0);
            }
            _ => {}
        }
    }

    if duration_secs.is_none() && ticks.is_none() {
        duration_secs = Some(120);
    }

    Args {
        agents,
        duration_secs,
        ticks,
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Main
// ═══════════════════════════════════════════════════════════════════════════

fn main() {
    let args = parse_args();

    eprintln!(
        "perf_baseline: agents={} duration={:?} ticks={:?}",
        args.agents, args.duration_secs, args.ticks
    );

    let mut state = build_world_state(args.agents);
    let mut profiler = TickProfiler::new();

    let deadline = args
        .duration_secs
        .map(|s| Instant::now() + Duration::from_secs(s));
    let tick_cap = args.ticks.unwrap_or(u64::MAX);

    let run_start = Instant::now();
    let mut tick_no: u64 = 0;

    loop {
        if tick_no >= tick_cap {
            break;
        }
        if let Some(dl) = deadline {
            if Instant::now() >= dl {
                break;
            }
        }

        tick_no += 1;
        profiler.start_tick(tick_no);
        profiler.start_phase(TickPhase::Subsystems);
        let _events = state.tick();
        profiler.end_phase();
        profiler.end_tick();
    }

    let elapsed = run_start.elapsed();
    let report = profiler.report(args.agents);

    // Throughput summary on stderr.
    let secs = elapsed.as_secs_f64().max(1e-9);
    let tps = tick_no as f64 / secs;
    let mean_us = if report.total_tick.count > 0 {
        report.total_tick.sum_us as f64 / report.total_tick.count as f64
    } else {
        0.0
    };
    eprintln!(
        "perf_baseline: ticks={} elapsed={:.3}s ticks/s={:.1} mean_tick={:.1}us",
        tick_no, secs, tps, mean_us
    );

    // Emit JSON report on stdout.
    let mut json = profiler.report_json(args.agents);

    // Augment with run-level metadata so the report is self-describing.
    let run_meta = format!(
        ",\n  \"run\": {{\n    \"ticks\": {t},\n    \"wall_seconds\": {w:.3},\n    \"ticks_per_second\": {tps:.2},\n    \"mean_tick_us\": {m:.1}\n  }}",
        t = tick_no,
        w = secs,
        tps = tps,
        m = mean_us
    );
    if let Some(pos) = json.rfind('}') {
        json.insert_str(pos, run_meta.as_str());
    }

    println!("{}", json);
}
