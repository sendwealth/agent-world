# Agent World Plugin SDK (Python)

Python SDK for building [Agent World](https://github.com/sendwealth/agent-world) skill plugins.

## Installation

```bash
pip install agent-world-plugin-sdk
```

For the HTTP client (optional):

```bash
pip install "agent-world-plugin-sdk[client]"
```

## Quick Start

### 1. Define Your Plugin

```python
from agent_world_plugin_sdk import (
    SkillPlugin,
    PluginInfo,
    ActionContext,
    ActionResult,
    TokenCost,
    WorldContext,
)


class HelloWorldPlugin(SkillPlugin):
    """A minimal hello-world plugin."""

    @classmethod
    def init(cls, config):
        return PluginInfo(
            id="author/hello-world",
            name="Hello World",
            version="0.1.0",
            description="Greets the agent",
            author="author",
            min_engine_version="1.0.0",
            tags=["example"],
        )

    @classmethod
    def register(cls):
        return ["hello"]

    @classmethod
    def execute(cls, ctx: ActionContext) -> ActionResult:
        agent_name = ctx.world.agent.name if ctx.world.agent else "stranger"
        return ActionResult(
            success=True,
            message=f"Hello, {agent_name}! (tick #{ctx.world.tick})",
            tokens_consumed=1,
        )

    @classmethod
    def cost_estimate(cls, ctx: ActionContext) -> TokenCost:
        return TokenCost(
            estimated=1,
            confidence=1.0,
            breakdown="Fixed cost: 1 token per execution",
        )
```

### 2. Serialize / Deserialize

All data types support JSON serialization:

```python
# Serialize
info = HelloWorldPlugin.init({})
json_str = info.to_json()
data_dict = info.to_dict()

# Deserialize
restored = PluginInfo.from_json(json_str)
restored2 = PluginInfo.from_dict(data_dict)
```

### 3. Use the HTTP Client (Optional)

```python
from agent_world_plugin_sdk.client import PluginClient

client = PluginClient(base_url="http://localhost:8080")

# List registered plugins
plugins = client.list_plugins()

# Register a new plugin
client.register("author/my-plugin", config={"key": "value"})

# Execute a skill
result = client.execute(
    plugin_id="author/hello-world",
    skill_id="hello",
    agent_id="agent-42",
)
```

## Types Reference

| Type | Description |
|------|-------------|
| `PluginInfo` | Plugin metadata (id, name, version, author, etc.) |
| `AgentSnapshot` | Read-only snapshot of an agent's public state |
| `WorldContext` | Read-only world state (tick, agents, globals, events) |
| `ActionContext` | Full context for plugin execution (world + params + config) |
| `ActionResult` | Result of execution (success, message, mutations, events) |
| `StateMutation` | A state change request (kind, target, field, value) |
| `MutationKind` | Enum of mutation types (CREDIT_TOKENS, SET_SKILL, etc.) |
| `TokenCost` | Token cost estimate (estimated, confidence, breakdown) |
| `PluginError` | Exception class with error codes (init_failed, etc.) |

## SkillPlugin Methods

| Method | Required | Description |
|--------|----------|-------------|
| `init(config)` | Yes | Return plugin metadata. Called once after loading. |
| `register()` | Yes | Return list of skill IDs this plugin provides. |
| `execute(ctx)` | Yes | Execute the plugin's core logic. |
| `cost_estimate(ctx)` | Yes | Estimate token cost before execution. |
| `shutdown()` | No | Graceful cleanup (default: no-op). |
| `on_event(event, ctx)` | No | Handle a subscribed world event (default: None). |

## Development

```bash
# Install with dev dependencies
pip install -e ".[dev]"

# Run tests
pytest tests/ -v
```

## License

MIT
