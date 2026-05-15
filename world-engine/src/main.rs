use std::sync::Arc;
use tokio::sync::Mutex;

mod api;
mod economy;
mod lifecycle;
mod rules;
pub mod world;

use economy::task::TaskBoard;

#[tokio::main]
async fn main() {
    let event_bus = world::EventBus::new(256);

    println!("Agent World Engine v0.1.0");
    println!("   Status: initializing...");
    println!("   EventBus: created (capacity: 256)");

    // Initialize task board with event bus
    let task_board = Arc::new(Mutex::new(TaskBoard::with_event_bus(event_bus)));

    // Build the HTTP API router
    let app = api::create_router(task_board);

    // Start the HTTP server
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], 3000));
    println!("   API server: http://{}", addr);
    println!("   Status: ready");

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    if let Err(e) = axum::serve(listener, app).await {
        eprintln!("Server error: {}", e);
    }
}
