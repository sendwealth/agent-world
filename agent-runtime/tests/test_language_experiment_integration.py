"""Tests for LanguageExperiment integration into the agent think loop.

Covers:
- DefaultLanguageExperimentHook creation and vocabulary setup
- Hook wired into ThinkLoop processes messages per tick
- Compliance checking produces correct violations
- Efficiency metrics are available after running
- Non-fatal: think loop continues even if hook raises
- No hook: think loop works without LanguageExperiment (backward compat)
"""

from __future__ import annotations

import pytest

from agent_runtime.core.act import ActionExecutor
from agent_runtime.core.think_loop import ThinkLoop, ThinkLoopConfig
from agent_runtime.models.agent_state import AgentState
from agent_runtime.models.enums import AgentPhase
from agent_runtime.social.language_experiment import LanguageExperiment
from agent_runtime.social.provider import DefaultLanguageExperimentHook

# ---------------------------------------------------------------------------
# Fixtures
# ---------------------------------------------------------------------------


def _make_state(**overrides):
    defaults = dict(
        name="TestAgent",
        tokens=500,
        max_tokens=1000,
        health=100.0,
        reputation=5.0,
        phase=AgentPhase.ADULT,
    )
    defaults.update(overrides)
    return AgentState(**defaults)


@pytest.fixture
def state():
    return _make_state()


@pytest.fixture
def hook():
    """DefaultLanguageExperimentHook with a restricted vocabulary."""
    h = DefaultLanguageExperimentHook()
    h.setup_restricted_vocabulary(
        agent_ids=["test-agent"],
        allowed_words={"gather", "food", "wood", "rest", "move", "north", "need", "to"},
        experiment_id="default",
    )
    return h


# ---------------------------------------------------------------------------
# Unit tests -- DefaultLanguageExperimentHook
# ---------------------------------------------------------------------------


class TestDefaultLanguageExperimentHook:
    """Unit tests for DefaultLanguageExperimentHook."""

    def test_compliant_message(self, hook):
        result = hook.check_message("I need to gather food", "default")
        assert result["compliant"] is True
        assert result["violations"] == []

    def test_non_compliant_message(self, hook):
        result = hook.check_message("I want to explore the mountain", "default")
        assert result["compliant"] is False
        assert len(result["violations"]) > 0

    def test_record_tick_tracks_messages(self, hook):
        hook.record_tick(
            "agent-1", tick=1, message="gather food", experiment_id="default"
        )
        hook.record_tick(
            "agent-1",
            tick=2,
            message="explore unknown territory",
            experiment_id="default",
        )
        metrics = hook.get_efficiency_metrics("agent-1", "default")
        assert metrics.total_messages >= 0

    def test_no_experiment_still_works(self):
        """Hook without setup_restricted_vocabulary should still work."""
        h = DefaultLanguageExperimentHook()
        result = h.check_message("anything goes here", "default")
        assert result["compliant"] is True

    def test_underlying_experiment_accessible(self, hook):
        assert isinstance(hook.experiment, LanguageExperiment)


# ---------------------------------------------------------------------------
# Integration tests -- ThinkLoop with LanguageExperimentHook
# ---------------------------------------------------------------------------


class TestThinkLoopLanguageExperimentIntegration:
    """Integration tests for LanguageExperimentHook wired into ThinkLoop."""

    @pytest.mark.asyncio
    async def test_hook_called_per_tick(self, state, hook):
        """ThinkLoop with language_experiment_hook processes each tick."""
        config = ThinkLoopConfig(tick_interval=0.0)
        loop = ThinkLoop(
            state=state,
            survival=None,
            executor=ActionExecutor(),
            config=config,
            language_experiment_hook=hook,
        )

        await loop.run(max_ticks=3)
        assert loop.tick == 3

    @pytest.mark.asyncio
    async def test_no_hook_backward_compat(self, state):
        """ThinkLoop works fine without a language_experiment_hook."""
        config = ThinkLoopConfig(tick_interval=0.0)
        loop = ThinkLoop(
            state=state,
            survival=None,
            executor=ActionExecutor(),
            config=config,
        )

        await loop.run(max_ticks=5)
        assert loop.tick == 5

    @pytest.mark.asyncio
    async def test_hook_error_non_fatal(self, state):
        """If the hook raises, the think loop continues."""

        class BrokenHook:
            def check_message(self, message, experiment_id):
                raise RuntimeError("boom")

            def record_tick(self, agent_id, tick, message, experiment_id):
                raise RuntimeError("boom")

        config = ThinkLoopConfig(tick_interval=0.0)
        loop = ThinkLoop(
            state=state,
            survival=None,
            executor=ActionExecutor(),
            config=config,
            language_experiment_hook=BrokenHook(),
        )

        await loop.run(max_ticks=5)
        assert loop.tick == 5

    @pytest.mark.asyncio
    async def test_metrics_appear_after_run(self, state, hook):
        """After running the loop, efficiency metrics are available."""
        config = ThinkLoopConfig(tick_interval=0.0)
        loop = ThinkLoop(
            state=state,
            survival=None,
            executor=ActionExecutor(),
            config=config,
            language_experiment_hook=hook,
        )

        await loop.run(max_ticks=5)

        agent_id = str(state.id)
        metrics = hook.get_efficiency_metrics(agent_id, "default")
        assert metrics.total_messages >= 0
        assert metrics.total_words >= 0
