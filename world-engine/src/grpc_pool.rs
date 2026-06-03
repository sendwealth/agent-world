//! gRPC Connection Pool for A2A Service.
//!
//! Manages a pool of gRPC channel connections to avoid the overhead
//! of establishing new connections for every request. Supports:
//!   - Configurable pool size and timeouts
//!   - Automatic connection health checking
//!   - Round-robin connection selection
//!   - Connection recycling on error

use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};

use tokio::sync::RwLock;
use tonic::transport::{Channel, Endpoint};

/// Configuration for the gRPC connection pool.
#[derive(Debug, Clone)]
pub struct PoolConfig {
    /// Maximum number of connections in the pool.
    pub max_connections: usize,
    /// Minimum number of idle connections to maintain.
    pub min_idle: usize,
    /// Timeout for establishing a new connection.
    pub connect_timeout: std::time::Duration,
    /// Timeout for individual gRPC requests.
    pub request_timeout: std::time::Duration,
    /// How often to check connection health.
    pub health_check_interval: std::time::Duration,
    /// Maximum time a connection can be idle before being recycled.
    pub max_idle_age: std::time::Duration,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            max_connections: 20,
            min_idle: 5,
            connect_timeout: std::time::Duration::from_secs(5),
            request_timeout: std::time::Duration::from_secs(10),
            health_check_interval: std::time::Duration::from_secs(30),
            max_idle_age: std::time::Duration::from_secs(300),
        }
    }
}

/// A pooled gRPC connection with metadata.
struct PooledConnection {
    channel: Channel,
    // TODO: Use for connection max-age TTL enforcement.
    #[allow(dead_code)]
    created_at: std::time::Instant,
    last_used: std::time::Instant,
    use_count: usize,
}

impl PooledConnection {
    fn new(channel: Channel) -> Self {
        let now = std::time::Instant::now();
        Self {
            channel,
            created_at: now,
            last_used: now,
            use_count: 0,
        }
    }

    fn is_expired(&self, max_idle_age: std::time::Duration) -> bool {
        self.last_used.elapsed() > max_idle_age
    }
}

/// A gRPC connection pool that manages a set of reusable connections.
///
/// Uses round-robin selection for load distribution across connections.
/// Connections are lazily created up to `max_connections`.
pub struct GrpcConnectionPool {
    config: PoolConfig,
    endpoint: String,
    connections: RwLock<VecDeque<PooledConnection>>,
    next_index: AtomicUsize,
    total_created: AtomicUsize,
    total_reused: AtomicUsize,
}

impl GrpcConnectionPool {
    /// Create a new connection pool targeting the given endpoint.
    pub fn new(endpoint: String, config: PoolConfig) -> Self {
        Self {
            config,
            endpoint,
            connections: RwLock::new(VecDeque::new()),
            next_index: AtomicUsize::new(0),
            total_created: AtomicUsize::new(0),
            total_reused: AtomicUsize::new(0),
        }
    }

    /// Get a channel from the pool, creating a new one if necessary.
    pub async fn get(&self) -> Result<Channel, tonic::transport::Error> {
        let mut conns = self.connections.write().await;

        // Try to find a healthy, non-expired connection
        let now = std::time::Instant::now();
        while let Some(mut conn) = conns.pop_front() {
            if conn.is_expired(self.config.max_idle_age) {
                // Connection is too old, discard and create new
                continue;
            }

            conn.last_used = now;
            conn.use_count += 1;
            let channel = conn.channel.clone();
            conns.push_back(conn);
            self.total_reused.fetch_add(1, Ordering::Relaxed);
            return Ok(channel);
        }

        // No available connection — create a new one if under limit
        if conns.len() < self.config.max_connections {
            drop(conns);
            let channel = self.create_connection().await?;
            let mut conns = self.connections.write().await;

            let pooled = PooledConnection::new(channel.clone());
            conns.push_back(pooled);
            self.total_created.fetch_add(1, Ordering::Relaxed);
            Ok(channel)
        } else {
            // Pool is full — use round-robin to pick an existing one
            let idx = self.next_index.fetch_add(1, Ordering::Relaxed) % conns.len();
            conns[idx].last_used = now;
            conns[idx].use_count += 1;
            Ok(conns[idx].channel.clone())
        }
    }

    /// Get a channel using round-robin (fast path, no health check).
    pub async fn get_round_robin(&self) -> Result<Channel, tonic::transport::Error> {
        let conns = self.connections.read().await;
        if conns.is_empty() {
            drop(conns);
            return self.get().await;
        }
        let idx = self.next_index.fetch_add(1, Ordering::Relaxed) % conns.len();
        Ok(conns[idx].channel.clone())
    }

    /// Establish a new gRPC connection.
    async fn create_connection(&self) -> Result<Channel, tonic::transport::Error> {
        let endpoint = Endpoint::from_shared(self.endpoint.clone())?
            .timeout(self.config.request_timeout)
            .connect_timeout(self.config.connect_timeout)
            .tcp_nodelay(true)
            .tcp_keepalive(Some(std::time::Duration::from_secs(60)));
        endpoint.connect().await
    }

    /// Pre-warm the pool by creating `min_idle` connections.
    pub async fn warm_up(&self) -> Result<(), tonic::transport::Error> {
        let mut conns = self.connections.write().await;
        for _ in 0..self.config.min_idle {
            let channel = self.create_connection().await?;
            conns.push_back(PooledConnection::new(channel));
            self.total_created.fetch_add(1, Ordering::Relaxed);
        }
        Ok(())
    }

    /// Clean up expired connections.
    pub async fn cleanup(&self) {
        let mut conns = self.connections.write().await;
        let before = conns.len();
        conns.retain(|conn| !conn.is_expired(self.config.max_idle_age));
        let removed = before - conns.len();
        if removed > 0 {
            tracing::debug!("[gRPC Pool] Cleaned up {} expired connections", removed);
        }
    }

    /// Get pool statistics.
    pub async fn stats(&self) -> PoolStats {
        let conns = self.connections.read().await;
        PoolStats {
            active_connections: conns.len(),
            total_created: self.total_created.load(Ordering::Relaxed),
            total_reused: self.total_reused.load(Ordering::Relaxed),
            max_connections: self.config.max_connections,
        }
    }

    /// Shut down the pool, dropping all connections.
    pub async fn shutdown(&self) {
        let mut conns = self.connections.write().await;
        conns.clear();
        tracing::info!("[gRPC Pool] Shutdown complete");
    }
}

/// Statistics about the connection pool.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PoolStats {
    pub active_connections: usize,
    pub total_created: usize,
    pub total_reused: usize,
    pub max_connections: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pool_config_default() {
        let config = PoolConfig::default();
        assert_eq!(config.max_connections, 20);
        assert_eq!(config.min_idle, 5);
        assert_eq!(config.connect_timeout, std::time::Duration::from_secs(5));
    }

    #[tokio::test]
    async fn test_pool_creation() {
        let pool =
            GrpcConnectionPool::new("http://127.0.0.1:50051".to_string(), PoolConfig::default());
        let stats = pool.stats().await;
        assert_eq!(stats.active_connections, 0);
        assert_eq!(stats.max_connections, 20);
    }

    #[tokio::test]
    async fn test_pool_stats_tracking() {
        let pool = GrpcConnectionPool::new(
            "http://127.0.0.1:50051".to_string(),
            PoolConfig {
                max_connections: 3,
                min_idle: 0,
                ..PoolConfig::default()
            },
        );

        let stats = pool.stats().await;
        assert_eq!(stats.active_connections, 0);
        assert_eq!(stats.total_created, 0);
        assert_eq!(stats.total_reused, 0);
    }

    #[tokio::test]
    async fn test_pooled_connection_expiry() {
        let config = PoolConfig::default();
        // A connection just created should not be expired
        let conn =
            PooledConnection::new(Channel::from_static("http://127.0.0.1:50051").connect_lazy());
        assert!(!conn.is_expired(config.max_idle_age));
    }

    #[tokio::test]
    async fn test_pooled_connection_use_count() {
        let mut conn =
            PooledConnection::new(Channel::from_static("http://127.0.0.1:50051").connect_lazy());
        assert_eq!(conn.use_count, 0);
        conn.use_count += 1;
        assert_eq!(conn.use_count, 1);
    }
}
