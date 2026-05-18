"""Deterministic JSON serialization, message signing, and signature verification."""

from __future__ import annotations

import json
from typing import Any, Dict, Optional

from cryptography.exceptions import InvalidSignature
from cryptography.hazmat.primitives.asymmetric.ed25519 import (
    Ed25519PublicKey,
)

from .keys import KeyPair


def canonicalize(data: Dict[str, Any]) -> bytes:
    """Deterministically serialize a dict to UTF-8 JSON bytes.

    Rules:
    - Sort keys recursively (including nested dicts).
    - No whitespace (separators=(',', ':')).
    - UTF-8 encoded.
    - Ensures the same logical message always produces the same bytes.
    """
    return json.dumps(
        _sort_keys_recursive(data),
        separators=(",", ":"),
        ensure_ascii=False,
    ).encode("utf-8")


def _sort_keys_recursive(obj: Any) -> Any:
    """Recursively sort dictionary keys for deterministic serialization."""
    if isinstance(obj, dict):
        return {k: _sort_keys_recursive(v) for k, v in sorted(obj.items())}
    if isinstance(obj, list):
        return [_sort_keys_recursive(item) for item in obj]
    return obj


def sign_message(
    message: Dict[str, Any],
    key_pair: KeyPair,
    fields: Optional[list[str]] = None,
) -> str:
    """Sign selected fields of a message and return hex-encoded signature.

    Args:
        message: The full message dict.
        key_pair: The signer's Ed25519 key pair.
        fields: Specific keys to include in the signed payload.
                Defaults to all keys except 'signature'.

    Returns:
        Hex-encoded ed25519 signature string.
    """
    if fields is None:
        fields = [k for k in message if k != "signature"]

    payload = {k: message[k] for k in fields if k in message}
    canonical = canonicalize(payload)
    sig_bytes = key_pair.private_key.sign(canonical)
    return sig_bytes.hex()


def verify_message(
    message: Dict[str, Any],
    signature_hex: str,
    public_key_bytes: bytes,
    fields: Optional[list[str]] = None,
) -> bool:
    """Verify an ed25519 signature on a message.

    Args:
        message: The full message dict.
        signature_hex: Hex-encoded signature to verify.
        public_key_bytes: Raw 32-byte ed25519 public key of the signer.
        fields: Specific keys included in the signed payload.
                Defaults to all keys except 'signature'.

    Returns:
        True if the signature is valid, False otherwise.
    """
    if fields is None:
        fields = [k for k in message if k != "signature"]

    payload = {k: message[k] for k in fields if k in message}
    canonical = canonicalize(payload)
    sig_bytes = bytes.fromhex(signature_hex)
    public_key = Ed25519PublicKey.from_public_bytes(public_key_bytes)

    try:
        public_key.verify(sig_bytes, canonical)
        return True
    except InvalidSignature:
        return False
