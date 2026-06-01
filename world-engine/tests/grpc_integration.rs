//! Integration test for the A2A gRPC server.
//!
//! Starts a gRPC server on a random port, registers test agents,
//! and verifies Discover / SendMessage RPCs via a tonic client.

use std::net::SocketAddr;
use std::sync::Arc;

use agent_world_engine::a2a::registry::AgentRegistry;
use agent_world_engine::a2a::router::MessageRouter;
use agent_world_engine::a2a::service::A2aServiceImpl;
use agent_world_engine::a2a::world_message_router::WorldMessageRouter;
use agent_world_engine::agentworld::a2a::v1::a2a_service_client::A2aServiceClient;
use agent_world_engine::agentworld::a2a::v1::a2a_service_server::A2aServiceServer;
use agent_world_engine::agentworld::a2a::v1::{A2aMessage, DiscoverRequest, MessageAck};
use agent_world_engine::world::EventBus;

/// Pick a random available port.
fn find_free_port() -> u16 {
    use std::net::{Ipv4Addr, TcpListener};
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    listener.local_addr().unwrap().port()
}

/// Helper to build and start a gRPC server with a pre-populated registry.
async fn start_grpc_server_with_registry(
    addr: SocketAddr,
    _event_bus: Arc<EventBus>,
    registry: Arc<AgentRegistry>,
) -> Result<tokio::task::JoinHandle<()>, Box<dyn std::error::Error + Send + Sync>> {
    let router = Arc::new(MessageRouter::new(registry.clone()));
    let world_msg_router = Arc::new(WorldMessageRouter::new());
    let service = A2aServiceImpl::new(registry, router, world_msg_router);

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

#[tokio::test]
async fn grpc_server_starts_and_responds() {
    let port = find_free_port();
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let event_bus = Arc::new(EventBus::new(64));

    // Create registry with test agents
    let registry = Arc::new(AgentRegistry::new(event_bus.clone()));
    registry
        .register(
            "agent-alice".into(),
            "Alice".into(),
            vec!["coding".into(), "research".into()],
            "pk-alice".into(),
        )
        .await;
    registry
        .register(
            "agent-bob".into(),
            "Bob".into(),
            vec!["trading".into()],
            "pk-bob".into(),
        )
        .await;

    // Start the gRPC server
    let handle = start_grpc_server_with_registry(addr, event_bus, registry)
        .await
        .expect("gRPC server should start");

    // Give the server a moment to bind
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Connect a client
    let url = format!("http://127.0.0.1:{}", port);
    let mut client = A2aServiceClient::connect(url)
        .await
        .expect("Should connect to gRPC server");

    // Test Discover RPC
    let discover_req = tonic::Request::new(DiscoverRequest {
        agent_id: "test-client".into(),
        capabilities: vec![],
    });
    let discover_resp = client.discover(discover_req).await.unwrap();
    let agents = discover_resp.into_inner().agents;
    assert_eq!(agents.len(), 2, "Should discover 2 agents");

    let alice = agents.iter().find(|a| a.agent_id == "agent-alice").unwrap();
    assert_eq!(alice.name, "Alice");

    // Test Discover with capability filter
    let discover_req2 = tonic::Request::new(DiscoverRequest {
        agent_id: "test-client".into(),
        capabilities: vec!["coding".into()],
    });
    let discover_resp2 = client.discover(discover_req2).await.unwrap();
    let coders = discover_resp2.into_inner().agents;
    assert_eq!(coders.len(), 1, "Should find 1 agent with 'coding' skill");
    assert_eq!(coders[0].agent_id, "agent-alice");

    // Test SendMessage RPC
    let send_req = tonic::Request::new(A2aMessage {
        id: "msg-001".into(),
        from_agent: "agent-alice".into(),
        to_agent: "agent-bob".into(),
        r#type: 4, // INFORM
        payload: b"Hello Bob!".to_vec(),
        timestamp: 1234567890,
        signature: String::new(),
        nonce: "nonce-1".into(),
    });
    let send_resp = client.send_message(send_req).await.unwrap();
    let ack: MessageAck = send_resp.into_inner();
    assert!(ack.received, "Message should be received");
    assert!(ack.error.is_empty(), "No error expected");

    // Test SendMessage to unknown agent
    let send_req2 = tonic::Request::new(A2aMessage {
        id: "msg-002".into(),
        from_agent: "agent-alice".into(),
        to_agent: "unknown-agent".into(),
        r#type: 4,
        payload: vec![],
        timestamp: 1234567890,
        signature: String::new(),
        nonce: "nonce-2".into(),
    });
    let send_resp2 = client.send_message(send_req2).await.unwrap();
    let ack2: MessageAck = send_resp2.into_inner();
    assert!(!ack2.received, "Message to unknown agent should fail");
    assert!(!ack2.error.is_empty(), "Should have error message");

    // Cleanup
    handle.abort();
}

#[tokio::test(flavor = "multi_thread")]
async fn grpc_stream_messages_bidirectional() {
    let port = find_free_port();
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let event_bus = Arc::new(EventBus::new(64));

    let registry = Arc::new(AgentRegistry::new(event_bus.clone()));
    registry
        .register(
            "agent-alice".into(),
            "Alice".into(),
            vec!["coding".into()],
            "pk-alice".into(),
        )
        .await;
    registry
        .register(
            "agent-bob".into(),
            "Bob".into(),
            vec!["trading".into()],
            "pk-bob".into(),
        )
        .await;

    let router = Arc::new(MessageRouter::new(registry.clone()));
    let world_msg_router = Arc::new(WorldMessageRouter::new());
    let service = A2aServiceImpl::new(registry, router.clone(), world_msg_router);

    let handle = tokio::spawn(async move {
        tonic::transport::Server::builder()
            .add_service(A2aServiceServer::new(service))
            .serve(addr)
            .await
            .unwrap();
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let url = format!("http://127.0.0.1:{}", port);

    // ---- Open Alice's stream ----
    let mut alice_client = A2aServiceClient::connect(url.clone())
        .await
        .expect("Alice should connect");

    // Pre-load Alice's identifying message into the stream BEFORE calling stream_messages
    let (alice_tx, alice_rx) = tokio::sync::mpsc::channel(16);
    alice_tx
        .send(A2aMessage {
            id: "alice-init".into(),
            from_agent: "agent-alice".into(),
            to_agent: "agent-bob".into(),
            r#type: 4,
            payload: b"Hello Bob".to_vec(),
            timestamp: 1,
            signature: String::new(),
            nonce: "n1".into(),
        })
        .await
        .unwrap();

    // Now open the stream - the first message is already in the channel
    let alice_response = alice_client
        .stream_messages(tokio_stream::wrappers::ReceiverStream::new(alice_rx))
        .await
        .expect("Alice stream should open");
    let mut alice_stream = alice_response.into_inner();

    // Verify alice's stream is registered
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
    assert_eq!(
        router.active_stream_count().await,
        1,
        "Alice stream should be registered"
    );

    // Use SendMessage RPC to send a message from bob to alice
    let mut sender_client = A2aServiceClient::connect(url)
        .await
        .expect("Sender should connect");

    let send_resp = sender_client
        .send_message(tonic::Request::new(A2aMessage {
            id: "msg-from-bob".into(),
            from_agent: "agent-bob".into(),
            to_agent: "agent-alice".into(),
            r#type: 4,
            payload: b"Hello Alice".to_vec(),
            timestamp: 2,
            signature: String::new(),
            nonce: "n2".into(),
        }))
        .await
        .expect("Send should succeed");

    assert!(
        send_resp.into_inner().received,
        "Message should be received"
    );

    // Alice should receive the message on her stream
    let received =
        tokio::time::timeout(tokio::time::Duration::from_secs(10), alice_stream.message())
            .await
            .expect("Timed out waiting for alice to receive message");

    match received {
        Ok(Some(msg)) => {
            assert_eq!(msg.from_agent, "agent-bob", "Message should be from bob");
            assert_eq!(msg.to_agent, "agent-alice", "Message should be to alice");
        }
        Ok(None) => panic!("Stream ended unexpectedly"),
        Err(e) => panic!("Stream error: {:?}", e),
    }

    // Cleanup
    drop(alice_tx);
    handle.abort();
}

#[tokio::test]
async fn grpc_eventbus_integration() {
    let port = find_free_port();
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let event_bus = Arc::new(EventBus::new(64));

    // Subscribe to events before starting server
    let mut event_rx = event_bus.subscribe();

    let registry = Arc::new(AgentRegistry::new(event_bus.clone()));
    registry
        .register(
            "agent-alice".into(),
            "Alice".into(),
            vec!["coding".into()],
            "pk-alice".into(),
        )
        .await;
    registry
        .register(
            "agent-bob".into(),
            "Bob".into(),
            vec!["trading".into()],
            "pk-bob".into(),
        )
        .await;

    let handle = start_grpc_server_with_registry(addr, event_bus.clone(), registry)
        .await
        .expect("gRPC server should start");

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let url = format!("http://127.0.0.1:{}", port);
    let mut client = A2aServiceClient::connect(url)
        .await
        .expect("Should connect to gRPC server");

    // Send a message via gRPC
    let send_req = tonic::Request::new(A2aMessage {
        id: "msg-eventbus-001".into(),
        from_agent: "agent-alice".into(),
        to_agent: "agent-bob".into(),
        r#type: 4, // INFORM
        payload: b"EventBus test".to_vec(),
        timestamp: 1234567890,
        signature: String::new(),
        nonce: "nonce-eb-1".into(),
    });
    let send_resp = client.send_message(send_req).await.unwrap();
    assert!(send_resp.into_inner().received);

    // Verify that the EventBus received an event from the registry (AgentSpawned)
    let event = tokio::time::timeout(tokio::time::Duration::from_secs(5), event_rx.recv())
        .await
        .expect("Should receive event within timeout")
        .expect("Should get event");

    match event {
        agent_world_engine::world::event::WorldEvent::AgentSpawned { agent_id, .. } => {
            assert!(agent_id == "agent-alice" || agent_id == "agent-bob");
        }
        agent_world_engine::world::event::WorldEvent::AgentRegistered { agent_id, .. } => {
            assert!(agent_id == "agent-alice" || agent_id == "agent-bob");
        }
        other => panic!(
            "Expected AgentSpawned or AgentRegistered event, got {:?}",
            other.event_type()
        ),
    }

    handle.abort();
}
