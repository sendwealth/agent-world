"""A2A gRPC client — connects the Think Loop to the World Engine.

Submodules:
    config      — connection configuration and retry policy
    message     — A2AMessage builder and converter helpers
    client      — low-level gRPC client (sync + streaming)
    world_client — GRPCWorldClient implementing WorldClientProtocol (ACT phase)
    perception  — GRPCPerceptionProvider implementing PerceptionProvider (SENSE phase)
    batch_client — BatchA2AClient for coalesced message sending
"""

from .batch_client import BatchA2AClient
from .client import A2AClient
from .config import A2AClientConfig, RetryPolicy
from .perception import GRPCPerceptionProvider
from .world_client import GRPCWorldClient

__all__ = [
    "A2AClient",
    "A2AClientConfig",
    "BatchA2AClient",
    "GRPCPerceptionProvider",
    "GRPCWorldClient",
    "RetryPolicy",
]
