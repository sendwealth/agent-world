//! {{project-name}} — Agent World Plugin
//!
//! A skill plugin for the Agent World simulation engine.
//! Implements the SkillPlugin trait via WASM ABI.

use std::collections::HashMap;
use std::alloc::{alloc, dealloc, Layout};

use serde::{Deserialize, Serialize};

// ─── Plugin API Types ─────────────────────────────────────────────────────
// These types mirror the SkillPlugin trait from the interface spec.
// When the SDK crate is published, these will be re-exported from
// `agent-world-plugin-sdk`.

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

/// Allocate a buffer in WASM memory and return (ptr, len).
/// The engine will read the JSON from this pointer.
fn allocate_output(json: &str) -> usize {
    let bytes = json.as_bytes();
    let len = bytes.len();
    let layout = Layout::array::<u8>(len).expect("invalid layout");
    let ptr = unsafe { alloc(layout) };
    unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr, len) };
    // Pack ptr and len: upper 32 bits = ptr, lower 32 bits = len
    ((ptr as u64) << 32 | len as u64) as usize
}

/// Read input JSON from WASM memory.
fn read_input(ptr: *const u8, len: usize) -> String {
    let slice = unsafe { std::slice::from_raw_parts(ptr, len) };
    String::from_utf8_lossy(slice).into_owned()
}

// ─── Plugin Implementation ────────────────────────────────────────────────
// ✏️ Edit below to implement your plugin logic.

fn plugin_init(config: HashMap<String, String>) -> Result<PluginInfo, PluginError> {
    // Validate configuration
    let greeting = config.get("greeting").cloned().unwrap_or_else(|| "Hello".into());

    // Store config for later use (in a real plugin, you'd use a static or thread-local)
    // For this template, we just validate and return metadata.

    Ok(PluginInfo {
        id: "{{plugin-id}}".into(),
        name: "{{project-name}}".into(),
        version: env!("CARGO_PKG_VERSION").into(),
        description: "{{project-description}}".into(),
        author: "{{authors}}".into(),
        min_engine_version: "1.0.0".into(),
        required_skills: vec![],
        config_schema: Some(r#"{"type":"object","properties":{"greeting":{"type":"string","default":"Hello"}}}"#.into()),
        tags: vec!["example".into()],
    })
}

fn plugin_register() -> Vec<String> {
    vec!["{{skill-id}}".into()]
}

fn plugin_execute(ctx: ActionContext) -> Result<ActionResult, PluginError> {
    let greeting = ctx.config.get("greeting").cloned().unwrap_or_else(|| "Hello".into());

    let agent_name = ctx.world.agent
        .as_ref()
        .map(|a| a.name.as_str())
        .unwrap_or("stranger");

    let message = format!("{}, {}! (tick #{})", greeting, agent_name, ctx.world.tick);

    Ok(ActionResult {
        success: true,
        message,
        mutations: vec![],
        events: vec![format!("{{\"type\":\"plugin_greeting\",\"plugin\":\"{{plugin-id}}\"}}")],
        data: {
            let mut d = HashMap::new();
            d.insert("greeting".into(), greeting);
            d
        },
        tokens_consumed: 1,
    })
}

fn plugin_cost_estimate(_ctx: &ActionContext) -> Result<TokenCost, PluginError> {
    Ok(TokenCost {
        estimated: 1,
        confidence: 1.0,
        breakdown: Some("Fixed cost: 1 token per execution".into()),
    })
}

fn plugin_shutdown() {
    // Cleanup resources if needed
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

    fn mock_action_context() -> ActionContext {
        ActionContext {
            world: WorldContext {
                tick: 42,
                agent: Some(AgentSnapshot {
                    id: "agent-001".into(),
                    name: "Alice".into(),
                    phase: "adult".into(),
                    money: 1000,
                    tokens: 500,
                    reputation: 50.0,
                    skills: HashMap::new(),
                    alive: true,
                    age: 10,
                }),
                visible_agents: vec![],
                globals: HashMap::new(),
                recent_events: vec![],
            },
            params: HashMap::new(),
            config: {
                let mut c = HashMap::new();
                c.insert("greeting".into(), "Hi".into());
                c
            },
        }
    }

    #[test]
    fn test_init() {
        let mut config = HashMap::new();
        config.insert("greeting".into(), "Hey".into());
        let info = plugin_init(config).unwrap();
        assert_eq!(info.id, "{{plugin-id}}");
        assert_eq!(info.name, "{{project-name}}");
    }

    #[test]
    fn test_register() {
        let skills = plugin_register();
        assert_eq!(skills, vec!["{{skill-id}}"]);
    }

    #[test]
    fn test_execute() {
        let ctx = mock_action_context();
        let result = plugin_execute(ctx).unwrap();
        assert!(result.success);
        assert!(result.message.contains("Alice"));
        assert!(result.message.contains("Hi"));
        assert_eq!(result.tokens_consumed, 1);
    }

    #[test]
    fn test_cost_estimate() {
        let ctx = mock_action_context();
        let cost = plugin_cost_estimate(&ctx).unwrap();
        assert_eq!(cost.estimated, 1);
        assert_eq!(cost.confidence, 1.0);
    }

    #[test]
    fn test_shutdown() {
        plugin_shutdown(); // Should not panic
    }
}
