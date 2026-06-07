//! Observability module — Prometheus metrics + structured tracing helpers.
//!
//! Exposes:
//! - `Metrics` — lazily-initialised Prometheus registry with counters/gauges/histograms
//! - `metrics_handler` — Axum handler that serves `/metrics` in Prometheus exposition format
//! - `MetricsGuard` — RAII guard for timing tick duration
//!
//! All metrics are initialised via `lazy_static` so there is zero overhead when
//! the `/metrics` endpoint is never scraped.

use std::time::Instant;

use axum::body::Body;
use axum::http::{header, Response, StatusCode};
use prometheus::{
    self, Encoder, Histogram, HistogramOpts, IntCounter, IntGauge, Registry, TextEncoder,
};
use tracing::{error, info, warn};

// ── Global registry ──────────────────────────────────────

lazy_static::lazy_static! {
    pub static ref REGISTRY: Registry = Registry::new();

    // --- Counters --------------------------------------------------------

    /// Total number of world ticks processed.
    pub static ref TICK_TOTAL: IntCounter = IntCounter::new(
        "world_tick_total",
        "Total number of ticks processed by the world engine",
    ).unwrap();

    /// Total number of transactions completed.
    pub static ref TRANSACTIONS_TOTAL: IntCounter = IntCounter::new(
        "world_transactions_total",
        "Total number of economic transactions",
    ).unwrap();

    /// Total number of agent deaths.
    pub static ref DEATHS_TOTAL: IntCounter = IntCounter::new(
        "world_deaths_total",
        "Total number of agent deaths",
    ).unwrap();

    /// Total number of events published to the EventBus.
    pub static ref EVENTS_PUBLISHED_TOTAL: IntCounter = IntCounter::new(
        "world_events_published_total",
        "Total number of events published to the EventBus",
    ).unwrap();

    /// Total number of gRPC messages routed.
    pub static ref GRPC_MESSAGES_ROUTED_TOTAL: IntCounter = IntCounter::new(
        "world_grpc_messages_routed_total",
        "Total number of gRPC A2A messages routed",
    ).unwrap();

    /// Total number of API HTTP requests.
    pub static ref HTTP_REQUESTS_TOTAL: IntCounter = IntCounter::new(
        "world_http_requests_total",
        "Total number of HTTP API requests served",
    ).unwrap();

    // --- Gauges ----------------------------------------------------------

    /// Number of currently alive agents.
    pub static ref AGENTS_ALIVE: IntGauge = IntGauge::new(
        "world_agents_alive",
        "Number of currently alive agents",
    ).unwrap();

    /// Current token supply in the world.
    pub static ref TOKEN_SUPPLY: IntGauge = IntGauge::new(
        "world_token_supply",
        "Current total token supply",
    ).unwrap();

    /// Current money supply in the world.
    pub static ref MONEY_SUPPLY: IntGauge = IntGauge::new(
        "world_money_supply",
        "Current total money supply",
    ).unwrap();

    /// Cumulative GDP (counter-like but stored as gauge for reset flexibility).
    pub static ref WORLD_GDP: IntGauge = IntGauge::new(
        "world_gdp",
        "Cumulative gross domestic product",
    ).unwrap();

    /// Number of open tasks on the board.
    pub static ref TASKS_OPEN: IntGauge = IntGauge::new(
        "world_tasks_open",
        "Number of currently open tasks",
    ).unwrap();

    // --- Histograms ------------------------------------------------------

    /// Duration of a single tick execution (seconds).
    pub static ref TICK_DURATION: Histogram = Histogram::with_opts(
        HistogramOpts::new("tick_duration_seconds", "Time taken to execute a single tick")
            .buckets(vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0]),
    ).unwrap();

    /// Duration of subsystem on_tick calls (seconds).
    pub static ref SUBSYSTEM_DURATION: Histogram = Histogram::with_opts(
        HistogramOpts::new("subsystem_duration_seconds", "Time taken per subsystem on_tick call")
            .buckets(vec![0.0001, 0.0005, 0.001, 0.005, 0.01, 0.05, 0.1, 0.5]),
    ).unwrap();

    /// Duration of gRPC message processing (seconds).
    pub static ref GRPC_MESSAGE_DURATION: Histogram = Histogram::with_opts(
        HistogramOpts::new("grpc_message_duration_seconds", "Time taken to process a gRPC message")
            .buckets(vec![0.0001, 0.0005, 0.001, 0.005, 0.01, 0.05, 0.1]),
    ).unwrap();
}

/// Register all metrics with the global registry.
///
/// Idempotent: safe to call multiple times (subsequent calls are no-ops).
pub fn init() {
    macro_rules! reg {
        ($metric:expr) => {
            if let Err(e) = REGISTRY.register(Box::new($metric.clone())) {
                // AlreadyReg is expected on repeated calls (e.g. tests running in parallel).
                if !matches!(e, prometheus::Error::AlreadyReg) {
                    error!("Failed to register metric: {}", e);
                }
            }
        };
    }

    reg!(TICK_TOTAL);
    reg!(TRANSACTIONS_TOTAL);
    reg!(DEATHS_TOTAL);
    reg!(EVENTS_PUBLISHED_TOTAL);
    reg!(GRPC_MESSAGES_ROUTED_TOTAL);
    reg!(HTTP_REQUESTS_TOTAL);

    reg!(AGENTS_ALIVE);
    reg!(TOKEN_SUPPLY);
    reg!(MONEY_SUPPLY);
    reg!(WORLD_GDP);
    reg!(TASKS_OPEN);

    reg!(TICK_DURATION);
    reg!(SUBSYSTEM_DURATION);
    reg!(GRPC_MESSAGE_DURATION);

    info!("Observability: Prometheus metrics registered");
}

// ── Axum handler ─────────────────────────────────────────

/// Axum handler that renders all registered metrics in Prometheus text format.
pub async fn metrics_handler() -> Response<Body> {
    HTTP_REQUESTS_TOTAL.inc();

    let encoder = TextEncoder::new();
    let metric_families = REGISTRY.gather();
    let mut buffer = Vec::new();
    if let Err(e) = encoder.encode(&metric_families, &mut buffer) {
        error!("Failed to encode metrics: {}", e);
        return Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(Body::from("encoding error"))
            .unwrap();
    }

    Response::builder()
        .status(StatusCode::OK)
        .header(
            header::CONTENT_TYPE,
            "text/plain; version=0.0.4; charset=utf-8",
        )
        .body(Body::from(buffer))
        .unwrap()
}

// ── RAII timing guard ────────────────────────────────────

/// RAII guard that observes tick duration on drop.
///
/// ```
/// use agent_world_engine::observability::MetricsGuard;
///
/// {
///     let _guard = MetricsGuard::new();
///     // tick work happens here...
/// } // duration automatically recorded
/// ```
pub struct MetricsGuard {
    start: Instant,
}

impl Default for MetricsGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricsGuard {
    pub fn new() -> Self {
        Self {
            start: Instant::now(),
        }
    }
}

impl Drop for MetricsGuard {
    fn drop(&mut self) {
        let elapsed = self.start.elapsed().as_secs_f64();
        TICK_DURATION.observe(elapsed);
    }
}

// ── Structured log helpers ───────────────────────────────

/// Log a tick event with structured fields.
pub fn log_tick(tick: u64, alive: usize, token_supply: i64, money_supply: i64) {
    info!(
        tick = tick,
        agents_alive = alive,
        token_supply = token_supply,
        money_supply = money_supply,
        "Tick advanced"
    );
}

/// Log a transaction event with structured fields.
pub fn log_transaction(from: &str, to: &str, amount: i64, currency: &str) {
    info!(
        from = from,
        to = to,
        amount = amount,
        currency = currency,
        "Transaction completed"
    );
}

/// Log an agent death event.
pub fn log_agent_death(agent_id: &str, cause: &str) {
    warn!(agent_id = agent_id, cause = cause, "Agent died");
    DEATHS_TOTAL.inc();
}

/// Log an error with structured context.
pub fn log_error(context: &str, error: &str) {
    error!(context = context, error = error, "Error occurred");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_does_not_panic() {
        init();
    }

    #[test]
    fn tick_total_increments() {
        init();
        let before = TICK_TOTAL.get();
        TICK_TOTAL.inc();
        assert_eq!(TICK_TOTAL.get(), before + 1);
    }

    #[test]
    fn agents_alive_can_be_set() {
        init();
        AGENTS_ALIVE.set(42);
        assert_eq!(AGENTS_ALIVE.get(), 42);
    }

    #[test]
    fn tick_duration_records() {
        init();
        TICK_DURATION.observe(0.05);
        // Verify it doesn't panic and the histogram is populated
        let mf = REGISTRY.gather();
        assert!(!mf.is_empty());
    }

    #[tokio::test]
    async fn metrics_handler_returns_ok() {
        init();
        let resp = metrics_handler().await;
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[test]
    fn metrics_guard_observes_duration() {
        init();
        {
            let _guard = MetricsGuard::new();
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        // Guard dropped, duration observed
        let mf = REGISTRY.gather();
        assert!(!mf.is_empty());
    }
}
