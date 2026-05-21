//! Plugin Test Runner — Local test framework for Agent World plugins.
//!
//! Tests plugin WASM modules without requiring the full World Engine.
//! Provides mock WorldContext and ActionContext, runs the plugin lifecycle,
//! and reports results.

mod mock;
mod report;
mod runner;

use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "plugin-test-runner")]
#[command(about = "Test Agent World plugins locally without the World Engine")]
#[command(version)]
struct Cli {
    /// Path to the plugin WASM file
    #[arg(value_name = "PLUGIN_WASM")]
    plugin: PathBuf,

    /// Path to plugin manifest (skills.yaml)
    #[arg(short, long, default_value = "skills.yaml")]
    manifest: PathBuf,

    /// Path to test config JSON
    #[arg(short, long)]
    config: Option<PathBuf>,

    /// Verbose output
    #[arg(short, long)]
    verbose: bool,

    /// Only run lifecycle tests (init → register → shutdown)
    #[arg(long)]
    lifecycle_only: bool,

    /// Custom test scenario JSON file
    #[arg(long)]
    scenario: Option<PathBuf>,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Validate inputs
    if !cli.plugin.exists() {
        anyhow::bail!("Plugin not found: {}", cli.plugin.display());
    }
    if !cli.manifest.exists() {
        anyhow::bail!("Manifest not found: {}", cli.manifest.display());
    }

    let config = if let Some(config_path) = &cli.config {
        let raw = std::fs::read_to_string(config_path)?;
        serde_json::from_str(&raw)?
    } else {
        serde_json::json!({"greeting": "Hello"}).as_object().unwrap().clone()
    };

    if cli.verbose {
        eprintln!("🔌 Plugin: {}", cli.plugin.display());
        eprintln!("📋 Manifest: {}", cli.manifest.display());
    }

    // Run tests
    let mut test_runner = runner::TestRunner::new(
        &cli.plugin,
        &cli.manifest,
        config,
        cli.verbose,
    );

    // Lifecycle tests
    test_runner.run_lifecycle_tests()?;

    // Execute tests (unless lifecycle_only)
    if !cli.lifecycle_only {
        if let Some(scenario_path) = &cli.scenario {
            let scenario_raw = std::fs::read_to_string(scenario_path)?;
            let scenario: serde_json::Value = serde_json::from_str(&scenario_raw)?;
            test_runner.run_scenario(&scenario)?;
        } else {
            test_runner.run_default_execute_tests()?;
        }
    }

    // Print report
    let report = test_runner.report();
    report.print();

    if report.all_passed() {
        std::process::exit(0);
    } else {
        std::process::exit(1);
    }
}
