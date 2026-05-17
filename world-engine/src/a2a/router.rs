use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::world::{EventBus, WorldEvent};

use super::discovery::AgentRegistry;

/// A pending message waiting to be delivered to a recipient agent.
#[derive(Debug, Clone)]
pub struct PendingMessage {
    pub id: String,
    pub from_agent: String,
    pub to_agent: String,
    pub msg_type: String,
    pub payload: Vec<u8>,
    pub timestamp: i64,
}

/// Routes A2A messages between agents and integrates with the EventBus.
#[derive(Clone)]
pub struct MessageRouter {
    /// Per-agent message queues (agent_id -> list of pending messages).
    queues: Arc<RwLock<HashMap<String, Vec<PendingMessage>>>>,
    /// Reference to the EventBus for emitting message events.
    event_bus: Arc<EventBus>,
    /// Agent registry for validating recipients.
    registry: Arc<AgentRegistry>,
}

impl MessageRouter {
    pub fn new(event_bus: Arc<EventBus>, registry: Arc<AgentRegistry>) -> Self {
        Self {
            queues: Arc::new(RwLock::new(HashMap::new())),
            event_bus,
            registry,
        }
    }

    /// Route a message to the target agent's queue.
    /// Returns Ok(()) if the recipient exists, Err if not found.
    pub async fn route_message(&self, msg: PendingMessage) -> Result<(), String> {
        // Validate recipient exists
        let recipient = self.registry.get(&msg.to_agent).await;
        if recipient.is_none() {
            return Err(format!("recipient agent '{}' not found", msg.to_agent));
        }

        // Enqueue the message
        {
            let mut queues = self.queues.write().await;
            queues
                .entry(msg.to_agent.clone())
                .or_insert_with(Vec::new)
                .push(msg.clone());
        }

        // Emit event to the EventBus so other subsystems can react
        self.event_bus.emit(WorldEvent::TransactionCompleted {
            from: msg.from_agent.clone(),
            to: msg.to_agent.clone(),
            amount: 0,
            currency: crate::world::enums::Currency::Token,
        });

        Ok(())
    }

    /// Deliver all pending messages for a specific agent.
    /// Returns the messages and removes them from the queue.
    pub async fn deliver_messages(&self, agent_id: &str) -> Vec<PendingMessage> {
        let mut queues = self.queues.write().await;
        queues.remove(agent_id).unwrap_or_default()
    }

    /// Check how many messages are pending for an agent.
    pub async fn pending_count(&self, agent_id: &str) -> usize {
        let queues = self.queues.read().await;
        queues.get(agent_id).map(|q| q.len()).unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::a2a::discovery::AgentRecord;

    #[tokio::test]
    async fn route_message_to_registered_agent() {
        let event_bus = Arc::new(EventBus::new(64));
        let registry = Arc::new(AgentRegistry::new());

        registry
            .register(AgentRecord {
                agent_id: "a1".into(),
                name: "Alice".into(),
                tokens: 0,
                money: 0,
                skills: vec![],
                reputation: 0.0,
                phase: "adult".into(),
            })
            .await;
        registry
            .register(AgentRecord {
                agent_id: "a2".into(),
                name: "Bob".into(),
                tokens: 0,
                money: 0,
                skills: vec![],
                reputation: 0.0,
                phase: "adult".into(),
            })
            .await;

        let router = MessageRouter::new(event_bus, registry);

        let msg = PendingMessage {
            id: "msg-001".into(),
            from_agent: "a1".into(),
            to_agent: "a2".into(),
            msg_type: "inform".into(),
            payload: b"hello".to_vec(),
            timestamp: 1000,
        };

        router.route_message(msg).await.unwrap();
        assert_eq!(router.pending_count("a2").await, 1);

        let delivered = router.deliver_messages("a2").await;
        assert_eq!(delivered.len(), 1);
        assert_eq!(delivered[0].payload, b"hello");
        assert_eq!(router.pending_count("a2").await, 0);
    }

    #[tokio::test]
    async fn route_message_to_unknown_agent_fails() {
        let event_bus = Arc::new(EventBus::new(64));
        let registry = Arc::new(AgentRegistry::new());
        let router = MessageRouter::new(event_bus, registry);

        let msg = PendingMessage {
            id: "msg-002".into(),
            from_agent: "a1".into(),
            to_agent: "unknown".into(),
            msg_type: "inform".into(),
            payload: vec![],
            timestamp: 1000,
        };

        let result = router.route_message(msg).await;
        assert!(result.is_err());
    }
}
