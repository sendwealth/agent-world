"""Built-in tools for the Agent Runtime tools framework.

Provides:
- HttpRequestTool: Make HTTP requests (sandbox mode by default)
- FileOpsTool: Read/write/list/delete files within a sandboxed directory
- CodeExecTool: Execute Python code snippets (requires permission)
"""

from __future__ import annotations

from typing import List, Optional

from .code_exec import CodeExecTool
from .file_ops import FileOpsTool
from .http_request import HttpRequestTool

__all__ = [
    "HttpRequestTool",
    "FileOpsTool",
    "CodeExecTool",
]


def create_builtin_tools(
    *,
    sandbox_http: bool = True,
    file_ops_base_dir: Optional[str] = None,
) -> List[Tool]:
    """Create instances of all built-in tools.

    Args:
        sandbox_http: If True (default), HTTP tool returns simulated responses.
        file_ops_base_dir: Base directory for file operations. Defaults to
            ``.agent_workspace`` in the current working directory.

    Returns:
        List of Tool instances ready to register.
    """
    return [
        HttpRequestTool(sandbox=sandbox_http),
        FileOpsTool(base_dir=file_ops_base_dir),
        CodeExecTool(),
    ]
