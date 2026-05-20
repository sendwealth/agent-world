use std::pin::Pin;
use std::sync::Arc;

use tokio_stream::{Stream, StreamExt};
use tonic::{Request, Response, Status};

use crate::agentworld::a2a::v1::{
    a2a_service_server::A2aService, A2aMessage, AgentInfo, DeregisterAgentRequest,
    DeregisterAgentResponse, DiscoverRequest, DiscoverResponse, HeartbeatRequest,
    HeartbeatResponse, MessageAck, RegisterAgentRequest, RegisterAgentResponse,
};

use super::registry::AgentRegistry;
use super::router::MessageRouter;

/// Maximum number of messages that can be sent in a single batch.
const MAX_BATCH_SIZE: usize = 256;

/// Concrete A2A gRPC service implementation.
#[derive(Clone)]
pub struct A2aServiceImpl {
    registry: Arc<AgentRegistry>,
    router: Arc<MessageRouter>,
}

impl A2aServiceImpl {
    pub fn new(registry: Arc<AgentRegistry>, router: Arc<MessageRouter>) -> Self {
        Self { registry, router }
    }
}

#[tonic::async_trait]
impl A2aService for A2aServiceImpl {
    async fn register_agent(
        &self,
        request: Request<RegisterAgentRequest>,
    ) -> Result<Response<RegisterAgentResponse>, Status> {
        let req = request.into_inner();

        if req.agent_id.is_empty() {
            return Ok(Response::new(RegisterAgentResponse {
                success: false,
                error: "agent_id is required".into(),
                timestamp: 0,
            }));
        }
        if req.name.is_empty() {
            return Ok(Response::new(RegisterAgentResponse {
                success: false,
                error: "name is required".into(),
                timestamp: 0,
            }));
        }

        let now = chrono::Utc::now().timestamp();
        self.registry
            .register(req.agent_id, req.name, req.capabilities, req.public_key)
            .await;

        Ok(Response::new(RegisterAgentResponse {
            success: true,
            error: String::new(),
            timestamp: now,
        }))
    }

    async fn heartbeat(
        &self,
        request: Request<HeartbeatRequest>,
    ) -> Result<Response<HeartbeatResponse>, Status> {
        let req = request.into_inner();
        let now = chrono::Utc::now().timestamp();

        if req.agent_id.is_empty() {
            return Err(Status::invalid_argument("agent_id is required"));
        }

        let alive = self.registry.heartbeat(&req.agent_id).await;

        Ok(Response::new(HeartbeatResponse {
            alive,
            server_time: now,
        }))
    }

    async fn deregister_agent(
        &self,
        request: Request<DeregisterAgentRequest>,
    ) -> Result<Response<DeregisterAgentResponse>, Status> {
        let req = request.into_inner();

        if req.agent_id.is_empty() {
            return Ok(Response::new(DeregisterAgentResponse {
                success: false,
                error: "agent_id is required".into(),
            }));
        }

        // Close any active message stream
        self.router.close_stream(&req.agent_id).await;

        let removed = self.registry.deregister(&req.agent_id).await;
        if removed {
            Ok(Response::new(DeregisterAgentResponse {
                success: true,
                error: String::new(),
            }))
        } else {
            Ok(Response::new(DeregisterAgentResponse {
                success: false,
                error: "agent not found".into(),
            }))
        }
    }

    async fn discover(
        &self,
        request: Request<DiscoverRequest>,
    ) -> Result<Response<DiscoverResponse>, Status> {
        let req = request.into_inner();
        let agents = self.registry.discover(&req.capabilities).await;

        let agent_infos: Vec<AgentInfo> = agents
            .into_iter()
            .map(|a| AgentInfo {
                agent_id: a.agent_id,
                name: a.name,
                tokens: a.tokens,
                money: a.money,
                skills: a.skills,
                reputation: a.reputation,
                phase: a.phase,
                last_seen: a.last_seen,
            })
            .collect();

        Ok(Response::new(DiscoverResponse {
            agents: agent_infos,
        }))
    }

    async fn send_message(
        &self,
        request: Request<A2aMessage>,
    ) -> Result<Response<MessageAck>, Status> {
        let msg = request.into_inner();

        if msg.from_agent.is_empty() {
            return Ok(Response::new(MessageAck {
                received: false,
                error: "from_agent is required".into(),
            }));
        }

        if msg.id.is_empty() {
            return Ok(Response::new(MessageAck {
                received: false,
                error: "message id is required".into(),
            }));
        }

        match self.router.route(msg).await {
            Ok(()) => Ok(Response::new(MessageAck {
                received: true,
                error: String::new(),
            })),
            Err(status) => Ok(Response::new(MessageAck {
                received: false,
                error: status.message().to_string(),
            })),
        }
    }

    type StreamMessagesStream = Pin<Box<dyn Stream<Item = Result<A2aMessage, Status>> + Send>>;

    async fn stream_messages(
        &self,
        request: Request<tonic::Streaming<A2aMessage>>,
    ) -> Result<Response<Self::StreamMessagesStream>, Status> {
        let mut inbound = request.into_inner();
        let router = self.router.clone();

        // We need to identify the agent. Peek at the first message to get agent_id.
        let first_msg = inbound
            .message()
            .await?
            .ok_or_else(|| Status::invalid_argument("Stream closed before first message"))?;

        let agent_id = if first_msg.from_agent.is_empty() {
            return Err(Status::invalid_argument(
                "First message must contain from_agent",
            ));
        } else {
            first_msg.clone().from_agent
        };

        // Route the first message
        router.route(first_msg).await?;

        // Open a downstream receiver for this agent
        let rx = router.open_stream(agent_id.clone()).await;

        // Spawn a task that forwards inbound messages to the router
        let router_clone = router.clone();
        let agent_id_clone = agent_id.clone();
        tokio::spawn(async move {
            while let Ok(Some(msg)) = inbound.message().await {
                if let Err(e) = router_clone.route(msg).await {
                    tracing::warn!(agent_id = %agent_id_clone, error = %e, "Failed to route stream message");
                }
            }
            // When the inbound stream closes, clean up
            router_clone.close_stream(&agent_id_clone).await;
        });

        // Map the receiver stream to produce Result<A2aMessage, Status>
        #[allow(clippy::result_large_err)]
        let output = Box::pin(rx.map(Ok));

        Ok(Response::new(output))
    }
}

// ── Batch operations ────────────────────────────────────────────────────

impl A2aServiceImpl {
    /// Send multiple messages in a single RPC call.
    ///
    /// Processes each message through the router, collecting individual ACKs.
    /// If any message fails validation, it is recorded as a failed ACK but
    /// does not prevent other messages from being processed.
    ///
    /// Returns a vector of `MessageAck` in the same order as the input messages.
    pub async fn send_message_batch(
        &self,
        messages: Vec<A2aMessage>,
    ) -> Result<Response<Vec<MessageAck>>, Status> {
        if messages.is_empty() {
            return Ok(Response::new(vec![]));
        }
        if messages.len() > MAX_BATCH_SIZE {
            return Err(Status::invalid_argument(format!(
                "Batch size {} exceeds maximum of {}",
                messages.len(),
                MAX_BATCH_SIZE
            )));
        }

        let mut acks = Vec::with_capacity(messages.len());

        for msg in messages {
            if msg.from_agent.is_empty() || msg.id.is_empty() {
                acks.push(MessageAck {
                    received: false,
                    error: "from_agent and id are required".into(),
                });
                continue;
            }

            match self.router.route(msg).await {
                Ok(()) => acks.push(MessageAck {
                    received: true,
                    error: String::new(),
                }),
                Err(status) => acks.push(MessageAck {
                    received: false,
                    error: status.message().to_string(),
                }),
            }
        }

        Ok(Response::new(acks))
    }

    /// Batch discovery — returns all registered agents in a single call.
    ///
    /// Equivalent to calling `discover` with no capability filter, but
    /// optimized to avoid per-agent allocation overhead when the caller
    /// needs the full roster.
    pub async fn discover_all(&self) -> Result<Response<DiscoverResponse>, Status> {
        let agents = self.registry.discover(&[]).await;

        let agent_infos: Vec<AgentInfo> = agents
            .into_iter()
            .map(|a| AgentInfo {
                agent_id: a.agent_id,
                name: a.name,
                tokens: a.tokens,
                money: a.money,
                skills: a.skills,
                reputation: a.reputation,
                phase: a.phase,
                last_seen: a.last_seen,
            })
            .collect();

        Ok(Response::new(DiscoverResponse {
            agents: agent_infos,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::EventBus;

    fn make_service() -> (A2aServiceImpl, Arc<AgentRegistry>, Arc<MessageRouter>) {
        let bus = Arc::new(EventBus::new(256));
        let registry = Arc::new(AgentRegistry::new(bus));
        let router = Arc::new(MessageRouter::new(Arc::clone(&registry)));
        let service = A2aServiceImpl::new(Arc::clone(&registry), Arc::clone(&router));
        (service, registry, router)
    }

    #[tokio::test]
    async fn register_agent_success() {
        let (service, _, _) = make_service();
        let resp = service
            .register_agent(Request::new(RegisterAgentRequest {
                agent_id: "a1".into(),
                name: "Alice".into(),
                capabilities: vec!["coding".into()],
                public_key: "pk1".into(),
            }))
            .await
            .unwrap();

        let body = resp.into_inner();
        assert!(body.success);
        assert!(body.timestamp > 0);
    }

    #[tokio::test]
    async fn register_agent_missing_id() {
        let (service, _, _) = make_service();
        let resp = service
            .register_agent(Request::new(RegisterAgentRequest {
                agent_id: String::new(),
                name: "Alice".into(),
                capabilities: vec![],
                public_key: String::new(),
            }))
            .await
            .unwrap();

        let body = resp.into_inner();
        assert!(!body.success);
        assert!(body.error.contains("agent_id"));
    }

    #[tokio::test]
    async fn register_agent_missing_name() {
        let (service, _, _) = make_service();
        let resp = service
            .register_agent(Request::new(RegisterAgentRequest {
                agent_id: "a1".into(),
                name: String::new(),
                capabilities: vec![],
                public_key: String::new(),
            }))
            .await
            .unwrap();

        let body = resp.into_inner();
        assert!(!body.success);
        assert!(body.error.contains("name"));
    }

    #[tokio::test]
    async fn heartbeat_registered_agent() {
        let (service, registry, _) = make_service();
        registry
            .register("a1".into(), "Alice".into(), vec![], "pk".into())
            .await;

        let resp = service
            .heartbeat(Request::new(HeartbeatRequest {
                agent_id: "a1".into(),
            }))
            .await
            .unwrap();

        let body = resp.into_inner();
        assert!(body.alive);
    }

    #[tokio::test]
    async fn heartbeat_unregistered_agent() {
        let (service, _, _) = make_service();
        let resp = service
            .heartbeat(Request::new(HeartbeatRequest {
                agent_id: "unknown".into(),
            }))
            .await
            .unwrap();

        assert!(!resp.into_inner().alive);
    }

    #[tokio::test]
    async fn heartbeat_empty_agent_id() {
        let (service, _, _) = make_service();
        let result = service
            .heartbeat(Request::new(HeartbeatRequest {
                agent_id: String::new(),
            }))
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn deregister_agent_success() {
        let (service, registry, _) = make_service();
        registry
            .register("a1".into(), "Alice".into(), vec![], "pk".into())
            .await;

        let resp = service
            .deregister_agent(Request::new(DeregisterAgentRequest {
                agent_id: "a1".into(),
                signature: String::new(),
            }))
            .await
            .unwrap();

        assert!(resp.into_inner().success);
        assert_eq!(registry.count().await, 0);
    }

    #[tokio::test]
    async fn deregister_agent_not_found() {
        let (service, _, _) = make_service();
        let resp = service
            .deregister_agent(Request::new(DeregisterAgentRequest {
                agent_id: "unknown".into(),
                signature: String::new(),
            }))
            .await
            .unwrap();

        assert!(!resp.into_inner().success);
    }

    #[tokio::test]
    async fn discover_returns_registered_agents() {
        let (service, registry, _) = make_service();
        registry
            .register("a1".into(), "Alice".into(), vec!["coding".into()], "pk1".into())
            .await;
        registry
            .register("a2".into(), "Bob".into(), vec!["research".into()], "pk2".into())
            .await;

        let resp = service
            .discover(Request::new(DiscoverRequest {
                agent_id: String::new(),
                capabilities: vec![],
            }))
            .await
            .unwrap();

        let body = resp.into_inner();
        assert_eq!(body.agents.len(), 2);
    }

    #[tokio::test]
    async fn discover_filter_by_capability() {
        let (service, registry, _) = make_service();
        registry
            .register("a1".into(), "Alice".into(), vec!["coding".into()], "pk1".into())
            .await;
        registry
            .register("a2".into(), "Bob".into(), vec!["research".into()], "pk2".into())
            .await;

        let resp = service
            .discover(Request::new(DiscoverRequest {
                agent_id: String::new(),
                capabilities: vec!["coding".into()],
            }))
            .await
            .unwrap();

        let body = resp.into_inner();
        assert_eq!(body.agents.len(), 1);
        assert_eq!(body.agents[0].agent_id, "a1");
    }

    #[tokio::test]
    async fn send_message_success() {
        let (service, registry, _) = make_service();
        registry
            .register("a1".into(), "Alice".into(), vec![], "pk1".into())
            .await;
        registry
            .register("a2".into(), "Bob".into(), vec![], "pk2".into())
            .await;

        let resp = service
            .send_message(Request::new(A2aMessage {
                id: "msg-1".into(),
                from_agent: "a1".into(),
                to_agent: "a2".into(),
                r#type: 4,
                payload: b"hello".to_vec(),
                timestamp: 1000,
                signature: String::new(),
                nonce: "n1".into(),
            }))
            .await
            .unwrap();

        assert!(resp.into_inner().received);
    }

    #[tokio::test]
    async fn send_message_unknown_recipient() {
        let (service, registry, _) = make_service();
        registry
            .register("a1".into(), "Alice".into(), vec![], "pk1".into())
            .await;

        let resp = service
            .send_message(Request::new(A2aMessage {
                id: "msg-1".into(),
                from_agent: "a1".into(),
                to_agent: "unknown".into(),
                r#type: 4,
                payload: Vec::new(),
                timestamp: 0,
                signature: String::new(),
                nonce: String::new(),
            }))
            .await
            .unwrap();

        assert!(!resp.into_inner().received);
    }

    #[tokio::test]
    async fn send_message_missing_from_agent() {
        let (service, _, _) = make_service();
        let resp = service
            .send_message(Request::new(A2aMessage {
                id: "msg-1".into(),
                from_agent: String::new(),
                to_agent: "a2".into(),
                r#type: 4,
                payload: Vec::new(),
                timestamp: 0,
                signature: String::new(),
                nonce: String::new(),
            }))
            .await
            .unwrap();

        assert!(!resp.into_inner().received);
    }

    #[tokio::test]
    async fn send_message_missing_id() {
        let (service, _, _) = make_service();
        let resp = service
            .send_message(Request::new(A2aMessage {
                id: String::new(),
                from_agent: "a1".into(),
                to_agent: "a2".into(),
                r#type: 4,
                payload: Vec::new(),
                timestamp: 0,
                signature: String::new(),
                nonce: String::new(),
            }))
            .await
            .unwrap();

        assert!(!resp.into_inner().received);
    }

    #[tokio::test]
    async fn send_message_batch_success() {
        let (service, registry, _) = make_service();
        registry
            .register("a1".into(), "Alice".into(), vec![], "pk1".into())
            .await;
        registry
            .register("a2".into(), "Bob".into(), vec![], "pk2".into())
            .await;

        let messages = vec![
            A2aMessage {
                id: "msg-1".into(),
                from_agent: "a1".into(),
                to_agent: "a2".into(),
                r#type: 4,
                payload: b"hello".to_vec(),
                timestamp: 1000,
                signature: String::new(),
                nonce: "n1".into(),
            },
            A2aMessage {
                id: "msg-2".into(),
                from_agent: "a2".into(),
                to_agent: "a1".into(),
                r#type: 4,
                payload: b"world".to_vec(),
                timestamp: 1001,
                signature: String::new(),
                nonce: "n2".into(),
            },
        ];

        let resp = service.send_message_batch(messages).await.unwrap();
        let acks = resp.into_inner();
        assert_eq!(acks.len(), 2);
        assert!(acks[0].received);
        assert!(acks[1].received);
    }

    #[tokio::test]
    async fn send_message_batch_empty() {
        let (service, _, _) = make_service();
        let resp = service.send_message_batch(vec![]).await.unwrap();
        assert!(resp.into_inner().is_empty());
    }

    #[tokio::test]
    async fn send_message_batch_too_large() {
        let (service, _, _) = make_service();
        let messages: Vec<A2aMessage> = (0..257)
            .map(|i| A2aMessage {
                id: format!("msg-{}", i),
                from_agent: "a1".into(),
                to_agent: "a2".into(),
                r#type: 4,
                payload: Vec::new(),
                timestamp: 0,
                signature: String::new(),
                nonce: String::new(),
            })
            .collect();

        let result = service.send_message_batch(messages).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn send_message_batch_partial_failure() {
        let (service, registry, _) = make_service();
        registry
            .register("a1".into(), "Alice".into(), vec![], "pk1".into())
            .await;
        // a2 is NOT registered

        let messages = vec![
            A2aMessage {
                id: "msg-1".into(),
                from_agent: "a1".into(),
                to_agent: "a2".into(),
                r#type: 4,
                payload: Vec::new(),
                timestamp: 0,
                signature: String::new(),
                nonce: "n1".into(),
            },
            A2aMessage {
                id: "msg-2".into(),
                from_agent: String::new(), // missing from_agent
                to_agent: "a1".into(),
                r#type: 4,
                payload: Vec::new(),
                timestamp: 0,
                signature: String::new(),
                nonce: "n2".into(),
            },
        ];

        let resp = service.send_message_batch(messages).await.unwrap();
        let acks = resp.into_inner();
        assert_eq!(acks.len(), 2);
        assert!(!acks[0].received); // unknown recipient
        assert!(!acks[1].received); // missing from_agent
    }

    #[tokio::test]
    async fn discover_all_returns_all_agents() {
        let (service, registry, _) = make_service();
        registry
            .register("a1".into(), "Alice".into(), vec!["coding".into()], "pk1".into())
            .await;
        registry
            .register("a2".into(), "Bob".into(), vec!["research".into()], "pk2".into())
            .await;

        let resp = service.discover_all().await.unwrap();
        let body = resp.into_inner();
        assert_eq!(body.agents.len(), 2);
    }
}
