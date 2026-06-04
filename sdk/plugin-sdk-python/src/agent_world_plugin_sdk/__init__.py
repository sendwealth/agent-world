"""Agent World Plugin SDK — Python interface for building skill plugins.

Provides all data types and the abstract :class:`SkillPlugin` base class
needed to implement a plugin for the Agent World simulation engine.

Quick start::

    from agent_world_plugin_sdk import (
        SkillPlugin, PluginInfo, ActionContext, ActionResult,
        WorldContext, AgentSnapshot, TokenCost, StateMutation,
        MutationKind, PluginError,
    )

    class MyPlugin(SkillPlugin):
        @classmethod
        def init(cls, config):
            return PluginInfo(
                id="author/my-plugin",
                name="My Plugin",
                version="0.1.0",
                description="Does something useful",
                author="author",
                min_engine_version="1.0.0",
            )

        @classmethod
        def register(cls):
            return ["my_skill"]

        @classmethod
        def execute(cls, ctx):
            return ActionResult(success=True, message="Done!")

        @classmethod
        def cost_estimate(cls, ctx):
            return TokenCost(estimated=1)
"""

from __future__ import annotations

from abc import ABC, abstractmethod
from typing import Dict, List, Optional

# Re-export all types from the types module
from agent_world_plugin_sdk.types import (
    AgentId,
    AgentSnapshot,
    ActionContext,
    ActionResult,
    MutationKind,
    PluginError,
    PluginId,
    PluginInfo,
    SemVer,
    SkillId,
    StateMutation,
    TokenCost,
    WorldContext,
    JsonSerializable,
)

__all__ = [
    # Type aliases
    "PluginId",
    "SemVer",
    "SkillId",
    "AgentId",
    # Data classes
    "PluginInfo",
    "AgentSnapshot",
    "WorldContext",
    "ActionContext",
    "ActionResult",
    "StateMutation",
    "MutationKind",
    "TokenCost",
    "PluginError",
    # Base class
    "SkillPlugin",
    # Mixin
    "JsonSerializable",
]

__version__ = "0.1.0"


class SkillPlugin(ABC):
    """Abstract base class for Agent World skill plugins.

    All WASM skill plugins must subclass this and implement the abstract
    methods. The engine calls these methods during the plugin lifecycle:

    1. :meth:`init` — Called once after the WASM module is instantiated.
    2. :meth:`register` — Called after ``init`` succeeds; registers skill IDs.
    3. :meth:`execute` — Called each simulation tick or on event trigger.
    4. :meth:`cost_estimate` — Pre-flight budget check before execution.
    5. :meth:`shutdown` — Graceful teardown when the engine stops.
    6. :meth:`on_event` — Optional handler for subscribed world events.
    """

    @classmethod
    @abstractmethod
    def init(cls, config: Dict[str, str]) -> PluginInfo:
        """Return plugin metadata. Called once after loading.

        Use this to validate configuration and prepare internal state.

        Args:
            config: Plugin configuration key-value pairs from the engine.

        Returns:
            Plugin metadata.

        Raises:
            PluginError: If initialization fails.
        """
        ...

    @classmethod
    @abstractmethod
    def register(cls) -> List[SkillId]:
        """Return the skill IDs this plugin provides.

        Called after :meth:`init` succeeds. The engine registers these
        skills in the world's skill tree.

        Returns:
            List of skill identifier strings.
        """
        ...

    @classmethod
    @abstractmethod
    def execute(cls, ctx: ActionContext) -> ActionResult:
        """Execute the plugin's core logic.

        Receives an :class:`ActionContext` with world state and parameters.
        Returns an :class:`ActionResult` with success status and any
        requested state mutations.

        Args:
            ctx: Full execution context including world state, params,
                 and plugin config.

        Returns:
            Execution result with optional mutations and events.

        Raises:
            PluginError: If execution fails.
        """
        ...

    @classmethod
    @abstractmethod
    def cost_estimate(cls, ctx: ActionContext) -> TokenCost:
        """Estimate the token cost of execution *before* calling execute.

        The engine uses this to decide whether to proceed based on the
        agent's remaining token budget. Return a cost of 0 for free actions.

        Args:
            ctx: The execution context that would be passed to :meth:`execute`.

        Returns:
            Estimated token cost with confidence level.

        Raises:
            PluginError: If cost estimation fails.
        """
        ...

    @classmethod
    def shutdown(cls) -> None:
        """Graceful shutdown.

        Called when the engine is stopping or unloading the plugin.
        Override to release resources. Default is a no-op.
        """
        pass

    @classmethod
    def on_event(cls, event: str, ctx: WorldContext) -> Optional[ActionResult]:
        """Handle a world event.

        Called when an event matching the plugin's subscriptions fires.
        Override to react to events. Default does nothing.

        Args:
            event: The event string (JSON-encoded or plain).
            ctx: Current read-only world context.

        Returns:
            Optional action result if the event triggers an action.
        """
        return None
