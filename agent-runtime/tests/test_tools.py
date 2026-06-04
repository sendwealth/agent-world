"""Tests for the tools framework: Tool base class, ToolRegistry, built-in tools,
and integration with the think loop / context engine.

Covers:
- Tool abstract base class, ToolResult, ToolParameters
- ToolRegistry: register, unregister, replace, enable/disable, invoke, stats
- Built-in tools: http_request (sandbox), file_ops, code_exec
- Permission model: allowed_tools allowlist, requires_permission flag
- Schema export for LLM function-calling
- Context integration: tool results can be rendered as context text
"""

from __future__ import annotations

import asyncio
import tempfile

import pytest

from agent_runtime.tools import (
    CodeExecTool,
    FileOpsTool,
    HttpRequestTool,
    Tool,
    ToolParameters,
    ToolRegistry,
    ToolResult,
    ToolStatus,
    create_builtin_tools,
    create_registry_with_builtins,
)

# _RegistryEntry is internal to registry.py; not imported here

# ============================================================
# Async helper -- safe event-loop usage for Python 3.9
# ============================================================


def _run_async(coro):
    """Run an async coroutine without polluting the event loop policy.

    ``asyncio.run()`` in Python 3.9 leaves ``_set_called=True`` in the
    thread-local event-loop policy, preventing subsequent code from
    creating new loops.  This helper creates a fresh loop via
    ``new_event_loop()``, runs the coroutine, and closes the loop
    *without* calling ``asyncio.run()``, so the global policy state
    remains untouched.
    """
    loop = asyncio.new_event_loop()
    try:
        return loop.run_until_complete(coro)
    finally:
        loop.close()



# ============================================================
# Helpers
# ============================================================


class EchoParams(ToolParameters):
    """Simple parameter schema for testing."""

    message: str = ""
    uppercase: bool = False


class EchoTool(Tool):
    """A simple test tool that echoes back the input message."""

    @property
    def name(self) -> str:
        return "echo"

    @property
    def description(self) -> str:
        return "Echo back the input message"

    @property
    def category(self) -> str:
        return "test"

    @property
    def parameters_schema(self) -> type[ToolParameters]:
        return EchoParams

    def execute(self, params: ToolParameters) -> ToolResult:
        assert isinstance(params, EchoParams)
        text = params.message.upper() if params.uppercase else params.message
        return ToolResult(
            tool_name=self.name,
            status=ToolStatus.SUCCESS,
            output={"echo": text},
        )


class FailingTool(Tool):
    """A tool that always raises an exception."""

    @property
    def name(self) -> str:
        return "failing"

    @property
    def description(self) -> str:
        return "Always fails"

    @property
    def category(self) -> str:
        return "test"

    def execute(self, params: ToolParameters) -> ToolResult:
        raise RuntimeError("Intentional failure")


class PermissionTool(Tool):
    """A tool that requires permission."""

    @property
    def name(self) -> str:
        return "dangerous"

    @property
    def description(self) -> str:
        return "Requires permission"

    @property
    def category(self) -> str:
        return "security"

    @property
    def requires_permission(self) -> bool:
        return True

    def execute(self, params: ToolParameters) -> ToolResult:
        return ToolResult(
            tool_name=self.name,
            status=ToolStatus.SUCCESS,
            output={"secret": "unlocked"},
        )


def _make_registry() -> ToolRegistry:
    """Create a registry with the echo tool."""
    registry = ToolRegistry()
    registry.register(EchoTool())
    return registry


# ============================================================
# ToolResult tests
# ============================================================


class TestToolResult:
    def test_success_property(self):
        result = ToolResult(tool_name="test", status=ToolStatus.SUCCESS)
        assert result.success is True

    def test_error_property(self):
        result = ToolResult(
            tool_name="test", status=ToolStatus.ERROR, error="something broke"
        )
        assert result.success is False
        assert result.error == "something broke"

    def test_to_context_text_success(self):
        result = ToolResult(
            tool_name="http_request",
            status=ToolStatus.SUCCESS,
            output={"status_code": 200},
        )
        text = result.to_context_text()
        assert "[Tool:http_request]" in text
        assert "200" in text

    def test_to_context_text_error(self):
        result = ToolResult(
            tool_name="test", status=ToolStatus.ERROR, error="failed"
        )
        text = result.to_context_text()
        assert "ERROR" in text
        assert "failed" in text

    def test_frozen(self):
        result = ToolResult(tool_name="test")
        with pytest.raises(AttributeError):
            result.tool_name = "changed"  # type: ignore[misc]

    def test_metadata(self):
        result = ToolResult(
            tool_name="test",
            metadata={"latency_ms": 42.5, "sandbox": True},
        )
        assert result.metadata["latency_ms"] == 42.5


# ============================================================
# Tool base class tests
# ============================================================


class TestToolBase:
    def test_echo_tool_execute(self):
        tool = EchoTool()
        params = tool.validate_params({"message": "hello"})
        result = tool.execute(params)
        assert result.success
        assert result.output["echo"] == "hello"

    def test_echo_tool_uppercase(self):
        tool = EchoTool()
        params = tool.validate_params({"message": "hello", "uppercase": True})
        result = tool.execute(params)
        assert result.output["echo"] == "HELLO"

    def test_get_schema_dict(self):
        tool = EchoTool()
        schema = tool.get_schema_dict()
        assert schema["name"] == "echo"
        assert schema["description"] == "Echo back the input message"
        assert schema["category"] == "test"
        assert "properties" in schema["parameters"]
        assert "message" in schema["parameters"]["properties"]

    def test_run_validates_params(self):
        tool = EchoTool()
        # Missing required type but extra=allow so it passes
        result = _run_async(tool.run({"message": "test"}))
        assert result.success

    def test_run_catches_exception(self):
        tool = FailingTool()
        result = _run_async(tool.run({}))
        assert result.status == ToolStatus.ERROR
        assert "Intentional failure" in result.error

    def test_default_timeout(self):
        tool = EchoTool()
        assert tool.timeout == 30.0

    def test_default_requires_permission(self):
        tool = EchoTool()
        assert tool.requires_permission is False

    def test_repr(self):
        tool = EchoTool()
        assert repr(tool) == "Tool(name='echo', category='test')"


# ============================================================
# ToolRegistry tests
# ============================================================


class TestToolRegistryRegister:
    def test_register_single(self):
        registry = ToolRegistry()
        registry.register(EchoTool())
        assert registry.has("echo")
        assert registry.count == 1

    def test_register_duplicate_raises(self):
        registry = ToolRegistry()
        registry.register(EchoTool())
        with pytest.raises(ValueError, match="already registered"):
            registry.register(EchoTool())

    def test_register_non_tool_raises(self):
        registry = ToolRegistry()
        with pytest.raises(TypeError, match="Expected a Tool instance"):
            registry.register("not a tool")  # type: ignore[arg-type]

    def test_register_multiple(self):
        registry = ToolRegistry()
        for tool in create_builtin_tools():
            registry.register(tool)
        assert registry.count == 3


class TestToolRegistryUnregister:
    def test_unregister_existing(self):
        registry = _make_registry()
        removed = registry.unregister("echo")
        assert removed.name == "echo"
        assert not registry.has("echo")

    def test_unregister_nonexistent_raises(self):
        registry = ToolRegistry()
        with pytest.raises(KeyError, match="not registered"):
            registry.unregister("nothing")


class TestToolRegistryReplace:
    def test_replace_existing(self):
        registry = _make_registry()
        new_tool = EchoTool()
        # The class name is "EchoTool" but the tool name property is "echo"
        registry.replace(new_tool)
        assert registry.get("echo") is new_tool

    def test_replace_nonexistent_raises(self):
        registry = ToolRegistry()
        with pytest.raises(KeyError, match="not registered"):
            registry.replace(EchoTool())


class TestToolRegistryEnableDisable:
    def setup_method(self):
        self.registry = _make_registry()

    def test_disable_tool(self):
        self.registry.disable("echo")
        assert not self.registry.is_enabled("echo")

    def test_enable_tool(self):
        self.registry.disable("echo")
        self.registry.enable("echo")
        assert self.registry.is_enabled("echo")

    def test_invoke_disabled_returns_error(self):
        self.registry.disable("echo")
        result = _run_async(
            self.registry.invoke("echo", {"message": "hi"})
        )
        assert result.status == ToolStatus.ERROR
        assert "disabled" in result.error


class TestToolRegistryQuery:
    def setup_method(self):
        self.registry = create_registry_with_builtins()
        self.registry.register(EchoTool())

    def test_get_existing(self):
        tool = self.registry.get("echo")
        assert tool.name == "echo"

    def test_get_nonexistent_raises(self):
        with pytest.raises(KeyError, match="not registered"):
            self.registry.get("nonexistent")

    def test_has(self):
        assert self.registry.has("echo")
        assert not self.registry.has("nonexistent")

    def test_list_tools_all(self):
        tools = self.registry.list_tools()
        names = [t.name for t in tools]
        assert "echo" in names
        assert "http_request" in names
        assert "file_ops" in names
        assert "code_exec" in names

    def test_list_tools_sorted_by_name(self):
        tools = self.registry.list_tools()
        names = [t.name for t in tools]
        assert names == sorted(names)

    def test_list_tools_by_category(self):
        tools = self.registry.list_tools(category="network")
        assert len(tools) == 1
        assert tools[0].name == "http_request"

    def test_list_tools_enabled_only(self):
        self.registry.disable("echo")
        tools = self.registry.list_tools(enabled_only=True)
        names = [t.name for t in tools]
        assert "echo" not in names

    def test_categories(self):
        cats = self.registry.categories()
        assert "test" in cats
        assert "network" in cats
        assert "io" in cats
        assert "compute" in cats

    def test_count(self):
        assert self.registry.count == 4


class TestToolRegistryInvoke:
    def test_invoke_echo(self):
        registry = _make_registry()
        result = _run_async(
            registry.invoke("echo", {"message": "hello"})
        )
        assert result.success
        assert result.output["echo"] == "hello"

    def test_invoke_nonexistent_raises(self):
        registry = ToolRegistry()
        with pytest.raises(KeyError, match="not registered"):
            _run_async(registry.invoke("nope", {}))

    def test_invoke_tracks_stats(self):
        registry = _make_registry()
        _run_async(registry.invoke("echo", {"message": "a"}))
        _run_async(registry.invoke("echo", {"message": "b"}))
        stats = registry.get_stats()
        assert stats["echo"]["invoke_count"] == 2
        assert stats["echo"]["error_count"] == 0

    def test_invoke_tracks_errors(self):
        registry = ToolRegistry()
        registry.register(FailingTool())
        _run_async(registry.invoke("failing", {}))
        stats = registry.get_stats()
        assert stats["failing"]["error_count"] == 1


class TestToolRegistryPermissions:
    def test_allowed_tools_permits(self):
        registry = ToolRegistry(allowed_tools={"echo"})
        registry.register(EchoTool())
        result = _run_async(
            registry.invoke("echo", {"message": "hi"})
        )
        assert result.success

    def test_allowed_tools_blocks(self):
        registry = ToolRegistry(allowed_tools={"echo"})
        registry.register(EchoTool())
        registry.register(FailingTool())
        result = _run_async(
            registry.invoke("failing", {})
        )
        assert result.status == ToolStatus.PERMISSION_DENIED

    def test_requires_permission_blocked(self):
        registry = ToolRegistry(allowed_tools=set())  # empty allowlist
        registry.register(PermissionTool())
        result = _run_async(registry.invoke("dangerous", {}))
        assert result.status == ToolStatus.PERMISSION_DENIED

    def test_requires_permission_allowed(self):
        registry = ToolRegistry(allowed_tools={"dangerous"})
        registry.register(PermissionTool())
        result = _run_async(
            registry.invoke("dangerous", {}, skip_permission_check=True)
        )
        assert result.success

    def test_skip_permission_check(self):
        registry = ToolRegistry(allowed_tools={"other"})
        registry.register(EchoTool())
        result = _run_async(
            registry.invoke("echo", {"message": "hi"}, skip_permission_check=True)
        )
        assert result.success


class TestToolRegistrySchemas:
    def test_get_all_schemas(self):
        registry = create_registry_with_builtins()
        schemas = registry.get_all_schemas()
        assert len(schemas) == 3
        names = [s["name"] for s in schemas]
        assert "http_request" in names
        assert "file_ops" in names
        assert "code_exec" in names

    def test_get_schemas_by_category(self):
        registry = create_registry_with_builtins()
        schemas = registry.get_all_schemas(category="network")
        assert len(schemas) == 1
        assert schemas[0]["name"] == "http_request"

    def test_schema_has_parameters(self):
        registry = create_registry_with_builtins()
        schemas = registry.get_all_schemas()
        for schema in schemas:
            assert "parameters" in schema
            assert "properties" in schema["parameters"]


class TestToolRegistryStats:
    def test_initial_stats(self):
        registry = _make_registry()
        stats = registry.get_stats()
        assert stats["echo"]["invoke_count"] == 0
        assert stats["echo"]["error_count"] == 0
        assert stats["echo"]["enabled"] is True
        assert stats["echo"]["category"] == "test"


# ============================================================
# Built-in tool tests: HttpRequestTool
# ============================================================


class TestHttpRequestTool:
    def test_sandbox_mode(self):
        tool = HttpRequestTool(sandbox=True)
        result = _run_async(
            tool.run({"url": "https://example.com", "method": "GET"})
        )
        assert result.success
        assert result.output["status_code"] == 200
        assert result.output["body"]["sandbox"] is True

    def test_sandbox_post(self):
        tool = HttpRequestTool(sandbox=True)
        result = _run_async(
            tool.run({
                "url": "https://api.example.com/data",
                "method": "POST",
                "body": '{"key": "value"}',
            })
        )
        assert result.success
        assert "POST" in result.output["body"]["message"]

    def test_invalid_method(self):
        tool = HttpRequestTool(sandbox=True)
        result = _run_async(
            tool.run({"url": "https://example.com", "method": "INVALID"})
        )
        assert result.status == ToolStatus.ERROR
        assert "Invalid HTTP method" in result.error

    def test_schema(self):
        tool = HttpRequestTool()
        schema = tool.get_schema_dict()
        assert schema["name"] == "http_request"
        assert schema["category"] == "network"
        assert "url" in schema["parameters"]["properties"]


# ============================================================
# Built-in tool tests: FileOpsTool
# ============================================================


class TestFileOpsTool:
    def setup_method(self):
        self.tmpdir = tempfile.mkdtemp()
        self.tool = FileOpsTool(base_dir=self.tmpdir)

    def test_write_and_read(self):
        result = _run_async(
            self.tool.run({
                "operation": "write",
                "path": "test.txt",
                "content": "Hello, Agent!",
            })
        )
        assert result.success
        assert result.output["written"] is True

        result = _run_async(
            self.tool.run({"operation": "read", "path": "test.txt"})
        )
        assert result.success
        assert result.output["content"] == "Hello, Agent!"

    def test_write_creates_dirs(self):
        result = _run_async(
            self.tool.run({
                "operation": "write",
                "path": "nested/deep/file.txt",
                "content": "nested content",
                "create_dirs": True,
            })
        )
        assert result.success

        result = _run_async(
            self.tool.run({"operation": "read", "path": "nested/deep/file.txt"})
        )
        assert result.success
        assert result.output["content"] == "nested content"

    def test_list_directory(self):
        # Create a few files
        for name in ["a.txt", "b.txt"]:
            _run_async(
                self.tool.run({
                    "operation": "write",
                    "path": name,
                    "content": f"content of {name}",
                })
            )

        result = _run_async(
            self.tool.run({"operation": "list", "path": "."})
        )
        assert result.success
        assert result.output["count"] == 2

    def test_exists(self):
        _run_async(
            self.tool.run({
                "operation": "write",
                "path": "exists.txt",
                "content": "yes",
            })
        )

        result = _run_async(
            self.tool.run({"operation": "exists", "path": "exists.txt"})
        )
        assert result.success
        assert result.output["exists"] is True
        assert result.output["is_file"] is True

        result = _run_async(
            self.tool.run({"operation": "exists", "path": "nope.txt"})
        )
        assert result.success
        assert result.output["exists"] is False

    def test_delete(self):
        _run_async(
            self.tool.run({
                "operation": "write",
                "path": "delete_me.txt",
                "content": "temporary",
            })
        )

        result = _run_async(
            self.tool.run({"operation": "delete", "path": "delete_me.txt"})
        )
        assert result.success
        assert result.output["deleted"] is True

        # Verify it's gone
        result = _run_async(
            self.tool.run({"operation": "exists", "path": "delete_me.txt"})
        )
        assert result.output["exists"] is False

    def test_read_nonexistent(self):
        result = _run_async(
            self.tool.run({"operation": "read", "path": "nonexistent.txt"})
        )
        assert result.status == ToolStatus.ERROR
        assert "not found" in result.error

    def test_delete_nonexistent(self):
        result = _run_async(
            self.tool.run({"operation": "delete", "path": "nonexistent.txt"})
        )
        assert result.status == ToolStatus.ERROR

    def test_write_without_content(self):
        result = _run_async(
            self.tool.run({"operation": "write", "path": "test.txt"})
        )
        assert result.status == ToolStatus.ERROR
        assert "content is required" in result.error

    def test_unknown_operation(self):
        result = _run_async(
            self.tool.run({"operation": "chmod", "path": "test.txt"})
        )
        assert result.status == ToolStatus.ERROR
        assert "Unknown operation" in result.error

    def test_path_traversal_blocked(self):
        result = _run_async(
            self.tool.run({"operation": "read", "path": "../../../etc/passwd"})
        )
        assert result.status == ToolStatus.PERMISSION_DENIED
        assert "traversal" in result.error.lower() or "Permission" in result.error

    def test_schema(self):
        schema = self.tool.get_schema_dict()
        assert schema["name"] == "file_ops"
        assert schema["category"] == "io"
        assert "operation" in schema["parameters"]["properties"]


# ============================================================
# Built-in tool tests: CodeExecTool
# ============================================================


class TestCodeExecTool:
    def test_simple_calculation(self):
        tool = CodeExecTool()
        result = _run_async(
            tool.run({"code": "result = 2 + 3"})
        )
        assert result.success
        assert result.output["result"] == "5"

    def test_print_output(self):
        tool = CodeExecTool()
        result = _run_async(
            tool.run({"code": "print('hello from agent')"})
        )
        assert result.success
        assert "hello from agent" in result.output["stdout"]

    def test_variables_captured(self):
        tool = CodeExecTool()
        result = _run_async(
            tool.run({"code": "x = 10\ny = 20\nresult = x + y"})
        )
        assert result.success
        assert result.output["result"] == "30"
        assert "x" in result.output["variables"]
        assert "y" in result.output["variables"]

    def test_execution_error(self):
        tool = CodeExecTool()
        result = _run_async(
            tool.run({"code": "1 / 0"})
        )
        assert result.status == ToolStatus.ERROR
        assert "ZeroDivisionError" in result.error

    def test_unsupported_language(self):
        tool = CodeExecTool()
        result = _run_async(
            tool.run({"code": "console.log('hi')", "language": "javascript"})
        )
        assert result.status == ToolStatus.ERROR
        assert "Unsupported language" in result.error

    def test_requires_permission(self):
        tool = CodeExecTool()
        assert tool.requires_permission is True

    def test_schema(self):
        tool = CodeExecTool()
        schema = tool.get_schema_dict()
        assert schema["name"] == "code_exec"
        assert schema["category"] == "compute"
        assert schema["requires_permission"] is True


# ============================================================
# Integration tests
# ============================================================


class TestIntegration:
    def test_full_workflow_with_builtins(self):
        """Test complete workflow: create registry, invoke tools, check stats."""
        with tempfile.TemporaryDirectory() as tmpdir:
            registry = create_registry_with_builtins(
                file_ops_base_dir=tmpdir,
                allowed_tools={"http_request", "file_ops", "code_exec"},
            )

            # 1. HTTP request (sandbox)
            result = _run_async(
                registry.invoke(
                    "http_request",
                    {"url": "https://example.com", "method": "GET"},
                )
            )
            assert result.success

            # 2. File ops
            result = _run_async(
                registry.invoke(
                    "file_ops",
                    {"operation": "write", "path": "data.txt", "content": "test"},
                )
            )
            assert result.success

            result = _run_async(
                registry.invoke(
                    "file_ops",
                    {"operation": "read", "path": "data.txt"},
                )
            )
            assert result.success
            assert result.output["content"] == "test"

            # 3. Code exec
            result = _run_async(
                registry.invoke(
                    "code_exec",
                    {"code": "result = 42"},
                )
            )
            assert result.success

            # 4. Stats
            stats = registry.get_stats()
            assert stats["http_request"]["invoke_count"] == 1
            assert stats["file_ops"]["invoke_count"] == 2
            assert stats["code_exec"]["invoke_count"] == 1

    def test_tool_result_as_context(self):
        """ToolResult.to_context_text() produces LLM-friendly output."""
        result = ToolResult(
            tool_name="http_request",
            status=ToolStatus.SUCCESS,
            output={"status_code": 200, "body": {"price": 42.5}},
        )
        text = result.to_context_text()
        assert "[Tool:http_request]" in text
        assert "42.5" in text

    def test_schema_export_for_llm(self):
        """All built-in tools export valid JSON schemas."""
        registry = create_registry_with_builtins()
        schemas = registry.get_all_schemas()
        for schema in schemas:
            assert "name" in schema
            assert "description" in schema
            assert "parameters" in schema
            assert "properties" in schema["parameters"]
            # Ensure JSON-serialisable
            import json

            json.dumps(schema)

    def test_custom_tool_registration(self):
        """Register a custom tool and invoke it through the registry."""
        registry = ToolRegistry()
        registry.register(EchoTool())

        result = _run_async(
            registry.invoke("echo", {"message": "custom", "uppercase": True})
        )
        assert result.success
        assert result.output["echo"] == "CUSTOM"

    def test_invoke_sync(self):
        """invoke_sync works for non-async code paths."""
        registry = _make_registry()
        result = registry.invoke_sync("echo", {"message": "sync test"})
        assert result.success
        assert result.output["echo"] == "sync test"
