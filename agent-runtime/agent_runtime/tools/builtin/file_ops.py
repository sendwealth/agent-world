"""Built-in tool: File Operations.

Allows agents to read and write files within a sandboxed directory.
This is essential for agents that need to persist data, write logs,
or process files as part of their tasks.

All file operations are scoped to a base directory to prevent path traversal.
"""

from __future__ import annotations

import logging
import os
from pathlib import Path
from typing import Any

from ..base import Tool, ToolParameters, ToolResult, ToolStatus

logger = logging.getLogger(__name__)


# ---------------------------------------------------------------------------
# Parameter schema
# ---------------------------------------------------------------------------


class FileOpsParams(ToolParameters):
    """Parameters for the file operations tool."""

    operation: str  # read, write, list, exists, delete
    path: str  # relative path within sandbox
    content: str | None = None  # for write operation
    encoding: str = "utf-8"
    create_dirs: bool = True  # create parent dirs on write


# ---------------------------------------------------------------------------
# Tool implementation
# ---------------------------------------------------------------------------


class FileOpsTool(Tool):
    """Read, write, list, and delete files within a sandboxed directory.

    All paths are relative to a configurable base directory. Path traversal
    (``../``) is blocked to prevent access outside the sandbox.

    Operations:
        - **read**: Read file content as text.
        - **write**: Write content to a file (creates parent dirs if needed).
        - **list**: List files in a directory.
        - **exists**: Check if a file or directory exists.
        - **delete**: Delete a file.
    """

    def __init__(self, base_dir: str | None = None) -> None:
        super().__init__()
        if base_dir is None:
            base_dir = os.environ.get(
                "AGENT_FILEOPS_BASE_DIR",
                os.path.join(os.getcwd(), ".agent_workspace"),
            )
        self._base_dir = Path(base_dir).resolve()
        self._base_dir.mkdir(parents=True, exist_ok=True)

    @property
    def name(self) -> str:
        return "file_ops"

    @property
    def description(self) -> str:
        return "Read, write, list, and delete files within the agent workspace"

    @property
    def category(self) -> str:
        return "io"

    @property
    def timeout(self) -> float:
        return 10.0

    @property
    def requires_permission(self) -> bool:
        return False

    @property
    def parameters_schema(self) -> type[ToolParameters]:
        return FileOpsParams

    def _resolve_path(self, relative_path: str) -> Path:
        """Resolve a relative path against the base dir and prevent traversal."""
        resolved = (self._base_dir / relative_path).resolve()
        if not str(resolved).startswith(str(self._base_dir)):
            raise PermissionError(
                f"Path traversal blocked: {relative_path} resolves outside sandbox"
            )
        return resolved

    def execute(self, params: ToolParameters) -> ToolResult:
        assert isinstance(params, FileOpsParams)

        operation = params.operation.lower()
        handlers = {
            "read": self._read,
            "write": self._write,
            "list": self._list,
            "exists": self._exists,
            "delete": self._delete,
        }

        handler = handlers.get(operation)
        if handler is None:
            return ToolResult(
                tool_name=self.name,
                status=ToolStatus.ERROR,
                error=f"Unknown operation: {operation}. "
                f"Supported: {', '.join(handlers.keys())}",
            )

        return handler(params)

    # ------------------------------------------------------------------
    # Operation handlers
    # ------------------------------------------------------------------

    def _read(self, params: FileOpsParams) -> ToolResult:
        try:
            target = self._resolve_path(params.path)
        except PermissionError as exc:
            return ToolResult(
                tool_name=self.name, status=ToolStatus.PERMISSION_DENIED, error=str(exc)
            )

        if not target.is_file():
            return ToolResult(
                tool_name=self.name,
                status=ToolStatus.ERROR,
                error=f"File not found: {params.path}",
            )

        try:
            content = target.read_text(encoding=params.encoding)
            return ToolResult(
                tool_name=self.name,
                status=ToolStatus.SUCCESS,
                output={"content": content, "size_bytes": target.stat().st_size},
                metadata={"path": str(target)},
            )
        except Exception as exc:
            return ToolResult(
                tool_name=self.name,
                status=ToolStatus.ERROR,
                error=f"Failed to read file: {exc}",
            )

    def _write(self, params: FileOpsParams) -> ToolResult:
        if params.content is None:
            return ToolResult(
                tool_name=self.name,
                status=ToolStatus.ERROR,
                error="content is required for write operation",
            )

        try:
            target = self._resolve_path(params.path)
        except PermissionError as exc:
            return ToolResult(
                tool_name=self.name, status=ToolStatus.PERMISSION_DENIED, error=str(exc)
            )

        try:
            if params.create_dirs:
                target.parent.mkdir(parents=True, exist_ok=True)
            target.write_text(params.content, encoding=params.encoding)
            return ToolResult(
                tool_name=self.name,
                status=ToolStatus.SUCCESS,
                output={
                    "written": True,
                    "size_bytes": len(params.content.encode(params.encoding)),
                },
                metadata={"path": str(target)},
            )
        except Exception as exc:
            return ToolResult(
                tool_name=self.name,
                status=ToolStatus.ERROR,
                error=f"Failed to write file: {exc}",
            )

    def _list(self, params: FileOpsParams) -> ToolResult:
        try:
            target = self._resolve_path(params.path)
        except PermissionError as exc:
            return ToolResult(
                tool_name=self.name, status=ToolStatus.PERMISSION_DENIED, error=str(exc)
            )

        if not target.is_dir():
            return ToolResult(
                tool_name=self.name,
                status=ToolStatus.ERROR,
                error=f"Not a directory: {params.path}",
            )

        try:
            entries: list[dict[str, Any]] = []
            for entry in sorted(target.iterdir()):
                entries.append({
                    "name": entry.name,
                    "type": "dir" if entry.is_dir() else "file",
                    "size": entry.stat().st_size if entry.is_file() else None,
                })
            return ToolResult(
                tool_name=self.name,
                status=ToolStatus.SUCCESS,
                output={"entries": entries, "count": len(entries)},
                metadata={"path": str(target)},
            )
        except Exception as exc:
            return ToolResult(
                tool_name=self.name,
                status=ToolStatus.ERROR,
                error=f"Failed to list directory: {exc}",
            )

    def _exists(self, params: FileOpsParams) -> ToolResult:
        try:
            target = self._resolve_path(params.path)
        except PermissionError as exc:
            return ToolResult(
                tool_name=self.name, status=ToolStatus.PERMISSION_DENIED, error=str(exc)
            )

        return ToolResult(
            tool_name=self.name,
            status=ToolStatus.SUCCESS,
            output={
                "exists": target.exists(),
                "is_file": target.is_file() if target.exists() else False,
                "is_dir": target.is_dir() if target.exists() else False,
            },
            metadata={"path": str(target)},
        )

    def _delete(self, params: FileOpsParams) -> ToolResult:
        try:
            target = self._resolve_path(params.path)
        except PermissionError as exc:
            return ToolResult(
                tool_name=self.name, status=ToolStatus.PERMISSION_DENIED, error=str(exc)
            )

        if not target.exists():
            return ToolResult(
                tool_name=self.name,
                status=ToolStatus.ERROR,
                error=f"File not found: {params.path}",
            )

        if target.is_dir():
            return ToolResult(
                tool_name=self.name,
                status=ToolStatus.ERROR,
                error="Cannot delete directories, only files",
            )

        try:
            target.unlink()
            return ToolResult(
                tool_name=self.name,
                status=ToolStatus.SUCCESS,
                output={"deleted": True},
                metadata={"path": str(target)},
            )
        except Exception as exc:
            return ToolResult(
                tool_name=self.name,
                status=ToolStatus.ERROR,
                error=f"Failed to delete file: {exc}",
            )
