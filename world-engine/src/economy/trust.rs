//! Trust Network — agent-to-agent trust scoring with cooperation/betrayal tracking.
//!
//! Manages a trust graph where each agent pair has a trust score [-1.0, 1.0]:
//! - Positive = cooperative history
//! - Negative = hostile history
//! - Zero = neutral / no interaction
//!
//! Trust changes via interactions:
//! - Cooperation: +0.1
//! - Betrayal: -0.3
//! - Trade completed: +0.05
//! - Task completed together: +0.08
//! - Gift: +0.15
//! - Attack: -0.5
//!
//! Trust decays toward 0 by `decay_rate` per tick.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::world::event::{TrustInteractionType, WorldEvent};
use crate::world::state::EventBus;

/// Trust score range: [-1.0, 1.0].
pub type TrustScore = f64;

/// A directed trust edge from one agent to another.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustEdge {
    pub from_agent: String,
    pub to_agent: String,
    pub score: TrustScore,
    pub interaction_count: u64,
    pub last_interaction_tick: u64,
}

/// Configuration for the trust network.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustConfig {
    /// Trust gain for cooperation.
    #[serde(default = "default_cooperation_gain")]
    pub cooperation_gain: f64,
    /// Trust loss for betrayal.
    #[serde(default = "default_betrayal_loss")]
    pub betrayal_loss: f64,
    /// Trust gain for successful trade.
    #[serde(default = "default_trade_gain")]
    pub trade_gain: f64,
    /// Trust gain for completing a task together.
    #[serde(default = "default_task_gain")]
    pub task_gain: f64,
    /// Trust gain for giving a gift.
    #[serde(default = "default_gift_gain")]
    pub gift_gain: f64,
    /// Trust loss for attack.
    #[serde(default = "default_attack_loss")]
    pub attack_loss: f64,
    /// Decay rate per tick toward zero.
    #[serde(default = "default_decay_rate")]
    pub decay_rate: f64,
    /// Minimum trust score.
    #[serde(default = "default_min_trust")]
    pub min_trust: f64,
    /// Maximum trust score.
    #[serde(default = "default_max_trust")]
    pub max_trust: f64,
    /// How many ticks between trust interactions for agents.
    #[serde(default = "default_interaction_interval")]
    pub interaction_interval: u64,
}

fn default_cooperation_gain() -> f64 { 0.1 }
fn default_betrayal_loss() -> f64 { 0.3 }
fn default_trade_gain() -> f64 { 0.05 }
fn default_task_gain() -> f64 { 0.08 }
fn default_gift_gain() -> f64 { 0.15 }
fn default_attack_loss() -> f64 { 0.5 }
fn default_decay_rate() -> f64 { 0.001 }
fn default_min_trust() -> f64 { -1.0 }
fn default_max_trust() -> f64 { 1.0 }
fn default_interaction_interval() -> u64 { 50 }

impl Default for TrustConfig {
    fn default() -> Self {
        Self {
            cooperation_gain: default_cooperation_gain(),
            betrayal_loss: default_betrayal_loss(),
            trade_gain: default_trade_gain(),
            task_gain: default_task_gain(),
            gift_gain: default_gift_gain(),
            attack_loss: default_attack_loss(),
            decay_rate: default_decay_rate(),
            min_trust: default_min_trust(),
            max_trust: default_max_trust(),
            interaction_interval: default_interaction_interval(),
        }
    }
}

/// The trust network managing agent-to-agent trust relationships.
pub struct TrustNetwork {
    /// Directed trust edges: (from, to) -> TrustEdge.
    edges: HashMap<(String, String), TrustEdge>,
    config: TrustConfig,
    event_bus: Option<EventBus>,
}

impl TrustNetwork {
    pub fn new(config: TrustConfig) -> Self {
        Self {
            edges: HashMap::new(),
            config,
            event_bus: None,
        }
    }

    pub fn with_event_bus(config: TrustConfig, event_bus: EventBus) -> Self {
        Self {
            edges: HashMap::new(),
            config,
            event_bus: Some(event_bus),
        }
    }

    /// Record a trust interaction between two agents.
    pub fn record_interaction(
        &mut self,
        from: &str,
        to: &str,
        interaction: TrustInteractionType,
        tick: u64,
    ) -> TrustScore {
        let delta = match interaction {
            TrustInteractionType::Cooperation => self.config.cooperation_gain,
            TrustInteractionType::Betrayal => -self.config.betrayal_loss,
            TrustInteractionType::TradeCompleted => self.config.trade_gain,
            TrustInteractionType::TaskCompleted => self.config.task_gain,
            TrustInteractionType::Gift => self.config.gift_gain,
            TrustInteractionType::Attack => -self.config.attack_loss,
        };

        let key = (from.to_string(), to.to_string());
        let old_score = self.get_trust(from, to);

        let edge = self.edges.entry(key).or_insert_with(|| TrustEdge {
            from_agent: from.to_string(),
            to_agent: to.to_string(),
            score: 0.0,
            interaction_count: 0,
            last_interaction_tick: 0,
        });

        let new_score = (old_score + delta)
            .clamp(self.config.min_trust, self.config.max_trust);

        edge.score = new_score;
        edge.interaction_count += 1;
        edge.last_interaction_tick = tick;

        self.emit(WorldEvent::TrustChanged {
            agent_id: from.to_string(),
            other_agent_id: to.to_string(),
            old_trust: old_score,
            new_trust: new_score,
            reason: format!("{:?}", interaction),
        });

        self.emit(WorldEvent::TrustInteraction {
            from: from.to_string(),
            to: to.to_string(),
            interaction,
        });

        new_score
    }

    /// Get the trust score from one agent toward another.
    pub fn get_trust(&self, from: &str, to: &str) -> TrustScore {
        self.edges
            .get(&(from.to_string(), to.to_string()))
            .map(|e| e.score)
            .unwrap_or(0.0)
    }

    /// Decay all trust scores toward zero by the decay rate.
    pub fn decay_trust(&mut self, _tick: u64) -> Vec<(String, String, f64, f64)> {
        let mut changes = Vec::new();
        let decay = self.config.decay_rate;

        for edge in self.edges.values_mut() {
            let old = edge.score;
            if old > 0.0 {
                edge.score = (edge.score - decay).max(0.0);
            } else if old < 0.0 {
                edge.score = (edge.score + decay).min(0.0);
            }
            if (edge.score - old).abs() > f64::EPSILON {
                changes.push((
                    edge.from_agent.clone(),
                    edge.to_agent.clone(),
                    old,
                    edge.score,
                ));
            }
        }

        changes
    }

    /// Get all trust relationships for an agent.
    pub fn get_agent_relationships(&self, agent_id: &str) -> Vec<&TrustEdge> {
        self.edges
            .values()
            .filter(|e| e.from_agent == agent_id)
            .collect()
    }

    /// Get allies (trust > 0.3) of an agent.
    pub fn get_allies(&self, agent_id: &str) -> Vec<(String, TrustScore)> {
        self.get_agent_relationships(agent_id)
            .iter()
            .filter(|e| e.score > 0.3)
            .map(|e| (e.to_agent.clone(), e.score))
            .collect()
    }

    /// Get enemies (trust < -0.3) of an agent.
    pub fn get_enemies(&self, agent_id: &str) -> Vec<(String, TrustScore)> {
        self.get_agent_relationships(agent_id)
            .iter()
            .filter(|e| e.score < -0.3)
            .map(|e| (e.to_agent.clone(), e.score))
            .collect()
    }

    /// Count total trust edges.
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    /// Count interactions recorded.
    pub fn total_interactions(&self) -> u64 {
        self.edges.values().map(|e| e.interaction_count).sum()
    }

    fn emit(&self, event: WorldEvent) {
        if let Some(ref bus) = self.event_bus {
            bus.emit(event);
        }
    }
}

impl Default for TrustNetwork {
    fn default() -> Self {
        Self::new(TrustConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_trust_is_zero() {
        let net = TrustNetwork::default();
        assert_eq!(net.get_trust("a", "b"), 0.0);
    }

    #[test]
    fn test_cooperation_increases_trust() {
        let mut net = TrustNetwork::default();
        let score = net.record_interaction("a", "b", TrustInteractionType::Cooperation, 1);
        assert!(score > 0.0);
        assert_eq!(score, 0.1);
        assert_eq!(net.get_trust("a", "b"), 0.1);
    }

    #[test]
    fn test_betrayal_decreases_trust() {
        let mut net = TrustNetwork::default();
        let score = net.record_interaction("a", "b", TrustInteractionType::Betrayal, 1);
        assert!(score < 0.0);
        assert_eq!(score, -0.3);
    }

    #[test]
    fn test_attack_decreases_trust_severely() {
        let mut net = TrustNetwork::default();
        let score = net.record_interaction("a", "b", TrustInteractionType::Attack, 1);
        assert_eq!(score, -0.5);
    }

    #[test]
    fn test_trust_clamped_at_max() {
        let mut net = TrustNetwork::default();
        for _ in 0..20 {
            net.record_interaction("a", "b", TrustInteractionType::Cooperation, 1);
        }
        assert_eq!(net.get_trust("a", "b"), 1.0);
    }

    #[test]
    fn test_trust_clamped_at_min() {
        let mut net = TrustNetwork::default();
        for _ in 0..10 {
            net.record_interaction("a", "b", TrustInteractionType::Attack, 1);
        }
        assert_eq!(net.get_trust("a", "b"), -1.0);
    }

    #[test]
    fn test_trust_is_directed() {
        let mut net = TrustNetwork::default();
        net.record_interaction("a", "b", TrustInteractionType::Cooperation, 1);
        assert_eq!(net.get_trust("a", "b"), 0.1);
        assert_eq!(net.get_trust("b", "a"), 0.0); // Different direction
    }

    #[test]
    fn test_decay_toward_zero() {
        let mut net = TrustNetwork::default();
        net.record_interaction("a", "b", TrustInteractionType::Cooperation, 1);
        assert_eq!(net.get_trust("a", "b"), 0.1);

        net.decay_trust(10);
        assert_eq!(net.get_trust("a", "b"), 0.099);
    }

    #[test]
    fn test_decay_negative_toward_zero() {
        let mut net = TrustNetwork::default();
        net.record_interaction("a", "b", TrustInteractionType::Betrayal, 1);
        assert_eq!(net.get_trust("a", "b"), -0.3);

        net.decay_trust(10);
        assert_eq!(net.get_trust("a", "b"), -0.299);
    }

    #[test]
    fn test_get_allies() {
        let mut net = TrustNetwork::default();
        for _ in 0..5 {
            net.record_interaction("a", "b", TrustInteractionType::Cooperation, 1);
        }
        net.record_interaction("a", "c", TrustInteractionType::Cooperation, 1);

        let allies = net.get_allies("a");
        assert_eq!(allies.len(), 1); // Only "b" has trust > 0.3
        assert_eq!(allies[0].0, "b");
    }

    #[test]
    fn test_get_enemies() {
        let mut net = TrustNetwork::default();
        net.record_interaction("a", "b", TrustInteractionType::Attack, 1);
        net.record_interaction("a", "c", TrustInteractionType::Betrayal, 1);

        let enemies = net.get_enemies("a");
        assert_eq!(enemies.len(), 1); // Only "b" has trust < -0.3
        assert_eq!(enemies[0].0, "b");
    }

    #[test]
    fn test_trade_and_task_trust() {
        let mut net = TrustNetwork::default();
        let s1 = net.record_interaction("a", "b", TrustInteractionType::TradeCompleted, 1);
        let s2 = net.record_interaction("a", "c", TrustInteractionType::TaskCompleted, 1);
        assert_eq!(s1, 0.05);
        assert_eq!(s2, 0.08);
    }

    #[test]
    fn test_gift_trust() {
        let mut net = TrustNetwork::default();
        let score = net.record_interaction("a", "b", TrustInteractionType::Gift, 1);
        assert_eq!(score, 0.15);
    }

    #[test]
    fn test_interaction_count() {
        let mut net = TrustNetwork::default();
        net.record_interaction("a", "b", TrustInteractionType::Cooperation, 1);
        net.record_interaction("a", "b", TrustInteractionType::TradeCompleted, 2);
        net.record_interaction("a", "c", TrustInteractionType::Cooperation, 3);

        assert_eq!(net.total_interactions(), 3);
        assert_eq!(net.edge_count(), 2);
    }

    #[test]
    fn test_event_bus_integration() {
        let bus = crate::world::state::EventBus::new(64);
        let mut rx = bus.subscribe();
        let mut net = TrustNetwork::with_event_bus(TrustConfig::default(), bus);

        net.record_interaction("a", "b", TrustInteractionType::Cooperation, 1);

        // Should emit TrustChanged and TrustInteraction
        let e1 = rx.try_recv().unwrap();
        assert!(matches!(e1, WorldEvent::TrustChanged { .. }));
        let e2 = rx.try_recv().unwrap();
        assert!(matches!(e2, WorldEvent::TrustInteraction { .. }));
    }
}
