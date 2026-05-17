use std::sync::Arc;
use std::net::SocketAddr;

use tonic::{Request, Response, Status, Streaming};
use tokio_stream::{wrappers::ReceiverStream, StreamExt};

use crate::world::EventBus;

use super::discovery::AgentRegistry;
use super::router::{MessageRouter, PendingMessage};

// Import generated protobuf types
pub mod proto {
    tonic::include_proto!("agentworld.a2a.v1");
}

use proto::a2a_service_server::{A2aService, A2aServiceServer};
use proto::{
    A2aMessage, DiscoverRequest, DiscoverResponse, AgentInfo as ProtoAgentInfo,
    MessageAck,
};

/// Shared state accessible to all gRPC handlers.
#[derive(Clone)]
pub struct A2AServiceImpl {
    registry: Arc<AgentRegistry>,
    router: Arc<MessageRouter>,
}

impl A2AServiceImpl {
    pub fn new(registry: Arc<AgentRegistry>, router: Arc<MessageRouter>) -> Self {
        Self { registry, router }
    }
}

#[tonic::async_trait]
impl A2aService for A2AServiceImpl {
    async fn discover(
        &self,
        request: Request<DiscoverRequest>,
    ) -> Result<Response<DiscoverResponse>, Status> {
        let req = request.into_inner();
        let agents = self.registry.discover(&req.capabilities).await;

        let proto_agents: Vec<ProtoAgentInfo> = agents
            .into_iter()
            .map(|a| ProtoAgentInfo {
                agent_id: a.agent_id,
                name: a.name,
                tokens: a.tokens,
                money: a.money,
                skills: a.skills,
                reputation: a.reputation,
                phase: match a.phase.as_str() {
                    "birth" => proto::AgentPhase::Birth as i32,
                    "childhood" => proto::AgentPhase::Childhood as i32,
                    "elder" => proto::AgentPhase::Elder as i32,
                    "dead" => proto::AgentPhase::Dead as i32,
                    _ => proto::AgentPhase::Adult as i32,
                },
            })
            .collect();

        Ok(Response::new(DiscoverResponse {
            agents: proto_agents,
        }))
    }

    async fn send_message(
        &self,
        request: Request<A2aMessage>,
    ) -> Result<Response<MessageAck>, Status> {
        let msg = request.into_inner();

        let pending = PendingMessage {
            id: msg.id,
            from_agent: msg.from_agent,
            to_agent: msg.to_agent,
            msg_type: match msg.r#type {
                0 => "discover".into(),
                1 => "propose".into(),
                2 => "accept".into(),
                3 => "reject".into(),
                4 => "inform".into(),
                5 => "teach".into(),
                6 => "reproduce".into(),
                7 => "will".into(),
                8 => "threat".into(),
                _ => "unknown".into(),
            },
            payload: msg.payload,
            timestamp: msg.timestamp,
        };

        match self.router.route_message(pending).await {
            Ok(()) => Ok(Response::new(MessageAck {
                received: true,
                error: String::new(),
            })),
            Err(e) => Ok(Response::new(MessageAck {
                received: false,
                error: e,
            })),
        }
    }

    type StreamMessagesStream = ReceiverStream<Result<A2aMessage, Status>>;

    async fn stream_messages(
        &self,
        request: Request<Streaming<A2aMessage>>,
    ) -> Result<Response<Self::StreamMessagesStream>, Status> {
        let mut stream = request.into_inner();
        let router = self.router.clone();

        let (tx, rx) = tokio::sync::mpsc::channel(128);

        tokio::spawn(async move {
            while let Some(result) = stream.next().await {
                let msg = match result {
                    Ok(m) => m,
                    Err(e) => {
                        let _ = tx.send(Err(Status::internal(e.to_string()))).await;
                        break;
                    }
                };

                // Route the incoming message
                let pending = PendingMessage {
                    id: msg.id.clone(),
                    from_agent: msg.from_agent.clone(),
                    to_agent: msg.to_agent.clone(),
                    msg_type: format!("{}", msg.r#type),
                    payload: msg.payload.clone(),
                    timestamp: msg.timestamp,
                };

                if let Err(e) = router.route_message(pending).await {
                    let _ = tx
                        .send(Ok(A2aMessage {
                            id: String::new(),
                            from_agent: String::new(),
                            to_agent: msg.from_agent.clone(),
                            r#type: 3, // REJECT
                            payload: e.into_bytes(),
                            timestamp: chrono::Utc::now().timestamp(),
                            signature: String::new(),
                            nonce: String::new(),
                        }))
                        .await;
                    continue;
                }

                // Echo back an acknowledgement message
                let ack = A2aMessage {
                    id: uuid::Uuid::new_v4().to_string(),
                    from_agent: String::new(),
                    to_agent: msg.from_agent.clone(),
                    r#type: 2, // ACCEPT
                    payload: format!("Message {} delivered", msg.id).into_bytes(),
                    timestamp: chrono::Utc::now().timestamp(),
                    signature: String::new(),
                    nonce: String::new(),
                };

                if tx.send(Ok(ack)).await.is_err() {
                    break;
                }
            }
        });

        Ok(Response::new(ReceiverStream::new(rx)))
    }
}

/// Start the gRPC server on the given address.
/// Returns a tokio JoinHandle for graceful shutdown.
pub async fn start_grpc_server(
    addr: SocketAddr,
    event_bus: Arc<EventBus>,
) -> Result<tokio::task::JoinHandle<()>, Box<dyn std::error::Error + Send + Sync>> {
    let registry = Arc::new(AgentRegistry::new());
    let router = Arc::new(MessageRouter::new(event_bus, registry.clone()));
    let service = A2AServiceImpl::new(registry, router);

    println!("   gRPC server: listening on {}", addr);

    let handle = tokio::spawn(async move {
        if let Err(e) = tonic::transport::Server::builder()
            .add_service(A2aServiceServer::new(service))
            .serve(addr)
            .await
        {
            eprintln!("[gRPC] Server error: {}", e);
        }
    });

    Ok(handle)
}

/// Start the gRPC server with a pre-populated agent registry.
/// Useful for testing and for restoring state from WAL recovery.
pub async fn start_grpc_server_with_registry(
    addr: SocketAddr,
    event_bus: Arc<EventBus>,
    registry: Arc<AgentRegistry>,
) -> Result<tokio::task::JoinHandle<()>, Box<dyn std::error::Error + Send + Sync>> {
    let router = Arc::new(MessageRouter::new(event_bus, registry.clone()));
    let service = A2AServiceImpl::new(registry, router);

    println!("   gRPC server: listening on {}", addr);

    let handle = tokio::spawn(async move {
        if let Err(e) = tonic::transport::Server::builder()
            .add_service(A2aServiceServer::new(service))
            .serve(addr)
            .await
        {
            eprintln!("[gRPC] Server error: {}", e);
        }
    });

    Ok(handle)
}
