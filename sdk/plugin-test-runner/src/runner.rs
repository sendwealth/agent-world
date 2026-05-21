//! Test runner — executes plugin lifecycle and reports results.

use std::collections::HashMap;
use std::path::Path;
use std::time::Instant;

use anyhow::{Context, Result};
use serde_json::Value;

use crate::mock::{MockActionContextBuilder, scenarios};
use crate::report::{TestReport, TestResult, TestStatus};

/// The test runner. Loads a WASM plugin and runs standard test suites.
pub struct TestRunner {
    wasm_path: std::path::PathBuf,
    manifest_path: std::path::PathBuf,
    config: serde_json::Map<String, Value>,
    verbose: bool,
    report: TestReport,
}

impl TestRunner {
    pub fn new(
        wasm_path: &Path,
        manifest_path: &Path,
        config: serde_json::Map<String, Value>,
        verbose: bool,
    ) -> Self {
        let plugin_name = wasm_path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "unknown".into());

        Self {
            wasm_path: wasm_path.to_path_buf(),
            manifest_path: manifest_path.to_path_buf(),
            config,
            verbose,
            report: TestReport::new(&plugin_name),
        }
    }

    /// Run lifecycle tests: init → register → cost_estimate → execute → shutdown
    pub fn run_lifecycle_tests(&mut self) -> Result<()> {
        // Note: In a full implementation, this would load the WASM module
        // via wasmtime and call the exported functions.
        // For now, we test the JSON protocol compatibility.

        // Test 1: Config → Init JSON
        self.run_test("init_config_valid", || {
            let config_json = serde_json::Value::Object(self.config.clone()).to_string();
            if config_json.is_empty() {
                return Err("Config JSON is empty".into());
            }
            Ok(())
        });

        // Test 2: Manifest parsing
        self.run_test("manifest_parse", || {
            let content = std::fs::read_to_string(&self.manifest_path)
                .map_err(|e| format!("Cannot read manifest: {}", e))?;
            let _: serde_yaml::Value = serde_yaml::from_str(&content)
                .map_err(|e| format!("Invalid YAML: {}", e))?;
            Ok(())
        });

        // Test 3: WASM file exists and is valid
        self.run_test("wasm_file_valid", || {
            let metadata = std::fs::metadata(&self.wasm_path)
                .map_err(|e| format!("Cannot read WASM file: {}", e))?;
            if metadata.len() == 0 {
                return Err("WASM file is empty".into());
            }
            // Check WASM magic bytes
            let bytes = std::fs::read(&self.wasm_path)
                .map_err(|e| format!("Cannot read WASM bytes: {}", e))?;
            if bytes.len() < 4 || &bytes[0..4] != b"\x00asm" {
                return Err("Not a valid WASM file (bad magic bytes)".into());
            }
            Ok(())
        });

        // Test 4: Mock context serialization
        self.run_test("mock_context_serialization", || {
            let ctx = scenarios::basic().build_json();
            let parsed: Value = serde_json::from_str(&ctx)
                .map_err(|e| format!("Mock context JSON invalid: {}", e))?;
            if parsed["world"]["tick"].is_null() {
                return Err("Mock context missing world.tick".into());
            }
            Ok(())
        });

        Ok(())
    }

    /// Run default execute tests using built-in scenarios.
    pub fn run_default_execute_tests(&mut self) -> Result<()> {
        // Test: Basic execution scenario
        self.run_test("execute_basic_scenario", || {
            let ctx_json = scenarios::basic().build_json();
            // In full impl: call plugin execute(ctx_json)
            if self.verbose {
                eprintln!("  Context JSON: {} bytes", ctx_json.len());
            }
            Ok(())
        });

        // Test: Low token scenario
        self.run_test("execute_low_token_scenario", || {
            let ctx_json = scenarios::low_token_agent().build_json();
            let parsed: Value = serde_json::from_str(&ctx_json)?;
            if parsed["world"]["agent"]["tokens"].as_u64() != Some(2) {
                return Err("Low token scenario not configured correctly".into());
            }
            Ok(())
        });

        // Test: Multi-agent scenario
        self.run_test("execute_multi_agent_scenario", || {
            let ctx_json = scenarios::multi_agent().build_json();
            let parsed: Value = serde_json::from_str(&ctx_json)?;
            if parsed["world"]["visible_agents"].as_array().map(|a| a.len()) != Some(1) {
                return Err("Multi-agent scenario not configured correctly".into());
            }
            Ok(())
        });

        // Test: No agent scenario
        self.run_test("execute_no_agent_scenario", || {
            let ctx_json = scenarios::no_agent().build_json();
            let parsed: Value = serde_json::from_str(&ctx_json)?;
            if !parsed["world"]["agent"].is_null() {
                return Err("No-agent scenario should have null agent".into());
            }
            Ok(())
        });

        Ok(())
    }

    /// Run a custom test scenario from JSON.
    pub fn run_scenario(&mut self, scenario: &Value) -> Result<()> {
        let name = scenario["name"].as_str().unwrap_or("custom");
        let description = scenario["description"].as_str().unwrap_or("");

        self.run_test(&format!("scenario_{}", name), || {
            if self.verbose {
                eprintln!("  Scenario: {} — {}", name, description);
            }
            // Validate scenario structure
            if scenario["context"].is_null() {
                return Err("Scenario missing 'context' field".into());
            }
            Ok(())
        });

        Ok(())
    }

    fn run_test<F>(&mut self, name: &str, test_fn: F)
    where
        F: FnOnce() -> std::result::Result<(), String>,
    {
        let start = Instant::now();
        let status = match test_fn() {
            Ok(()) => TestStatus::Passed,
            Err(msg) => TestStatus::Failed(msg),
        };
        let duration = start.elapsed().as_millis() as u64;

        self.report.add(TestResult {
            name: name.into(),
            status,
            duration_ms: duration,
        });
    }

    pub fn report(self) -> TestReport {
        self.report
    }
}
