"""Communication analyzer — detect language patterns and dialect emergence in agent groups.

Analyzes agent messages to extract word frequency, sentence structure, and
expression habits, then compares groups and detects emerging dialects over time.

Pure Python string processing + basic statistics. No external NLP dependencies.
"""

from __future__ import annotations

import logging
import math
import re
import string
from collections import Counter
from dataclasses import dataclass, field
from typing import Any

logger = logging.getLogger(__name__)

# Common English stop words to filter out when computing meaningful vocab.
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

# Regex to extract word tokens (letters + digits, min length 2).
_WORD_RE = re.compile(r"[a-zA-Z0-9]{2,}")
# Regex to split sentences on punctuation.
_SENTENCE_SPLIT_RE = re.compile(r"[.!?;]+")


@dataclass
class MessagePattern:
    """Statistical summary of an agent's or group's communication patterns."""

    agent_id: str = ""
    total_messages: int = 0
    total_words: int = 0
    avg_words_per_message: float = 0.0
    unique_words: int = 0
    # Top-N word frequency distribution (word -> count).
    top_words: dict[str, int] = field(default_factory=dict)
    # Average sentence length (in words).
    avg_sentence_length: float = 0.0
    # Vocabulary richness = unique_words / total_words (0.0–1.0).
    vocabulary_richness: float = 0.0
    # Punctuation density (punctuation chars / total chars).
    punctuation_density: float = 0.0
    # Average message length in characters.
    avg_message_length: float = 0.0


@dataclass
class DialectReport:
    """Report on dialect / linguistic differentiation between groups."""

    has_dialect: bool = False
    dialect_strength: float = 0.0  # 0.0–1.0
    distinguishing_features: list[str] = field(default_factory=list)
    group_signatures: dict[str, list[str]] = field(default_factory=dict)
    # Timestamp of the analysis window (ISO 8601 or tick number).
    period_label: str = ""


def _tokenize(text: str) -> list[str]:
    """Extract lowercase word tokens from text."""
    return [w.lower() for w in _WORD_RE.findall(text)]


def _count_punctuation(text: str) -> int:
    """Count punctuation characters in text."""
    return sum(1 for c in text if c in string.punctuation)


def _split_sentences(text: str) -> list[str]:
    """Split text into sentence fragments."""
    parts = _SENTENCE_SPLIT_RE.split(text)
    return [p.strip() for p in parts if p.strip()]


class CommunicationAnalyzer:
    """Analyzes agent communication patterns, group differences, and dialect emergence.

    All methods are synchronous and lightweight — call from async code with
    ``await asyncio.to_thread(analyzer.analyze_message_patterns, ...)`` to
    avoid blocking the Think Loop.
    """

    def analyze_message_patterns(
        self,
        agent_id: str,
        messages: list[str],
        top_n: int = 20,
    ) -> MessagePattern:
        """Analyze a single agent's message patterns.

        Args:
            agent_id: Identifier for the agent.
            messages: List of message strings the agent has produced.
            top_n: Number of top words to include in the result.

        Returns:
            MessagePattern with statistical summary.
        """
        if not messages:
            return MessagePattern(agent_id=agent_id)

        all_words: list[str] = []
        total_chars = 0
        total_punct = 0
        sentence_lengths: list[int] = []

        for msg in messages:
            total_chars += len(msg)
            total_punct += _count_punctuation(msg)
            words = _tokenize(msg)
            all_words.extend(words)
            for sent in _split_sentences(msg):
                sent_words = _tokenize(sent)
                if sent_words:
                    sentence_lengths.append(len(sent_words))

        word_counts = Counter(all_words)
        total_words = len(all_words)
        unique_words = len(word_counts)

        return MessagePattern(
            agent_id=agent_id,
            total_messages=len(messages),
            total_words=total_words,
            avg_words_per_message=round(total_words / len(messages), 3) if messages else 0.0,
            unique_words=unique_words,
            top_words=dict(word_counts.most_common(top_n)),
            avg_sentence_length=(
                round(sum(sentence_lengths) / len(sentence_lengths), 3)
                if sentence_lengths
                else 0.0
            ),
            vocabulary_richness=round(unique_words / total_words, 4) if total_words else 0.0,
            punctuation_density=round(total_punct / total_chars, 4) if total_chars else 0.0,
            avg_message_length=round(total_chars / len(messages), 3) if messages else 0.0,
        )

    def compare_group_patterns(
        self,
        group_a: dict[str, list[str]],
        group_b: dict[str, list[str]],
    ) -> dict[str, Any]:
        """Compare communication patterns between two groups.

        Args:
            group_a: Mapping of agent_id -> list of message strings for group A.
            group_b: Mapping of agent_id -> list of message strings for group B.

        Returns:
            Dict with keys: distance, shared_vocab_ratio, a_summary, b_summary,
            distinguishing_words.
        """
        all_msgs_a = [m for msgs in group_a.values() for m in msgs]
        all_msgs_b = [m for msgs in group_b.values() for m in msgs]

        pattern_a = self.analyze_message_patterns("group_a", all_msgs_a)
        pattern_b = self.analyze_message_patterns("group_b", all_msgs_b)

        vocab_a = set(pattern_a.top_words.keys())
        vocab_b = set(pattern_b.top_words.keys())

        shared = vocab_a & vocab_b
        union = vocab_a | vocab_b
        shared_ratio = len(shared) / len(union) if union else 0.0

        # Compute a simple cosine-distance on the top-20 merged vocab.
        all_terms = sorted(union)
        vec_a = [pattern_a.top_words.get(t, 0) for t in all_terms]
        vec_b = [pattern_b.top_words.get(t, 0) for t in all_terms]
        distance = _cosine_distance(vec_a, vec_b)

        # Words distinctive to each group (present in one but absent in other).
        dist_a = vocab_a - vocab_b
        dist_b = vocab_b - vocab_a

        return {
            "distance": round(distance, 4),
            "shared_vocab_ratio": round(shared_ratio, 4),
            "a_summary": pattern_a,
            "b_summary": pattern_b,
            "distinguishing_words": {
                "group_a_only": sorted(dist_a),
                "group_b_only": sorted(dist_b),
            },
        }

    def detect_emerging_dialect(
        self,
        messages_over_time: list[dict[str, Any]],
        distance_threshold: float = 0.3,
    ) -> DialectReport:
        """Detect dialect emergence from a chronological series of group messages.

        Args:
            messages_over_time: List of dicts, each with keys:
                ``period`` (str label), ``groups`` (dict of group_name ->
                list of message strings).  Ordered chronologically.
            distance_threshold: Minimum inter-group distance to flag as dialect.

        Returns:
            DialectReport indicating whether a dialect has emerged.
        """
        if len(messages_over_time) < 2:
            return DialectReport(period_label="insufficient_data")

        # Track distances over time.
        max_distance = 0.0
        best_period = ""
        best_features: list[str] = []
        best_signatures: dict[str, list[str]] = {}

        for entry in messages_over_time:
            period = entry.get("period", "")
            groups = entry.get("groups", {})
            group_names = list(groups.keys())

            if len(group_names) < 2:
                continue

            # Compare first two groups as primary axis.
            ga = {group_names[0]: groups[group_names[0]]}
            gb = {group_names[1]: groups[group_names[1]]}
            comp = self.compare_group_patterns(ga, gb)

            dist = comp["distance"]
            if dist > max_distance:
                max_distance = dist
                best_period = period
                dist_words = comp["distinguishing_words"]
                best_features = (
                    dist_words.get("group_a_only", [])[:5]
                    + dist_words.get("group_b_only", [])[:5]
                )
                sig_a = dist_words.get("group_a_only", [])[:3]
                sig_b = dist_words.get("group_b_only", [])[:3]
                best_signatures = {
                    group_names[0]: sig_a,
                    group_names[1]: sig_b,
                }

        has_dialect = max_distance >= distance_threshold
        return DialectReport(
            has_dialect=has_dialect,
            dialect_strength=round(max_distance, 4),
            distinguishing_features=best_features[:10],
            group_signatures=best_signatures,
            period_label=best_period,
        )


def _cosine_distance(a: list[int], b: list[int]) -> float:
    """Compute cosine distance (1 - cosine_similarity) between two vectors."""
    dot = sum(x * y for x, y in zip(a, b))
    mag_a = math.sqrt(sum(x * x for x in a))
    mag_b = math.sqrt(sum(x * x for x in b))
    if mag_a == 0 or mag_b == 0:
        return 1.0
    similarity = dot / (mag_a * mag_b)
    return 1.0 - similarity
