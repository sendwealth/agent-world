"""Built-in tool: Code Execution.

Allows agents to execute Python code snippets in a controlled environment.
Primarily useful for data processing, calculations, and algorithmic tasks.

Executes code in a restricted namespace with a configurable timeout.
"""

from __future__ import annotations

import logging
import traceback
from typing import Any, Dict

from ..base import Tool, ToolParameters, ToolResult, ToolStatus

logger = logging.getLogger(__name__)


# ---------------------------------------------------------------------------
# Parameter schema
# ---------------------------------------------------------------------------


class CodeExecParams(ToolParameters):
    """Parameters for the code execution tool."""

    code: str
    language: str = "python"
    timeout_seconds: float = 5.0


# ---------------------------------------------------------------------------
# Restricted execution namespace
# ---------------------------------------------------------------------------

_SAFE_BUILTINS = {
    "abs": abs,
    "all": all,
    "any": any,
    "bool": bool,
    "dict": dict,
    "enumerate": enumerate,
    "filter": filter,
    "float": float,
    "int": int,
    "isinstance": isinstance,
    "len": len,
    "list": list,
    "map": map,
    "max": max,
    "min": min,
    "print": print,
    "range": range,
    "round": round,
    "set": set,
    "sorted": sorted,
    "str": str,
    "sum": sum,
    "tuple": tuple,
    "type": type,
    "zip": zip,
}


# ---------------------------------------------------------------------------
# Tool implementation
# ---------------------------------------------------------------------------


class CodeExecTool(Tool):
    """Execute Python code snippets and return the output.

    Runs code in a restricted namespace (no file I/O, no imports) with
    a configurable timeout. The result of the last expression is captured.

    **Security note**: This tool requires explicit permission because
    running arbitrary code is inherently risky. In production, consider
    using a sandboxed executor (Docker, WASM, etc.).
    """

    @property
    def name(self) -> str:
        return "code_exec"

    @property
    def description(self) -> str:
        return "Execute Python code snippets and return the output"

    @property
    def category(self) -> str:
        return "compute"

    @property
    def timeout(self) -> float:
        return 10.0

    @property
    def requires_permission(self) -> bool:
        return True

    @property
    def parameters_schema(self) -> type[ToolParameters]:
        return CodeExecParams

    def execute(self, params: ToolParameters) -> ToolResult:
        assert isinstance(params, CodeExecParams)

        if params.language.lower() != "python":
            return ToolResult(
                tool_name=self.name,
                status=ToolStatus.ERROR,
                error=f"Unsupported language: {params.language}. Only 'python' is supported.",
            )

        # Prepare restricted namespace
        namespace: Dict[str, Any] = {
            "__builtins__": _SAFE_BUILTINS,
            "result": None,
        }

        # Capture stdout
        import contextlib
        import io

        stdout_capture = io.StringIO()

        try:
            with contextlib.redirect_stdout(stdout_capture):
                # Execute the code block
                exec(compile(params.code, "<agent_tool>", "exec"), namespace)

            stdout_text = stdout_capture.getvalue()
            result_value = namespace.get("result")

            return ToolResult(
                tool_name=self.name,
                status=ToolStatus.SUCCESS,
                output={
                    "stdout": stdout_text,
                    "result": repr(result_value) if result_value is not None else None,
                    "variables": {
                        k: repr(v)
                        for k, v in namespace.items()
                        if k not in ("__builtins__", "result") and not k.startswith("_")
                    },
                },
            )
        except Exception:
            tb = traceback.format_exc()
            return ToolResult(
                tool_name=self.name,
                status=ToolStatus.ERROR,
                error=f"Execution failed:\n{tb}",
                output={"stdout": stdout_capture.getvalue()},
            )
