//! Mock types for testing plugins without the World Engine.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Builder for creating mock `WorldContext` instances.
#[derive(Debug, Clone)]
pub struct MockWorldContextBuilder {
    tick: u64,
    agent: Option<MockAgentSnapshot>,
    visible_agents: Vec<MockAgentSnapshot>,
    globals: HashMap<String, String>,
    recent_events: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockAgentSnapshot {
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

impl MockWorldContextBuilder {
    pub fn new() -> Self {
        Self {
            tick: 1,
            agent: None,
            visible_agents: vec![],
            globals: HashMap::new(),
            recent_events: vec![],
        }
    }

    pub fn tick(mut self, tick: u64) -> Self {
        self.tick = tick;
        self
    }

    pub fn agent(mut self, agent: MockAgentSnapshot) -> Self {
        self.agent = Some(agent);
        self
    }

    pub fn default_agent(self) -> Self {
        self.agent(MockAgentSnapshot {
            id: "agent-001".into(),
            name: "TestAgent".into(),
            phase: "adult".into(),
            money: 1000,
            tokens: 500,
            reputation: 50.0,
            skills: HashMap::new(),
            alive: true,
            age: 10,
        })
    }

    pub fn visible_agent(mut self, agent: MockAgentSnapshot) -> Self {
        self.visible_agents.push(agent);
        self
    }

    pub fn global(mut self, key: &str, value: &str) -> Self {
        self.globals.insert(key.into(), value.into());
        self
    }

    pub fn event(mut self, event: &str) -> Self {
        self.recent_events.push(event.into());
        self
    }

    /// Build the mock world context as a JSON string for passing to the plugin.
    pub fn build_json(&self) -> String {
        serde_json::json!({
            "tick": self.tick,
            "agent": self.agent,
            "visible_agents": self.visible_agents,
            "globals": self.globals,
            "recent_events": self.recent_events,
        })
        .to_string()
    }
}

impl Default for MockWorldContextBuilder {
    fn default() -> Self {
        Self::new().default_agent()
    }
}

/// Builder for creating mock `ActionContext` instances.
#[derive(Debug, Clone)]
pub struct MockActionContextBuilder {
    world: MockWorldContextBuilder,
    params: HashMap<String, String>,
    config: HashMap<String, String>,
}

impl MockActionContextBuilder {
    pub fn new() -> Self {
        Self {
            world: MockWorldContextBuilder::default(),
            params: HashMap::new(),
            config: HashMap::new(),
        }
    }

    pub fn world(mut self, world: MockWorldContextBuilder) -> Self {
        self.world = world;
        self
    }

    pub fn param(mut self, key: &str, value: &str) -> Self {
        self.params.insert(key.into(), value.into());
        self
    }

    pub fn config(mut self, key: &str, value: &str) -> Self {
        self.config.insert(key.into(), value.into());
        self
    }

    pub fn build_json(&self) -> String {
        let world_json: serde_json::Value =
            serde_json::from_str(&self.world.build_json()).unwrap();
        serde_json::json!({
            "world": world_json,
            "params": self.params,
            "config": self.config,
        })
        .to_string()
    }
}

impl Default for MockActionContextBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Pre-built mock scenarios.
pub mod scenarios {
    use super::*;

    /// A basic scenario with one agent and default config.
    pub fn basic() -> MockActionContextBuilder {
        MockActionContextBuilder::new()
            .config("greeting", "Hello")
    }

    /// A scenario with a low-token agent.
    pub fn low_token_agent() -> MockActionContextBuilder {
        MockActionContextBuilder::new()
            .world(
                MockWorldContextBuilder::new().agent(MockAgentSnapshot {
                    id: "agent-poor".into(),
                    name: "PoorAgent".into(),
                    phase: "adult".into(),
                    money: 10,
                    tokens: 2,
                    reputation: 5.0,
                    skills: HashMap::new(),
                    alive: true,
                    age: 1,
                }),
            )
    }

    /// A scenario with multiple visible agents.
    pub fn multi_agent() -> MockActionContextBuilder {
        MockActionContextBuilder::new()
            .world(
                MockWorldContextBuilder::new()
                    .default_agent()
                    .visible_agent(MockAgentSnapshot {
                        id: "agent-002".into(),
                        name: "Bob".into(),
                        phase: "adult".into(),
                        money: 500,
                        tokens: 300,
                        reputation: 40.0,
                        skills: HashMap::new(),
                        alive: true,
                        age: 8,
                    }),
            )
    }

    /// A scenario with no agent (world-level event).
    pub fn no_agent() -> MockActionContextBuilder {
        MockActionContextBuilder::new()
            .world(MockWorldContextBuilder::new().tick(100))
    }
}
