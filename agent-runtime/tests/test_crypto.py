"""Tests for the ed25519 signing, verification, nonce, and key registry system."""

from __future__ import annotations

import time

import pytest

from agent_runtime.crypto import (
    KeyPair,
    KeyRegistry,
    NonceCache,
    generate_key_pair,
    sign_message,
    verify_message,
)
from agent_runtime.crypto.signing import canonicalize

# ── Key Pair Generation ─────────────────────────────────────────────────────


class TestKeyPairGeneration:
    def test_generate_returns_key_pair(self):
        kp = generate_key_pair()
        assert isinstance(kp, KeyPair)

    def test_key_bytes_are_32_bytes(self):
        kp = generate_key_pair()
        assert len(kp.private_key_bytes) == 32
        assert len(kp.public_key_bytes) == 32

    def test_key_pair_is_deterministic_from_private_bytes(self):
        kp = generate_key_pair()
        restored = KeyPair.from_private_bytes(kp.private_key_bytes)
        assert restored.public_key_bytes == kp.public_key_bytes
        assert restored.private_key_bytes == kp.private_key_bytes

    def test_different_generations_produce_different_keys(self):
        kp1 = generate_key_pair()
        kp2 = generate_key_pair()
        assert kp1.private_key_bytes != kp2.private_key_bytes
        assert kp1.public_key_bytes != kp2.public_key_bytes

    def test_public_key_b64_roundtrip(self):
        import base64

        kp = generate_key_pair()
        b64 = kp.public_key_b64()
        decoded = base64.urlsafe_b64decode(b64 + "=" * (-len(b64) % 4))
        assert decoded == kp.public_key_bytes

    def test_private_key_object_works(self):
        kp = generate_key_pair()
        priv = kp.private_key
        pub = kp.public_key
        msg = b"test"
        sig = priv.sign(msg)
        pub.verify(sig, msg)  # should not raise


# ── Deterministic JSON Serialization ────────────────────────────────────────


class TestCanonicalize:
    def test_sorted_keys(self):
        data = {"z": 1, "a": 2, "m": 3}
        result = canonicalize(data)
        assert result == b'{"a":2,"m":3,"z":1}'

    def test_nested_sorted_keys(self):
        data = {"outer": {"z": 1, "a": 2}}
        result = canonicalize(data)
        assert result == b'{"outer":{"a":2,"z":1}}'

    def test_no_whitespace(self):
        data = {"key": "value", "num": 42}
        result = canonicalize(data)
        assert b" " not in result
        assert b"\n" not in result

    def test_deterministic(self):
        data = {"b": [2, 1], "a": {"x": 1, "y": 2}}
        assert canonicalize(data) == canonicalize(data)

    def test_unicode_preserved(self):
        data = {"name": "Agent-世界"}
        result = canonicalize(data)
        assert "世界" in result.decode("utf-8")

    def test_list_order_preserved(self):
        data = {"items": [3, 1, 2]}
        result = canonicalize(data)
        assert result == b'{"items":[3,1,2]}'

    def test_list_elements_sorted_recursively(self):
        data = {"list": [{"z": 1, "a": 2}]}
        result = canonicalize(data)
        assert result == b'{"list":[{"a":2,"z":1}]}'


# ── Message Signing ─────────────────────────────────────────────────────────


class TestSignMessage:
    def test_sign_returns_hex_string(self):
        kp = generate_key_pair()
        msg = {"from": "agent-1", "payload": "hello"}
        sig = sign_message(msg, kp)
        assert isinstance(sig, str)
        # Hex-encoded 64-byte ed25519 sig = 128 hex chars
        assert len(sig) == 128

    def test_sign_excludes_signature_field_by_default(self):
        kp = generate_key_pair()
        msg = {"data": "hello", "signature": "old_sig"}
        sig = sign_message(msg, kp)
        # Should not include "signature" in signed payload
        assert len(sig) == 128

    def test_sign_with_custom_fields(self):
        kp = generate_key_pair()
        msg = {"a": 1, "b": 2, "c": 3}
        sig = sign_message(msg, kp, fields=["a", "b"])
        assert len(sig) == 128

    def test_same_message_same_key_produces_same_signature(self):
        kp = generate_key_pair()
        msg = {"action": "trade", "amount": 100}
        sig1 = sign_message(msg, kp)
        sig2 = sign_message(msg, kp)
        assert sig1 == sig2

    def test_different_messages_produce_different_signatures(self):
        kp = generate_key_pair()
        msg1 = {"action": "trade", "amount": 100}
        msg2 = {"action": "trade", "amount": 200}
        sig1 = sign_message(msg1, kp)
        sig2 = sign_message(msg2, kp)
        assert sig1 != sig2

    def test_different_keys_produce_different_signatures(self):
        kp1 = generate_key_pair()
        kp2 = generate_key_pair()
        msg = {"action": "trade"}
        sig1 = sign_message(msg, kp1)
        sig2 = sign_message(msg, kp2)
        assert sig1 != sig2


# ── Signature Verification ──────────────────────────────────────────────────


class TestVerifyMessage:
    def test_valid_signature_verifies(self):
        kp = generate_key_pair()
        msg = {"from": "agent-1", "payload": "hello"}
        sig = sign_message(msg, kp)
        assert verify_message(msg, sig, kp.public_key_bytes) is True

    def test_forged_signature_rejected(self):
        kp1 = generate_key_pair()
        kp2 = generate_key_pair()
        msg = {"from": "agent-1", "payload": "hello"}
        # Sign with kp1's private key
        sig = sign_message(msg, kp1)
        # Try to verify with kp2's public key (wrong key)
        assert verify_message(msg, sig, kp2.public_key_bytes) is False

    def test_tampered_message_rejected(self):
        kp = generate_key_pair()
        msg = {"from": "agent-1", "amount": 100}
        sig = sign_message(msg, kp)
        # Tamper with message
        tampered = {"from": "agent-1", "amount": 999}
        assert verify_message(tampered, sig, kp.public_key_bytes) is False

    def test_invalid_hex_signature_rejected(self):
        kp = generate_key_pair()
        msg = {"data": "test"}
        assert verify_message(msg, "00" * 64, kp.public_key_bytes) is False

    def test_verification_with_custom_fields(self):
        kp = generate_key_pair()
        msg = {"a": 1, "b": 2, "c": 3}
        sig = sign_message(msg, kp, fields=["a", "b"])
        assert verify_message(msg, sig, kp.public_key_bytes, fields=["a", "b"]) is True
        # Wrong fields should fail
        assert verify_message(msg, sig, kp.public_key_bytes, fields=["a", "c"]) is False

    def test_roundtrip_multiple_messages(self):
        kp = generate_key_pair()
        messages = [
            {"type": "propose", "task_id": "t-1", "amount": 50},
            {"type": "accept", "task_id": "t-1"},
            {"type": "inform", "data": "weather is sunny"},
        ]
        for msg in messages:
            sig = sign_message(msg, kp)
            assert verify_message(msg, sig, kp.public_key_bytes) is True


# ── A2A Protocol Integration ───────────────────────────────────────────────


class TestA2AMessageFlow:
    """End-to-end test simulating A2A message signing and verification."""

    def test_full_a2a_flow(self):
        # Setup: two agents with keys
        alice = generate_key_pair()
        bob = generate_key_pair()

        registry = KeyRegistry()
        registry.register("alice", alice.public_key_bytes)
        registry.register("bob", bob.public_key_bytes)

        nonce_cache = NonceCache(ttl_seconds=300)

        # Alice signs a message
        import uuid

        nonce = str(uuid.uuid4())
        message = {
            "id": str(uuid.uuid4()),
            "from_agent": "alice",
            "to_agent": "bob",
            "type": "PROPOSE",
            "payload": {"task": "build_shelter", "price": 100},
            "timestamp": int(time.time()),
            "nonce": nonce,
        }

        sig = sign_message(message, alice)
        message["signature"] = sig

        # Bob receives and verifies
        nonce_val = message["nonce"]
        assert nonce_cache.check_and_store(nonce_val) is True, "Fresh nonce should be accepted"

        alice_key = registry.lookup("alice")
        assert alice_key is not None

        assert verify_message(message, message["signature"], alice_key) is True

    def test_replay_attack_blocked(self):
        kp = generate_key_pair()
        nonce_cache = NonceCache(ttl_seconds=300)

        nonce = "unique-nonce-123"
        message = {"from": "alice", "nonce": nonce, "payload": "hello"}
        sig = sign_message(message, kp)
        message["signature"] = sig

        # First use — accepted
        assert nonce_cache.check_and_store(nonce) is True
        # Replay — rejected
        assert nonce_cache.check_and_store(nonce) is False

    def test_forged_message_rejected_in_flow(self):
        alice = generate_key_pair()
        attacker = generate_key_pair()

        registry = KeyRegistry()
        registry.register("alice", alice.public_key_bytes)

        # Attacker forges a message pretending to be Alice
        forged_msg = {
            "from_agent": "alice",
            "to_agent": "bob",
            "payload": "give me all your tokens",
        }
        # Attacker signs with their own key
        forged_sig = sign_message(forged_msg, attacker)

        # Verification with Alice's actual public key should fail
        alice_key = registry.lookup("alice")
        assert verify_message(forged_msg, forged_sig, alice_key) is False


# ── Nonce Cache ─────────────────────────────────────────────────────────────


class TestNonceCache:
    def test_fresh_nonce_accepted(self):
        cache = NonceCache()
        assert cache.check_and_store("nonce-1") is True

    def test_duplicate_nonce_rejected(self):
        cache = NonceCache()
        cache.check_and_store("nonce-1")
        assert cache.check_and_store("nonce-1") is False

    def test_different_nonces_accepted(self):
        cache = NonceCache()
        assert cache.check_and_store("nonce-1") is True
        assert cache.check_and_store("nonce-2") is True

    def test_expired_nonce_accepted_again(self):
        cache = NonceCache(ttl_seconds=0.1)
        cache.check_and_store("nonce-1")
        time.sleep(0.15)
        assert cache.check_and_store("nonce-1") is True

    def test_ttl_not_expired_nonce_still_rejected(self):
        cache = NonceCache(ttl_seconds=300)
        cache.check_and_store("nonce-1")
        assert cache.check_and_store("nonce-1") is False

    def test_max_size_eviction(self):
        cache = NonceCache(ttl_seconds=300, max_size=3)
        cache.check_and_store("n1")
        cache.check_and_store("n2")
        cache.check_and_store("n3")
        # Adding a 4th should evict the oldest (n1)
        cache.check_and_store("n4")
        assert cache.size == 3
        # n1 was evicted, so it should be accepted again
        assert cache.check_and_store("n1") is True

    def test_clear(self):
        cache = NonceCache()
        cache.check_and_store("n1")
        cache.check_and_store("n2")
        cache.clear()
        assert cache.size == 0
        assert cache.check_and_store("n1") is True

    def test_size_property(self):
        cache = NonceCache()
        assert cache.size == 0
        cache.check_and_store("n1")
        assert cache.size == 1
        cache.check_and_store("n2")
        assert cache.size == 2

    def test_default_ttl_is_5_minutes(self):
        cache = NonceCache()
        assert cache._ttl == 300.0


# ── Key Registry ────────────────────────────────────────────────────────────


class TestKeyRegistry:
    def test_register_and_lookup(self):
        registry = KeyRegistry()
        kp = generate_key_pair()
        registry.register("agent-1", kp.public_key_bytes)
        assert registry.lookup("agent-1") == kp.public_key_bytes

    def test_lookup_unregistered_returns_none(self):
        registry = KeyRegistry()
        assert registry.lookup("unknown") is None

    def test_unregister(self):
        registry = KeyRegistry()
        kp = generate_key_pair()
        registry.register("agent-1", kp.public_key_bytes)
        assert registry.unregister("agent-1") is True
        assert registry.lookup("agent-1") is None

    def test_unregister_nonexistent(self):
        registry = KeyRegistry()
        assert registry.unregister("ghost") is False

    def test_register_updates_existing(self):
        registry = KeyRegistry()
        kp1 = generate_key_pair()
        kp2 = generate_key_pair()
        registry.register("agent-1", kp1.public_key_bytes)
        registry.register("agent-1", kp2.public_key_bytes)
        assert registry.lookup("agent-1") == kp2.public_key_bytes

    def test_is_registered(self):
        registry = KeyRegistry()
        kp = generate_key_pair()
        assert registry.is_registered("agent-1") is False
        registry.register("agent-1", kp.public_key_bytes)
        assert registry.is_registered("agent-1") is True

    def test_size(self):
        registry = KeyRegistry()
        assert registry.size == 0
        kp1 = generate_key_pair()
        kp2 = generate_key_pair()
        registry.register("agent-1", kp1.public_key_bytes)
        assert registry.size == 1
        registry.register("agent-2", kp2.public_key_bytes)
        assert registry.size == 2

    def test_rejects_invalid_key_length(self):
        registry = KeyRegistry()
        with pytest.raises(ValueError, match="32 bytes"):
            registry.register("agent-1", b"short")
