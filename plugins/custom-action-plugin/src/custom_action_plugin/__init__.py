"""
Custom Emote Plugin — Agent World

Lets agents perform expressive emote actions with configurable formatting.
Agents can use any emote text, which gets prefixed and broadcast as a social
event visible to nearby agents in the simulation.
"""

from __future__ import annotations

import json
from typing import Dict, List, Optional
from dataclasses import dataclass, field
from enum import Enum


# ─── Plugin API Types ───────────────────────────────────────────────────
# When the SDK package is published, these will be imported from:
#   from agent_world_plugin_sdk import (
#       SkillPlugin, PluginInfo, ActionContext, ActionResult,
#       WorldContext, AgentSnapshot, TokenCost, StateMutation, MutationKind,
#       PluginError,
#   )


@dataclass
class AgentSnapshot:
    """Read-only snapshot of an agent."""
    id: str
    name: str
    phase: str
    money: int
    tokens: int
    reputation: float
    skills: Dict[str, int] = field(default_factory=dict)
    alive: bool = True
    age: int = 0


@dataclass
class WorldContext:
    """Read-only world state snapshot."""
    tick: int
    agent: Optional[AgentSnapshot] = None
    visible_agents: List[AgentSnapshot] = field(default_factory=list)
    globals: Dict[str, str] = field(default_factory=dict)
    recent_events: List[str] = field(default_factory=list)


@dataclass
class ActionContext:
    """Full context for plugin execution."""
    world: WorldContext
    params: Dict[str, str] = field(default_factory=dict)
    config: Dict[str, str] = field(default_factory=dict)


class MutationKind(Enum):
    CREDIT_TOKENS = "credit_tokens"
    DEBIT_TOKENS = "debit_tokens"
    CREDIT_MONEY = "credit_money"
    DEBIT_MONEY = "debit_money"
    SET_SKILL = "set_skill"
    ADJUST_REPUTATION = "adjust_reputation"
    SET_GLOBAL = "set_global"
    EMIT_EVENT = "emit_event"


@dataclass
class StateMutation:
    """A state mutation requested by the plugin."""
    kind: MutationKind
    target_agent: Optional[str] = None
    field: str = ""
    value: str = ""


@dataclass
class ActionResult:
    """Result of execute()."""
    success: bool
    message: str
    mutations: List[StateMutation] = field(default_factory=list)
    events: List[str] = field(default_factory=list)
    data: Dict[str, str] = field(default_factory=dict)
    tokens_consumed: int = 0


@dataclass
class TokenCost:
    """Token cost estimate."""
    estimated: int
    confidence: float = 1.0
    breakdown: Optional[str] = None


@dataclass
class PluginInfo:
    """Metadata returned from init()."""
    id: str
    name: str
    version: str
    description: str
    author: str
    min_engine_version: str
    required_skills: List[str] = field(default_factory=list)
    config_schema: Optional[str] = None
    tags: List[str] = field(default_factory=list)


class PluginError(Exception):
    """Base plugin error."""
    def __init__(self, code: str = "custom", message: str = ""):
        self.code = code
        self.message = message
        super().__init__(f"[{code}] {message}")


# ─── Plugin Implementation ──────────────────────────────────────────────


class CustomEmotePlugin:
    """
    Custom Emote plugin for Agent World.

    Agents can perform expressive emote actions that are broadcast to
    nearby agents as social events. The emote text is prefixed with a
    configurable character (default: '*').

    Example usage from agent context:
        params: {"emote_text": "waves hello cheerfully"}
        → message: "* Alice waves hello cheerfully"

    Configuration:
        emote_prefix: str — Prefix for emote text (default: "*")
        max_emote_length: int — Max characters allowed (default: 200)
    """

    # Default configuration values
    DEFAULT_PREFIX = "*"
    DEFAULT_MAX_LENGTH = 200

    @classmethod
    def init(cls, config: Dict[str, str]) -> PluginInfo:
        """
        Return plugin metadata. Called once after loading.

        Validates configuration and prepares internal state.
        """
        # Validate config values if provided
        max_length_str = config.get("max_emote_length", str(cls.DEFAULT_MAX_LENGTH))
        try:
            max_length = int(max_length_str)
            if max_length <= 0:
                raise ValueError("max_emote_length must be positive")
        except (ValueError, TypeError) as e:
            raise PluginError(
                code="config_error",
                message=f"Invalid max_emote_length: {e}",
            )

        return PluginInfo(
            id="community/custom-emote",
            name="Custom Emote Plugin",
            version="1.0.0",
            description="Lets agents perform custom emote actions with "
                        "configurable prefix and style",
            author="Agent World Community",
            min_engine_version="1.0.0",
            required_skills=[],
            config_schema=json.dumps({
                "type": "object",
                "properties": {
                    "emote_prefix": {
                        "type": "string",
                        "default": "*",
                        "description": "Prefix character(s) for emote text output",
                    },
                    "max_emote_length": {
                        "type": "integer",
                        "default": 200,
                        "description": "Maximum character length for an emote message",
                    },
                },
                "required": [],
            }),
            tags=["action", "social"],
        )

    @classmethod
    def register(cls) -> List[str]:
        """Return the skill IDs this plugin provides."""
        return ["custom_emote"]

    @classmethod
    def execute(cls, ctx: ActionContext) -> ActionResult:
        """
        Execute the emote action.

        Reads the emote text from ctx.params["emote_text"], applies the
        configured prefix, and broadcasts the result as a social event.

        Returns an ActionResult with:
        - message: The formatted emote string (e.g. "* Alice waves")
        - events: A social event for nearby agents
        - data: Raw emote metadata
        - tokens_consumed: 1 per emote action
        """
        # Read configuration
        prefix = ctx.config.get("emote_prefix", cls.DEFAULT_PREFIX)
        max_length = int(ctx.config.get("max_emote_length", cls.DEFAULT_MAX_LENGTH))

        # Read emote text from params
        emote_text = ctx.params.get("emote_text", "").strip()

        if not emote_text:
            return ActionResult(
                success=False,
                message="No emote text provided. Use 'emote_text' parameter.",
                mutations=[],
                events=[],
                data={},
                tokens_consumed=1,
            )

        # Enforce max length
        if len(emote_text) > max_length:
            emote_text = emote_text[:max_length - 3] + "..."

        # Get agent name
        agent_name = "Unknown Agent"
        agent_id = None
        if ctx.world.agent:
            agent_name = ctx.world.agent.name
            agent_id = ctx.world.agent.id

        # Build the formatted emote message
        formatted_emote = f"{prefix} {agent_name} {emote_text}"

        # Build the event payload for nearby agents
        event_payload = json.dumps({
            "type": "agent_emote",
            "plugin": "community/custom-emote",
            "agent_id": agent_id,
            "agent_name": agent_name,
            "emote_text": emote_text,
            "formatted": formatted_emote,
            "tick": ctx.world.tick,
        })

        return ActionResult(
            success=True,
            message=formatted_emote,
            mutations=[],
            events=[event_payload],
            data={
                "emote_text": emote_text,
                "formatted_emote": formatted_emote,
                "agent_name": agent_name,
                "prefix": prefix,
            },
            tokens_consumed=1,
        )

    @classmethod
    def cost_estimate(cls, ctx: ActionContext) -> TokenCost:
        """Estimate token cost before execution. Fixed at 1 token per emote."""
        return TokenCost(
            estimated=1,
            confidence=1.0,
            breakdown="Fixed cost: 1 token per emote action",
        )

    @classmethod
    def shutdown(cls) -> None:
        """Graceful shutdown. No resources to clean up."""
        pass

    @classmethod
    def on_event(cls, event: str, ctx: WorldContext) -> Optional[ActionResult]:
        """
        Handle a world event.

        Reacts to 'agent_interact' events by checking if the interaction
        contains emote data and logging it.
        """
        try:
            event_data = json.loads(event)
        except (json.JSONDecodeError, TypeError):
            return None

        if event_data.get("type") == "agent_interact":
            # An interaction occurred — we could react, but for emotes
            # we only act when the agent explicitly uses the custom_emote skill.
            return None

        return None


# ─── WASM Entry Points ──────────────────────────────────────────────────
# These functions are called by the WASM runtime via ComponentizePy.

PLUGIN = CustomEmotePlugin


def init(config_json: str) -> str:
    """WASM entry: initialize plugin with config JSON."""
    config = json.loads(config_json)
    info = PLUGIN.init(config)
    return json.dumps({
        "id": info.id,
        "name": info.name,
        "version": info.version,
        "description": info.description,
        "author": info.author,
        "min_engine_version": info.min_engine_version,
        "required_skills": info.required_skills,
        "config_schema": info.config_schema,
        "tags": info.tags,
    })


def register() -> str:
    """WASM entry: return skill IDs."""
    return json.dumps(PLUGIN.register())


def execute(ctx_json: str) -> str:
    """WASM entry: execute the emote action."""
    data = json.loads(ctx_json)
    world_data = data["world"]
    world = WorldContext(
        tick=world_data["tick"],
        agent=AgentSnapshot(**world_data["agent"]) if world_data.get("agent") else None,
        visible_agents=[
            AgentSnapshot(**a) for a in world_data.get("visible_agents", [])
        ],
        globals=world_data.get("globals", {}),
        recent_events=world_data.get("recent_events", []),
    )
    ctx = ActionContext(
        world=world,
        params=data.get("params", {}),
        config=data.get("config", {}),
    )
    result = PLUGIN.execute(ctx)
    return json.dumps({
        "success": result.success,
        "message": result.message,
        "mutations": [
            {
                "kind": m.kind.value,
                "target_agent": m.target_agent,
                "field": m.field,
                "value": m.value,
            }
            for m in result.mutations
        ],
        "events": result.events,
        "data": result.data,
        "tokens_consumed": result.tokens_consumed,
    })


def cost_estimate(ctx_json: str) -> str:
    """WASM entry: estimate token cost."""
    data = json.loads(ctx_json)
    world_data = data["world"]
    world = WorldContext(
        tick=world_data["tick"],
        agent=AgentSnapshot(**world_data["agent"]) if world_data.get("agent") else None,
        visible_agents=[
            AgentSnapshot(**a) for a in world_data.get("visible_agents", [])
        ],
        globals=world_data.get("globals", {}),
        recent_events=world_data.get("recent_events", []),
    )
    ctx = ActionContext(
        world=world,
        params=data.get("params", {}),
        config=data.get("config", {}),
    )
    cost = PLUGIN.cost_estimate(ctx)
    return json.dumps({
        "estimated": cost.estimated,
        "confidence": cost.confidence,
        "breakdown": cost.breakdown,
    })


def shutdown() -> None:
    """WASM entry: graceful shutdown."""
    PLUGIN.shutdown()
