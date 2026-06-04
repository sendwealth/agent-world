//! Data Analysis Plugin — Agent World
//!
//! A WASM skill plugin that collects and analyzes agent economy data.
//! Computes:
//! - **GDP**: Sum of all agent money (total wealth in the system)
//! - **Average wealth**: Mean money across all visible agents
//! - **Gini coefficient**: Measure of wealth inequality (0 = perfect equality, 1 = max inequality)
//!
//! Targets `wasm32-unknown-unknown` for execution in the wasmtime sandbox.

use std::collections::HashMap;
use std::alloc::{alloc, Layout};

use serde::{Deserialize, Serialize};

// ─── Plugin API Types ─────────────────────────────────────────────────────
// These types mirror the SkillPlugin trait from the interface spec.

/// Plugin metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginInfo {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub min_engine_version: String,
    pub required_skills: Vec<String>,
    pub config_schema: Option<String>,
    pub tags: Vec<String>,
}

/// Read-only agent snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSnapshot {
    pub id: String,
    pub name: String,
    pub phase: String,
    pub money: u64,
    pub tokens: u64,
    pub reputation: f64,
    pub skills: HashMap<String, u64>,
    pub alive: bool,
    pub age: u64,
}

/// Read-only world state snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldContext {
    pub tick: u64,
    pub agent: Option<AgentSnapshot>,
    pub visible_agents: Vec<AgentSnapshot>,
    pub globals: HashMap<String, String>,
    pub recent_events: Vec<String>,
}

/// Full execution context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionContext {
    pub world: WorldContext,
    pub params: HashMap<String, String>,
    pub config: HashMap<String, String>,
}

/// Kind of state mutation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MutationKind {
    CreditTokens,
    DebitTokens,
    CreditMoney,
    DebitMoney,
    SetSkill,
    AdjustReputation,
    SetGlobal,
    EmitEvent,
}

/// A state mutation request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateMutation {
    pub kind: MutationKind,
    pub target_agent: Option<String>,
    pub field: String,
    pub value: String,
}

/// Result of execute().
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionResult {
    pub success: bool,
    pub message: String,
    pub mutations: Vec<StateMutation>,
    pub events: Vec<String>,
    pub data: HashMap<String, String>,
    pub tokens_consumed: u64,
}

/// Token cost estimate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenCost {
    pub estimated: u64,
    pub confidence: f64,
    pub breakdown: Option<String>,
}

/// Plugin error.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PluginError {
    InitFailed { reason: String },
    ExecutionFailed { reason: String },
    ConfigError { key: String, message: String },
    MissingSkill { skill_id: String },
    CostEstimateFailed { reason: String },
    InvalidState { expected: String, actual: String },
    Custom { code: String, message: String },
}

// ─── WASM ABI Helpers ─────────────────────────────────────────────────────

/// Allocate a buffer in WASM memory and return a packed (ptr, len) value.
fn allocate_output(json: &str) -> usize {
    let bytes = json.as_bytes();
    let len = bytes.len();
    let layout = Layout::array::<u8>(len).expect("invalid layout");
    let ptr = unsafe { alloc(layout) };
    unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr, len) };
    // Pack: upper 32 bits = ptr, lower 32 bits = len
    ((ptr as u64) << 32 | len as u64) as usize
}

/// Read input JSON from WASM memory.
fn read_input(ptr: *const u8, len: usize) -> String {
    let slice = unsafe { std::slice::from_raw_parts(ptr, len) };
    String::from_utf8_lossy(slice).into_owned()
}

// ─── Economy Analysis Logic ───────────────────────────────────────────────

/// Calculate the Gini coefficient from a sorted list of wealth values.
///
/// The Gini coefficient measures inequality:
/// - 0.0 = perfect equality (everyone has the same wealth)
/// - 1.0 = maximal inequality (one person has everything)
///
/// Uses the formula: G = (2 * Σ(i * x_i)) / (n * Σ(x_i)) - (n + 1) / n
/// where x_i is sorted in ascending order and i is 1-indexed.
fn calculate_gini(values: &[u64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    if values.len() == 1 {
        return 0.0; // Single agent = perfect equality by definition
    }

    let n = values.len() as f64;
    let sum: u64 = values.iter().sum();

    if sum == 0 {
        return 0.0; // All zero = perfect equality
    }

    // Sort ascending for Gini calculation
    let mut sorted = values.to_vec();
    sorted.sort();

    let sum_indexed: f64 = sorted
        .iter()
        .enumerate()
        .map(|(i, &v)| ((i as f64) + 1.0) * (v as f64))
        .sum();

    let gini = (2.0 * sum_indexed) / (n * (sum as f64)) - (n + 1.0) / n;
    // Clamp to [0, 1] for floating point safety
    gini.max(0.0).min(1.0)
}

/// Analyze the economy based on visible agents.
///
/// Returns a HashMap with analysis results:
/// - "gdp": Total wealth (sum of all agent money)
/// - "agent_count": Number of agents analyzed
/// - "avg_wealth": Average wealth per agent
/// - "gini_coefficient": Wealth inequality measure
/// - "wealthiest_agent": Name of the agent with the most money
/// - "poorest_agent": Name of the agent with the least money
/// - "max_wealth": Maximum wealth held by any agent
/// - "min_wealth": Minimum wealth held by any agent
/// - "median_wealth": Median wealth
fn analyze_economy(agents: &[AgentSnapshot], gini_precision: usize) -> HashMap<String, String> {
    let mut result = HashMap::new();

    if agents.is_empty() {
        result.insert("gdp".into(), "0".into());
        result.insert("agent_count".into(), "0".into());
        result.insert("avg_wealth".into(), "0".into());
        result.insert("gini_coefficient".into(), "0".into());
        result.insert("wealthiest_agent".into(), "N/A".into());
        result.insert("poorest_agent".into(), "N/A".into());
        result.insert("max_wealth".into(), "0".into());
        result.insert("min_wealth".into(), "0".into());
        result.insert("median_wealth".into(), "0".into());
        return result;
    }

    // Collect wealth values
    let wealths: Vec<u64> = agents.iter().map(|a| a.money).collect();

    // GDP = total wealth in the system
    let gdp: u64 = wealths.iter().sum();

    // Average wealth
    let avg_wealth = gdp as f64 / wealths.len() as f64;

    // Gini coefficient
    let gini = calculate_gini(&wealths);

    // Find wealthiest and poorest
    let mut wealthiest = &agents[0];
    let mut poorest = &agents[0];
    let mut max_wealth = agents[0].money;
    let mut min_wealth = agents[0].money;

    for agent in &agents[1..] {
        if agent.money > max_wealth {
            max_wealth = agent.money;
            wealthiest = agent;
        }
        if agent.money < min_wealth {
            min_wealth = agent.money;
            poorest = agent;
        }
    }

    // Median wealth
    let mut sorted_wealths = wealths.clone();
    sorted_wealths.sort();
    let median = if sorted_wealths.len() % 2 == 0 {
        let mid = sorted_wealths.len() / 2;
        (sorted_wealths[mid - 1] + sorted_wealths[mid]) as f64 / 2.0
    } else {
        sorted_wealths[sorted_wealths.len() / 2] as f64
    };

    result.insert("gdp".into(), gdp.to_string());
    result.insert("agent_count".into(), agents.len().to_string());
    result.insert("avg_wealth".into(), format!("{:.2}", avg_wealth));
    result.insert(
        "gini_coefficient".into(),
        format!("{:.1$}", gini, gini_precision),
    );
    result.insert("wealthiest_agent".into(), wealthiest.name.clone());
    result.insert("poorest_agent".into(), poorest.name.clone());
    result.insert("max_wealth".into(), max_wealth.to_string());
    result.insert("min_wealth".into(), min_wealth.to_string());
    result.insert("median_wealth".into(), format!("{:.2}", median));

    result
}

// ─── Plugin Implementation ────────────────────────────────────────────────

fn plugin_init(config: HashMap<String, String>) -> Result<PluginInfo, PluginError> {
    // Validate gini_precision if provided
    if let Some(prec_str) = config.get("gini_precision") {
        if let Ok(prec) = prec_str.parse::<usize>() {
            if prec > 10 {
                return Err(PluginError::ConfigError {
                    key: "gini_precision".into(),
                    message: "Precision must be between 0 and 10".into(),
                });
            }
        } else {
            return Err(PluginError::ConfigError {
                key: "gini_precision".into(),
                message: "Must be a valid integer".into(),
            });
        }
    }

    Ok(PluginInfo {
        id: "community/data-analysis".into(),
        name: "Economy Data Analysis Plugin".into(),
        version: env!("CARGO_PKG_VERSION").into(),
        description: "Collects and analyzes agent economy data — GDP, average wealth, Gini coefficient".into(),
        author: "Agent World Community".into(),
        min_engine_version: "1.0.0".into(),
        required_skills: vec![],
        config_schema: Some(
            r#"{"type":"object","properties":{"include_transaction_log":{"type":"boolean","default":false},"gini_precision":{"type":"integer","default":4}},"required":[]}"#.into(),
        ),
        tags: vec!["analytics".into(), "economy".into()],
    })
}

fn plugin_register() -> Vec<String> {
    vec!["economy_analysis".into()]
}

fn plugin_execute(ctx: ActionContext) -> Result<ActionResult, PluginError> {
    let gini_precision = ctx
        .config
        .get("gini_precision")
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(4);

    // Collect all visible agents (including the calling agent if present)
    let mut all_agents: Vec<AgentSnapshot> = ctx.world.visible_agents.clone();

    // Also include the calling agent if present and not already in visible_agents
    if let Some(ref agent) = ctx.world.agent {
        if !all_agents.iter().any(|a| a.id == agent.id) {
            all_agents.push(agent.clone());
        }
    }

    // Only analyze alive agents
    let alive_agents: Vec<AgentSnapshot> =
        all_agents.into_iter().filter(|a| a.alive).collect();

    let agent_count = alive_agents.len();

    // Run the analysis
    let analysis = analyze_economy(&alive_agents, gini_precision);

    // Build a human-readable summary
    let default_zero = "0".to_string();
    let message = if agent_count == 0 {
        "No agents found for economy analysis.".into()
    } else {
        let gdp = analysis.get("gdp").unwrap_or(&default_zero);
        let avg = analysis.get("avg_wealth").unwrap_or(&default_zero);
        let gini = analysis.get("gini_coefficient").unwrap_or(&default_zero);
        let median = analysis.get("median_wealth").unwrap_or(&default_zero);
        format!(
            "Economy Analysis (tick #{}, {} agents): GDP={}, Avg Wealth={}, Median={}, Gini={}",
            ctx.world.tick, agent_count, gdp, avg, median, gini
        )
    };

    // Emit an analytics event
    let event_payload = serde_json::json!({
        "type": "economy_analysis",
        "plugin": "community/data-analysis",
        "tick": ctx.world.tick,
        "agent_count": agent_count,
        "gdp": analysis.get("gdp").unwrap_or(&"0".into()),
        "gini": analysis.get("gini_coefficient").unwrap_or(&"0".into()),
    })
    .to_string();

    Ok(ActionResult {
        success: true,
        message,
        mutations: vec![],
        events: vec![event_payload],
        data: analysis,
        tokens_consumed: 5,
    })
}

fn plugin_cost_estimate(_ctx: &ActionContext) -> Result<TokenCost, PluginError> {
    Ok(TokenCost {
        estimated: 5,
        confidence: 0.95,
        breakdown: Some(
            "Fixed cost: 5 tokens per analysis (GDP + Gini computation)".into(),
        ),
    })
}

fn plugin_shutdown() {
    // No resources to clean up
}

// ─── WASM Exports ─────────────────────────────────────────────────────────

#[no_mangle]
pub extern "C" fn init(ptr: *const u8, len: usize) -> usize {
    let input = read_input(ptr, len);
    let config: HashMap<String, String> = match serde_json::from_str(&input) {
        Ok(c) => c,
        Err(e) => {
            let err = PluginError::ConfigError {
                key: "input".into(),
                message: e.to_string(),
            };
            return allocate_output(&serde_json::to_string(&err).unwrap());
        }
    };

    let result = plugin_init(config);
    allocate_output(&serde_json::to_string(&result).unwrap())
}

#[no_mangle]
pub extern "C" fn register() -> usize {
    let skills = plugin_register();
    allocate_output(&serde_json::to_string(&skills).unwrap())
}

#[no_mangle]
pub extern "C" fn execute(ptr: *const u8, len: usize) -> usize {
    let input = read_input(ptr, len);
    let ctx: ActionContext = match serde_json::from_str(&input) {
        Ok(c) => c,
        Err(e) => {
            let err = PluginError::ExecutionFailed {
                reason: format!("Invalid input: {}", e),
            };
            return allocate_output(&serde_json::to_string(&err).unwrap());
        }
    };

    let result = plugin_execute(ctx);
    allocate_output(&serde_json::to_string(&result).unwrap())
}

#[no_mangle]
pub extern "C" fn cost_estimate(ptr: *const u8, len: usize) -> usize {
    let input = read_input(ptr, len);
    let ctx: ActionContext = match serde_json::from_str(&input) {
        Ok(c) => c,
        Err(e) => {
            let err = PluginError::CostEstimateFailed {
                reason: format!("Invalid input: {}", e),
            };
            return allocate_output(&serde_json::to_string(&err).unwrap());
        }
    };

    let result = plugin_cost_estimate(&ctx);
    allocate_output(&serde_json::to_string(&result).unwrap())
}

#[no_mangle]
pub extern "C" fn shutdown() {
    plugin_shutdown()
}

// ─── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn mock_agent(id: &str, name: &str, money: u64) -> AgentSnapshot {
        AgentSnapshot {
            id: id.into(),
            name: name.into(),
            phase: "adult".into(),
            money,
            tokens: 100,
            reputation: 50.0,
            skills: HashMap::new(),
            alive: true,
            age: 10,
        }
    }

    fn mock_action_context(agents: Vec<AgentSnapshot>) -> ActionContext {
        ActionContext {
            world: WorldContext {
                tick: 100,
                agent: None,
                visible_agents: agents,
                globals: HashMap::new(),
                recent_events: vec![],
            },
            params: HashMap::new(),
            config: HashMap::new(),
        }
    }

    // ─── Init Tests ───────────────────────────────────────────────────

    #[test]
    fn test_init_returns_plugin_info() {
        let config = HashMap::new();
        let info = plugin_init(config).unwrap();
        assert_eq!(info.id, "community/data-analysis");
        assert_eq!(info.name, "Economy Data Analysis Plugin");
        assert_eq!(info.version, "1.0.0");
    }

    #[test]
    fn test_init_has_correct_tags() {
        let info = plugin_init(HashMap::new()).unwrap();
        assert!(info.tags.contains(&"analytics".to_string()));
        assert!(info.tags.contains(&"economy".to_string()));
    }

    #[test]
    fn test_init_rejects_high_precision() {
        let mut config = HashMap::new();
        config.insert("gini_precision".into(), "15".into());
        let result = plugin_init(config);
        assert!(result.is_err());
    }

    #[test]
    fn test_init_accepts_valid_precision() {
        let mut config = HashMap::new();
        config.insert("gini_precision".into(), "4".into());
        let info = plugin_init(config).unwrap();
        assert_eq!(info.id, "community/data-analysis");
    }

    // ─── Register Tests ───────────────────────────────────────────────

    #[test]
    fn test_register_returns_economy_analysis() {
        let skills = plugin_register();
        assert_eq!(skills, vec!["economy_analysis"]);
    }

    // ─── Execute Tests ────────────────────────────────────────────────

    #[test]
    fn test_execute_basic_analysis() {
        let agents = vec![
            mock_agent("a1", "Alice", 1000),
            mock_agent("a2", "Bob", 500),
            mock_agent("a3", "Charlie", 200),
        ];
        let ctx = mock_action_context(agents);
        let result = plugin_execute(ctx).unwrap();

        assert!(result.success);
        assert_eq!(result.tokens_consumed, 5);
        assert_eq!(result.data["gdp"], "1700");
        assert_eq!(result.data["agent_count"], "3");
        assert!(result.data["avg_wealth"].contains("566.67"));
        assert_eq!(result.data["wealthiest_agent"], "Alice");
        assert_eq!(result.data["poorest_agent"], "Charlie");
    }

    #[test]
    fn test_execute_gini_perfect_equality() {
        let agents = vec![
            mock_agent("a1", "Alice", 500),
            mock_agent("a2", "Bob", 500),
            mock_agent("a3", "Charlie", 500),
        ];
        let ctx = mock_action_context(agents);
        let result = plugin_execute(ctx).unwrap();

        assert!(result.success);
        // Gini should be ~0 for perfect equality
        let gini: f64 = result.data["gini_coefficient"].parse().unwrap();
        assert!(gini.abs() < 0.01, "Gini should be near 0 for equality, got {}", gini);
    }

    #[test]
    fn test_execute_gini_max_inequality() {
        let agents = vec![
            mock_agent("a1", "Rich", 10000),
            mock_agent("a2", "Poor1", 0),
            mock_agent("a3", "Poor2", 0),
            mock_agent("a4", "Poor3", 0),
        ];
        let ctx = mock_action_context(agents);
        let result = plugin_execute(ctx).unwrap();

        let gini: f64 = result.data["gini_coefficient"].parse().unwrap();
        assert!(gini > 0.7, "Gini should be high for inequality, got {}", gini);
    }

    #[test]
    fn test_execute_empty_agents() {
        let ctx = mock_action_context(vec![]);
        let result = plugin_execute(ctx).unwrap();

        assert!(result.success);
        assert_eq!(result.data["gdp"], "0");
        assert_eq!(result.data["agent_count"], "0");
    }

    #[test]
    fn test_execute_single_agent() {
        let agents = vec![mock_agent("a1", "Solo", 1000)];
        let ctx = mock_action_context(agents);
        let result = plugin_execute(ctx).unwrap();

        assert!(result.success);
        assert_eq!(result.data["gdp"], "1000");
        assert_eq!(result.data["gini_coefficient"], "0.0000");
        assert_eq!(result.data["wealthiest_agent"], "Solo");
    }

    #[test]
    fn test_execute_includes_calling_agent() {
        let agents = vec![mock_agent("a2", "Bob", 500)];
        let mut ctx = mock_action_context(agents);
        ctx.world.agent = Some(mock_agent("a1", "Alice", 1000));

        let result = plugin_execute(ctx).unwrap();

        assert_eq!(result.data["agent_count"], "2");
        assert_eq!(result.data["gdp"], "1500");
    }

    #[test]
    fn test_execute_fires_event() {
        let agents = vec![mock_agent("a1", "Alice", 1000)];
        let ctx = mock_action_context(agents);
        let result = plugin_execute(ctx).unwrap();

        assert_eq!(result.events.len(), 1);
        let event: serde_json::Value = serde_json::from_str(&result.events[0]).unwrap();
        assert_eq!(event["type"], "economy_analysis");
        assert_eq!(event["plugin"], "community/data-analysis");
    }

    #[test]
    fn test_execute_filters_dead_agents() {
        let mut dead_agent = mock_agent("a1", "Ghost", 9999);
        dead_agent.alive = false;
        let agents = vec![
            dead_agent,
            mock_agent("a2", "Alive", 100),
        ];
        let ctx = mock_action_context(agents);
        let result = plugin_execute(ctx).unwrap();

        assert_eq!(result.data["agent_count"], "1");
        assert_eq!(result.data["gdp"], "100");
    }

    // ─── Gini Calculation Tests ───────────────────────────────────────

    #[test]
    fn test_gini_empty() {
        assert_eq!(calculate_gini(&[]), 0.0);
    }

    #[test]
    fn test_gini_single_value() {
        assert_eq!(calculate_gini(&[100]), 0.0);
    }

    #[test]
    fn test_gini_two_equal() {
        let gini = calculate_gini(&[100, 100]);
        assert!(gini.abs() < 0.01);
    }

    #[test]
    fn test_gini_two_extreme() {
        let gini = calculate_gini(&[0, 1000]);
        assert!(gini > 0.4, "Two agents with extreme inequality should have high Gini, got {}", gini);
    }

    #[test]
    fn test_gini_all_zeros() {
        assert_eq!(calculate_gini(&[0, 0, 0, 0]), 0.0);
    }

    // ─── Cost Estimate Tests ──────────────────────────────────────────

    #[test]
    fn test_cost_estimate() {
        let ctx = mock_action_context(vec![]);
        let cost = plugin_cost_estimate(&ctx).unwrap();
        assert_eq!(cost.estimated, 5);
        assert!((cost.confidence - 0.95).abs() < 0.01);
    }

    #[test]
    fn test_cost_estimate_has_breakdown() {
        let ctx = mock_action_context(vec![]);
        let cost = plugin_cost_estimate(&ctx).unwrap();
        assert!(cost.breakdown.is_some());
        assert!(cost.breakdown.as_ref().unwrap().contains("5 tokens"));
    }

    // ─── Shutdown Tests ───────────────────────────────────────────────

    #[test]
    fn test_shutdown_no_panic() {
        plugin_shutdown();
    }
}
