"""Language experiment — restricted-vocabulary experiments and efficiency measurement.

Optional module for observing whether agents develop new expressions when their
vocabulary is constrained, and for measuring communication efficiency changes
over time.

Pure Python string processing + basic statistics. No external NLP dependencies.
"""

from __future__ import annotations

import logging
import re
from dataclasses import dataclass, field
from typing import Any

logger = logging.getLogger(__name__)

_WORD_RE = re.compile(r"[a-zA-Z0-9]{2,}")


@dataclass
class VocabConstraint:
    """Configuration for a restricted-vocabulary experiment."""

    agent_ids: list[str] = field(default_factory=list)
    allowed_words: set[str] = field(default_factory=set)
    # Whether the constraint is currently active.
    active: bool = True


@dataclass
class EfficiencyMetrics:
    """Communication efficiency measurement results."""

    total_messages: int = 0
    total_words: int = 0
    avg_message_length: float = 0.0
    # Unique words used (vocabulary diversity).
    unique_words: int = 0
    # Words per idea: lower means more efficient communication.
    words_per_message: float = 0.0
    # Violation rate: fraction of messages containing disallowed words.
    constraint_violation_rate: float = 0.0
    # Novel words that emerged during the experiment.
    novel_words: list[str] = field(default_factory=list)


class LanguageExperiment:
    """Manage restricted-vocabulary experiments and measure communication efficiency.

    All methods are synchronous and lightweight — call from async code with
    ``await asyncio.to_thread(experiment.measure_communication_efficiency, ...)``
    to avoid blocking the Think Loop.
    """

    def __init__(self) -> None:
        self._constraints: dict[str, VocabConstraint] = {}

    def setup_restricted_vocabulary(
        self,
        agent_ids: list[str],
        allowed_words: set[str],
        experiment_id: str = "default",
    ) -> None:
        """Restrict agents' allowed vocabulary for an experiment.

        Args:
            agent_ids: Agents participating in the experiment.
            allowed_words: Words agents are permitted to use.
            experiment_id: Identifier for this experiment instance.
        """
        normalized = {w.lower() for w in allowed_words}
        self._constraints[experiment_id] = VocabConstraint(
            agent_ids=list(agent_ids),
            allowed_words=normalized,
            active=True,
        )
        logger.info(
            "Vocabulary constraint '%s' set up for %d agents with %d allowed words",
            experiment_id,
            len(agent_ids),
            len(normalized),
        )

    def check_message(
        self,
        message: str,
        experiment_id: str = "default",
    ) -> dict[str, Any]:
        """Check a message against the active vocabulary constraint.

        Args:
            message: The message to check.
            experiment_id: Which experiment constraint to check against.

        Returns:
            Dict with keys: compliant (bool), violations (list of disallowed words).
        """
        constraint = self._constraints.get(experiment_id)
        if constraint is None or not constraint.active:
            return {"compliant": True, "violations": []}

        words = {w.lower() for w in _WORD_RE.findall(message)}
        violations = sorted(words - constraint.allowed_words)
        return {
            "compliant": len(violations) == 0,
            "violations": violations,
        }

    def measure_communication_efficiency(
        self,
        before_messages: list[str],
        after_messages: list[str],
        experiment_id: str = "default",
    ) -> EfficiencyMetrics:
        """Measure communication efficiency change before vs after constraint.

        Args:
            before_messages: Messages produced before vocabulary restriction.
            after_messages: Messages produced after vocabulary restriction.
            experiment_id: Experiment constraint to check violations against.

        Returns:
            EfficiencyMetrics with comparison data focused on the *after* period.
        """
        constraint = self._constraints.get(experiment_id)

        # Analyze "after" messages.
        after_words: list[str] = []
        violations = 0
        novel_words: set[str] = set()

        before_vocab = {w.lower() for msg in before_messages for w in _WORD_RE.findall(msg)}

        for msg in after_messages:
            tokens = [w.lower() for w in _WORD_RE.findall(msg)]
            after_words.extend(tokens)

            # Detect novel words (not in before vocabulary).
            for t in tokens:
                if t not in before_vocab:
                    novel_words.add(t)

            # Check constraint violations.
            if constraint and constraint.active:
                disallowed = set(tokens) - constraint.allowed_words
                if disallowed:
                    violations += 1

        total_after = len(after_messages)
        unique_after = len(set(after_words))

        return EfficiencyMetrics(
            total_messages=total_after,
            total_words=len(after_words),
            avg_message_length=(
                round(len(after_words) / total_after, 3) if total_after else 0.0
            ),
            unique_words=unique_after,
            words_per_message=(
                round(len(after_words) / total_after, 3) if total_after else 0.0
            ),
            constraint_violation_rate=(
                round(violations / total_after, 4) if total_after else 0.0
            ),
            novel_words=sorted(novel_words),
        )

    def deactivate(self, experiment_id: str = "default") -> None:
        """Deactivate a vocabulary constraint."""
        constraint = self._constraints.get(experiment_id)
        if constraint:
            constraint.active = False

    def is_active(self, experiment_id: str = "default") -> bool:
        """Check whether a constraint is currently active."""
        constraint = self._constraints.get(experiment_id)
        return constraint.active if constraint else False
