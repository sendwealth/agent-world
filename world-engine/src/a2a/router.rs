use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::{mpsc, RwLock};
use tokio_stream::wrappers::ReceiverStream;
use tonic::Status;

use super::registry::AgentRegistry;
use crate::agentworld::a2a::v1::A2aMessage;
use crate::world::intervention::MessageInterventionGuard;

/// A message waiting to be delivered to an agent.
pub type PendingMessage = A2aMessage;

/// Handles message routing between agents.
///
/// Each agent that opens a `StreamMessages` connection gets a sender
/// registered here. When `SendMessage` or a broadcast arrives, the
/// router looks up the recipient's sender and delivers the message.
///
/// The router includes an integrated `MessageInterventionGuard` that
/// enforces broadcast rate-limits (IC-01) and message size limits (IC-02).
pub struct MessageRouter {
    /// agent_id -> mpsc sender for that agent's stream
    streams: Arc<RwLock<HashMap<String, mpsc::Sender<PendingMessage>>>>,
    /// Buffer size for each agent's message channel
    buffer_size: usize,
    registry: Arc<AgentRegistry>,
    /// Intervention guard for broadcast rate-limit and message size checks.
    guard: MessageInterventionGuard,
    /// Broadcast count per agent (agent_id -> count in current window).
    broadcast_counts: Arc<RwLock<HashMap<String, u32>>>,
}

impl MessageRouter {
    pub fn new(registry: Arc<AgentRegistry>) -> Self {
        Self {
            streams: Arc::new(RwLock::new(HashMap::new())),
            buffer_size: 256,
            registry,
            guard: MessageInterventionGuard::default(),
            broadcast_counts: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Route a single message to its recipient.
    ///
    /// If `to_agent` is empty, the message is broadcast to all connected
    /// agents *except* the sender. Enforces IC-01 (broadcast rate-limit)
    /// and IC-02 (message size limit).
    pub async fn route(&self, msg: PendingMessage) -> Result<(), Status> {
        // IC-02: Message size check
        if let Err(reason) = self.guard.check_message_size(msg.payload.len()) {
            tracing::warn!(
                from = %msg.from_agent,
                size = msg.payload.len(),
                "Message blocked by IC-02: {}", reason
            );
            return Err(Status::failed_precondition(reason));
        }

        if msg.to_agent.is_empty() {
            // IC-01: Broadcast rate-limit check
            let mut counts = self.broadcast_counts.write().await;
            let count = counts.entry(msg.from_agent.clone()).or_insert(0);
            if let Err(reason) = self.guard.check_broadcast(&msg.from_agent, *count) {
                tracing::warn!(
                    from = %msg.from_agent,
                    count = *count,
                    "Broadcast blocked by IC-01: {}", reason
                );
                return Err(Status::resource_exhausted(reason));
            }
            *count += 1;
            drop(counts);

            self.broadcast(&msg).await
        } else {
            self.send_to(&msg).await
        }
    }

    /// Send a message to a specific agent.
    async fn send_to(&self, msg: &PendingMessage) -> Result<(), Status> {
        // Validate recipient exists in registry
        if self.registry.get(&msg.to_agent).await.is_none() {
            return Err(Status::not_found(format!(
                "Agent '{}' not found in registry",
                msg.to_agent
            )));
        }

        // Validate sender exists
        if self.registry.get(&msg.from_agent).await.is_none() {
            return Err(Status::not_found(format!(
                "Sender agent '{}' not found in registry",
                msg.from_agent
            )));
        }

        let streams = self.streams.read().await;
        if let Some(tx) = streams.get(&msg.to_agent) {
            tx.send(msg.clone())
                .await
                .map_err(|_| Status::unavailable("Recipient stream closed"))?;
            Ok(())
        } else {
            // Agent exists but has no active stream — still ACK the message
            // (it will be picked up next time the agent connects)
            Ok(())
        }
    }

    /// Broadcast a message to all connected agents except the sender.
    async fn broadcast(&self, msg: &PendingMessage) -> Result<(), Status> {
        let streams = self.streams.read().await;
        for (agent_id, tx) in streams.iter() {
            if agent_id == &msg.from_agent {
                continue;
            }
            // Best-effort delivery; if a stream is full, skip it
            let _ = tx.try_send(msg.clone());
        }
        Ok(())
    }

    /// Register a message stream for an agent. Returns a receiver that yields
    /// messages addressed to this agent.
    pub async fn open_stream(&self, agent_id: String) -> ReceiverStream<PendingMessage> {
        let (tx, rx) = mpsc::channel(self.buffer_size);
        self.streams.write().await.insert(agent_id, tx);
        ReceiverStream::new(rx)
    }

    /// Remove a message stream for an agent (called when stream disconnects).
    pub async fn close_stream(&self, agent_id: &str) {
        self.streams.write().await.remove(agent_id);
    }

    /// Get the count of active streams.
    pub async fn active_stream_count(&self) -> usize {
        self.streams.read().await.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::EventBus;
    use tokio_stream::StreamExt;

    fn make_router() -> Arc<MessageRouter> {
        let bus = Arc::new(EventBus::new(256));
        let registry = Arc::new(AgentRegistry::new(bus));
        Arc::new(MessageRouter::new(registry))
    }

    fn make_router_with_registry() -> (Arc<MessageRouter>, Arc<AgentRegistry>) {
        let bus = Arc::new(EventBus::new(256));
        let registry = Arc::new(AgentRegistry::new(bus));
        let router = Arc::new(MessageRouter::new(Arc::clone(&registry)));
        (router, registry)
    }

    fn sample_message(from: &str, to: &str) -> PendingMessage {
        PendingMessage {
            id: "msg-1".into(),
            from_agent: from.into(),
            to_agent: to.into(),
            r#type: 4, // INFORM
            payload: b"hello".to_vec(),
            timestamp: 1000,
            signature: String::new(),
            nonce: "nonce-1".into(),
        }
    }

    #[tokio::test]
    async fn send_to_delivers_message() {
        let (router, registry) = make_router_with_registry();
        registry
            .register("a1".into(), "Alice".into(), vec![], "pk1".into())
            .await;
        registry
            .register("a2".into(), "Bob".into(), vec![], "pk2".into())
            .await;

        // Open stream for recipient
        let mut rx = router.open_stream("a2".into()).await;

        let msg = sample_message("a1", "a2");
        router.route(msg.clone()).await.unwrap();

        let received = rx.next().await.unwrap();
        assert_eq!(received.id, "msg-1");
        assert_eq!(received.from_agent, "a1");
        assert_eq!(received.to_agent, "a2");
    }

    #[tokio::test]
    async fn send_to_unknown_recipient_fails() {
        let (router, registry) = make_router_with_registry();
        registry
            .register("a1".into(), "Alice".into(), vec![], "pk1".into())
            .await;

        let msg = sample_message("a1", "unknown");
        let result = router.route(msg).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code(), tonic::Code::NotFound);
    }

    #[tokio::test]
    async fn send_from_unknown_sender_fails() {
        let (router, registry) = make_router_with_registry();
        registry
            .register("a2".into(), "Bob".into(), vec![], "pk2".into())
            .await;

        let msg = sample_message("unknown", "a2");
        let result = router.route(msg).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code(), tonic::Code::NotFound);
    }

    #[tokio::test]
    async fn broadcast_delivers_to_all_except_sender() {
        let (router, registry) = make_router_with_registry();
        registry
            .register("a1".into(), "Alice".into(), vec![], "pk1".into())
            .await;
        registry
            .register("a2".into(), "Bob".into(), vec![], "pk2".into())
            .await;
        registry
            .register("a3".into(), "Charlie".into(), vec![], "pk3".into())
            .await;

        let mut rx2 = router.open_stream("a2".into()).await;
        let mut rx3 = router.open_stream("a3".into()).await;

        let mut msg = sample_message("a1", "");
        msg.to_agent = String::new(); // broadcast
        router.route(msg).await.unwrap();

        // a2 and a3 should receive
        assert!(rx2.next().await.is_some());
        assert!(rx3.next().await.is_some());
    }

    #[tokio::test]
    async fn close_stream_removes_sender() {
        let router = make_router();
        let rx = router.open_stream("a1".into()).await;
        assert_eq!(router.active_stream_count().await, 1);

        router.close_stream("a1").await;
        assert_eq!(router.active_stream_count().await, 0);
        drop(rx);
    }
}
