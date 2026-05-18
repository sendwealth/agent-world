/// Integration test for the A2A gRPC server.
///
/// Starts a gRPC server on a random port, registers test agents,
/// and verifies Discover / SendMessage RPCs via a tonic client.

use std::net::SocketAddr;
use std::sync::Arc;

use agent_world_engine::a2a::discovery::{AgentRecord, AgentRegistry};
use agent_world_engine::a2a::server::{
    start_grpc_server_with_registry, proto::a2a_service_client::A2aServiceClient,
    proto::DiscoverRequest, proto::A2aMessage, proto::MessageAck,
};
use agent_world_engine::world::EventBus;

/// Pick a random available port.
fn find_free_port() -> u16 {
    use std::net::{TcpListener, Ipv4Addr};
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    listener.local_addr().unwrap().port()
}

#[tokio::test]
async fn grpc_server_starts_and_responds() {
    let port = find_free_port();
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let event_bus = Arc::new(EventBus::new(64));

    // Create registry with test agents
    let registry = Arc::new(AgentRegistry::new());
    registry
        .register(AgentRecord {
            agent_id: "agent-alice".into(),
            name: "Alice".into(),
            tokens: 1000,
            money: 500,
            skills: vec!["coding".into(), "research".into()],
            reputation: 4.5,
            phase: "adult".into(),
        })
        .await;
    registry
        .register(AgentRecord {
            agent_id: "agent-bob".into(),
            name: "Bob".into(),
            tokens: 800,
            money: 300,
            skills: vec!["trading".into()],
            reputation: 3.8,
            phase: "adult".into(),
        })
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
    assert_eq!(alice.tokens, 1000);
    assert_eq!(alice.skills, vec!["coding".to_string(), "research".to_string()]);

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

#[tokio::test]
async fn grpc_stream_messages_bidirectional() {
    let port = find_free_port();
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let event_bus = Arc::new(EventBus::new(64));

    let registry = Arc::new(AgentRegistry::new());
    registry
        .register(AgentRecord {
            agent_id: "agent-alice".into(),
            name: "Alice".into(),
            tokens: 1000,
            money: 500,
            skills: vec!["coding".into()],
            reputation: 4.5,
            phase: "adult".into(),
        })
        .await;
    registry
        .register(AgentRecord {
            agent_id: "agent-bob".into(),
            name: "Bob".into(),
            tokens: 800,
            money: 300,
            skills: vec!["trading".into()],
            reputation: 3.8,
            phase: "adult".into(),
        })
        .await;

    let handle = start_grpc_server_with_registry(addr, event_bus, registry)
        .await
        .expect("gRPC server should start");

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let url = format!("http://127.0.0.1:{}", port);
    let mut client = A2aServiceClient::connect(url)
        .await
        .expect("Should connect to gRPC server");

    // Open a bidirectional stream
    let (tx, rx) = tokio::sync::mpsc::channel(4);
    let response = client.stream_messages(tokio_stream::wrappers::ReceiverStream::new(rx)).await.unwrap();
    let mut response_stream = response.into_inner();

    // Send a message through the stream
    tx.send(A2aMessage {
        id: "stream-msg-001".into(),
        from_agent: "agent-alice".into(),
        to_agent: "agent-bob".into(),
        r#type: 4, // INFORM
        payload: b"Hello from stream!".to_vec(),
        timestamp: 1234567890,
        signature: String::new(),
        nonce: "nonce-stream-1".into(),
    }).await.unwrap();

    // Should receive an ACK back
    let ack_msg = tokio::time::timeout(
        tokio::time::Duration::from_secs(5),
        response_stream.message(),
    )
    .await
    .expect("Should receive response within timeout")
    .expect("No error in stream")
    .expect("Should get a response message");

    assert_eq!(ack_msg.r#type, 2, "Response should be ACCEPT (type 2)");
    assert_eq!(ack_msg.to_agent, "agent-alice");
    assert!(ack_msg.payload.starts_with(b"Message stream-msg-001 delivered"));

    // Send a message to an unknown agent
    tx.send(A2aMessage {
        id: "stream-msg-002".into(),
        from_agent: "agent-alice".into(),
        to_agent: "unknown-agent".into(),
        r#type: 4,
        payload: vec![],
        timestamp: 1234567891,
        signature: String::new(),
        nonce: "nonce-stream-2".into(),
    }).await.unwrap();

    let reject_msg = tokio::time::timeout(
        tokio::time::Duration::from_secs(5),
        response_stream.message(),
    )
    .await
    .expect("Should receive reject within timeout")
    .expect("No error in stream")
    .expect("Should get a reject message");

    assert_eq!(reject_msg.r#type, 3, "Response should be REJECT (type 3)");
    assert_eq!(reject_msg.to_agent, "agent-alice");

    // Drop sender to close the stream
    drop(tx);
    handle.abort();
}

#[tokio::test]
async fn grpc_eventbus_integration() {
    let port = find_free_port();
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let event_bus = Arc::new(EventBus::new(64));

    // Subscribe to events before starting server
    let mut event_rx = event_bus.subscribe();

    let registry = Arc::new(AgentRegistry::new());
    registry
        .register(AgentRecord {
            agent_id: "agent-alice".into(),
            name: "Alice".into(),
            tokens: 1000,
            money: 500,
            skills: vec!["coding".into()],
            reputation: 4.5,
            phase: "adult".into(),
        })
        .await;
    registry
        .register(AgentRecord {
            agent_id: "agent-bob".into(),
            name: "Bob".into(),
            tokens: 800,
            money: 300,
            skills: vec!["trading".into()],
            reputation: 3.8,
            phase: "adult".into(),
        })
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

    // Verify that the EventBus received a TransactionCompleted event
    let event = tokio::time::timeout(
        tokio::time::Duration::from_secs(5),
        event_rx.recv(),
    )
    .await
    .expect("Should receive event within timeout")
    .expect("Should get event");

    match event {
        agent_world_engine::world::event::WorldEvent::TransactionCompleted { from, to, .. } => {
            assert_eq!(from, "agent-alice");
            assert_eq!(to, "agent-bob");
        }
        other => panic!("Expected TransactionCompleted event, got {:?}", other.event_type()),
    }

    handle.abort();
}
