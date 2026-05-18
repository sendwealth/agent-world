"""Embedding providers — pluggable embedding generation for semantic search.

Provides an ``EmbeddingProviderProtocol`` and three built-in implementations:

- ``HashEmbeddingProvider``: Deterministic hash-based embeddings that require
  no external model.  Fast, zero-dependency, suitable for development and
  testing.  Produces fixed-dimension vectors where semantically similar text
  gets similar embeddings (via character n-gram hashing).

- ``OpenAIEmbeddingProvider``: Uses the OpenAI embeddings API (text-embedding-
  3-small by default) for production-quality semantic embeddings.

- ``SentenceTransformerEmbeddingProvider``: Uses a local sentence-transformers
  model for zero-cost (no API) semantic embeddings.  Suitable for production
  use without external API dependencies.
"""

from __future__ import annotations

import hashlib
import logging
import math
from typing import Protocol, runtime_checkable

logger = logging.getLogger(__name__)

# Default embedding dimension for hash-based provider
_DEFAULT_DIMENSION: int = 128


# ---------------------------------------------------------------------------
# Protocol
# ---------------------------------------------------------------------------


@runtime_checkable
class EmbeddingProviderProtocol(Protocol):
    """Minimal interface for embedding generation."""

    @property
    def dimension(self) -> int:
        """Dimensionality of the embedding vectors produced."""
        ...

    def embed(self, text: str) -> list[float]:
        """Generate an embedding vector for the given text."""
        ...

    def embed_batch(self, texts: list[str]) -> list[list[float]]:
        """Generate embedding vectors for multiple texts."""
        ...


# ---------------------------------------------------------------------------
# Hash-based embedding provider
# ---------------------------------------------------------------------------


class HashEmbeddingProvider:
    """Deterministic hash-based embedding provider.

    Uses character n-gram hashing to produce fixed-dimension vectors.
    Semantically similar text (sharing n-grams) produces vectors with
    higher cosine similarity.  No external model or network required.

    Parameters
    ----------
    dimension : int
        Dimensionality of the output vectors.  Default 128.
    ngram_size : int
        Size of character n-grams used for hashing.  Default 3.
    """

    def __init__(
        self, dimension: int = _DEFAULT_DIMENSION, ngram_size: int = 3
    ) -> None:
        if dimension <= 0:
            raise ValueError("dimension must be a positive integer")
        if ngram_size <= 0:
            raise ValueError("ngram_size must be a positive integer")
        self._dimension = dimension
        self._ngram_size = ngram_size

    @property
    def dimension(self) -> int:
        return self._dimension

    def embed(self, text: str) -> list[float]:
        """Generate a deterministic embedding vector from text.

        Uses character n-gram hashing: each n-gram maps to a bucket in the
        vector, and the value is the count of n-grams mapping to that bucket.
        The vector is then L2-normalized.
        """
        vector = [0.0] * self._dimension
        normalized_text = text.lower().strip()
        if not normalized_text:
            return vector

        # Generate character n-grams and hash them to vector positions
        for i in range(len(normalized_text) - self._ngram_size + 1):
            ngram = normalized_text[i : i + self._ngram_size]
            h = hashlib.md5(ngram.encode("utf-8")).hexdigest()
            idx = int(h[:8], 16) % self._dimension
            # Use a secondary hash for the sign to reduce collisions
            sign = 1 if int(h[8:16], 16) % 2 == 0 else -1
            vector[idx] += sign * 1.0

        # Also hash individual words for term-level similarity
        words = normalized_text.split()
        for word in words:
            h = hashlib.md5(word.encode("utf-8")).hexdigest()
            idx = int(h[:8], 16) % self._dimension
            sign = 1 if int(h[8:16], 16) % 2 == 0 else -1
            vector[idx] += sign * 0.5

        # L2-normalize
        norm = math.sqrt(sum(v * v for v in vector))
        if norm > 0:
            vector = [v / norm for v in vector]

        return vector

    def embed_batch(self, texts: list[str]) -> list[list[float]]:
        """Generate embedding vectors for multiple texts."""
        return [self.embed(t) for t in texts]


# ---------------------------------------------------------------------------
# OpenAI embedding provider
# ---------------------------------------------------------------------------


class OpenAIEmbeddingProvider:
    """OpenAI API-based embedding provider.

    Uses the OpenAI embeddings API for production-quality vectors.

    Parameters
    ----------
    model : str
        The OpenAI embedding model name.  Default ``text-embedding-3-small``.
    api_key : str | None
        OpenAI API key.  If None, reads from ``OPENAI_API_KEY`` env var.
    dimension : int | None
        Override the output dimensionality.  If None, uses the model default.
    """

    def __init__(
        self,
        model: str = "text-embedding-3-small",
        api_key: str | None = None,
        dimension: int | None = None,
    ) -> None:
        self._model = model
        self._api_key = api_key
        # text-embedding-3-small default dimension is 1536
        self._dimension = dimension or 1536

    @property
    def dimension(self) -> int:
        return self._dimension

    @property
    def model(self) -> str:
        return self._model

    def embed(self, text: str) -> list[float]:
        """Generate an embedding vector via the OpenAI API."""
        return self.embed_batch([text])[0]

    def embed_batch(self, texts: list[str]) -> list[list[float]]:
        """Generate embedding vectors for multiple texts via the OpenAI API."""
        import os

        import httpx

        api_key = self._api_key or os.environ.get("OPENAI_API_KEY", "")
        headers = {
            "Authorization": f"Bearer {api_key}",
            "Content-Type": "application/json",
        }
        payload: dict = {
            "input": texts,
            "model": self._model,
        }
        if self._dimension and "3-small" in self._model:
            payload["dimensions"] = self._dimension

        response = httpx.post(
            "https://api.openai.com/v1/embeddings",
            headers=headers,
            json=payload,
            timeout=30.0,
        )
        response.raise_for_status()
        data = response.json()
        # Sort by index to maintain order
        embeddings_data = sorted(data["data"], key=lambda x: x["index"])
        return [item["embedding"] for item in embeddings_data]


# ---------------------------------------------------------------------------
# Sentence-Transformers embedding provider
# ---------------------------------------------------------------------------


class SentenceTransformerEmbeddingProvider:
    """Local sentence-transformers embedding provider.

    Uses a locally-downloaded sentence-transformers model for zero API cost
    semantic embeddings.  The model is loaded lazily on first use.

    Parameters
    ----------
    model_name : str
        The sentence-transformers model name.  Default ``all-MiniLM-L6-v2``
        (80MB, 384-dim, good balance of speed and quality).
    device : str | None
        Torch device string (e.g. ``"cuda"``, ``"cpu"``).  If None, auto-
        detects CUDA availability.
    """

    def __init__(
        self,
        model_name: str = "all-MiniLM-L6-v2",
        device: str | None = None,
    ) -> None:
        self._model_name = model_name
        self._device = device
        self._model = None

    @property
    def dimension(self) -> int:
        """Dimensionality of the embedding vectors produced."""
        _dim_map: dict[str, int] = {
            "all-MiniLM-L6-v2": 384,
            "all-mpnet-base-v2": 768,
            "paraphrase-MiniLM-L6-v2": 384,
            "paraphrase-multilingual-MiniLM-L12-v2": 384,
        }
        return _dim_map.get(self._model_name, 384)

    @property
    def model_name(self) -> str:
        return self._model_name

    def _load_model(self):
        """Lazy-load the sentence-transformers model."""
        if self._model is not None:
            return
        try:
            from sentence_transformers import SentenceTransformer
        except ImportError as e:
            raise ImportError(
                "sentence-transformers is required for "
                "SentenceTransformerEmbeddingProvider. "
                "Install with: pip install sentence-transformers"
            ) from e
        self._model = SentenceTransformer(self._model_name, device=self._device)

    def embed(self, text: str) -> list[float]:
        """Generate an embedding vector using the local model."""
        return self.embed_batch([text])[0]

    def embed_batch(self, texts: list[str]) -> list[list[float]]:
        """Generate embedding vectors for multiple texts using the local model."""
        self._load_model()
        import numpy as np

        embeddings = self._model.encode(texts, convert_to_numpy=True, normalize_embeddings=True)
        if isinstance(embeddings, np.ndarray):
            return embeddings.tolist()
        return [e.tolist() for e in embeddings]
