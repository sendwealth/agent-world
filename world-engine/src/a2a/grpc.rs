use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use std::time::{SystemTime, UNIX_EPOCH};

use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::StreamExt;
use tonic::{Request, Response, Status, Streaming};

use crate::a2a::router::{A2ARouter, RegisteredAgent, RouterMessage};

// Import generated protobuf types
pub mod proto {
    tonic::include_proto!("agentworld.a2a.v1");
}

use proto::a2a_service_server::{A2aService, A2aServiceServer};
use proto::{
    A2aMessage as ProtoA2aMessage, DiscoverRequest, DiscoverResponse, MessageAck,
    AgentInfo as ProtoAgentInfo,
};

/// Shared reference to the A2A router.
pub type SharedA2ARouter = Arc<A2ARouter>;

/// Implementation of the A2A gRPC service.
pub struct A2AServiceImpl {
    router: SharedA2ARouter,
    current_tick: Arc<AtomicU64>,
}

impl A2AServiceImpl {
    pub fn new(router: SharedA2ARouter) -> Self {
        Self {
            router,
            current_tick: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn with_tick(router: SharedA2ARouter, tick: u64) -> Self {
        Self {
            router,
            current_tick: Arc::new(AtomicU64::new(tick)),
        }
    }

    /// Get the current time in milliseconds since epoch.
    fn now_ms() -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64
    }
}

#[tonic::async_trait]
impl A2aService for A2AServiceImpl {
    async fn discover(
        &self,
        request: Request<DiscoverRequest>,
    ) -> Result<Response<DiscoverResponse>, Status> {
        let req = request.into_inner();

        let agents = self.router.discover(&req.agent_id, &req.capabilities).await;

        let proto_agents: Vec<ProtoAgentInfo> = agents
            .into_iter()
            .map(|a| ProtoAgentInfo {
                agent_id: a.agent_id,
                name: a.name,
                tokens: a.tokens,
                money: a.money,
                skills: a.skills,
                reputation: a.reputation,
                phase: a.phase,
            })
            .collect();

        Ok(Response::new(DiscoverResponse {
            agents: proto_agents,
        }))
    }

    async fn send_message(
        &self,
        request: Request<ProtoA2aMessage>,
    ) -> Result<Response<MessageAck>, Status> {
        let proto_msg = request.into_inner();

        // Validate required fields
        if proto_msg.from_agent.is_empty() {
            return Ok(Response::new(MessageAck {
                received: false,
                error: "from_agent is required".to_string(),
            }));
        }
        if proto_msg.id.is_empty() {
            return Ok(Response::new(MessageAck {
                received: false,
                error: "message id is required".to_string(),
            }));
        }

        let msg = RouterMessage {
            id: proto_msg.id,
            from_agent: proto_msg.from_agent,
            to_agent: proto_msg.to_agent,
            message_type: proto_msg.r#type,
            payload: proto_msg.payload,
            timestamp: proto_msg.timestamp,
            signature: proto_msg.signature,
            nonce: proto_msg.nonce,
        };

        let now_ms = Self::now_ms();
        let tick = self.current_tick.load(Ordering::Relaxed);

        match self.router.route_message(msg, now_ms, tick).await {
            Ok(()) => Ok(Response::new(MessageAck {
                received: true,
                error: String::new(),
            })),
            Err(e) => Ok(Response::new(MessageAck {
                received: false,
                error: e.to_string(),
            })),
        }
    }

    type StreamMessagesStream = ReceiverStream<Result<ProtoA2aMessage, Status>>;

    async fn stream_messages(
        &self,
        request: Request<Streaming<ProtoA2aMessage>>,
    ) -> Result<Response<Self::StreamMessagesStream>, Status> {
        let mut stream = request.into_inner();
        let router = self.router.clone();
        let tick = self.current_tick.clone();

        // Create a channel for outgoing messages to this client
        let (tx, rx) = mpsc::channel(128);

        // Register a temporary agent for streaming using a generated ID
        // The first message from the stream should identify the agent
        let agent_id = format!("stream-{}", uuid::Uuid::new_v4());

        let registered_agent = RegisteredAgent {
            agent_id: agent_id.clone(),
            name: format!("stream-client-{}", &agent_id[..8]),
            tokens: 0,
            money: 0,
            skills: vec![],
            reputation: 0.0,
            phase: 2, // ADULT
        };

        let mut inbox_rx = router
            .register(registered_agent)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let agent_id_clone = agent_id.clone();
        let router_clone = router.clone();

        // Task 1: Forward incoming messages from the stream to the router
        let tx_err = tx.clone();
        let forward_handle = tokio::spawn(async move {
            while let Some(result) = stream.next().await {
                match result {
                    Ok(proto_msg) => {
                        let msg = RouterMessage {
                            id: proto_msg.id,
                            from_agent: proto_msg.from_agent.clone(),
                            to_agent: proto_msg.to_agent,
                            message_type: proto_msg.r#type,
                            payload: proto_msg.payload,
                            timestamp: proto_msg.timestamp,
                            signature: proto_msg.signature,
                            nonce: proto_msg.nonce,
                        };

                        let now_ms = Self::now_ms();
                        let current_tick =
                            tick.load(Ordering::Relaxed);

                        if let Err(e) = router.route_message(msg, now_ms, current_tick).await {
                            let _ = tx_err
                                .send(Err(Status::internal(format!(
                                    "route error: {}",
                                    e
                                ))))
                                .await;
                        }
                    }
                    Err(e) => {
                        let _ = tx_err.send(Err(e)).await;
                        break;
                    }
                }
            }
        });

        // Task 2: Forward messages from the inbox to the outbound stream
        let outbound_handle = tokio::spawn(async move {
            while let Some(msg) = inbox_rx.recv().await {
                let proto_msg = ProtoA2aMessage {
                    id: msg.id,
                    from_agent: msg.from_agent,
                    to_agent: msg.to_agent,
                    r#type: msg.message_type,
                    payload: msg.payload,
                    timestamp: msg.timestamp,
                    signature: msg.signature,
                    nonce: msg.nonce,
                };
                if tx.send(Ok(proto_msg)).await.is_err() {
                    break;
                }
            }
        });

        // Cleanup: deregister when the stream ends
        let cleanup_id = agent_id_clone.clone();
        tokio::spawn(async move {
            let _ = forward_handle.await;
            let _ = outbound_handle.await;
            let _ = router_clone.deregister(&cleanup_id).await;
        });

        Ok(Response::new(ReceiverStream::new(rx)))
    }
}

/// Build a gRPC server builder with the A2A service.
pub fn create_a2a_server(
    router: SharedA2ARouter,
) -> A2aServiceServer<A2AServiceImpl> {
    A2aServiceServer::new(A2AServiceImpl::new(router))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_router() -> SharedA2ARouter {
        Arc::new(A2ARouter::new(crate::a2a::router::A2AConfig::default()))
    }

    fn make_proto_message(from: &str, to: &str) -> ProtoA2aMessage {
        ProtoA2aMessage {
            id: uuid::Uuid::new_v4().to_string(),
            from_agent: from.to_string(),
            to_agent: to.to_string(),
            r#type: 4, // INFORM
            payload: b"hello".to_vec(),
            timestamp: A2AServiceImpl::now_ms(),
            signature: String::new(),
            nonce: uuid::Uuid::new_v4().to_string(),
        }
    }

    #[tokio::test]
    async fn test_discover_empty() {
        let router = make_router();
        let service = A2AServiceImpl::new(router);

        let request = tonic::Request::new(DiscoverRequest {
            agent_id: "test".to_string(),
            capabilities: vec![],
        });

        let response = service.discover(request).await.unwrap();
        assert_eq!(response.into_inner().agents.len(), 0);
    }

    #[tokio::test]
    async fn test_send_message_success() {
        let router = make_router();
        router
            .register(RegisteredAgent {
                agent_id: "alice".to_string(),
                name: "Alice".to_string(),
                tokens: 1000,
                money: 0,
                skills: vec![],
                reputation: 0.0,
                phase: 2,
            })
            .await
            .unwrap();
        router
            .register(RegisteredAgent {
                agent_id: "bob".to_string(),
                name: "Bob".to_string(),
                tokens: 1000,
                money: 0,
                skills: vec![],
                reputation: 0.0,
                phase: 2,
            })
            .await
            .unwrap();

        let service = A2AServiceImpl::new(router);

        let msg = make_proto_message("alice", "bob");
        let request = tonic::Request::new(msg);
        let response = service.send_message(request).await.unwrap();
        let ack = response.into_inner();
        assert!(ack.received);
        assert!(ack.error.is_empty());
    }

    #[tokio::test]
    async fn test_send_message_missing_from() {
        let router = make_router();
        let service = A2AServiceImpl::new(router);

        let mut msg = make_proto_message("alice", "bob");
        msg.from_agent = String::new();
        let request = tonic::Request::new(msg);
        let response = service.send_message(request).await.unwrap();
        let ack = response.into_inner();
        assert!(!ack.received);
    }

    #[tokio::test]
    async fn test_send_message_missing_id() {
        let router = make_router();
        let service = A2AServiceImpl::new(router);

        let mut msg = make_proto_message("alice", "bob");
        msg.id = String::new();
        let request = tonic::Request::new(msg);
        let response = service.send_message(request).await.unwrap();
        let ack = response.into_inner();
        assert!(!ack.received);
    }

    #[tokio::test]
    async fn test_two_agents_exchange_via_grpc() {
        let router = make_router();
        let mut rx_alice = router
            .register(RegisteredAgent {
                agent_id: "alice".to_string(),
                name: "Alice".to_string(),
                tokens: 1000,
                money: 0,
                skills: vec![],
                reputation: 0.0,
                phase: 2,
            })
            .await
            .unwrap();
        let mut rx_bob = router
            .register(RegisteredAgent {
                agent_id: "bob".to_string(),
                name: "Bob".to_string(),
                tokens: 1000,
                money: 0,
                skills: vec![],
                reputation: 0.0,
                phase: 2,
            })
            .await
            .unwrap();

        let service = A2AServiceImpl::new(router);

        // Alice -> Bob
        let msg = ProtoA2aMessage {
            id: "msg-1".to_string(),
            from_agent: "alice".to_string(),
            to_agent: "bob".to_string(),
            r#type: 1, // PROPOSE
            payload: b"Want to trade?".to_vec(),
            timestamp: A2AServiceImpl::now_ms(),
            signature: String::new(),
            nonce: "n1".to_string(),
        };
        let resp = service
            .send_message(tonic::Request::new(msg))
            .await
            .unwrap()
            .into_inner();
        assert!(resp.received);

        // Bob receives the message
        let received = rx_bob.try_recv().unwrap();
        assert_eq!(received.from_agent, "alice");
        assert_eq!(received.payload, b"Want to trade?");

        // Bob -> Alice
        let msg2 = ProtoA2aMessage {
            id: "msg-2".to_string(),
            from_agent: "bob".to_string(),
            to_agent: "alice".to_string(),
            r#type: 2, // ACCEPT
            payload: b"Sure!".to_vec(),
            timestamp: A2AServiceImpl::now_ms(),
            signature: String::new(),
            nonce: "n2".to_string(),
        };
        let resp2 = service
            .send_message(tonic::Request::new(msg2))
            .await
            .unwrap()
            .into_inner();
        assert!(resp2.received);

        // Alice receives the message
        let received2 = rx_alice.try_recv().unwrap();
        assert_eq!(received2.from_agent, "bob");
        assert_eq!(received2.payload, b"Sure!");
    }
}
