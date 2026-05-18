"""Ed25519 key pair generation and serialization."""

from __future__ import annotations

from dataclasses import dataclass

from cryptography.hazmat.primitives.asymmetric.ed25519 import (
    Ed25519PrivateKey,
    Ed25519PublicKey,
)
from cryptography.hazmat.primitives.serialization import (
    Encoding,
    NoEncryption,
    PrivateFormat,
    PublicFormat,
)


@dataclass(frozen=True)
class KeyPair:
    """An Ed25519 key pair with raw bytes access."""

    private_key_bytes: bytes
    public_key_bytes: bytes

    @property
    def private_key(self) -> Ed25519PrivateKey:
        return Ed25519PrivateKey.from_private_bytes(self.private_key_bytes)

    @property
    def public_key(self) -> Ed25519PublicKey:
        return Ed25519PublicKey.from_public_bytes(self.public_key_bytes)

    def public_key_b64(self) -> str:
        """Return the public key as URL-safe base64 (no padding)."""
        import base64

        return base64.urlsafe_b64encode(self.public_key_bytes).decode("ascii").rstrip("=")

    def private_key_b64(self) -> str:
        """Return the private key as URL-safe base64 (no padding)."""
        import base64

        return base64.urlsafe_b64encode(self.private_key_bytes).decode("ascii").rstrip("=")

    @classmethod
    def from_private_bytes(cls, raw: bytes) -> KeyPair:
        """Reconstruct a KeyPair from the 32-byte private key seed."""
        priv = Ed25519PrivateKey.from_private_bytes(raw)
        pub_bytes = priv.public_key().public_bytes(Encoding.Raw, PublicFormat.Raw)
        return cls(private_key_bytes=raw, public_key_bytes=pub_bytes)

    @classmethod
    def from_private_key(cls, key: Ed25519PrivateKey) -> KeyPair:
        """Reconstruct a KeyPair from an Ed25519PrivateKey object."""
        priv_bytes = key.private_bytes(Encoding.Raw, PrivateFormat.Raw, NoEncryption())
        pub_bytes = key.public_key().public_bytes(Encoding.Raw, PublicFormat.Raw)
        return cls(private_key_bytes=priv_bytes, public_key_bytes=pub_bytes)


def generate_key_pair() -> KeyPair:
    """Generate a new Ed25519 key pair."""
    private_key = Ed25519PrivateKey.generate()
    return KeyPair.from_private_key(private_key)
