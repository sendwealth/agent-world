use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::{mpsc, RwLock};
use tokio_stream::wrappers::ReceiverStream;

use crate::agentworld::a2a::v1::WorldMessage;

/// Buffer size for each agent's world message channel.
const WORLD_MSG_BUFFER: usize = 128;

/// Routes Oracle and Bounty messages from the World Engine to connected
/// Agent Runtime instances via the `ConsumeMessages` gRPC stream.
///
/// Each agent that calls `ConsumeMessages(agent_id)` gets a per-agent
/// `mpsc::Sender<WorldMessage>` registered here. When the HTTP API creates
/// an Oracle or Bounty, it calls `deliver_oracle` / `deliver_bounty` which
/// pushes a `WorldMessage` into the matching agent's channel.
pub struct WorldMessageRouter {
    /// agent_id -> mpsc sender for that agent's world message stream
    streams: Arc<RwLock<HashMap<String, mpsc::Sender<WorldMessage>>>>,
}

impl WorldMessageRouter {
    pub fn new() -> Self {
        Self {
            streams: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a message stream for an agent.
    ///
    /// Returns a `ReceiverStream` that yields `WorldMessage` items
    /// (Oracle, Bounty) addressed to this agent.
    pub async fn open_stream(&self, agent_id: String) -> ReceiverStream<WorldMessage> {
        let (tx, rx) = mpsc::channel(WORLD_MSG_BUFFER);
        self.streams.write().await.insert(agent_id, tx);
        ReceiverStream::new(rx)
    }

    /// Remove a message stream for an agent (called when stream disconnects).
    pub async fn close_stream(&self, agent_id: &str) {
        self.streams.write().await.remove(agent_id);
    }

    /// Deliver a `WorldMessage` to a specific agent's stream.
    ///
    /// Returns `true` if the agent has an active stream and the message was
    /// sent, `false` if the agent is not connected or the channel is full.
    pub async fn deliver(&self, agent_id: &str, msg: WorldMessage) -> bool {
        let streams = self.streams.read().await;
        if let Some(tx) = streams.get(agent_id) {
            tx.try_send(msg).is_ok()
        } else {
            false
        }
    }

    /// Broadcast a `WorldMessage` to all connected agent streams.
    ///
    /// Used for bounties that have no specific target agent.
    /// Returns the number of agents the message was sent to.
    pub async fn broadcast(&self, msg: WorldMessage) -> usize {
        let streams = self.streams.read().await;
        let mut count = 0;
        for (_, tx) in streams.iter() {
            // Use the same message for all recipients (WorldMessage is Clone)
            if tx.try_send(msg.clone()).is_ok() {
                count += 1;
            }
        }
        count
    }

    /// Get the count of active streams.
    pub async fn active_stream_count(&self) -> usize {
        self.streams.read().await.len()
    }
}

pub type SharedWorldMessageRouter = Arc<WorldMessageRouter>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agentworld::a2a::v1::{BountyPayload, OraclePayload, OracleType};
    use tokio_stream::StreamExt;

    fn sample_oracle_message(id: &str) -> WorldMessage {
        WorldMessage {
            id: id.into(),
            payload: Some(
                crate::agentworld::a2a::v1::world_message::Payload::Oracle(OraclePayload {
                    oracle_id: id.into(),
                    oracle_type: OracleType::Guidance as i32,
                    content: "test oracle".into(),
                    from_human: true,
                    human_id: "human-1".into(),
                }),
            ),
            timestamp: 1000,
        }
    }

    fn sample_bounty_message(id: &str) -> WorldMessage {
        WorldMessage {
            id: id.into(),
            payload: Some(
                crate::agentworld::a2a::v1::world_message::Payload::Bounty(BountyPayload {
                    bounty_id: id.into(),
                    title: "test bounty".into(),
                    description: String::new(),
                    reward: 100,
                    deadline_tick: 0,
                    human_id: "human-1".into(),
                }),
            ),
            timestamp: 1000,
        }
    }

    #[tokio::test]
    async fn deliver_to_connected_agent() {
        let router = Arc::new(WorldMessageRouter::new());
        let mut rx = router.open_stream("agent-1".into()).await;

        let msg = sample_oracle_message("o-1");
        assert!(router.deliver("agent-1", msg).await);

        let received = rx.next().await.unwrap();
        assert_eq!(received.id, "o-1");
    }

    #[tokio::test]
    async fn deliver_to_disconnected_agent() {
        let router = WorldMessageRouter::new();
        let msg = sample_oracle_message("o-1");
        assert!(!router.deliver("agent-1", msg).await);
    }

    #[tokio::test]
    async fn broadcast_to_all_agents() {
        let router = Arc::new(WorldMessageRouter::new());
        let mut rx1 = router.open_stream("agent-1".into()).await;
        let mut rx2 = router.open_stream("agent-2".into()).await;

        let msg = sample_bounty_message("b-1");
        let count = router.broadcast(msg).await;
        assert_eq!(count, 2);

        assert!(rx1.next().await.is_some());
        assert!(rx2.next().await.is_some());
    }

    #[tokio::test]
    async fn close_stream_removes_sender() {
        let router = WorldMessageRouter::new();
        let _rx = router.open_stream("agent-1".into()).await;
        assert_eq!(router.active_stream_count().await, 1);

        router.close_stream("agent-1").await;
        assert_eq!(router.active_stream_count().await, 0);
    }
}
