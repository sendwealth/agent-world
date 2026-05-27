//! gRPC connection pool for outbound connections from the world engine.
//!
//! When the world engine needs to push perception data to agents or forward
//! messages, it reuses connections from this pool rather than creating new
//! channels each time.  This reduces TCP handshake and HTTP/2 setup overhead,
//! especially critical at 50–100 agent scale.
//!
//! The pool is keyed by `agent_id` and stores one channel per agent endpoint.
//! Channels are lazily created on first use and automatically evicted when an
//! agent deregisters.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;
use tonic::transport::{Channel, Endpoint};

use super::registry::RegisteredAgent;

/// A pooled gRPC channel bound to a specific agent endpoint.
#[derive(Clone)]
pub struct PooledChannel {
    /// The agent ID this channel connects to.
    pub agent_id: String,
    /// The underlying tonic channel (reused across calls).
    pub channel: Channel,
}

/// Connection pool that manages gRPC channels to agent runtimes.
///
/// Each agent gets at most one channel.  Channels are created on first access
/// and removed when the agent deregisters or the channel becomes unhealthy.
pub struct ConnectionPool {
    channels: Arc<RwLock<HashMap<String, PooledChannel>>>,
    /// Default port for agent runtimes if only host is known.
    default_port: u16,
}

impl ConnectionPool {
    /// Create a new connection pool.
    pub fn new(default_port: u16) -> Self {
        Self {
            channels: Arc::new(RwLock::new(HashMap::new())),
            default_port,
        }
    }

    /// Get or create a channel for the given agent.
    ///
    /// If a channel already exists for this agent, returns the cached one.
    /// Otherwise creates a new channel from the agent's address info.
    pub async fn get_or_connect(
        &self,
        agent: &RegisteredAgent,
    ) -> Result<PooledChannel, tonic::transport::Error> {
        // Fast path: check read lock first
        {
            let channels = self.channels.read().await;
            if let Some(pc) = channels.get(&agent.agent_id) {
                return Ok(pc.clone());
            }
        }

        // Slow path: create channel under write lock
        let mut channels = self.channels.write().await;
        // Double-check after acquiring write lock
        if let Some(pc) = channels.get(&agent.agent_id) {
            return Ok(pc.clone());
        }

        let addr = format!("http://{}:{}", agent.agent_id, self.default_port);
        let channel = Endpoint::from_shared(addr)?.connect_lazy();
        let pooled = PooledChannel {
            agent_id: agent.agent_id.clone(),
            channel,
        };
        channels.insert(agent.agent_id.clone(), pooled.clone());
        Ok(pooled)
    }

    /// Get an existing channel without creating a new one.
    pub async fn get(&self, agent_id: &str) -> Option<PooledChannel> {
        self.channels.read().await.get(agent_id).cloned()
    }

    /// Remove a channel from the pool (called on deregister).
    pub async fn remove(&self, agent_id: &str) {
        self.channels.write().await.remove(agent_id);
    }

    /// Return the number of currently pooled channels.
    pub async fn len(&self) -> usize {
        self.channels.read().await.len()
    }

    /// Return true if the pool is empty.
    pub async fn is_empty(&self) -> bool {
        self.channels.read().await.is_empty()
    }

    /// Get channel references for all connected agents.
    /// Used by batch operations to push perception data to all agents at once.
    pub async fn all_channels(&self) -> Vec<PooledChannel> {
        self.channels.read().await.values().cloned().collect()
    }
}

impl Default for ConnectionPool {
    fn default() -> Self {
        Self::new(50052) // Default agent runtime gRPC port
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_pool() -> ConnectionPool {
        ConnectionPool::new(50052)
    }

    #[allow(dead_code)]
    fn make_agent(id: &str) -> RegisteredAgent {
        RegisteredAgent {
            agent_id: id.to_string(),
            name: id.to_string(),
            capabilities: vec![],
            public_key: String::new(),
            tokens: 0,
            money: 0,
            skills: vec![],
            reputation: 0.0,
            phase: 0,
            last_seen: 0,
        }
    }

    #[tokio::test]
    async fn pool_starts_empty() {
        let pool = make_pool();
        assert!(pool.is_empty().await);
        assert_eq!(pool.len().await, 0);
    }

    #[tokio::test]
    async fn remove_nonexistent_is_noop() {
        let pool = make_pool();
        pool.remove("nonexistent").await;
        assert_eq!(pool.len().await, 0);
    }

    #[tokio::test]
    async fn get_nonexistent_returns_none() {
        let pool = make_pool();
        assert!(pool.get("nonexistent").await.is_none());
    }

    #[tokio::test]
    async fn all_channels_empty() {
        let pool = make_pool();
        assert!(pool.all_channels().await.is_empty());
    }
}
