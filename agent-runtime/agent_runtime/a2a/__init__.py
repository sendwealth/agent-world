"""A2A gRPC client — connects the Think Loop to the World Engine.

Submodules:
    config      — connection configuration and retry policy
    message     — A2AMessage builder and converter helpers
    client      — low-level gRPC client (sync + streaming)
    world_client — GRPCWorldClient implementing WorldClientProtocol (ACT phase)
    perception  — GRPCPerceptionProvider implementing PerceptionProvider (SENSE phase)
"""
