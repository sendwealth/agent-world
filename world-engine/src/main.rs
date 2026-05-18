use std::sync::Arc;
use tokio::sync::Mutex;

use agent_world_engine::economy::task::TaskBoard;
use agent_world_engine::wal::WAL;
use agent_world_engine::world::AgentRegistry;

#[tokio::main]
async fn main() {
    let event_bus = agent_world_engine::world::EventBus::new(256);
    let shared_event_bus = Arc::new(event_bus);

    println!("Agent World Engine v0.1.0");
    println!("   Status: initializing...");

    // Initialize WAL and recover from crash
    let mut wal = WAL::new("./data");
    match wal.open() {
        Ok(()) => {
            match wal.recover() {
                Ok(result) => {
                    if result.recovered_from_snapshot || result.wal_entries_replayed > 0 {
                        println!(
                            "   WAL recovery: snapshot={}, replayed={}, corrupted={}",
                            result.recovered_from_snapshot,
                            result.wal_entries_replayed,
                            result.corrupted_records
                        );
                    }
                    println!("   WAL: opened ({} events recovered)", result.event_counter);
                }
                Err(e) => {
                    eprintln!("   WAL recovery failed: {}", e);
                }
            }
        }
        Err(e) => {
            eprintln!("   WAL open failed: {}", e);
        }
    }

    println!("   EventBus: created (capacity: 256)");

    // Subscribe WAL to all events (write-ahead logging)
    let wal_writer = Arc::new(Mutex::new(wal));
    let wal_subscriber = wal_writer.clone();
    let mut wal_rx = shared_event_bus.subscribe();

    // Spawn background task to write events to WAL
    let wal_handle = tokio::spawn(async move {
        loop {
            match wal_rx.recv().await {
                Ok(event) => {
                    let mut wal = wal_subscriber.lock().await;
                    if let Err(e) = wal.append_event(&event) {
                        eprintln!("[WAL] Failed to write event: {}", e);
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    eprintln!("[WAL] Lagged {} events", n);
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    break;
                }
            }
        }
    });

    // Initialize task board with shared event bus
    let task_board = Arc::new(Mutex::new(TaskBoard::with_event_bus(
        (*shared_event_bus).clone(),
    )));

    // Initialize agent registry with shared event bus
    let agents = Arc::new(Mutex::new(AgentRegistry::with_event_bus(
        (*shared_event_bus).clone(),
    )));

    // Build the HTTP API router with full state
    let app = agent_world_engine::api::create_router_full(
        task_board,
        wal_writer.clone(),
        agents,
        shared_event_bus,
    );

    // Start the HTTP server
    let host: std::net::IpAddr = std::env::var("HOST")
        .unwrap_or_else(|_| "127.0.0.1".to_string())
        .parse()
        .unwrap_or_else(|_| "127.0.0.1".parse().unwrap());
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(3000);
    let addr = std::net::SocketAddr::from((host, port));
    println!("   API server: http://{}", addr);
    println!("   Endpoints:");
    println!("     GET  /agents           - List agents");
    println!("     POST /agents           - Create agent");
    println!("     GET  /agents/:id       - Get agent detail");
    println!("     GET  /tasks            - List tasks");
    println!("     POST /tasks            - Create task");
    println!("     GET  /world/stats      - World statistics");
    println!("     GET  /world/events     - SSE event stream");
    println!("     GET  /world/leaderboard - Leaderboard");
    println!("   Status: ready");

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();

    // Graceful shutdown on ctrl-c
    let shutdown_wal = wal_writer.clone();
    let server = axum::serve(listener, app);

    tokio::select! {
        result = server => {
            if let Err(e) = result {
                eprintln!("Server error: {}", e);
            }
        }
        _ = tokio::signal::ctrl_c() => {
            println!("\n[Server] SIGINT received, shutting down gracefully...");
        }
    }

    // Final snapshot and close WAL
    {
        let mut wal = shutdown_wal.lock().await;
        if let Err(e) = wal.take_snapshot(&[], 0) {
            eprintln!("[WAL] Final snapshot failed: {}", e);
        }
        wal.close();
    }

    wal_handle.abort();
    println!("[Server] Shutdown complete.");
}
