use std::collections::HashMap;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::enums::AgentPhase;
use super::event::WorldEvent;
use super::state::EventBus;

/// An agent in the world simulation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Agent {
    pub id: String,
    pub name: String,
    pub phase: AgentPhase,
    pub money: u64,
    pub tokens: u64,
    pub reputation: f64,
    pub skills: HashMap<String, u64>,
    pub alive: bool,
    pub age: u64,
    pub created_at: String,
}

/// The agent registry tracks all agents in the world.
pub struct AgentRegistry {
    agents: HashMap<String, Agent>,
    event_bus: Option<EventBus>,
    /// Current tick counter.
    tick: u64,
}

impl AgentRegistry {
    pub fn new() -> Self {
        Self {
            agents: HashMap::new(),
            event_bus: None,
            tick: 0,
        }
    }

    pub fn with_event_bus(event_bus: EventBus) -> Self {
        Self {
            agents: HashMap::new(),
            event_bus: Some(event_bus),
            tick: 0,
        }
    }

    /// Spawn a new agent in the world.
    pub fn spawn_agent(&mut self, name: String, initial_tokens: u64, initial_money: u64) -> String {
        let id = Uuid::new_v4().to_string();
        let name_for_event = name.clone();
        let agent = Agent {
            id: id.clone(),
            name,
            phase: AgentPhase::Birth,
            money: initial_money,
            tokens: initial_tokens,
            reputation: 0.0,
            skills: HashMap::new(),
            alive: true,
            age: 0,
            created_at: Utc::now().to_rfc3339(),
        };

        self.agents.insert(id.clone(), agent);

        self.emit(WorldEvent::AgentSpawned {
            agent_id: id.clone(),
            name: name_for_event,
        });

        id
    }

    /// Get an agent by ID.
    pub fn get(&self, id: &str) -> Option<&Agent> {
        self.agents.get(id)
    }

    /// Get a mutable agent by ID.
    pub fn get_mut(&mut self, id: &str) -> Option<&mut Agent> {
        self.agents.get_mut(id)
    }

    /// List all agents.
    pub fn list(&self) -> Vec<&Agent> {
        self.agents.values().collect()
    }

    /// Count alive agents.
    pub fn alive_count(&self) -> usize {
        self.agents.values().filter(|a| a.alive).count()
    }

    /// Count dead agents.
    pub fn dead_count(&self) -> usize {
        self.agents.values().filter(|a| !a.alive).count()
    }

    /// Total agent count.
    pub fn count(&self) -> usize {
        self.agents.len()
    }

    /// Total money across all alive agents.
    pub fn total_money(&self) -> u64 {
        self.agents.values().filter(|a| a.alive).map(|a| a.money).sum()
    }

    /// Set the current tick.
    pub fn set_tick(&mut self, tick: u64) {
        self.tick = tick;
    }

    /// Get the current tick.
    pub fn tick(&self) -> u64 {
        self.tick
    }

    /// Update agent skill.
    pub fn update_skill(&mut self, agent_id: &str, skill: String, level: u64) {
        if let Some(agent) = self.agents.get_mut(agent_id) {
            agent.skills.insert(skill, level);
        }
    }

    /// Add reputation to an agent.
    pub fn add_reputation(&mut self, agent_id: &str, delta: f64) {
        if let Some(agent) = self.agents.get_mut(agent_id) {
            agent.reputation = (agent.reputation + delta).max(0.0);
        }
    }

    /// Kill an agent.
    pub fn kill_agent(&mut self, agent_id: &str) {
        if let Some(agent) = self.agents.get_mut(agent_id) {
            agent.alive = false;
            agent.phase = AgentPhase::Dead;
        }
    }

    fn emit(&self, event: WorldEvent) {
        if let Some(ref bus) = self.event_bus {
            bus.emit(event);
        }
    }
}

impl Default for AgentRegistry {
    fn default() -> Self {
        Self::new()
    }
}
