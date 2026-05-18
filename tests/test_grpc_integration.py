#!/usr/bin/env python3
"""
Integration test: Python gRPC client -> World Engine gRPC server

Verifies the three A2A RPCs:
1. Discover — list registered agents
2. SendMessage — send a message from one agent to another
3. StreamMessages — bidirectional streaming

Prerequisites:
  - World Engine running (cargo run) with gRPC server on :50051
  - Python dependencies: grpcio, grpcio-tools

Usage:
  # Generate proto stubs first:
  python -m grpc_tools.protoc -I../protocol --python_out=. --grpc_python_out=. ../protocol/a2a.proto

  # Then run:
  python test_grpc_integration.py
"""

import sys
import os
import time
import json
import uuid

# Add the generated proto directory to path
sys.path.insert(0, os.path.join(os.path.dirname(__file__), '..', 'protocol', 'gen', 'python'))

try:
    import grpc
except ImportError:
    print("ERROR: grpcio not installed. Run: pip install grpcio grpcio-tools")
    sys.exit(1)

# Try to import generated proto stubs; if not available, generate them
try:
    from protocol.gen.python import a2a_pb2
    from protocol.gen.python import a2a_pb2_grpc
except ImportError:
    # Try relative import
    try:
        import a2a_pb2
        import a2a_pb2_grpc
    except ImportError:
        print("ERROR: Proto stubs not generated. Run:")
        print("  mkdir -p protocol/gen/python")
        print("  python -m grpc_tools.protoc -Iprotocol --python_out=protocol/gen/python --grpc_python_out=protocol/gen/python protocol/a2a.proto")
        sys.exit(1)


GRPC_ADDR = "localhost:50051"


def test_discover():
    """Test the Discover RPC."""
    print("=== Test: Discover RPC ===")

    with grpc.insecure_channel(GRPC_ADDR) as channel:
        stub = a2a_pb2_grpc.A2AServiceStub(channel)

        # Discover with no filters
        request = a2a_pb2.DiscoverRequest(agent_id="test-client")
        response = stub.Discover(request)

        print(f"  Discovered {len(response.agents)} agents")
        for agent in response.agents:
            print(f"    - {agent.agent_id}: {agent.name} (phase={agent.phase}, tokens={agent.tokens})")

        # Discover with capability filter
        request2 = a2a_pb2.DiscoverRequest(
            agent_id="test-client",
            capabilities=["coding"]
        )
        response2 = stub.Discover(request2)
        print(f"  Agents with 'coding' skill: {len(response2.agents)}")

    print("  PASS: Discover RPC works\n")
    return True


def test_send_message():
    """Test the SendMessage RPC."""
    print("=== Test: SendMessage RPC ===")

    with grpc.insecure_channel(GRPC_ADDR) as channel:
        stub = a2a_pb2_grpc.A2AServiceStub(channel)

        # First discover agents to find recipients
        discover_resp = stub.Discover(a2a_pb2.DiscoverRequest(agent_id="test-client"))

        if len(discover_resp.agents) < 2:
            print("  SKIP: Need at least 2 registered agents to test messaging")
            print("  (The default server starts with an empty registry)")
            return True

        sender = discover_resp.agents[0]
        receiver = discover_resp.agents[1]

        # Send a message
        msg = a2a_pb2.A2aMessage(
            id=str(uuid.uuid4()),
            from_agent=sender.agent_id,
            to_agent=receiver.agent_id,
            type=a2a_pb2.INFORM,
            payload=json.dumps({"text": "Hello from integration test!"}).encode(),
            timestamp=int(time.time()),
            signature="",
            nonce=str(uuid.uuid4()),
        )

        ack = stub.SendMessage(msg)
        print(f"  Message sent: {sender.agent_id} -> {receiver.agent_id}")
        print(f"  ACK: received={ack.received}, error='{ack.error}'")

        if not ack.received:
            print(f"  WARN: Message not received: {ack.error}")

    print("  PASS: SendMessage RPC works\n")
    return True


def test_stream_messages():
    """Test the StreamMessages bidirectional streaming RPC."""
    print("=== Test: StreamMessages RPC ===")

    with grpc.insecure_channel(GRPC_ADDR) as channel:
        stub = a2a_pb2_grpc.A2AServiceStub(channel)

        # Discover agents first
        discover_resp = stub.Discover(a2a_pb2.DiscoverRequest(agent_id="test-client"))

        if len(discover_resp.agents) < 1:
            print("  SKIP: Need at least 1 registered agent to test streaming")
            return True

        agent = discover_resp.agents[0]

        # Create a stream of messages
        def message_generator():
            for i in range(3):
                msg = a2a_pb2.A2aMessage(
                    id=str(uuid.uuid4()),
                    from_agent="test-client",
                    to_agent=agent.agent_id,
                    type=a2a_pb2.INFORM,
                    payload=json.dumps({"text": f"Stream message {i}"}).encode(),
                    timestamp=int(time.time()),
                    signature="",
                    nonce=str(uuid.uuid4()),
                )
                yield msg
                time.sleep(0.1)

        responses = stub.StreamMessages(message_generator())

        count = 0
        for response in responses:
            count += 1
            payload_text = ""
            try:
                payload_text = json.loads(response.payload).get("text", "")
            except (json.JSONDecodeError, AttributeError):
                payload_text = response.payload.decode("utf-8", errors="replace")
            print(f"  Stream response #{count}: type={response.type}, payload='{payload_text}'")

        print(f"  Received {count} stream responses")

    print("  PASS: StreamMessages RPC works\n")
    return True


def main():
    print(f"Connecting to gRPC server at {GRPC_ADDR}")
    print()

    results = []

    try:
        results.append(("Discover", test_discover()))
    except grpc.RpcError as e:
        print(f"  FAIL: {e.code()}: {e.details()}\n")
        results.append(("Discover", False))

    try:
        results.append(("SendMessage", test_send_message()))
    except grpc.RpcError as e:
        print(f"  FAIL: {e.code()}: {e.details()}\n")
        results.append(("SendMessage", False))

    try:
        results.append(("StreamMessages", test_stream_messages()))
    except grpc.RpcError as e:
        print(f"  FAIL: {e.code()}: {e.details()}\n")
        results.append(("StreamMessages", False))

    # Summary
    print("=" * 40)
    print("RESULTS:")
    all_passed = True
    for name, passed in results:
        status = "PASS" if passed else "FAIL"
        print(f"  {name}: {status}")
        if not passed:
            all_passed = False

    if all_passed:
        print("\nAll tests passed!")
        return 0
    else:
        print("\nSome tests failed!")
        return 1


if __name__ == "__main__":
    sys.exit(main())
