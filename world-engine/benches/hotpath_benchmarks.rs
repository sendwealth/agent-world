//! Hot-path benchmarks for the World Engine.
//!
//! Measures performance of critical paths:
//!   1. Task creation (single-threaded + concurrent)
//!   2. Task lifecycle (full publish→complete flow)
//!   3. Read-heavy workload (get/list with and without cache)
//!   4. EventBus throughput
//!   5. Token burn engine (batch of 100 agents)

use std::sync::Arc;
use std::time::Duration;

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

use agent_world_engine::economy::task::{TaskBoard, TaskStatus};
use agent_world_engine::economy::token_burn::{AgentRecord, ConsumptionConfig, TokenBurnEngine};
use agent_world_engine::world::enums::AgentPhase;
use agent_world_engine::world::event::WorldEvent;
use agent_world_engine::world::state::EventBus;

// ── Task Creation (single-threaded) ────────────────────────

fn bench_task_create(c: &mut Criterion) {
    let mut group = c.benchmark_group("task_create");
    group.sample_size(50);

    for count in [10, 100, 500] {
        group.bench_with_input(BenchmarkId::from_parameter(count), &count, |b, &count| {
            b.iter(|| {
                let mut board = TaskBoard::new();
                board.set_balance("publisher", 1_000_000);
                for i in 0..count {
                    board
                        .create_task(
                            format!("Task {}", i),
                            format!("Description {}", i),
                            100,
                            "publisher".to_string(),
                            i as u64,
                            None,
                        )
                        .unwrap();
                }
                black_box(&board);
            });
        });
    }
    group.finish();
}

// ── Full Task Lifecycle ────────────────────────────────────

fn bench_task_lifecycle(c: &mut Criterion) {
    let mut group = c.benchmark_group("task_lifecycle");
    group.sample_size(50);

    group.bench_function("full_publish_to_complete", |b| {
        b.iter(|| {
            let mut board = TaskBoard::new();
            board.set_balance("publisher", 1_000_000);

            let id = board
                .create_task(
                    "Bench Task".to_string(),
                    "Benchmark description".to_string(),
                    100,
                    "publisher".to_string(),
                    0,
                    None,
                )
                .unwrap();

            board.claim_task(id, "worker".to_string()).unwrap();
            board.start_task(id).unwrap();
            board.submit_result(id, "Done".to_string()).unwrap();
            board.review_task(id, "publisher", true).unwrap();
            board.complete_task(id, 0).unwrap();

            assert_eq!(board.get(id).unwrap().status, TaskStatus::Completed);
        });
    });
    group.finish();
}

// ── Task Query ─────────────────────────────────────────────

fn bench_task_query(c: &mut Criterion) {
    let mut group = c.benchmark_group("task_query");
    group.sample_size(50);

    let mut board = TaskBoard::new();
    board.set_balance("publisher", 10_000_000);
    let mut ids = Vec::new();
    for i in 0..1000 {
        let id = board
            .create_task(
                format!("Task {}", i),
                format!("Description {}", i),
                100,
                "publisher".to_string(),
                i as u64,
                None,
            )
            .unwrap();
        ids.push(id);
    }

    // Benchmark get by id (O(1) HashMap lookup)
    group.bench_function("get_by_id_1k_tasks", |b| {
        b.iter(|| {
            let id = ids[black_box(42)];
            black_box(board.get(id));
        });
    });

    // Benchmark list all (O(n) iteration)
    group.bench_function("list_all_1k_tasks", |b| {
        b.iter(|| {
            black_box(board.list());
        });
    });

    // Benchmark list by status
    group.bench_function("list_by_status_1k_tasks", |b| {
        b.iter(|| {
            black_box(board.list_by_status(TaskStatus::Published));
        });
    });

    group.finish();
}

// ── EventBus Throughput ────────────────────────────────────

fn bench_event_bus_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("event_bus");
    group.sample_size(20);
    group.measurement_time(Duration::from_secs(3));

    for count in [100, 1000, 10000] {
        group.throughput(Throughput::Elements(count as u64));
        group.bench_with_input(BenchmarkId::from_parameter(count), &count, |b, &count| {
            b.iter(|| {
                let bus = EventBus::new(count + 256);
                for i in 0..count {
                    bus.emit(WorldEvent::TickAdvanced { tick: i as u64 });
                }
                black_box(&bus);
            });
        });
    }
    group.finish();
}

// ── Token Burn Engine (100 agents) ─────────────────────────

fn bench_token_burn_100_agents(c: &mut Criterion) {
    let mut group = c.benchmark_group("token_burn");
    group.sample_size(30);

    let engine = TokenBurnEngine::new(ConsumptionConfig::default());

    let agents: Vec<AgentRecord> = (0..100)
        .map(|i| AgentRecord {
            id: uuid::Uuid::new_v4(),
            name: format!("agent-{}", i),
            phase: AgentPhase::Adult,
            tokens: 500_000,
            skills: Default::default(),
            personality: String::new(),
            tasks_completed: 0,
            tasks_attempted: 0,
        })
        .collect();

    group.bench_function("100_agents_single_tick", |b| {
        b.iter(|| {
            let mut agents = agents.clone();
            let result = engine.process_tick(1, &mut agents);
            black_box(result);
        });
    });

    group.bench_function("100_agents_100_ticks", |b| {
        b.iter(|| {
            let mut agents = agents.clone();
            for tick in 1..=100 {
                let result = engine.process_tick(tick, &mut agents);
                black_box(result);
            }
        });
    });

    group.finish();
}

// ── Concurrent Task Creation (RwLock vs Mutex comparison) ─

fn bench_concurrent_task_create(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_task_create");
    group.sample_size(20);

    for num_tasks in [100, 500] {
        // RwLock-based (current implementation)
        group.bench_with_input(
            BenchmarkId::new("rwlock", num_tasks),
            &num_tasks,
            |b, &num_tasks| {
                b.iter(|| {
                    let rt = tokio::runtime::Runtime::new().unwrap();
                    rt.block_on(async {
                        let board = Arc::new(tokio::sync::RwLock::new(TaskBoard::new()));
                        {
                            let mut b = board.write().await;
                            b.set_balance("publisher", 10_000_000);
                        }

                        let mut handles = Vec::new();
                        for i in 0..num_tasks {
                            let board = board.clone();
                            handles.push(tokio::spawn(async move {
                                let mut b = board.write().await;
                                b.create_task(
                                    format!("Concurrent Task {}", i),
                                    format!("Desc {}", i),
                                    100,
                                    "publisher".to_string(),
                                    i as u64,
                                    None,
                                )
                                .unwrap();
                            }));
                        }
                        for h in handles {
                            h.await.unwrap();
                        }
                    })
                });
            },
        );
    }
    group.finish();
}

// ── Concurrent Read Heavy (80% reads, 20% writes) ──────────

fn bench_concurrent_read_heavy(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_read_heavy");
    group.sample_size(20);

    group.bench_function("80pct_reads_100_tasks", |b| {
        b.iter(|| {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let board = Arc::new(tokio::sync::RwLock::new(TaskBoard::new()));
                {
                    let mut b = board.write().await;
                    b.set_balance("publisher", 10_000_000);
                    // Pre-populate 100 tasks
                    for i in 0..100 {
                        b.create_task(
                            format!("Task {}", i),
                            format!("Desc {}", i),
                            100,
                            "publisher".to_string(),
                            i as u64,
                            None,
                        ).unwrap();
                    }
                }

                // 8 reads for every 2 writes
                let mut handles = Vec::new();
                for i in 0..200 {
                    let board = board.clone();
                    handles.push(tokio::spawn(async move {
                        if i % 5 == 0 {
                            // Write: claim a task
                            let mut b = board.write().await;
                            let task_id = b.list().first().map(|t| t.id);
                            if let Some(tid) = task_id {
                                let _ = b.claim_task(tid, format!("worker-{}", i));
                            }
                        } else {
                            // Read: list tasks
                            let b = board.read().await;
                            black_box(b.list());
                        }
                    }));
                }
                for h in handles {
                    h.await.unwrap();
                }
            })
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_task_create,
    bench_task_lifecycle,
    bench_task_query,
    bench_event_bus_throughput,
    bench_token_burn_100_agents,
    bench_concurrent_task_create,
    bench_concurrent_read_heavy,
);
criterion_main!(benches);
