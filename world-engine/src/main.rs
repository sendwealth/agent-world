/// Agent World — World Engine
///
/// The central authority for world state, time, economy, and rules.

mod economy;
mod lifecycle;
mod rules;

fn main() {
    println!("🌍 Agent World Engine v0.1.0");
    println!("   Status: initializing...");
    // TODO: Load genesis config
    // TODO: Initialize ledger
    // TODO: Start tick scheduler
    // TODO: Start gRPC server
    println!("   Status: ready (skeleton)");
}
