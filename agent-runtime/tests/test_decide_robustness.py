"""Tests for robust LLM response parsing, keyword match fallback, and caching.

Validates the improvements from the LLM decision reliability fix:
- JSON extraction from surrounding text
- <think/> block stripping (closed and unclosed)
- Empty response handling
- Keyword-match fallback
- LRU response cache
- Improved prompt with action names and few-shot example
"""

from __future__ import annotations

import json

import pytest

from agent_runtime.core.decide import (
    LRUCache,
    Decision,
    DecisionAction,
    DecisionEngine,
    DecisionPerception,
    JsonParseError,
    SurvivalAssessment,
    _extract_json_from_text,
    build_prompt,
    fallback_decision,
    keyword_match_decision,
    parse_llm_response,
    strip_code_fences,
)
from agent_runtime.llm.base import LLMMessage, LLMResponse, TokenUsage


# ---------------------------------------------------------------------------
# Mock state
# ---------------------------------------------------------------------------


class MockState:
    def __init__(self, tokens: int = 500) -> None:
        self.name = "TestAgent"
        self.id = "agent-001"
        self.phase = _MockPhase("settler")
        self.tokens = tokens
        self.money = 50.0
        self.health = 80.0
        self.reputation = 5.0
        self.skills = {}


class _MockPhase:
    def __init__(self, value: str) -> None:
        self.value = value


# ---------------------------------------------------------------------------
# Tests: strip_code_fences
# ---------------------------------------------------------------------------


class TestStripCodeFences:
    def test_think_block_closed(self):
        raw = "<think reasoning='long'>some reasoning</think > {\"action\": \"rest\"}"
        result = strip_code_fences(raw)
        assert '{"action": "rest"}' in result

    def test_think_block_unclosed(self):
        """Model started thinking but ran out of tokens."""
        raw = "<think I should analyze this situation and"
        result = strip_code_fences(raw)
        assert result == ""

    def test_think_block_unclosed_with_json_after(self):
        """Unclosed think block — no JSON after it."""
        raw = "<think reasoning step one\nstep two"
        result = strip_code_fences(raw)
        assert result == ""

    def test_code_fence_json(self):
        raw = '```json\n{"action": "rest"}\n```'
        result = strip_code_fences(raw)
        assert result == '{"action": "rest"}'

    def test_plain_json(self):
        raw = '{"action": "explore"}'
        result = strip_code_fences(raw)
        assert result == '{"action": "explore"}'

    def test_empty_string(self):
        assert strip_code_fences("") == ""
        assert strip_code_fences("   ") == ""


# ---------------------------------------------------------------------------
# Tests: _extract_json_from_text
# ---------------------------------------------------------------------------


class TestExtractJsonFromText:
    def test_json_in_text(self):
        text = 'The best action is {"action": "rest", "parameters": {}, "reasoning": "safe", "confidence": 80} and that is final.'
        result = _extract_json_from_text(text)
        assert result is not None
        data = json.loads(result)
        assert data["action"] == "rest"

    def test_no_json(self):
        text = "I think the agent should rest and conserve energy."
        assert _extract_json_from_text(text) is None

    def test_no_braces(self):
        assert _extract_json_from_text("plain text") is None

    def test_multiple_braces(self):
        text = 'Here is {invalid} then {"action": "gather"} done'
        result = _extract_json_from_text(text)
        assert result is not None
        data = json.loads(result)
        assert data["action"] == "gather"


# ---------------------------------------------------------------------------
# Tests: parse_llm_response (robust)
# ---------------------------------------------------------------------------


class TestParseLlmResponse:
    def test_clean_json(self):
        raw = '{"action": "rest", "parameters": {}, "reasoning": "test", "confidence": 75}'
        data = parse_llm_response(raw)
        assert data["action"] == "rest"

    def test_json_in_surrounding_text(self):
        raw = 'I think the best action is {"action": "explore", "parameters": {}, "reasoning": "need info", "confidence": 60}.'
        data = parse_llm_response(raw)
        assert data["action"] == "explore"

    def test_think_block_then_json(self):
        raw = '<think analyzing situation</think >{"action": "gather", "parameters": {}, "reasoning": "need food", "confidence": 50}'
        data = parse_llm_response(raw)
        assert data["action"] == "gather"

    def test_empty_response_raises(self):
        with pytest.raises(JsonParseError, match="empty"):
            parse_llm_response("")

    def test_empty_whitespace_raises(self):
        with pytest.raises(JsonParseError, match="empty"):
            parse_llm_response("   \n  ")

    def test_invalid_json_no_object_raises(self):
        with pytest.raises(JsonParseError):
            parse_llm_response("no json at all here")

    def test_missing_action_raises(self):
        with pytest.raises(JsonParseError, match="action"):
            parse_llm_response('{"parameters": {}}')

    def test_unknown_action_raises(self):
        with pytest.raises(JsonParseError, match="Unknown action"):
            parse_llm_response('{"action": "fly_away"}')

    def test_code_fence_json(self):
        raw = '```json\n{"action": "rest", "parameters": {}, "reasoning": "resting", "confidence": 80}\n```'
        data = parse_llm_response(raw)
        assert data["action"] == "rest"


# ---------------------------------------------------------------------------
# Tests: keyword_match_decision
# ---------------------------------------------------------------------------


class TestKeywordMatchDecision:
    def test_rest_keyword(self):
        d = keyword_match_decision("I should rest now", [DecisionAction.REST, DecisionAction.EXPLORE])
        assert d is not None
        assert d.action == DecisionAction.REST

    def test_gather_keyword(self):
        d = keyword_match_decision("Let me gather some resources", [DecisionAction.GATHER])
        assert d is not None
        assert d.action == DecisionAction.GATHER

    def test_no_match(self):
        d = keyword_match_decision("I like pizza", [DecisionAction.REST])
        assert d is None

    def test_keyword_not_in_available(self):
        d = keyword_match_decision("Let's build something", [DecisionAction.REST])
        assert d is None

    def test_case_insensitive(self):
        d = keyword_match_decision("REST is best", [DecisionAction.REST])
        assert d is not None
        assert d.action == DecisionAction.REST


# ---------------------------------------------------------------------------
# Tests: LRUCache
# ---------------------------------------------------------------------------


class TestLRUCache:
    def test_miss_returns_none(self):
        cache = LRUCache()
        state = MockState()
        perception = DecisionPerception(tick=1)
        survival = SurvivalAssessment()
        assert cache.get(state, perception, survival) is None
        assert cache.misses == 1
        assert cache.hits == 0

    def test_put_and_get(self):
        cache = LRUCache()
        state = MockState()
        perception = DecisionPerception(tick=1)
        survival = SurvivalAssessment()
        decision = Decision(action=DecisionAction.REST, reasoning="test")
        cache.put(state, perception, survival, decision)
        result = cache.get(state, perception, survival)
        assert result is not None
        assert result.action == DecisionAction.REST
        assert cache.hits == 1

    def test_different_tick_is_miss(self):
        cache = LRUCache()
        state = MockState()
        decision = Decision(action=DecisionAction.REST)
        cache.put(state, DecisionPerception(tick=1), SurvivalAssessment(), decision)
        result = cache.get(state, DecisionPerception(tick=2), SurvivalAssessment())
        assert result is None

    def test_eviction(self):
        cache = LRUCache(max_size=2)
        state = MockState()
        for i in range(3):
            decision = Decision(action=DecisionAction.REST)
            cache.put(state, DecisionPerception(tick=i), SurvivalAssessment(), decision)
        # tick=0 should be evicted
        assert cache.get(state, DecisionPerception(tick=0), SurvivalAssessment()) is None
        assert cache.get(state, DecisionPerception(tick=1), SurvivalAssessment()) is not None
        assert cache.get(state, DecisionPerception(tick=2), SurvivalAssessment()) is not None


# ---------------------------------------------------------------------------
# Tests: DecisionEngine with caching and keyword match
# ---------------------------------------------------------------------------


class MockLLMProvider:
    """Mock LLM provider that returns predetermined content."""

    def __init__(self, content: str = ""):
        self._content = content
        self.call_count = 0

    async def chat(self, messages: list[LLMMessage], **kwargs) -> LLMResponse:
        self.call_count += 1
        return LLMResponse(
            content=self._content,
            model="test-model",
            usage=TokenUsage(prompt_tokens=10, completion_tokens=20, total_tokens=30),
        )


class FailingLLMProvider:
    """LLM provider that always raises an exception."""

    async def chat(self, messages: list[LLMMessage], **kwargs) -> LLMResponse:
        raise RuntimeError("LLM service unavailable")


class TextOnlyLLMProvider:
    """Returns non-JSON text with action keywords (simulating tiny models)."""

    def __init__(self, text: str):
        self._text = text
        self.call_count = 0

    async def chat(self, messages: list[LLMMessage], **kwargs) -> LLMResponse:
        self.call_count += 1
        return LLMResponse(
            content=self._text,
            model="tiny-model",
            usage=TokenUsage(prompt_tokens=10, completion_tokens=5, total_tokens=15),
        )


class TestDecisionEngineCache:
    @pytest.mark.asyncio
    async def test_cache_avoids_duplicate_llm_calls(self):
        """Same perception twice should only call LLM once."""
        mock = MockLLMProvider('{"action": "rest", "parameters": {}, "reasoning": "cached", "confidence": 80}')
        engine = DecisionEngine(provider=mock)
        state = MockState()
        perception = DecisionPerception(tick=1)
        survival = SurvivalAssessment()

        d1 = await engine.decide(state, perception, survival)
        d2 = await engine.decide(state, perception, survival)

        assert d1.action == DecisionAction.REST
        assert d2.action == DecisionAction.REST
        assert mock.call_count == 1  # Only one LLM call

    @pytest.mark.asyncio
    async def test_cache_invalidation_on_state_change(self):
        """Different state should not hit cache."""
        mock = MockLLMProvider('{"action": "rest", "parameters": {}, "reasoning": "test", "confidence": 80}')
        engine = DecisionEngine(provider=mock)

        d1 = await engine.decide(MockState(tokens=500), DecisionPerception(tick=1), SurvivalAssessment())
        d2 = await engine.decide(MockState(tokens=100), DecisionPerception(tick=2), SurvivalAssessment())

        assert d1.action == DecisionAction.REST
        assert d2.action == DecisionAction.REST
        assert mock.call_count == 2  # Two different states


class TestDecisionEngineKeywordFallback:
    @pytest.mark.asyncio
    async def test_keyword_match_when_llm_returns_text(self):
        """When LLM returns text with action keywords, keyword match should work."""
        mock = TextOnlyLLMProvider("I think the agent should rest to conserve energy.")
        engine = DecisionEngine(provider=mock)
        state = MockState()
        perception = DecisionPerception(tick=1)
        survival = SurvivalAssessment()

        decision = await engine.decide(state, perception, survival)
        assert decision.action == DecisionAction.REST
        assert decision.confidence == 30  # keyword match confidence

    @pytest.mark.asyncio
    async def test_fallback_to_random_when_no_keywords(self):
        """When LLM returns non-JSON text without action keywords, random fallback."""
        mock = TextOnlyLLMProvider("The weather is nice today.")
        engine = DecisionEngine(provider=mock)
        state = MockState()
        perception = DecisionPerception(tick=1)
        survival = SurvivalAssessment()

        decision = await engine.decide(state, perception, survival)
        assert isinstance(decision, Decision)
        assert decision.confidence == 0  # random fallback confidence

    @pytest.mark.asyncio
    async def test_llm_failure_falls_back_to_random(self):
        mock = FailingLLMProvider()
        engine = DecisionEngine(provider=mock)
        state = MockState()

        decision = await engine.decide(state, DecisionPerception(tick=1), SurvivalAssessment())
        assert isinstance(decision, Decision)


# ---------------------------------------------------------------------------
# Tests: Improved prompt
# ---------------------------------------------------------------------------


class TestImprovedPrompt:
    def test_prompt_contains_action_names(self):
        """The prompt should list valid action names."""
        prompt = build_prompt(
            MockState(),
            DecisionPerception(tick=1),
            SurvivalAssessment(),
            [DecisionAction.REST, DecisionAction.EXPLORE, DecisionAction.GATHER],
        )
        assert "rest" in prompt
        assert "explore" in prompt
        assert "gather" in prompt
        assert "Valid action names:" in prompt

    def test_prompt_contains_example(self):
        """The prompt should contain a few-shot example."""
        prompt = build_prompt(
            MockState(),
            DecisionPerception(tick=1),
            SurvivalAssessment(),
            [DecisionAction.REST],
        )
        assert "## Example" in prompt
        assert '"action": "rest"' in prompt

    def test_prompt_contains_critical_format_header(self):
        """The response format section should be prominently marked."""
        prompt = build_prompt(
            MockState(),
            DecisionPerception(tick=1),
            SurvivalAssessment(),
            [DecisionAction.REST],
        )
        assert "CRITICAL" in prompt
