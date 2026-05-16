use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::{mpsc, Mutex, RwLock};
use tonic::Status;

use crate::world::EventBus;

/// A2A message as used internally by the router.
/// Mirrors the protobuf A2AMessage but with owned types.
#[derive(Debug, Clone)]
pub struct RouterMessage {
    pub id: String,
    pub from_agent: String,
    pub to_agent: String,        // empty string = broadcast
    pub message_type: i32,       // protobuf MessageType as i32
    pub payload: Vec<u8>,
    pub timestamp: i64,
    pub signature: String,
    pub nonce: String,
}

/// Information about a registered agent.
#[derive(Debug, Clone)]
pub struct RegisteredAgent {
    pub agent_id: String,
    pub name: String,
    pub tokens: i64,
    pub money: i64,
    pub skills: Vec<String>,
    pub reputation: f32,
    pub phase: i32, // protobuf AgentPhase as i32
}

/// Per-agent message inbox backed by an mpsc channel.
#[derive(Debug)]
pub struct AgentInbox {
    pub sender: mpsc::Sender<RouterMessage>,
    pub agent_info: RegisteredAgent,
}

/// Rate-limit counter: how many messages an agent has sent in the current tick.
#[derive(Debug, Default)]
pub struct RateLimitCounter {
    pub count: usize,
    pub current_tick: u64,
}

/// Configuration for the A2A router.
#[derive(Debug, Clone)]
pub struct A2AConfig {
    /// Channel capacity per agent inbox.
    pub inbox_capacity: usize,
    /// Message TTL in milliseconds. Messages older than this are dropped.
    pub message_ttl_ms: i64,
    /// Maximum messages an agent can send per tick.
    pub max_messages_per_tick: usize,
    /// Maximum message payload size in bytes.
    pub max_message_size_bytes: usize,
}

impl Default for A2AConfig {
    fn default() -> Self {
        Self {
            inbox_capacity: 256,
            message_ttl_ms: 30_000,
            max_messages_per_tick: 10,
            max_message_size_bytes: 64 * 1024, // 64 KB from genesis.yaml
        }
    }
}

/// Error type for router operations.
#[derive(Debug)]
pub enum RouterError {
    AgentNotFound(String),
    AgentAlreadyRegistered(String),
    MessageExpired(String),
    RateLimitExceeded { agent_id: String, limit: usize },
    MessageTooLarge { size: usize, max: usize },
    InboxFull(String),
    InvalidMessage(String),
}

impl std::fmt::Display for RouterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RouterError::AgentNotFound(id) => write!(f, "agent not found: {}", id),
            RouterError::AgentAlreadyRegistered(id) => write!(f, "agent already registered: {}", id),
            RouterError::MessageExpired(id) => write!(f, "message {} has expired", id),
            RouterError::RateLimitExceeded { agent_id, limit } => {
                write!(f, "agent {} exceeded rate limit of {} messages/tick", agent_id, limit)
            }
            RouterError::MessageTooLarge { size, max } => {
                write!(f, "message payload too large: {} bytes (max {})", size, max)
            }
            RouterError::InboxFull(id) => write!(f, "inbox full for agent: {}", id),
            RouterError::InvalidMessage(msg) => write!(f, "invalid message: {}", msg),
        }
    }
}

impl From<RouterError> for Status {
    fn from(err: RouterError) -> Self {
        match &err {
            RouterError::AgentNotFound(_) => Status::not_found(err.to_string()),
            RouterError::AgentAlreadyRegistered(_) => Status::already_exists(err.to_string()),
            RouterError::MessageExpired(_) => Status::invalid_argument(err.to_string()),
            RouterError::RateLimitExceeded { .. } => Status::resource_exhausted(err.to_string()),
            RouterError::MessageTooLarge { .. } => Status::invalid_argument(err.to_string()),
            RouterError::InboxFull(_) => Status::resource_exhausted(err.to_string()),
            RouterError::InvalidMessage(_) => Status::invalid_argument(err.to_string()),
        }
    }
}

/// The A2A message router.
///
/// Manages agent registration, per-agent message queues, directed and broadcast
/// delivery, TTL-based expiration, and per-tick rate limiting.
pub struct A2ARouter {
    config: A2AConfig,
    agents: RwLock<HashMap<String, AgentInbox>>,
    rate_limits: Mutex<HashMap<String, RateLimitCounter>>,
    event_bus: Option<Arc<EventBus>>,
}

impl A2ARouter {
    /// Create a new router with the given config.
    pub fn new(config: A2AConfig) -> Self {
        Self {
            config,
            agents: RwLock::new(HashMap::new()),
            rate_limits: Mutex::new(HashMap::new()),
            event_bus: None,
        }
    }

    /// Create a new router with event bus integration.
    pub fn with_event_bus(config: A2AConfig, event_bus: Arc<EventBus>) -> Self {
        Self {
            config,
            agents: RwLock::new(HashMap::new()),
            rate_limits: Mutex::new(HashMap::new()),
            event_bus: Some(event_bus),
        }
    }

    /// Register a new agent. Creates an inbox channel for the agent.
    /// Returns the receiver end of the inbox channel.
    pub async fn register(
        &self,
        agent: RegisteredAgent,
    ) -> Result<mpsc::Receiver<RouterMessage>, RouterError> {
        let mut agents = self.agents.write().await;
        if agents.contains_key(&agent.agent_id) {
            return Err(RouterError::AgentAlreadyRegistered(agent.agent_id.clone()));
        }

        let (tx, rx) = mpsc::channel(self.config.inbox_capacity);
        agents.insert(
            agent.agent_id.clone(),
            AgentInbox {
                sender: tx,
                agent_info: agent.clone(),
            },
        );

        if let Some(ref bus) = self.event_bus {
            bus.emit(crate::world::WorldEvent::AgentSpawned {
                agent_id: agent.agent_id.clone(),
                name: agent.name.clone(),
            });
        }

        Ok(rx)
    }

    /// Deregister an agent, removing their inbox.
    pub async fn deregister(&self, agent_id: &str) -> Result<(), RouterError> {
        let mut agents = self.agents.write().await;
        if agents.remove(agent_id).is_none() {
            return Err(RouterError::AgentNotFound(agent_id.to_string()));
        }

        // Clean up rate limit counter
        let mut limits = self.rate_limits.lock().await;
        limits.remove(agent_id);

        Ok(())
    }

    /// Route a message to its destination(s).
    ///
    /// - If `to_agent` is non-empty, performs directed delivery.
    /// - If `to_agent` is empty, broadcasts to all registered agents except the sender.
    ///
    /// Validates TTL, rate limits, and payload size before routing.
    pub async fn route_message(&self, msg: RouterMessage, now_ms: i64, current_tick: u64) -> Result<(), RouterError> {
        // Validate payload size
        if msg.payload.len() > self.config.max_message_size_bytes {
            return Err(RouterError::MessageTooLarge {
                size: msg.payload.len(),
                max: self.config.max_message_size_bytes,
            });
        }

        // Validate sender is registered
        let agents = self.agents.read().await;
        if !agents.contains_key(&msg.from_agent) {
            return Err(RouterError::AgentNotFound(msg.from_agent.clone()));
        }

        // Check TTL expiration
        if now_ms - msg.timestamp > self.config.message_ttl_ms {
            return Err(RouterError::MessageExpired(msg.id.clone()));
        }

        // Check rate limit
        drop(agents); // release read lock before acquiring rate_limits mutex
        {
            let mut limits = self.rate_limits.lock().await;
            let counter = limits.entry(msg.from_agent.clone()).or_default();

            // Reset counter if we're in a new tick
            if counter.current_tick != current_tick {
                counter.count = 0;
                counter.current_tick = current_tick;
            }

            if counter.count >= self.config.max_messages_per_tick {
                return Err(RouterError::RateLimitExceeded {
                    agent_id: msg.from_agent.clone(),
                    limit: self.config.max_messages_per_tick,
                });
            }
            counter.count += 1;
        }

        // Route the message
        let agents = self.agents.read().await;
        if msg.to_agent.is_empty() {
            // Broadcast to all agents except sender
            for (id, inbox) in agents.iter() {
                if id != &msg.from_agent {
                    let _ = inbox.sender.send(msg.clone()).await;
                }
            }
        } else {
            // Directed delivery
            match agents.get(&msg.to_agent) {
                Some(inbox) => {
                    let _ = inbox.sender.send(msg).await;
                }
                None => {
                    return Err(RouterError::AgentNotFound(msg.to_agent.clone()));
                }
            }
        }

        Ok(())
    }

    /// Discover agents matching optional capability filters.
    ///
    /// If `filter_skills` is non-empty, only agents possessing at least one
    /// of the listed skills are returned.
    pub async fn discover(
        &self,
        _requesting_agent: &str,
        filter_skills: &[String],
    ) -> Vec<RegisteredAgent> {
        let agents = self.agents.read().await;
        agents
            .values()
            .filter(|inbox| {
                if filter_skills.is_empty() {
                    return true;
                }
                inbox
                    .agent_info
                    .skills
                    .iter()
                    .any(|s| filter_skills.contains(s))
            })
            .map(|inbox| inbox.agent_info.clone())
            .collect()
    }

    /// Get info about a specific agent.
    pub async fn get_agent(&self, agent_id: &str) -> Option<RegisteredAgent> {
        let agents = self.agents.read().await;
        agents.get(agent_id).map(|inbox| inbox.agent_info.clone())
    }

    /// List all registered agents.
    pub async fn list_agents(&self) -> Vec<RegisteredAgent> {
        let agents = self.agents.read().await;
        agents
            .values()
            .map(|inbox| inbox.agent_info.clone())
            .collect()
    }

    /// Reset rate limit counters for a new tick. Called by the tick scheduler.
    pub async fn reset_rate_limits(&self, new_tick: u64) {
        let mut limits = self.rate_limits.lock().await;
        for counter in limits.values_mut() {
            if counter.current_tick != new_tick {
                counter.count = 0;
                counter.current_tick = new_tick;
            }
        }
    }

    /// Purge expired messages from all inboxes.
    /// This is a maintenance function called periodically.
    /// Note: TTL is checked at route time, so this is a secondary cleanup
    /// for any messages that may have aged in transit.
    pub async fn purge_expired(&self, now_ms: i64) -> usize {
        // Since we use mpsc channels, we can't directly inspect/remove messages.
        // The TTL check happens at route_message time, which is the primary defense.
        // This method is provided for future use with a different queue implementation.
        let _ = now_ms;
        0
    }

    /// Get the number of registered agents.
    pub async fn agent_count(&self) -> usize {
        self.agents.read().await.len()
    }

    /// Get a reference to the config.
    pub fn config(&self) -> &A2AConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn make_agent(id: &str, name: &str, skills: Vec<&str>) -> RegisteredAgent {
        RegisteredAgent {
            agent_id: id.to_string(),
            name: name.to_string(),
            tokens: 1000,
            money: 0,
            skills: skills.into_iter().map(|s| s.to_string()).collect(),
            reputation: 0.0,
            phase: 2, // ADULT
        }
    }

    fn make_message(from: &str, to: &str, ts: i64) -> RouterMessage {
        RouterMessage {
            id: Uuid::new_v4().to_string(),
            from_agent: from.to_string(),
            to_agent: to.to_string(),
            message_type: 4, // INFORM
            payload: b"hello".to_vec(),
            timestamp: ts,
            signature: String::new(),
            nonce: Uuid::new_v4().to_string(),
        }
    }

    #[tokio::test]
    async fn test_register_agent() {
        let router = A2ARouter::new(A2AConfig::default());
        let agent = make_agent("a1", "Alice", vec!["mining"]);

        let rx = router.register(agent).await.unwrap();
        assert_eq!(router.agent_count().await, 1);
        drop(rx); // receiver must live until we're done
    }

    #[tokio::test]
    async fn test_register_duplicate_fails() {
        let router = A2ARouter::new(A2AConfig::default());
        let agent1 = make_agent("a1", "Alice", vec![]);
        let agent2 = make_agent("a1", "Alice-Duplicate", vec![]);

        router.register(agent1).await.unwrap();
        let result = router.register(agent2).await;
        assert!(matches!(result, Err(RouterError::AgentAlreadyRegistered(_))));
    }

    #[tokio::test]
    async fn test_deregister_agent() {
        let router = A2ARouter::new(A2AConfig::default());
        let agent = make_agent("a1", "Alice", vec![]);
        let rx = router.register(agent).await.unwrap();

        router.deregister("a1").await.unwrap();
        assert_eq!(router.agent_count().await, 0);
        drop(rx);
    }

    #[tokio::test]
    async fn test_deregister_nonexistent_fails() {
        let router = A2ARouter::new(A2AConfig::default());
        let result = router.deregister("nonexistent").await;
        assert!(matches!(result, Err(RouterError::AgentNotFound(_))));
    }

    #[tokio::test]
    async fn test_directed_message_delivery() {
        let router = A2ARouter::new(A2AConfig::default());
        let agent_a = make_agent("a1", "Alice", vec![]);
        let agent_b = make_agent("a2", "Bob", vec![]);

        let mut rx_a = router.register(agent_a).await.unwrap();
        let mut rx_b = router.register(agent_b).await.unwrap();

        let msg = make_message("a1", "a2", 1000);
        router.route_message(msg.clone(), 1000, 1).await.unwrap();

        // Bob should receive the message
        let received = rx_b.try_recv().unwrap();
        assert_eq!(received.from_agent, "a1");
        assert_eq!(received.to_agent, "a2");
        assert_eq!(received.payload, b"hello");

        // Alice should NOT receive the message
        assert!(rx_a.try_recv().is_err());
    }

    #[tokio::test]
    async fn test_broadcast_message_delivery() {
        let router = A2ARouter::new(A2AConfig::default());
        let agent_a = make_agent("a1", "Alice", vec![]);
        let agent_b = make_agent("a2", "Bob", vec![]);
        let agent_c = make_agent("a3", "Charlie", vec![]);

        let mut rx_a = router.register(agent_a).await.unwrap();
        let mut rx_b = router.register(agent_b).await.unwrap();
        let mut rx_c = router.register(agent_c).await.unwrap();

        // to_agent is empty = broadcast
        let msg = make_message("a1", "", 1000);
        router.route_message(msg.clone(), 1000, 1).await.unwrap();

        // Bob and Charlie should receive the message
        let recv_b = rx_b.try_recv().unwrap();
        assert_eq!(recv_b.from_agent, "a1");
        assert!(recv_b.to_agent.is_empty());

        let recv_c = rx_c.try_recv().unwrap();
        assert_eq!(recv_c.from_agent, "a1");

        // Alice should NOT receive her own broadcast
        assert!(rx_a.try_recv().is_err());
    }

    #[tokio::test]
    async fn test_message_ttl_expired() {
        let router = A2ARouter::new(A2AConfig {
            message_ttl_ms: 5000,
            ..Default::default()
        });
        let agent_a = make_agent("a1", "Alice", vec![]);
        let agent_b = make_agent("a2", "Bob", vec![]);

        let _rx_a = router.register(agent_a).await.unwrap();
        let _rx_b = router.register(agent_b).await.unwrap();

        // Message was sent at ts=1000, now is 7000 -> TTL=5000, expired
        let msg = make_message("a1", "a2", 1000);
        let result = router.route_message(msg, 7000, 1).await;
        assert!(matches!(result, Err(RouterError::MessageExpired(_))));
    }

    #[tokio::test]
    async fn test_message_ttl_not_expired() {
        let router = A2ARouter::new(A2AConfig {
            message_ttl_ms: 5000,
            ..Default::default()
        });
        let agent_a = make_agent("a1", "Alice", vec![]);
        let agent_b = make_agent("a2", "Bob", vec![]);

        let _rx_a = router.register(agent_a).await.unwrap();
        let mut rx_b = router.register(agent_b).await.unwrap();

        // Message sent at ts=1000, now is 4000 -> TTL=5000, not expired
        let msg = make_message("a1", "a2", 1000);
        router.route_message(msg, 4000, 1).await.unwrap();
        assert!(rx_b.try_recv().is_ok());
    }

    #[tokio::test]
    async fn test_rate_limiting_per_tick() {
        let router = A2ARouter::new(A2AConfig {
            max_messages_per_tick: 2,
            ..Default::default()
        });
        let agent_a = make_agent("a1", "Alice", vec![]);
        let agent_b = make_agent("a2", "Bob", vec![]);

        let _rx_a = router.register(agent_a).await.unwrap();
        let mut rx_b = router.register(agent_b).await.unwrap();

        // Send 2 messages — should succeed
        let msg1 = make_message("a1", "a2", 1000);
        let msg2 = make_message("a1", "a2", 1001);
        router.route_message(msg1, 1000, 1).await.unwrap();
        router.route_message(msg2, 1000, 1).await.unwrap();

        // 3rd message in same tick — should be rate-limited
        let msg3 = make_message("a1", "a2", 1002);
        let result = router.route_message(msg3, 1000, 1).await;
        assert!(matches!(result, Err(RouterError::RateLimitExceeded { .. })));

        // Verify only 2 messages were delivered
        assert!(rx_b.try_recv().is_ok());
        assert!(rx_b.try_recv().is_ok());
        assert!(rx_b.try_recv().is_err());
    }

    #[tokio::test]
    async fn test_rate_limit_resets_on_new_tick() {
        let router = A2ARouter::new(A2AConfig {
            max_messages_per_tick: 2,
            ..Default::default()
        });
        let agent_a = make_agent("a1", "Alice", vec![]);
        let agent_b = make_agent("a2", "Bob", vec![]);

        let _rx_a = router.register(agent_a).await.unwrap();
        let mut rx_b = router.register(agent_b).await.unwrap();

        // Use up rate limit in tick 1
        let msg1 = make_message("a1", "a2", 1000);
        let msg2 = make_message("a1", "a2", 1001);
        router.route_message(msg1, 1000, 1).await.unwrap();
        router.route_message(msg2, 1000, 1).await.unwrap();

        // Tick 2 — rate limit should reset
        let msg3 = make_message("a1", "a2", 2000);
        router.route_message(msg3, 2000, 2).await.unwrap();
        assert!(rx_b.try_recv().is_ok());
        assert!(rx_b.try_recv().is_ok());
        assert!(rx_b.try_recv().is_ok());
    }

    #[tokio::test]
    async fn test_message_too_large() {
        let router = A2ARouter::new(A2AConfig {
            max_message_size_bytes: 10,
            ..Default::default()
        });
        let agent_a = make_agent("a1", "Alice", vec![]);
        let agent_b = make_agent("a2", "Bob", vec![]);

        let _rx_a = router.register(agent_a).await.unwrap();
        let _rx_b = router.register(agent_b).await.unwrap();

        let mut msg = make_message("a1", "a2", 1000);
        msg.payload = vec![0u8; 100]; // 100 bytes, exceeds 10 byte limit
        let result = router.route_message(msg, 1000, 1).await;
        assert!(matches!(result, Err(RouterError::MessageTooLarge { .. })));
    }

    #[tokio::test]
    async fn test_send_from_unregistered_agent() {
        let router = A2ARouter::new(A2AConfig::default());
        let msg = make_message("ghost", "anyone", 1000);
        let result = router.route_message(msg, 1000, 1).await;
        assert!(matches!(result, Err(RouterError::AgentNotFound(_))));
    }

    #[tokio::test]
    async fn test_send_to_nonexistent_agent() {
        let router = A2ARouter::new(A2AConfig::default());
        let agent = make_agent("a1", "Alice", vec![]);
        let _rx = router.register(agent).await.unwrap();

        let msg = make_message("a1", "nonexistent", 1000);
        let result = router.route_message(msg, 1000, 1).await;
        assert!(matches!(result, Err(RouterError::AgentNotFound(_))));
    }

    #[tokio::test]
    async fn test_discover_all_agents() {
        let router = A2ARouter::new(A2AConfig::default());
        router.register(make_agent("a1", "Alice", vec!["mining"])).await.unwrap();
        router.register(make_agent("a2", "Bob", vec!["trading"])).await.unwrap();

        let discovered = router.discover("a1", &[]).await;
        assert_eq!(discovered.len(), 2);
    }

    #[tokio::test]
    async fn test_discover_filter_by_skill() {
        let router = A2ARouter::new(A2AConfig::default());
        router.register(make_agent("a1", "Alice", vec!["mining"])).await.unwrap();
        router.register(make_agent("a2", "Bob", vec!["trading"])).await.unwrap();
        router.register(make_agent("a3", "Charlie", vec!["mining", "trading"])).await.unwrap();

        let filter = vec!["mining".to_string()];
        let discovered = router.discover("a1", &filter).await;
        // Alice and Charlie have mining
        assert_eq!(discovered.len(), 2);
        assert!(discovered.iter().all(|a| a.skills.contains(&"mining".to_string())));
    }

    #[tokio::test]
    async fn test_get_agent_info() {
        let router = A2ARouter::new(A2AConfig::default());
        router.register(make_agent("a1", "Alice", vec!["mining"])).await.unwrap();

        let info = router.get_agent("a1").await.unwrap();
        assert_eq!(info.name, "Alice");
        assert_eq!(info.tokens, 1000);
    }

    #[tokio::test]
    async fn test_get_nonexistent_agent() {
        let router = A2ARouter::new(A2AConfig::default());
        let result = router.get_agent("ghost").await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_reset_rate_limits() {
        let router = A2ARouter::new(A2AConfig {
            max_messages_per_tick: 1,
            ..Default::default()
        });
        router.register(make_agent("a1", "Alice", vec![])).await.unwrap();
        router.register(make_agent("a2", "Bob", vec![])).await.unwrap();

        // Exhaust rate limit
        let msg = make_message("a1", "a2", 1000);
        router.route_message(msg, 1000, 1).await.unwrap();

        // Next message should fail
        let msg2 = make_message("a1", "a2", 1001);
        assert!(router.route_message(msg2, 1001, 1).await.is_err());

        // Reset
        router.reset_rate_limits(2).await;

        // Now it should succeed (new tick)
        let msg3 = make_message("a1", "a2", 2000);
        assert!(router.route_message(msg3, 2000, 2).await.is_ok());
    }

    #[tokio::test]
    async fn test_two_agents_exchange_messages() {
        let router = A2ARouter::new(A2AConfig::default());
        let agent_a = make_agent("alice", "Alice", vec![]);
        let agent_b = make_agent("bob", "Bob", vec![]);

        let mut rx_a = router.register(agent_a).await.unwrap();
        let mut rx_b = router.register(agent_b).await.unwrap();

        // Alice -> Bob
        let msg_ab = RouterMessage {
            id: "msg-1".to_string(),
            from_agent: "alice".to_string(),
            to_agent: "bob".to_string(),
            message_type: 1, // PROPOSE
            payload: b"Want to trade?".to_vec(),
            timestamp: 1000,
            signature: String::new(),
            nonce: "n1".to_string(),
        };
        router.route_message(msg_ab, 1000, 1).await.unwrap();

        let recv = rx_b.try_recv().unwrap();
        assert_eq!(recv.from_agent, "alice");
        assert_eq!(recv.to_agent, "bob");
        assert_eq!(recv.payload, b"Want to trade?");

        // Bob -> Alice
        let msg_ba = RouterMessage {
            id: "msg-2".to_string(),
            from_agent: "bob".to_string(),
            to_agent: "alice".to_string(),
            message_type: 2, // ACCEPT
            payload: b"Sure, let's trade!".to_vec(),
            timestamp: 1500,
            signature: String::new(),
            nonce: "n2".to_string(),
        };
        router.route_message(msg_ba, 1500, 1).await.unwrap();

        let recv = rx_a.try_recv().unwrap();
        assert_eq!(recv.from_agent, "bob");
        assert_eq!(recv.to_agent, "alice");
        assert_eq!(recv.payload, b"Sure, let's trade!");
    }

    #[tokio::test]
    async fn test_list_agents() {
        let router = A2ARouter::new(A2AConfig::default());
        router.register(make_agent("a1", "Alice", vec![])).await.unwrap();
        router.register(make_agent("a2", "Bob", vec![])).await.unwrap();

        let agents = router.list_agents().await;
        assert_eq!(agents.len(), 2);
    }

    #[tokio::test]
    async fn test_deregister_cleans_up_rate_limits() {
        let router = A2ARouter::new(A2AConfig {
            max_messages_per_tick: 1,
            ..Default::default()
        });
        let rx = router.register(make_agent("a1", "Alice", vec![])).await.unwrap();
        router.register(make_agent("a2", "Bob", vec![])).await.unwrap();

        // Send a message to create a rate limit entry
        let msg = make_message("a1", "a2", 1000);
        router.route_message(msg, 1000, 1).await.unwrap();

        router.deregister("a1").await.unwrap();
        drop(rx);

        // The rate limit entry should be cleaned up
        let limits = router.rate_limits.lock().await;
        assert!(!limits.contains_key("a1"));
    }
}
