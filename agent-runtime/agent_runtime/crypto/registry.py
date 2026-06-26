"""Public key registry — binds agent IDs to their ed25519 public keys."""

from __future__ import annotations

from dataclasses import dataclass, field


@dataclass
class KeyRegistry:
    """In-memory registry mapping agent IDs to their public keys.

    Used during agent registration to bind a public key, and during
    message verification to look up the signer's key.
    """

    _keys: dict[str, bytes] = field(default_factory=dict)

    def register(self, agent_id: str, public_key_bytes: bytes) -> None:
        """Register or update an agent's public key.

        Args:
            agent_id: Unique agent identifier.
            public_key_bytes: Raw 32-byte ed25519 public key.
        """
        if len(public_key_bytes) != 32:
            raise ValueError(
                f"Ed25519 public key must be exactly 32 bytes, got {len(public_key_bytes)}"
            )
        self._keys[agent_id] = public_key_bytes

    def lookup(self, agent_id: str) -> bytes | None:
        """Look up an agent's public key.

        Returns None if the agent is not registered.
        """
        return self._keys.get(agent_id)

    def unregister(self, agent_id: str) -> bool:
        """Remove an agent's public key from the registry.

        Returns True if the agent was found and removed, False otherwise.
        """
        return self._keys.pop(agent_id, None) is not None

    def is_registered(self, agent_id: str) -> bool:
        """Check if an agent has a registered public key."""
        return agent_id in self._keys

    @property
    def size(self) -> int:
        """Number of registered agents."""
        return len(self._keys)
