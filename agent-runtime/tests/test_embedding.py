"""Tests for the embedding provider module.

Covers:
- HashEmbeddingProvider: deterministic hash-based embeddings
- Embedding dimension validation
- Batch embedding
- EmbeddingProviderProtocol compliance
- Cosine similarity properties (similar text -> higher similarity)
"""

from __future__ import annotations

import math

import pytest

from agent_runtime.memory.embedding import (
    EmbeddingProviderProtocol,
    HashEmbeddingProvider,
)


# ---------------------------------------------------------------------------
# Fixtures
# ---------------------------------------------------------------------------


@pytest.fixture
def provider() -> HashEmbeddingProvider:
    """Return a HashEmbeddingProvider with default settings."""
    return HashEmbeddingProvider(dimension=64)


@pytest.fixture
def small_provider() -> HashEmbeddingProvider:
    """Return a HashEmbeddingProvider with small dimension."""
    return HashEmbeddingProvider(dimension=16, ngram_size=2)


# ---------------------------------------------------------------------------
# Construction
# ---------------------------------------------------------------------------


class TestConstruction:
    def test_default_dimension(self) -> None:
        p = HashEmbeddingProvider()
        assert p.dimension == 128

    def test_custom_dimension(self) -> None:
        p = HashEmbeddingProvider(dimension=256)
        assert p.dimension == 256

    def test_zero_dimension_raises(self) -> None:
        with pytest.raises(ValueError, match="positive integer"):
            HashEmbeddingProvider(dimension=0)

    def test_negative_dimension_raises(self) -> None:
        with pytest.raises(ValueError, match="positive integer"):
            HashEmbeddingProvider(dimension=-10)

    def test_zero_ngram_raises(self) -> None:
        with pytest.raises(ValueError, match="positive integer"):
            HashEmbeddingProvider(ngram_size=0)


# ---------------------------------------------------------------------------
# Embedding Generation
# ---------------------------------------------------------------------------


class TestEmbed:
    def test_output_dimension(self, provider: HashEmbeddingProvider) -> None:
        vec = provider.embed("hello world")
        assert len(vec) == 64

    def test_deterministic(self, provider: HashEmbeddingProvider) -> None:
        """Same text always produces same embedding."""
        v1 = provider.embed("test text")
        v2 = provider.embed("test text")
        assert v1 == v2

    def test_different_text_different_embedding(
        self, provider: HashEmbeddingProvider
    ) -> None:
        v1 = provider.embed("hello world")
        v2 = provider.embed("goodbye moon")
        assert v1 != v2

    def test_empty_string(self, provider: HashEmbeddingProvider) -> None:
        vec = provider.embed("")
        assert len(vec) == 64
        assert all(v == 0.0 for v in vec)

    def test_normalized(self, provider: HashEmbeddingProvider) -> None:
        """Non-empty text produces a unit vector (L2 norm approx 1.0)."""
        vec = provider.embed("some text")
        norm = math.sqrt(sum(v * v for v in vec))
        assert abs(norm - 1.0) < 0.01 or norm == 0.0

    def test_case_insensitive_similarity(
        self, provider: HashEmbeddingProvider
    ) -> None:
        """Lowercase and uppercase versions should have high similarity."""
        v1 = provider.embed("Hello World")
        v2 = provider.embed("hello world")
        sim = _cosine_sim(v1, v2)
        assert sim > 0.9


class TestEmbedBatch:
    def test_batch_correct_count(self, provider: HashEmbeddingProvider) -> None:
        texts = ["hello", "world", "test"]
        vecs = provider.embed_batch(texts)
        assert len(vecs) == 3
        assert all(len(v) == 64 for v in vecs)

    def test_batch_matches_individual(
        self, provider: HashEmbeddingProvider
    ) -> None:
        texts = ["alpha", "beta"]
        batch = provider.embed_batch(texts)
        individual = [provider.embed(t) for t in texts]
        assert batch[0] == individual[0]
        assert batch[1] == individual[1]


# ---------------------------------------------------------------------------
# Semantic Properties
# ---------------------------------------------------------------------------


class TestSemanticProperties:
    def test_similar_text_high_similarity(
        self, provider: HashEmbeddingProvider
    ) -> None:
        """Semantically similar text should have higher cosine similarity."""
        v1 = provider.embed("trading goods at the market")
        v2 = provider.embed("trading goods at the market")
        v3 = provider.embed("fighting enemies in the dungeon")
        sim_similar = _cosine_sim(v1, v2)
        sim_different = _cosine_sim(v1, v3)
        assert sim_similar > sim_different

    def test_shared_words_similarity(
        self, provider: HashEmbeddingProvider
    ) -> None:
        """Texts sharing words should be more similar than unrelated texts."""
        v1 = provider.embed("buy food at market")
        v2 = provider.embed("sell food at market")
        v3 = provider.embed("fight dragon in cave")
        sim_shared = _cosine_sim(v1, v2)
        sim_unrelated = _cosine_sim(v1, v3)
        assert sim_shared > sim_unrelated


# ---------------------------------------------------------------------------
# Protocol
# ---------------------------------------------------------------------------


class TestProtocol:
    def test_hash_provider_satisfies_protocol(
        self, provider: HashEmbeddingProvider
    ) -> None:
        assert isinstance(provider, EmbeddingProviderProtocol)


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def _cosine_sim(a: list[float], b: list[float]) -> float:
    dot = sum(x * y for x, y in zip(a, b))
    na = math.sqrt(sum(x * x for x in a))
    nb = math.sqrt(sum(x * x for x in b))
    if na == 0 or nb == 0:
        return 0.0
    return dot / (na * nb)
