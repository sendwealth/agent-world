"""Nonce-based replay prevention with LRU cache and TTL expiry."""

from __future__ import annotations

import time
from collections import OrderedDict
from threading import Lock


class NonceCache:
    """Thread-safe LRU cache that tracks nonces with a configurable TTL.

    A nonce is considered valid (not a replay) if it has not been seen before
    or if its previous entry has expired beyond the TTL window.

    Usage:
        cache = NonceCache(ttl_seconds=300)  # 5-minute expiry
        if cache.check_and_store(nonce):
            # nonce is fresh, proceed
        else:
            # nonce was seen recently, reject as replay
    """

    def __init__(self, ttl_seconds: float = 300.0, max_size: int = 10_000) -> None:
        self._ttl = ttl_seconds
        self._max_size = max_size
        self._store: OrderedDict[str, float] = OrderedDict()
        self._lock = Lock()

    def check_and_store(self, nonce: str) -> bool:
        """Check if a nonce is fresh (not replayed) and store it.

        Returns True if the nonce is fresh and was stored.
        Returns False if the nonce was already seen within the TTL window.
        """
        now = time.monotonic()

        with self._lock:
            self._evict_expired(now)

            if nonce in self._store:
                # Move to end for LRU ordering
                self._store.move_to_end(nonce)
                return False  # replay detected

            self._store[nonce] = now
            self._store.move_to_end(nonce)

            # Evict oldest if over capacity
            while len(self._store) > self._max_size:
                self._store.popitem(last=False)

            return True

    def _evict_expired(self, now: float) -> None:
        """Remove all entries that have exceeded the TTL."""
        cutoff = now - self._ttl
        while self._store:
            oldest_key, oldest_time = next(iter(self._store.items()))
            if oldest_time < cutoff:
                self._store.popitem(last=False)
            else:
                break

    def clear(self) -> None:
        """Remove all entries."""
        with self._lock:
            self._store.clear()

    @property
    def size(self) -> int:
        """Current number of stored nonces."""
        with self._lock:
            return len(self._store)
