use std::sync::Arc;
use tokio::sync::Mutex;

use agent_world_engine::economy::task::TaskBoard;
use agent_world_engine::wal::WAL;
use agent_world_engine::world::discovery::AgentRegistry;

#[tokio::main]
async fn main() {
    let event_bus = agent_world_engine::world::EventBus::new(256);

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
    let mut wal_rx = event_bus.subscribe();

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

    // Initialize task board with event bus (takes ownership)
    let task_board = Arc::new(Mutex::new(TaskBoard::with_event_bus(event_bus)));

    // Initialize agent registry with its own event bus for discovery events
    let discovery_bus = agent_world_engine::world::EventBus::new(256);
    let mut discovery_wal_rx = discovery_bus.subscribe();
    let discovery_wal_writer = wal_writer.clone();
    let discovery_wal_handle = tokio::spawn(async move {
        loop {
            match discovery_wal_rx.recv().await {
                Ok(event) => {
                    let mut wal = discovery_wal_writer.lock().await;
                    if let Err(e) = wal.append_event(&event) {
                        eprintln!("[WAL] Failed to write discovery event: {}", e);
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    eprintln!("[WAL] Discovery lagged {} events", n);
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    break;
                }
            }
        }
    });

    let registry = Arc::new(Mutex::new(AgentRegistry::with_event_bus(discovery_bus)));
    println!("   AgentRegistry: initialized");

    // Build the HTTP API router with WAL support and agent discovery
    let app = agent_world_engine::api::create_router_with_wal(task_board, wal_writer.clone(), registry);

    // Start the HTTP server
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], 3000));
    println!("   API server: http://{}", addr);
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
    discovery_wal_handle.abort();
    println!("[Server] Shutdown complete.");
}
