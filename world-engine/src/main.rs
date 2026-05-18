use std::sync::Arc;
use tokio::sync::Mutex;

use agent_world_engine::config::{ConfigManager, SharedConfig, spawn_config_watcher};
use agent_world_engine::economy::task::TaskBoard;
use agent_world_engine::wal::WAL;
use agent_world_engine::world::EventBus;

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let event_bus = EventBus::new(256);

    println!("Agent World Engine v0.1.0");
    println!("   Status: initializing...");

    // ── Load genesis config ──────────────────────────────
    let genesis_path = std::env::var("GENESIS_PATH")
        .unwrap_or_else(|_| "./config/genesis.yaml".to_string());

    // Clone the EventBus for ConfigManager before moving the original into TaskBoard.
    let event_bus_for_config = event_bus.clone();
    let config_manager: SharedConfig = match ConfigManager::new(&genesis_path, Some(Arc::new(event_bus_for_config))) {
        Ok(mgr) => {
            let tick_ms = mgr.get().await.world.tick_interval_ms;
            println!("   Config: loaded from {} (tick_interval={}ms)", genesis_path, tick_ms);
            Arc::new(mgr)
        }
        Err(e) => {
            eprintln!("   FATAL: Failed to load genesis config from {}: {}", genesis_path, e);
            std::process::exit(1);
        }
    };

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

    // Initialize task board with event bus (event_bus is moved here)
    let task_board = Arc::new(Mutex::new(TaskBoard::with_event_bus(event_bus)));

    // ── Spawn config file watcher ────────────────────────
    let (watcher_handle, watcher_cancel) = match spawn_config_watcher(config_manager.clone()) {
        Ok((h, cancel)) => {
            println!("   ConfigWatcher: watching {}", genesis_path);
            (Some(h), Some(cancel))
        }
        Err(e) => {
            eprintln!("   ConfigWatcher: failed to start ({}), hot-reload disabled", e);
            (None, None)
        }
    };

    // ── Spawn tick loop ──────────────────────────────────
    let tick_config = config_manager.clone();

    let tick_handle = tokio::spawn(async move {
        loop {
            let interval_ms = {
                let config = tick_config.get().await;
                config.world.tick_interval_ms
            };

            tokio::time::sleep(std::time::Duration::from_millis(interval_ms)).await;

            // Apply any pending config reload at the tick boundary.
            tick_config.apply_pending().await;
        }
    });

    // Build the HTTP API router with WAL support
    let app = agent_world_engine::api::create_router_with_wal_and_config(
        task_board,
        wal_writer.clone(),
        config_manager.clone(),
    );

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

    // Cleanup
    tick_handle.abort();
    if let (Some(handle), Some(cancel)) = (watcher_handle, watcher_cancel) {
        let _ = cancel.send(());
        handle.abort();
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
