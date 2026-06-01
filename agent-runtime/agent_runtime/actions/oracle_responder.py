"""Oracle Responder -- strategy-based response generation for Oracle messages.

Generates contextually appropriate responses when an agent receives an Oracle
from a human player. Different Oracle types trigger different response strategies:

- guidance: Agent acknowledges the advice and commits to following it.
- warning:  Agent expresses caution and adjusts behaviour.
- blessing: Agent shows gratitude and positive emotion.
- curse:    Agent shows distress or determination to overcome.

Uses an LLM provider (optional) for natural-language responses; falls back to
template-based responses when no LLM is available.
"""

from __future__ import annotations

import logging
from dataclasses import dataclass
from enum import Enum

from agent_runtime.llm.base import LLMMessage, LLMProvider

logger = logging.getLogger(__name__)


class OracleType(str, Enum):
    """Oracle types -- aligned with World Engine OracleType."""

    GUIDANCE = "guidance"
    WARNING = "warning"
    BLESSING = "blessing"
    CURSE = "curse"


class OracleResponseStrategy(str, Enum):
    """Response strategy chosen based on Oracle type."""

    ACKNOWLEDGE_GUIDANCE = "acknowledge_guidance"
    HEED_WARNING = "heed_warning"
    EXPRESS_GRATITUDE = "express_gratitude"
    SHOW_RESILIENCE = "show_resilience"


_ORACLE_STRATEGY_MAP: dict[OracleType, OracleResponseStrategy] = {
    OracleType.GUIDANCE: OracleResponseStrategy.ACKNOWLEDGE_GUIDANCE,
    OracleType.WARNING: OracleResponseStrategy.HEED_WARNING,
    OracleType.BLESSING: OracleResponseStrategy.EXPRESS_GRATITUDE,
    OracleType.CURSE: OracleResponseStrategy.SHOW_RESILIENCE,
}

_FALLBACK_TEMPLATES: dict[OracleResponseStrategy, str] = {
    OracleResponseStrategy.ACKNOWLEDGE_GUIDANCE: (
        "Thank you for the guidance. I will follow this advice and adjust my actions accordingly."
    ),
    OracleResponseStrategy.HEED_WARNING: (
        "I hear your warning. I will be more cautious and reconsider my current approach."
    ),
    OracleResponseStrategy.EXPRESS_GRATITUDE: (
        "I am grateful for this blessing! It gives me strength to continue."
    ),
    OracleResponseStrategy.SHOW_RESILIENCE: (
        "This curse is a setback, but I will not give up. I will find a way to overcome this."
    ),
}

_ORACLE_RESPONSE_PROMPT = """\
You are {agent_name}, an autonomous agent in a simulated world.
A human has sent you an Oracle ({oracle_type}):

---
{oracle_content}
---

Respond as the agent in 1-3 sentences. Your response should reflect:
- Strategy: {strategy_description}
- Your personality and current situation
- Sincerity -- do not break character

Your response:"""

_STRATEGY_DESCRIPTIONS: dict[OracleResponseStrategy, str] = {
    OracleResponseStrategy.ACKNOWLEDGE_GUIDANCE: (
        "Acknowledge the guidance, express understanding, and commit to following the advice."
    ),
    OracleResponseStrategy.HEED_WARNING: (
        "Show that you take the warning seriously, express caution, and plan to adjust behaviour."
    ),
    OracleResponseStrategy.EXPRESS_GRATITUDE:
        "Show genuine gratitude and positive emotion. Feel uplifted.",
    OracleResponseStrategy.SHOW_RESILIENCE: (
        "Express determination and resilience. Acknowledge the curse but show resolve to overcome."
    ),
}


@dataclass(frozen=True)
class OracleResponseResult:
    """The result of generating an Oracle response."""

    oracle_id: str
    response: str
    strategy: OracleResponseStrategy
    oracle_type: OracleType
    used_llm: bool = False


class OracleResponder:
    """Generates strategic responses to Oracle messages.

    Uses an LLM provider when available; falls back to template-based
    responses when the LLM is unavailable or fails.
    """

    def __init__(self, llm_provider: LLMProvider | None = None) -> None:
        self._llm = llm_provider

    async def respond(
        self,
        oracle_id: str,
        oracle_type: str,
        content: str,
        agent_name: str,
    ) -> OracleResponseResult:
        """Generate a response to an Oracle message."""
        try:
            otype = OracleType(oracle_type)
        except ValueError:
            logger.warning("Unknown oracle type %s, defaulting to guidance", oracle_type)
            otype = OracleType.GUIDANCE

        strategy = _ORACLE_STRATEGY_MAP.get(otype, OracleResponseStrategy.ACKNOWLEDGE_GUIDANCE)

        response_text = await self._generate_llm_response(
            agent_name=agent_name,
            oracle_type=otype,
            content=content,
            strategy=strategy,
        )
        used_llm = response_text is not None

        if not used_llm or not response_text:
            response_text = _FALLBACK_TEMPLATES[strategy]
            used_llm = False

        return OracleResponseResult(
            oracle_id=oracle_id,
            response=response_text,
            strategy=strategy,
            oracle_type=otype,
            used_llm=used_llm,
        )

    async def _generate_llm_response(
        self,
        agent_name: str,
        oracle_type: OracleType,
        content: str,
        strategy: OracleResponseStrategy,
    ) -> str | None:
        """Try to generate a response via the LLM provider."""
        if self._llm is None:
            return None

        prompt = _ORACLE_RESPONSE_PROMPT.format(
            agent_name=agent_name,
            oracle_type=oracle_type.value,
            oracle_content=content,
            strategy_description=_STRATEGY_DESCRIPTIONS[strategy],
        )

        try:
            messages = [LLMMessage(role="user", content=prompt)]
            response = await self._llm.generate(messages)
            text = response.strip()
            if text:
                logger.debug("LLM oracle response generated (%d chars)", len(text))
                return text
            return None
        except Exception:
            logger.warning("LLM oracle response generation failed, using fallback")
            return None

    def get_strategy(self, oracle_type: str) -> OracleResponseStrategy:
        """Get the response strategy for a given Oracle type."""
        try:
            otype = OracleType(oracle_type)
        except ValueError:
            return OracleResponseStrategy.ACKNOWLEDGE_GUIDANCE
        return _ORACLE_STRATEGY_MAP.get(otype, OracleResponseStrategy.ACKNOWLEDGE_GUIDANCE)
