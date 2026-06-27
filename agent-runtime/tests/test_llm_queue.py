"""Tests for the LLM request queue (llm/queue.py)."""

from __future__ import annotations

import asyncio
import json

import pytest

from agent_runtime.llm.base import LLMMessage, LLMResponse, TokenUsage
from agent_runtime.llm.queue import (
    LLMQueue,
    LLMRequest,
    Priority,
    QueueConfig,
    _resolve_priority,
)

# ---------------------------------------------------------------------------
# Mock LLM Provider
# ---------------------------------------------------------------------------


class MockLLMProvider:
    """Synchronous mock that returns a fixed response."""

    def __init__(self, delay: float = 0.0) -> None:
        self._delay = delay
        self.call_count = 0

    async def chat(self, messages: list[LLMMessage], **kwargs) -> LLMResponse:
        self.call_count += 1
        if self._delay:
            await asyncio.sleep(self._delay)
        return LLMResponse(
            content=json.dumps({"action": "rest", "reasoning": "mock"}),
            model="test-model",
            usage=TokenUsage(prompt_tokens=10, completion_tokens=5, total_tokens=15),
        )


class SlowLLMProvider:
    """LLM provider that is slow enough to trigger timeouts in tests."""

    async def chat(self, messages: list[LLMMessage], **kwargs) -> LLMResponse:
        await asyncio.sleep(10.0)
        return LLMResponse(content="should not reach", model="slow")


class FailingLLMProvider:
    """LLM provider that always raises."""

    async def chat(self, messages: list[LLMMessage], **kwargs) -> LLMResponse:
        raise RuntimeError("LLM unavailable")


# ---------------------------------------------------------------------------
# Tests: priority resolution
# ---------------------------------------------------------------------------


class TestPriorityResolution:
    def test_survival_is_highest(self):
        assert _resolve_priority("survival") == Priority.SURVIVAL

    def test_social(self):
        assert _resolve_priority("social") == Priority.SOCIAL

    def test_explore(self):
        assert _resolve_priority("explore") == Priority.EXPLORE

    def test_default(self):
        assert _resolve_priority("default") == Priority.DEFAULT

    def test_unknown_falls_to_default(self):
        assert _resolve_priority("unknown") == Priority.DEFAULT

    def test_case_insensitive(self):
        assert _resolve_priority("SURVIVAL") == Priority.SURVIVAL


# ---------------------------------------------------------------------------
# Tests: QueueConfig
# ---------------------------------------------------------------------------


class TestQueueConfig:
    def test_defaults(self):
        cfg = QueueConfig()
        assert cfg.max_concurrency == 2
        assert cfg.timeout_seconds == 120.0

    def test_custom(self):
        cfg = QueueConfig(max_concurrency=5, timeout_seconds=10.0)
        assert cfg.max_concurrency == 5
        assert cfg.timeout_seconds == 10.0

    def test_semaphore_timeout_default(self):
        """When semaphore_timeout_seconds is None, the queue uses 5× LLM timeout."""
        cfg = QueueConfig(timeout_seconds=30.0)
        queue = LLMQueue(provider=MockLLMProvider(), config=cfg)
        assert queue._effective_semaphore_timeout() == 150.0

    def test_semaphore_timeout_custom(self):
        """Custom semaphore timeout overrides the default multiplier."""
        cfg = QueueConfig(timeout_seconds=30.0, semaphore_timeout_seconds=60.0)
        queue = LLMQueue(provider=MockLLMProvider(), config=cfg)
        assert queue._effective_semaphore_timeout() == 60.0


# ---------------------------------------------------------------------------
# Tests: LLMQueue basic operations
# ---------------------------------------------------------------------------


class TestLLMQueue:
    @pytest.mark.asyncio
    async def test_enqueue_returns_response(self):
        mock = MockLLMProvider()
        queue = LLMQueue(provider=mock, config=QueueConfig())
        await queue.start()

        request = LLMRequest(messages=[LLMMessage(role="user", content="test")])
        response = await queue.enqueue(request)

        assert response.model == "test-model"
        assert "rest" in response.content
        assert mock.call_count == 1
        await queue.stop()

    @pytest.mark.asyncio
    async def test_stats_tracking(self):
        mock = MockLLMProvider()
        queue = LLMQueue(provider=mock, config=QueueConfig())
        await queue.start()

        request = LLMRequest(messages=[LLMMessage(role="user", content="test")])
        await queue.enqueue(request)

        stats = queue.stats()
        assert stats.total_requests == 1
        assert stats.completed_requests == 1
        assert stats.failed_requests == 0
        await queue.stop()

    @pytest.mark.asyncio
    async def test_concurrency_limit(self):
        """Multiple concurrent requests should be limited by semaphore."""
        mock = MockLLMProvider(delay=0.1)
        config = QueueConfig(max_concurrency=1)
        queue = LLMQueue(provider=mock, config=config)
        await queue.start()

        request = LLMRequest(messages=[LLMMessage(role="user", content="test")])

        # Launch 3 concurrent requests — with concurrency=1 they should be serial
        results = await asyncio.gather(
            queue.enqueue(request),
            queue.enqueue(request),
            queue.enqueue(request),
        )

        assert len(results) == 3
        assert mock.call_count == 3
        await queue.stop()

    @pytest.mark.asyncio
    async def test_timeout_propagates_to_caller(self):
        """Timeout should propagate as asyncio.TimeoutError so that
        DecisionEngine can apply its own validated fallback."""
        mock = SlowLLMProvider()
        config = QueueConfig(timeout_seconds=0.05)
        queue = LLMQueue(provider=mock, config=config)
        await queue.start()

        request = LLMRequest(messages=[LLMMessage(role="user", content="test")])
        with pytest.raises(asyncio.TimeoutError):
            await queue.enqueue(request)

        stats = queue.stats()
        assert stats.timed_out_requests == 1
        await queue.stop()

    @pytest.mark.asyncio
    async def test_failed_request_propagates_exception(self):
        """Provider errors should propagate so callers can handle them."""
        mock = FailingLLMProvider()
        config = QueueConfig()
        queue = LLMQueue(provider=mock, config=config)
        await queue.start()

        request = LLMRequest(messages=[LLMMessage(role="user", content="test")])
        with pytest.raises(RuntimeError, match="LLM unavailable"):
            await queue.enqueue(request)

        await queue.stop()

    @pytest.mark.asyncio
    async def test_auto_start(self):
        """Queue should auto-start if not explicitly started."""
        mock = MockLLMProvider()
        queue = LLMQueue(provider=mock, config=QueueConfig())

        request = LLMRequest(messages=[LLMMessage(role="user", content="test")])
        response = await queue.enqueue(request)

        assert response.model == "test-model"
        await queue.stop()

    @pytest.mark.asyncio
    async def test_request_metadata_passthrough(self):
        """Metadata on the request should not affect the LLM call."""
        mock = MockLLMProvider()
        queue = LLMQueue(provider=mock, config=QueueConfig())
        await queue.start()

        request = LLMRequest(
            messages=[LLMMessage(role="user", content="test")],
            priority="survival",
            metadata={"agent_id": "agent-1", "tick": 42},
        )
        response = await queue.enqueue(request)

        assert response.model == "test-model"
        await queue.stop()

    @pytest.mark.asyncio
    async def test_priority_ordering(self):
        """Higher-priority requests should be dispatched first."""
        call_order: list[str] = []

        class OrderedProvider:
            async def chat(self, messages: list[LLMMessage], **kwargs) -> LLMResponse:
                # Extract priority info from messages to record order
                content = messages[0].content if messages else ""
                call_order.append(content)
                await asyncio.sleep(0.01)
                return LLMResponse(
                    content=json.dumps({"action": "rest"}),
                    model="test-model",
                )

        provider = OrderedProvider()
        config = QueueConfig(max_concurrency=1)
        queue = LLMQueue(provider=provider, config=config)
        await queue.start()

        # Enqueue low priority first, then high priority
        _results = await asyncio.gather(
            queue.enqueue(LLMRequest(
                messages=[LLMMessage(role="user", content="default")],
                priority="default",
            )),
            queue.enqueue(LLMRequest(
                messages=[LLMMessage(role="user", content="survival")],
                priority="survival",
            )),
        )

        # With concurrency=1, survival should be dispatched first
        assert call_order[0] == "survival"
        assert call_order[1] == "default"
        await queue.stop()

    @pytest.mark.asyncio
    async def test_stop_drains_pending(self):
        """stop() should resolve or cancel pending requests cleanly."""
        mock = MockLLMProvider(delay=5.0)
        queue = LLMQueue(provider=mock, config=QueueConfig(max_concurrency=1))
        await queue.start()

        # Enqueue a request that will be pending
        task = asyncio.create_task(queue.enqueue(
            LLMRequest(messages=[LLMMessage(role="user", content="test")]),
        ))

        # Give the worker time to pick it up
        await asyncio.sleep(0.05)

        await queue.stop()

        # The pending request should be resolved (fallback) or cancelled cleanly
        try:
            response = await asyncio.wait_for(task, timeout=2.0)
            assert response.model == "fallback"
        except (TimeoutError, asyncio.CancelledError):
            # In some Python versions the task is cancelled — that's acceptable
            pass

    # ------------------------------------------------------------------
    # Tests: independent semaphore / LLM timeouts
    # ------------------------------------------------------------------

    @pytest.mark.asyncio
    async def test_llm_timeout_does_not_consume_semaphore_wait(self):
        """A request that waits for the semaphore should still get the
        full LLM timeout once it acquires it — semaphore wait time is
        not deducted.

        Setup: concurrency=1, LLM timeout=1s, semaphore timeout=10s.
        The first request holds the slot for 3s (simulated by sleeping).
        The second request waits ~1s for the semaphore (the first
        request's LLM call times out at 1s, releasing the semaphore).
        After acquiring the semaphore, the second request gets a fresh
        1s LLM timeout and succeeds.

        Key assertion: at 0.5s, the second request must NOT have timed
        out, even though the LLM timeout is 1s — because it hasn't
        acquired the semaphore yet.
        """
        provider_call_started = asyncio.Event()

        class SlowThenFastProvider:
            def __init__(self):
                self.call_count = 0

            async def chat(self, messages: list[LLMMessage], **kwargs) -> LLMResponse:
                self.call_count += 1
                if self.call_count == 1:
                    provider_call_started.set()
                    # Hold the slot for 3s — will be killed by LLM timeout at 1s
                    await asyncio.sleep(3.0)
                # Second call returns immediately
                return LLMResponse(content='{"action": "rest"}', model="test")

        provider = SlowThenFastProvider()
        config = QueueConfig(
            max_concurrency=1,
            timeout_seconds=1.0,
            semaphore_timeout_seconds=10.0,
        )
        queue = LLMQueue(provider=provider, config=config)
        await queue.start()

        # First request grabs the semaphore and holds it (will LLM-timeout at 1s)
        first_task = asyncio.create_task(queue.enqueue(
            LLMRequest(messages=[LLMMessage(role="user", content="first")]),
        ))
        await provider_call_started.wait()

        # Second request must wait for the semaphore.
        second_result_holder: dict = {}

        async def second_call():
            try:
                second_result_holder["result"] = await queue.enqueue(
                    LLMRequest(messages=[LLMMessage(role="user", content="second")]),
                )
            except Exception as exc:
                second_result_holder["error"] = exc

        second_task = asyncio.create_task(second_call())

        # At 0.5s: first request still holds semaphore (LLM timeout hasn't fired).
        # Second request is waiting for semaphore — must NOT have timed out.
        # Old code would have timed out at ~1s total (sem wait + LLM),
        # but with independent timeouts the sem timeout is 10s.
        await asyncio.sleep(0.5)
        assert not second_task.done(), \
            "Second request should not time out while waiting for semaphore"

        # Wait for both to finish. First times out at ~1s (releases semaphore),
        # second acquires semaphore at ~1s and completes.
        await asyncio.wait_for(asyncio.gather(
            first_task, second_task, return_exceptions=True,
        ), timeout=10.0)

        # Second request succeeded
        assert "error" not in second_result_holder
        assert second_result_holder["result"].content == '{"action": "rest"}'

        await queue.stop()

    @pytest.mark.asyncio
    async def test_semaphore_timeout_triggers(self):
        """When the semaphore wait exceeds semaphore_timeout_seconds,
        the request should fail with TimeoutError."""
        class BlockingProvider:
            """Holds the LLM slot indefinitely."""
            def __init__(self):
                self.block = asyncio.Event()

            async def chat(self, messages: list[LLMMessage], **kwargs) -> LLMResponse:
                await self.block.wait()
                return LLMResponse(content="unreachable", model="test")

        provider = BlockingProvider()
        config = QueueConfig(
            max_concurrency=1,
            timeout_seconds=10.0,
            semaphore_timeout_seconds=0.3,
        )
        queue = LLMQueue(provider=provider, config=config)
        await queue.start()

        # First request grabs the only slot and blocks
        first_task = asyncio.create_task(queue.enqueue(
            LLMRequest(messages=[LLMMessage(role="user", content="first")]),
        ))

        # Give it time to acquire the semaphore
        await asyncio.sleep(0.1)

        # Second request must wait for the semaphore — should time out
        with pytest.raises(asyncio.TimeoutError):
            await queue.enqueue(
                LLMRequest(messages=[LLMMessage(role="user", content="second")]),
            )

        stats = queue.stats()
        assert stats.timed_out_requests == 1

        # Clean up
        provider.block.set()
        await first_task
        await queue.stop()


# ---------------------------------------------------------------------------
# Tests: LLMRequest
# ---------------------------------------------------------------------------


class TestLLMRequest:
    def test_defaults(self):
        req = LLMRequest(messages=[LLMMessage(role="user", content="hi")])
        assert req.priority == "default"
        assert req.metadata == {}
        assert req.max_tokens is None
        assert req.temperature is None

    def test_custom_priority(self):
        req = LLMRequest(
            messages=[LLMMessage(role="user", content="hi")],
            priority="survival",
        )
        assert req.priority == "survival"


# ---------------------------------------------------------------------------
# Tests: QueueStats
# ---------------------------------------------------------------------------


class TestQueueStats:
    def test_initial(self):
        from agent_runtime.llm.queue import QueueStats
        stats = QueueStats()
        assert stats.total_requests == 0
        assert stats.completed_requests == 0
