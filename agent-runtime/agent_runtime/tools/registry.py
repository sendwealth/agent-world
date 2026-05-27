"""ToolRegistry — dynamic registration, discovery, and invocation of agent tools.

The registry is the central hub for the tools framework. Agent Runtime components
(think loop, context engine) use the registry to discover available tools and
invoke them by name.

Usage::

    from agent_runtime.tools import ToolRegistry
    from agent_runtime.tools.builtin import create_registry_with_builtins

    # With built-in tools pre-loaded
    registry = create_registry_with_builtins()

    # Or start empty and register manually
    registry = ToolRegistry()
    registry.register(my_tool)

    # Discover
    tools = registry.list_tools(category="network")

    # Invoke
    result = await registry.invoke("http_request", {"url": "https://example.com", "method": "GET"})

    # Generate schemas for LLM function calling
    schemas = registry.get_all_schemas()
"""

from __future__ import annotations

import logging
from dataclasses import dataclass
from typing import Any, Dict, List, Optional, Set

from .base import Tool, ToolResult, ToolStatus

logger = logging.getLogger(__name__)


# ---------------------------------------------------------------------------
# Registry entry (bookkeeping wrapper)
# ---------------------------------------------------------------------------


@dataclass
class _RegistryEntry:
    """Internal bookkeeping for a registered tool."""

    tool: Tool
    enabled: bool = True
    invoke_count: int = 0
    error_count: int = 0


# ---------------------------------------------------------------------------
# ToolRegistry
# ---------------------------------------------------------------------------


class ToolRegistry:
    """Central registry for tool instances.

    Supports:
    - Dynamic registration / unregistration
    - Enable / disable tools without removing them
    - Category-based filtering
    - Invocation with error tracking
    - Schema export for LLM function-calling
    - Permission-aware invocation
    """

    def __init__(self, allowed_tools: Optional[Set[str]] = None) -> None:
        """Initialise the registry.

        Args:
            allowed_tools: If provided, only these tool names may be invoked.
                           Registration is unrestricted, but invocation of
                           non-allowed tools will return PERMISSION_DENIED.
        """
        self._entries: Dict[str, _RegistryEntry] = {}
        self._allowed_tools: Optional[Set[str]] = allowed_tools

    # ------------------------------------------------------------------
    # Registration
    # ------------------------------------------------------------------

    def register(self, tool: Tool) -> None:
        """Register a tool instance.

        Raises:
            ValueError: If a tool with the same name is already registered.
            TypeError: If the object is not a Tool subclass.
        """
        if not isinstance(tool, Tool):
            raise TypeError(f"Expected a Tool instance, got {type(tool).__name__}")

        name = tool.name
        if name in self._entries:
            raise ValueError(f"Tool '{name}' is already registered")
        self._entries[name] = _RegistryEntry(tool=tool)
        logger.debug("Registered tool: %s (category=%s)", name, tool.category)

    def unregister(self, name: str) -> Tool:
        """Remove a tool from the registry.

        Raises:
            KeyError: If the tool is not found.

        Returns:
            The removed Tool instance.
        """
        if name not in self._entries:
            raise KeyError(f"Tool '{name}' is not registered")
        entry = self._entries.pop(name)
        logger.debug("Unregistered tool: %s", name)
        return entry.tool

    def replace(self, tool: Tool) -> None:
        """Replace an existing tool with an updated instance.

        Raises:
            KeyError: If the tool is not currently registered.
        """
        if tool.name not in self._entries:
            raise KeyError(f"Tool '{tool.name}' is not registered")
        old_entry = self._entries[tool.name]
        self._entries[tool.name] = _RegistryEntry(
            tool=tool,
            enabled=old_entry.enabled,
        )
        logger.debug("Replaced tool: %s", tool.name)

    # ------------------------------------------------------------------
    # Enable / Disable
    # ------------------------------------------------------------------

    def enable(self, name: str) -> None:
        """Enable a disabled tool."""
        self._get_entry(name).enabled = True

    def disable(self, name: str) -> None:
        """Disable a tool without removing it."""
        self._get_entry(name).enabled = False

    # ------------------------------------------------------------------
    # Query
    # ------------------------------------------------------------------

    def get(self, name: str) -> Tool:
        """Get a tool instance by name.

        Raises:
            KeyError: If not found.
        """
        return self._get_entry(name).tool

    def has(self, name: str) -> bool:
        """Check whether a tool is registered."""
        return name in self._entries

    def is_enabled(self, name: str) -> bool:
        """Check whether a tool is both registered and enabled."""
        entry = self._entries.get(name)
        return entry is not None and entry.enabled

    def list_tools(
        self,
        category: Optional[str] = None,
        enabled_only: bool = False,
    ) -> List[Tool]:
        """Return registered tools, optionally filtered.

        Args:
            category: Filter by category.
            enabled_only: Exclude disabled tools.

        Returns:
            List of Tool instances sorted by name.
        """
        results: List[Tool] = []
        for entry in self._entries.values():
            if enabled_only and not entry.enabled:
                continue
            if category is not None and entry.tool.category != category:
                continue
            results.append(entry.tool)
        return sorted(results, key=lambda t: t.name)

    def categories(self) -> List[str]:
        """Return unique category names across all registered tools."""
        return sorted({e.tool.category for e in self._entries.values()})

    @property
    def count(self) -> int:
        """Total number of registered tools (including disabled)."""
        return len(self._entries)

    # ------------------------------------------------------------------
    # Invocation
    # ------------------------------------------------------------------

    async def invoke(
        self,
        name: str,
        params: Dict[str, Any],
        *,
        skip_permission_check: bool = False,
    ) -> ToolResult:
        """Invoke a tool by name with the given parameters.

        Args:
            name: Registered tool name.
            params: Raw parameter dict — validated against the tool's schema.
            skip_permission_check: Bypass the allowed_tools allowlist.

        Returns:
            A ToolResult with the outcome.

        Raises:
            KeyError: If the tool is not registered.
        """
        entry = self._get_entry(name)

        # Check enabled
        if not entry.enabled:
            return ToolResult(
                tool_name=name,
                status=ToolStatus.ERROR,
                error=f"Tool '{name}' is currently disabled",
            )

        # Permission check
        if (
            not skip_permission_check
            and self._allowed_tools is not None
            and name not in self._allowed_tools
        ):
            entry.error_count += 1
            return ToolResult(
                tool_name=name,
                status=ToolStatus.PERMISSION_DENIED,
                error=f"Tool '{name}' is not in the allowed tools set",
            )

        # Also check tool-level permission flag
        if entry.tool.requires_permission and not skip_permission_check:
            if self._allowed_tools is not None and name not in self._allowed_tools:
                entry.error_count += 1
                return ToolResult(
                    tool_name=name,
                    status=ToolStatus.PERMISSION_DENIED,
                    error=f"Tool '{name}' requires explicit permission",
                )

        # Execute
        entry.invoke_count += 1
        result = await entry.tool.run(params)
        if not result.success:
            entry.error_count += 1
        return result

    def invoke_sync(
        self,
        name: str,
        params: Dict[str, Any],
        **kwargs: Any,
    ) -> ToolResult:
        """Synchronous wrapper around invoke().

        Useful for non-async code paths. Runs invoke() in a new event loop.
        """
        import asyncio

        try:
            loop = asyncio.get_running_loop()
        except RuntimeError:
            loop = None

        if loop is not None and loop.is_running():
            # We're inside an existing event loop — use nest_asyncio-style fallback
            import concurrent.futures

            with concurrent.futures.ThreadPoolExecutor(max_workers=1) as pool:
                future = pool.submit(
                    asyncio.run, self.invoke(name, params, **kwargs)
                )
                timeout = entry.tool.timeout if (entry := self._entries.get(name)) else 60
                return future.result(timeout=timeout)
        else:
            return asyncio.run(self.invoke(name, params, **kwargs))

    # ------------------------------------------------------------------
    # Schema export
    # ------------------------------------------------------------------

    def get_all_schemas(
        self,
        category: Optional[str] = None,
        enabled_only: bool = True,
    ) -> List[Dict[str, Any]]:
        """Return JSON-serialisable schemas for all (or filtered) tools.

        Suitable for injecting into LLM prompts as function definitions.
        """
        tools = self.list_tools(category=category, enabled_only=enabled_only)
        return [t.get_schema_dict() for t in tools]

    # ------------------------------------------------------------------
    # Stats
    # ------------------------------------------------------------------

    def get_stats(self) -> Dict[str, Dict[str, Any]]:
        """Return invocation statistics for all tools."""
        return {
            name: {
                "enabled": entry.enabled,
                "invoke_count": entry.invoke_count,
                "error_count": entry.error_count,
                "category": entry.tool.category,
            }
            for name, entry in self._entries.items()
        }

    # ------------------------------------------------------------------
    # Internal helpers
    # ------------------------------------------------------------------

    def _get_entry(self, name: str) -> _RegistryEntry:
        if name not in self._entries:
            raise KeyError(f"Tool '{name}' is not registered")
        return self._entries[name]
