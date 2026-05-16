"""A2A client configuration — server address, retry policy, timeouts."""

from __future__ import annotations

from dataclasses import dataclass, field


@dataclass
class RetryPolicy:
    """Exponential-backoff retry configuration for gRPC calls.

    Attributes:
        max_retries: Maximum number of retry attempts (0 = no retry).
        base_delay: Initial delay in seconds before the first retry.
        max_delay: Maximum delay cap in seconds.
        jitter: Random jitter fraction (0.0 – 1.0) added to each delay.
        retryable_codes: gRPC status codes that should trigger a retry.
    """

    max_retries: int = 3
    base_delay: float = 0.5
    max_delay: float = 10.0
    jitter: float = 0.2
    retryable_codes: tuple[str, ...] = (
        "UNAVAILABLE",
        "DEADLINE_EXCEEDED",
        "RESOURCE_EXHAUSTED",
    )


@dataclass
class A2AClientConfig:
    """Configuration for the A2A gRPC client.

    Attributes:
        server_address: ``host:port`` of the World Engine gRPC server.
        agent_id: This agent's unique identifier.
        timeout: Default RPC timeout in seconds.
        retry_policy: Retry configuration for transient failures.
        enable_streaming: Whether to maintain a bidirectional streaming connection.
        stream_reconnect_delay: Seconds to wait before reconnecting a dropped stream.
    """

    server_address: str = "localhost:50051"
    agent_id: str = ""
    timeout: float = 5.0
    retry_policy: RetryPolicy = field(default_factory=RetryPolicy)
    enable_streaming: bool = True
    stream_reconnect_delay: float = 2.0
