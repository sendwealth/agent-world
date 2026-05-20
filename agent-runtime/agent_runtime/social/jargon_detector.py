"""Jargon detector — identify group-specific terms and measure linguistic distance.

Extracts high-frequency phrases from agent messages, detects terms that are
specific to a group (jargon), and computes pairwise linguistic distance between
groups based on vocabulary overlap.

Pure Python string processing + basic statistics. No external NLP dependencies.
"""

from __future__ import annotations

import logging
import math
import re
from collections import Counter
from dataclasses import dataclass, field
from typing import Any

logger = logging.getLogger(__name__)

# Regex to extract word tokens (letters + digits, min length 2).
_WORD_RE = re.compile(r"[a-zA-Z0-9]{2,}")

# Common English stop words.
_STOP_WORDS = frozenset(
    w
    for w in """
    a an the is are was were be been being have has had do does did will would
    shall should may might can could of in to for with on at by from as into
    through during before after above below between out off over under again
    further then once here there when where why how all both each few more most
    other some such no nor not only own same so than too very i me my we our
    you your he him his she her it its they them their what which who whom this
    that these those am about up if or and but just also now
    """.split()
)


@dataclass
class JargonTerm:
    """A term identified as group-specific jargon."""

    term: str
    frequency: int  # absolute count in the group
    specificity: float  # 0.0–1.0, how exclusive to this group
    group_id: str = ""
    # Ratio of usage in owning group vs all other groups combined.
    ownership_ratio: float = 0.0


def _tokenize(text: str) -> list[str]:
    """Extract lowercase word tokens from text."""
    return [w.lower() for w in _WORD_RE.findall(text)]


def _extract_ngrams(tokens: list[str], n: int) -> list[str]:
    """Extract n-grams from a list of tokens."""
    if len(tokens) < n:
        return []
    return [" ".join(tokens[i : i + n]) for i in range(len(tokens) - n + 1)]


class JargonDetector:
    """Detect group-specific jargon and measure linguistic distance.

    All methods are synchronous and lightweight — call from async code with
    ``await asyncio.to_thread(detector.extract_frequent_phrases, ...)`` to
    avoid blocking the Think Loop.
    """

    def extract_frequent_phrases(
        self,
        group_messages: list[str],
        min_freq: int = 3,
        ngram_range: tuple[int, int] = (2, 3),
        top_k: int = 20,
    ) -> list[dict[str, Any]]:
        """Extract high-frequency phrases from a group's messages.

        Args:
            group_messages: List of message strings from a group.
            min_freq: Minimum frequency to be considered a candidate.
            ngram_range: (min_n, max_n) for n-gram extraction.
            top_k: Maximum number of phrases to return.

        Returns:
            List of dicts with keys: phrase, frequency, ngram_size.
        """
        phrase_counter: Counter[str] = Counter()

        for msg in group_messages:
            tokens = _tokenize(msg)
            for n in range(ngram_range[0], ngram_range[1] + 1):
                for ngram in _extract_ngrams(tokens, n):
                    phrase_counter[ngram] += 1

        # Filter by minimum frequency and return top-K.
        results = []
        for phrase, count in phrase_counter.most_common(top_k * 2):
            if count < min_freq:
                break
            n = len(phrase.split())
            results.append({
                "phrase": phrase,
                "frequency": count,
                "ngram_size": n,
            })

        return results[:top_k]

    def detect_group_specific_terms(
        self,
        all_groups: dict[str, list[str]],
        min_freq: int = 2,
        specificity_threshold: float = 0.6,
    ) -> list[JargonTerm]:
        """Detect terms used predominantly by one group.

        A term is group-specific if its ``specificity`` (fraction of total
        occurrences that belong to one group) exceeds *specificity_threshold*.

        Args:
            all_groups: Mapping of group_id -> list of message strings.
            min_freq: Minimum total frequency across all groups.
            specificity_threshold: Minimum specificity to count as jargon.

        Returns:
            List of JargonTerm sorted by specificity descending.
        """
        # Build per-group word counts.
        group_word_counts: dict[str, Counter[str]] = {}
        total_word_counts: Counter[str] = Counter()

        for group_id, messages in all_groups.items():
            counts: Counter[str] = Counter()
            for msg in messages:
                tokens = _tokenize(msg)
                # Filter stop words for jargon detection — meaningful terms only.
                meaningful = [t for t in tokens if t not in _STOP_WORDS]
                counts.update(meaningful)
            group_word_counts[group_id] = counts
            total_word_counts.update(counts)

        # Identify group-specific terms.
        jargon_terms: list[JargonTerm] = []

        for word, total_freq in total_word_counts.items():
            if total_freq < min_freq:
                continue

            # Find which group uses it most.
            best_group = ""
            best_count = 0
            for group_id, counts in group_word_counts.items():
                c = counts.get(word, 0)
                if c > best_count:
                    best_count = c
                    best_group = group_id

            specificity = best_count / total_freq if total_freq else 0.0
            if specificity >= specificity_threshold:
                # Ownership ratio: usage in owning group / usage in all others.
                others_total = total_freq - best_count
                ownership = best_count / others_total if others_total > 0 else float("inf")

                jargon_terms.append(JargonTerm(
                    term=word,
                    frequency=best_count,
                    specificity=round(specificity, 4),
                    group_id=best_group,
                    ownership_ratio=round(min(ownership, 99.0), 2),
                ))

        # Sort by specificity descending, then frequency descending.
        jargon_terms.sort(key=lambda t: (-t.specificity, -t.frequency))
        return jargon_terms

    def compute_linguistic_distance(
        self,
        group_a_messages: list[str],
        group_b_messages: list[str],
    ) -> float:
        """Compute linguistic distance between two groups (0.0 = identical, 1.0 = disjoint).

        Uses Jaccard distance on vocabulary sets (excluding stop words).

        Args:
            group_a_messages: List of message strings from group A.
            group_b_messages: List of message strings from group B.

        Returns:
            Float in [0.0, 1.0].
        """
        vocab_a: set[str] = set()
        for msg in group_a_messages:
            tokens = _tokenize(msg)
            vocab_a.update(t for t in tokens if t not in _STOP_WORDS)

        vocab_b: set[str] = set()
        for msg in group_b_messages:
            tokens = _tokenize(msg)
            vocab_b.update(t for t in tokens if t not in _STOP_WORDS)

        if not vocab_a and not vocab_b:
            return 0.0
        if not vocab_a or not vocab_b:
            return 1.0

        intersection = vocab_a & vocab_b
        union = vocab_a | vocab_b
        # Jaccard distance = 1 - Jaccard similarity.
        return round(1.0 - len(intersection) / len(union), 4)
