"""Built-in skill: Trading.

The trading skill represents an agent's ability to analyze markets,
negotiate deals, and execute profitable trades. Higher levels improve
profit margins and unlock advanced strategies.
"""

from __future__ import annotations

from typing import Any, Dict

from ..models.skill import Skill
from .registry import SkillDefinition


def _execute_trading(agent_skills: Dict[str, Skill], **kwargs: Any) -> Dict[str, Any]:
    """Execute a trading task.

    Kwargs:
        action: Trade action type ("buy", "sell", "analyze", "negotiate").
        item: Item or asset being traded.
        quantity: Amount to trade (optional).

    Returns:
        Dict with success status, profit_margin, and result details.
    """
    trading_skill = agent_skills.get("trading")
    level = trading_skill.level if trading_skill else 0

    action = kwargs.get("action", "analyze")
    item = kwargs.get("item", "generic goods")
    quantity = kwargs.get("quantity", 1)

    strategy = "basic bartering"
    if level >= 3:
        strategy = "market analysis"
    if level >= 5:
        strategy = "arbitrage detection"
    if level >= 7:
        strategy = "portfolio optimization"
    if level >= 9:
        strategy = "algorithmic trading"

    # Profit margin scales with level (5% per level, capped at 50%)
    profit_margin = min(level * 0.05, 0.50)
    success = level >= 1

    return {
        "skill": "trading",
        "action": action,
        "item": item,
        "quantity": quantity,
        "strategy": strategy,
        "profit_margin": profit_margin,
        "success": success,
        "level_used": level,
    }


TRADING_SKILL = SkillDefinition(
    name="trading",
    description="Ability to analyze markets, negotiate deals, and execute profitable trades",
    max_level=10,
    execute_fn=_execute_trading,
    category="economic",
)
