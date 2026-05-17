use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use super::enums::{AgentPhase, Currency, DeathReason};

/// Discriminant for filtering events by kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    TickAdvanced,
    AgentSpawned,
    AgentDying,
    AgentDied,
    AgentRescued,
    TransactionCompleted,
    BalanceChanged,
    PhaseChanged,
    RuleViolated,
    SnapshotTaken,
    EscrowCreated,
    EscrowClaimed,
    EscrowReleased,
    EscrowRefunded,
    EscrowFrozen,
    TaskCreated,
    TaskClaimed,
    TaskStarted,
    TaskSubmitted,
    TaskReviewed,
    TaskCompleted,
    TaskExpired,
    RewardDistributed,
}

impl fmt::Display for EventType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = serde_json::to_value(self)
            .ok()
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_default();
        write!(f, "{}", s)
    }
}

impl FromStr for EventType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        serde_json::from_value(serde_json::Value::String(s.to_string()))
            .map_err(|_| format!("unknown event type: {:?}", s))
    }
}

/// Events emitted by the world engine.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
#[non_exhaustive]
pub enum WorldEvent {
    TickAdvanced { tick: u64 },
    AgentSpawned { agent_id: String, name: String },
    AgentDying { agent_id: String, reason: DeathReason, grace_ticks: u64 },
    AgentDied { agent_id: String, reason: DeathReason },
    AgentRescued { agent_id: String },
    TransactionCompleted { from: String, to: String, amount: u64, currency: Currency },
    BalanceChanged { agent_id: String, currency: Currency, old_balance: u64, new_balance: u64 },
    PhaseChanged { agent_id: String, old_phase: AgentPhase, new_phase: AgentPhase },
    RuleViolated { agent_id: String, rule: String, details: String },
    SnapshotTaken { tick: u64, path: String },
    EscrowCreated { escrow_id: String, publisher: String, reward: u64, currency: Currency },
    EscrowClaimed { escrow_id: String, claimant: String, deposit: u64 },
    EscrowReleased { escrow_id: String, recipient: String, amount: u64, currency: Currency },
    EscrowRefunded { escrow_id: String, recipient: String, amount: u64, currency: Currency },
    EscrowFrozen { escrow_id: String, reason: String },
    TaskCreated { task_id: String, publisher: String, reward: u64 },
    TaskClaimed { task_id: String, assignee: String },
    TaskStarted { task_id: String },
    TaskSubmitted { task_id: String },
    TaskReviewed { task_id: String, approved: bool },
    TaskCompleted { task_id: String },
    TaskExpired { task_id: String },
    RewardDistributed {
        task_id: String,
        assignee_id: String,
        gross_reward: u64,
        net_reward: u64,
        platform_fee: u64,
        xp_awarded: u64,
        reputation_change: f64,
    },
}

impl WorldEvent {
    pub fn event_type(&self) -> EventType {
        match self {
            WorldEvent::TickAdvanced { .. } => EventType::TickAdvanced,
            WorldEvent::AgentSpawned { .. } => EventType::AgentSpawned,
            WorldEvent::AgentDying { .. } => EventType::AgentDying,
            WorldEvent::AgentDied { .. } => EventType::AgentDied,
            WorldEvent::AgentRescued { .. } => EventType::AgentRescued,
            WorldEvent::TransactionCompleted { .. } => EventType::TransactionCompleted,
            WorldEvent::BalanceChanged { .. } => EventType::BalanceChanged,
            WorldEvent::PhaseChanged { .. } => EventType::PhaseChanged,
            WorldEvent::RuleViolated { .. } => EventType::RuleViolated,
            WorldEvent::SnapshotTaken { .. } => EventType::SnapshotTaken,
            WorldEvent::EscrowCreated { .. } => EventType::EscrowCreated,
            WorldEvent::EscrowClaimed { .. } => EventType::EscrowClaimed,
            WorldEvent::EscrowReleased { .. } => EventType::EscrowReleased,
            WorldEvent::EscrowRefunded { .. } => EventType::EscrowRefunded,
            WorldEvent::EscrowFrozen { .. } => EventType::EscrowFrozen,
            WorldEvent::TaskCreated { .. } => EventType::TaskCreated,
            WorldEvent::TaskClaimed { .. } => EventType::TaskClaimed,
            WorldEvent::TaskStarted { .. } => EventType::TaskStarted,
            WorldEvent::TaskSubmitted { .. } => EventType::TaskSubmitted,
            WorldEvent::TaskReviewed { .. } => EventType::TaskReviewed,
            WorldEvent::TaskCompleted { .. } => EventType::TaskCompleted,
            WorldEvent::TaskExpired { .. } => EventType::TaskExpired,
            WorldEvent::RewardDistributed { .. } => EventType::RewardDistributed,
        }
    }

    pub fn agent_id(&self) -> Option<&str> {
        match self {
            WorldEvent::TickAdvanced { .. } => None,
            WorldEvent::AgentSpawned { agent_id, .. } => Some(agent_id),
            WorldEvent::AgentDying { agent_id, .. } => Some(agent_id),
            WorldEvent::AgentDied { agent_id, .. } => Some(agent_id),
            WorldEvent::AgentRescued { agent_id } => Some(agent_id),
            WorldEvent::TransactionCompleted { from, .. } => Some(from),
            WorldEvent::BalanceChanged { agent_id, .. } => Some(agent_id),
            WorldEvent::PhaseChanged { agent_id, .. } => Some(agent_id),
            WorldEvent::RuleViolated { agent_id, .. } => Some(agent_id),
            WorldEvent::SnapshotTaken { .. } => None,
            WorldEvent::EscrowCreated { .. } => None,
            WorldEvent::EscrowClaimed { .. } => None,
            WorldEvent::EscrowReleased { .. } => None,
            WorldEvent::EscrowRefunded { .. } => None,
            WorldEvent::EscrowFrozen { .. } => None,
            WorldEvent::TaskCreated { .. } => None,
            WorldEvent::TaskClaimed { .. } => None,
            WorldEvent::TaskStarted { .. } => None,
            WorldEvent::TaskSubmitted { .. } => None,
            WorldEvent::TaskReviewed { .. } => None,
            WorldEvent::TaskCompleted { .. } => None,
            WorldEvent::TaskExpired { .. } => None,
            WorldEvent::RewardDistributed { assignee_id, .. } => Some(assignee_id),
        }
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("WorldEvent serialization is infallible")
    }

    pub fn to_json_pretty(&self) -> String {
        serde_json::to_string_pretty(self).expect("WorldEvent serialization is infallible")
    }

    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_tick_round_trip() {
        let event = WorldEvent::TickAdvanced { tick: 42 };
        let json = serde_json::to_string(&event).unwrap();
        let back: WorldEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, back);
    }

    #[test]
    fn event_agent_spawned_round_trip() {
        let event = WorldEvent::AgentSpawned {
            agent_id: "agent-001".into(),
            name: "Alice".into(),
        };
        let json = serde_json::to_string(&event).unwrap();
        let back: WorldEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, back);
    }

    #[test]
    fn event_agent_dying_round_trip() {
        let event = WorldEvent::AgentDying {
            agent_id: "agent-001".into(),
            reason: DeathReason::TokenDepleted,
            grace_ticks: 10,
        };
        let json = serde_json::to_string(&event).unwrap();
        let back: WorldEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, back);
    }

    #[test]
    fn event_agent_died_round_trip() {
        let event = WorldEvent::AgentDied {
            agent_id: "agent-001".into(),
            reason: DeathReason::TokenDepleted,
        };
        let json = serde_json::to_string(&event).unwrap();
        let back: WorldEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, back);
    }

    #[test]
    fn event_agent_rescued_round_trip() {
        let event = WorldEvent::AgentRescued {
            agent_id: "agent-001".into(),
        };
        let json = serde_json::to_string(&event).unwrap();
        let back: WorldEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, back);
    }

    #[test]
    fn event_transaction_round_trip() {
        let event = WorldEvent::TransactionCompleted {
            from: "agent-001".into(),
            to: "agent-002".into(),
            amount: 100,
            currency: Currency::Token,
        };
        let json = serde_json::to_string(&event).unwrap();
        let back: WorldEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, back);
    }

    #[test]
    fn event_phase_changed_round_trip() {
        let event = WorldEvent::PhaseChanged {
            agent_id: "agent-001".into(),
            old_phase: AgentPhase::Childhood,
            new_phase: AgentPhase::Adult,
        };
        let json = serde_json::to_string(&event).unwrap();
        let back: WorldEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, back);
    }

    #[test]
    fn event_serialized_format() {
        let event = WorldEvent::TickAdvanced { tick: 1 };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"tick_advanced\""));
    }

    #[test]
    fn event_death_reason_serialized() {
        let event = WorldEvent::AgentDied {
            agent_id: "a1".into(),
            reason: DeathReason::TokenDepleted,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("token_depleted"));
    }

    #[test]
    fn event_type_discriminant() {
        assert_eq!(
            WorldEvent::TickAdvanced { tick: 1 }.event_type(),
            EventType::TickAdvanced
        );
        assert_eq!(
            WorldEvent::AgentSpawned {
                agent_id: "a".into(),
                name: "b".into(),
            }
            .event_type(),
            EventType::AgentSpawned
        );
        assert_eq!(
            WorldEvent::AgentDied {
                agent_id: "a".into(),
                reason: DeathReason::TokenDepleted,
            }
            .event_type(),
            EventType::AgentDied
        );
    }

    #[test]
    fn agent_id_returns_none_for_tick() {
        assert!(WorldEvent::TickAdvanced { tick: 1 }.agent_id().is_none());
    }

    #[test]
    fn agent_id_returns_none_for_snapshot() {
        assert!(WorldEvent::SnapshotTaken {
            tick: 1,
            path: "snap.json".into(),
        }
        .agent_id()
        .is_none());
    }

    #[test]
    fn agent_id_returns_some_for_agent_events() {
        assert_eq!(
            WorldEvent::AgentSpawned {
                agent_id: "a1".into(),
                name: "Alice".into(),
            }
            .agent_id(),
            Some("a1")
        );
    }

    #[test]
    fn agent_id_transaction_returns_from() {
        assert_eq!(
            WorldEvent::TransactionCompleted {
                from: "sender".into(),
                to: "receiver".into(),
                amount: 50,
                currency: Currency::Money,
            }
            .agent_id(),
            Some("sender")
        );
    }

    #[test]
    fn to_json_and_from_json_roundtrip() {
        let event = WorldEvent::BalanceChanged {
            agent_id: "a1".into(),
            currency: Currency::Token,
            old_balance: 100,
            new_balance: 50,
        };
        let json = event.to_json();
        let back = WorldEvent::from_json(&json).unwrap();
        assert_eq!(event, back);
    }

    #[test]
    fn to_json_pretty_produces_multiline() {
        let event = WorldEvent::TickAdvanced { tick: 1 };
        let pretty = event.to_json_pretty();
        assert!(pretty.contains('\n'));
    }

    #[test]
    fn from_json_invalid_returns_error() {
        assert!(WorldEvent::from_json("not json").is_err());
    }
}
