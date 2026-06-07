//! Tick profiler — measures per-phase timing within each tick.
//!
//! Provides `TickProfiler` which records wall-clock time for each stage of
//! the tick pipeline:
//!   1. Subsystem execution
//!   2. Rule evaluation
//!   3. Event broadcast
//!   4. Task expiry
//!   5. TickAdvanced emit
//!
//! Results can be logged, collected as JSON, or accumulated for aggregate
//! statistics (p50 / p95 / p99).

use std::cmp::Ordering;
use std::time::Instant;

/// Named tick phase identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize)]
pub enum TickPhase {
    Subsystems,
    Rules,
    EventBroadcast,
    TaskExpiry,
    TickAdvanced,
}

impl TickPhase {
    /// All phases in execution order.
    pub fn all() -> &'static [TickPhase] {
        &[
            TickPhase::Subsystems,
            TickPhase::Rules,
            TickPhase::EventBroadcast,
            TickPhase::TaskExpiry,
            TickPhase::TickAdvanced,
        ]
    }

    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            TickPhase::Subsystems => "subsystems",
            TickPhase::Rules => "rules",
            TickPhase::EventBroadcast => "event_broadcast",
            TickPhase::TaskExpiry => "task_expiry",
            TickPhase::TickAdvanced => "tick_advanced",
        }
    }
}

/// Timing for a single tick, broken down by phase.
#[derive(Debug, Clone, serde::Serialize)]
pub struct TickTiming {
    /// Tick number.
    pub tick: u64,
    /// Per-phase durations in microseconds.
    pub phases: std::collections::HashMap<String, u64>,
    /// Total tick duration in microseconds.
    pub total_us: u64,
}

/// Maximum number of samples retained per phase before truncation.
const MAX_SAMPLES: usize = 10_000;

/// Accumulated statistics for a named metric.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct PhaseStats {
    pub label: String,
    pub count: u64,
    pub min_us: u64,
    pub max_us: u64,
    pub sum_us: u64,
    /// Samples for percentile calculation (capped at MAX_SAMPLES).
    pub samples_us: Vec<u64>,
}

/// Compute a percentile from a pre-sorted sample set using nearest-rank method.
fn percentile_from_sorted(sorted: &[u64], pct: f64) -> u64 {
    if sorted.is_empty() {
        return 0;
    }
    let rank = ((pct / 100.0) * sorted.len() as f64).ceil() as usize;
    sorted[rank.saturating_sub(1).min(sorted.len() - 1)]
}

impl PhaseStats {
    fn new(label: &str) -> Self {
        Self {
            label: label.to_string(),
            min_us: u64::MAX,
            ..Default::default()
        }
    }

    fn record(&mut self, us: u64) {
        self.count += 1;
        self.sum_us += us;
        self.min_us = self.min_us.min(us);
        self.max_us = self.max_us.max(us);
        // Cap sample count — drop oldest when limit reached
        if self.samples_us.len() >= MAX_SAMPLES {
            self.samples_us.remove(0);
        }
        self.samples_us.push(us);
    }

    /// p50 in microseconds.
    pub fn p50(&self) -> u64 {
        self.percentile(50.0)
    }

    /// p95 in microseconds.
    pub fn p95(&self) -> u64 {
        self.percentile(95.0)
    }

    /// p99 in microseconds.
    pub fn p99(&self) -> u64 {
        self.percentile(99.0)
    }

    /// Mean in microseconds.
    pub fn mean(&self) -> u64 {
        if self.count == 0 {
            return 0;
        }
        self.sum_us / self.count
    }

    /// Compute a single percentile — clones and sorts samples.
    /// For computing p50/p95/p99 together, prefer `percentiles()` which
    /// sorts only once.
    pub fn percentile(&self, pct: f64) -> u64 {
        let mut sorted = self.samples_us.clone();
        sorted.sort_unstable();
        percentile_from_sorted(&sorted, pct)
    }

    /// Compute p50, p95, p99 in a single pass — clones and sorts only once.
    pub fn percentiles(&self) -> (u64, u64, u64) {
        let mut sorted = self.samples_us.clone();
        sorted.sort_unstable();
        (
            percentile_from_sorted(&sorted, 50.0),
            percentile_from_sorted(&sorted, 95.0),
            percentile_from_sorted(&sorted, 99.0),
        )
    }
}

/// Accumulated profiler results across many ticks.
#[derive(Debug, Clone, serde::Serialize)]
pub struct TickProfileReport {
    /// Total number of ticks profiled.
    pub total_ticks: u64,
    /// Number of agents during profiling.
    pub agent_count: usize,
    /// Per-phase statistics.
    pub phases: Vec<PhaseStats>,
    /// Overall tick statistics.
    pub total_tick: PhaseStats,
    /// Top-3 bottlenecks (phase label + percentage of total).
    pub top3_bottlenecks: Vec<(String, f64)>,
}

/// Profiler that records per-phase timing during tick execution.
///
/// # Usage
///
/// ```
/// use agent_world_engine::world::tick_profiler::{TickProfiler, TickPhase};
///
/// let mut profiler = TickProfiler::new();
/// for tick in 0..3 {
///     profiler.start_tick(tick);
///     profiler.start_phase(TickPhase::Subsystems);
///     profiler.end_phase();
///     profiler.start_phase(TickPhase::Rules);
///     profiler.end_phase();
///     profiler.end_tick();
/// }
/// let report = profiler.report(10);
/// ```
pub struct TickProfiler {
    current_tick: u64,
    tick_start: Option<Instant>,
    phase_start: Option<Instant>,
    current_phase: Option<TickPhase>,
    phase_durations: std::collections::HashMap<TickPhase, Vec<u64>>,
    total_durations: Vec<u64>,
    timings: Vec<TickTiming>,
}

impl TickProfiler {
    /// Create a new empty profiler.
    pub fn new() -> Self {
        Self {
            current_tick: 0,
            tick_start: None,
            phase_start: None,
            current_phase: None,
            phase_durations: std::collections::HashMap::new(),
            total_durations: Vec::new(),
            timings: Vec::new(),
        }
    }

    /// Mark the start of a new tick.
    pub fn start_tick(&mut self, tick: u64) {
        self.current_tick = tick;
        self.tick_start = Some(Instant::now());
    }

    /// Mark the start of a phase within the current tick.
    pub fn start_phase(&mut self, phase: TickPhase) {
        self.current_phase = Some(phase);
        self.phase_start = Some(Instant::now());
    }

    /// Mark the end of the current phase and record its duration.
    pub fn end_phase(&mut self) {
        if let (Some(start), Some(phase)) = (self.phase_start.take(), self.current_phase.take()) {
            let us = start.elapsed().as_micros() as u64;
            self.phase_durations.entry(phase).or_default().push(us);
        }
    }

    /// Mark the end of the current tick and record total duration.
    pub fn end_tick(&mut self) {
        if let Some(start) = self.tick_start.take() {
            let total_us = start.elapsed().as_micros() as u64;
            self.total_durations.push(total_us);

            // Build timing entry
            let mut phases = std::collections::HashMap::new();
            for p in TickPhase::all() {
                if let Some(durations) = self.phase_durations.get(p) {
                    if let Some(&last) = durations.last() {
                        phases.insert(p.label().to_string(), last);
                    }
                }
            }
            self.timings.push(TickTiming {
                tick: self.current_tick,
                phases,
                total_us,
            });
        }
    }

    /// Get the raw timings for all recorded ticks.
    pub fn timings(&self) -> &[TickTiming] {
        &self.timings
    }

    /// Generate an aggregate report.
    pub fn report(&self, agent_count: usize) -> TickProfileReport {
        let mut phase_stats = Vec::new();
        let mut total_stats = PhaseStats::new("total_tick");
        for &us in &self.total_durations {
            total_stats.record(us);
        }

        for &phase in TickPhase::all() {
            let mut stats = PhaseStats::new(phase.label());
            if let Some(durations) = self.phase_durations.get(&phase) {
                for &us in durations {
                    stats.record(us);
                }
            }
            phase_stats.push(stats);
        }

        // Calculate top-3 bottlenecks by mean time as percentage of total
        let total_mean = if total_stats.count > 0 {
            total_stats.sum_us as f64 / total_stats.count as f64
        } else {
            1.0
        };

        let mut bottleneck_candidates: Vec<(String, f64)> = phase_stats
            .iter()
            .filter(|s| s.count > 0)
            .map(|s| {
                let mean = s.sum_us as f64 / s.count as f64;
                (s.label.clone(), mean / total_mean * 100.0)
            })
            .collect();
        bottleneck_candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(Ordering::Equal));
        bottleneck_candidates.truncate(3);

        TickProfileReport {
            total_ticks: self.total_durations.len() as u64,
            agent_count,
            phases: phase_stats,
            total_tick: total_stats,
            top3_bottlenecks: bottleneck_candidates,
        }
    }

    /// Generate a JSON string of the report.
    pub fn report_json(&self, agent_count: usize) -> String {
        let report = self.report(agent_count);
        serde_json::to_string_pretty(&report).unwrap_or_default()
    }
}

impl Default for TickProfiler {
    fn default() -> Self {
        Self::new()
    }
}

/// Instrumented wrapper around the sync `WorldState` tick.
///
/// This profiles the subsystem-based tick loop used by `world/state.rs`.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profiler_records_phases() {
        let mut profiler = TickProfiler::new();
        for tick in 1..=10 {
            profiler.start_tick(tick);
            profiler.start_phase(TickPhase::Subsystems);
            std::thread::sleep(std::time::Duration::from_micros(100));
            profiler.end_phase();
            profiler.start_phase(TickPhase::Rules);
            profiler.end_phase();
            profiler.end_tick();
        }

        let report = profiler.report(5);
        assert_eq!(report.total_ticks, 10);
        assert_eq!(report.agent_count, 5);
        assert!(report.top3_bottlenecks[0].0 == "subsystems");
    }

    #[test]
    fn phase_stats_percentiles() {
        let mut stats = PhaseStats::new("test");
        for v in [10, 20, 30, 40, 50, 60, 70, 80, 90, 100] {
            stats.record(v);
        }
        assert_eq!(stats.p50(), 50); // nearest-rank: ceil(0.5*10)=5, v[4]=50
        assert_eq!(stats.p95(), 100);
        assert_eq!(stats.p99(), 100);
        assert_eq!(stats.mean(), 55);
    }

    #[test]
    fn percentiles_sorts_once() {
        let mut stats = PhaseStats::new("test");
        for v in [10, 20, 30, 40, 50, 60, 70, 80, 90, 100] {
            stats.record(v);
        }
        let (p50, p95, p99) = stats.percentiles();
        assert_eq!(p50, 50);
        assert_eq!(p95, 100);
        assert_eq!(p99, 100);
    }

    #[test]
    fn max_samples_cap() {
        let mut stats = PhaseStats::new("test");
        for i in 0..15_000u64 {
            stats.record(i);
        }
        assert_eq!(stats.count, 15_000); // count tracks all records
        assert!(stats.samples_us.len() <= MAX_SAMPLES);
        // After capping, samples should contain the most recent values
        assert_eq!(*stats.samples_us.last().unwrap(), 14_999);
    }
}
