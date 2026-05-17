use std::sync::Arc;
use tokio::sync::Mutex;

use agent_world_engine::a2a::server::{GrpcServer, GrpcState};
use agent_world_engine::economy::task::TaskBoard;
use agent_world_engine::wal::WAL;
use agent_world_engine::world::EventBus;

#[tokio::main]
async fn main() {
    let event_bus = Arc::new(EventBus::new(256));

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

    // Initialize task board with event bus
    let task_board = Arc::new(Mutex::new(TaskBoard::with_event_bus((*event_bus).clone())));

    // Build the HTTP API router with WAL support
    let app = agent_world_engine::api::create_router_with_wal(task_board.clone(), wal_writer.clone());

    // Build the gRPC server
    let grpc_state = GrpcState::new(event_bus.clone(), task_board);
    let grpc_server = GrpcServer::new(grpc_state).into_service();

    // Start the HTTP server
    let http_addr = std::net::SocketAddr::from(([127, 0, 0, 1], 3000));
    println!("   API server: http://{}", http_addr);

    // Start the gRPC server
    let grpc_addr = std::net::SocketAddr::from(([127, 0, 0, 1], 50051));
    println!("   gRPC server: http://{}", grpc_addr);
    println!("   Status: ready");

    let http_listener = tokio::net::TcpListener::bind(http_addr).await.unwrap();
    let grpc_listener = tokio::net::TcpListener::bind(grpc_addr).await.unwrap();

    // Graceful shutdown on ctrl-c
    let shutdown_wal = wal_writer.clone();
    let http_server = axum::serve(http_listener, app);
    let grpc_server = tonic::transport::Server::builder()
        .add_service(grpc_server)
        .serve_with_incoming(tokio_stream::wrappers::TcpListenerStream::new(grpc_listener));

    tokio::select! {
        result = http_server => {
            if let Err(e) = result {
                eprintln!("HTTP server error: {}", e);
            }
        }
        result = grpc_server => {
            if let Err(e) = result {
                eprintln!("gRPC server error: {}", e);
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
