"""Agent Runtime Tools Framework — pluggable utilities for agent-world agents.

Tools are distinct from skills: skills are *learned capabilities* that level up
with XP, while tools are *stateless utilities* the agent can invoke to interact
with the outside world (HTTP requests, file I/O, code execution, etc.).

Provides:
- Tool: Abstract base class for all tools.
- ToolResult: Immutable result of a tool invocation.
- ToolParameters: Base Pydantic model for parameter validation.
- ToolRegistry: Central registry for registration, discovery, and invocation.
- Built-in tools: http_request, file_ops, code_exec.

Integration points:
- **Think Loop**: The decide step can choose to invoke a tool via the registry.
- **Context Engine**: Tool results can be injected as context items for LLM prompts.
"""

from __future__ import annotations

from typing import Optional, Set

from .base import Tool, ToolParameters, ToolResult, ToolStatus
from .builtin import (
    CodeExecTool,
    FileOpsTool,
    HttpRequestTool,
    create_builtin_tools,
)
from .registry import ToolRegistry

# Convenience: pre-built registry with all built-in tools


def create_registry_with_builtins(
    *,
    sandbox_http: bool = True,
    file_ops_base_dir: Optional[str] = None,
    allowed_tools: Optional[Set[str]] = None,
) -> ToolRegistry:
    """Create a ToolRegistry pre-loaded with all built-in tools.

    Args:
        sandbox_http: If True, HTTP tool returns simulated responses.
        file_ops_base_dir: Base directory for file operations.
        allowed_tools: If provided, only these tool names may be invoked.

    Returns:
        A ToolRegistry with built-in tools registered.
    """
    registry = ToolRegistry(allowed_tools=allowed_tools)
    for tool in create_builtin_tools(
        sandbox_http=sandbox_http,
        file_ops_base_dir=file_ops_base_dir,
    ):
        registry.register(tool)
    return registry


__all__ = [
    # Base
    "Tool",
    "ToolParameters",
    "ToolResult",
    "ToolStatus",
    # Registry
    "ToolRegistry",
    # Built-in tools
    "HttpRequestTool",
    "FileOpsTool",
    "CodeExecTool",
    "create_builtin_tools",
    "create_registry_with_builtins",
]
