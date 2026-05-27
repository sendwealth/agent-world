# {{project-name}}

{{project-description}}

## Quick Start

```bash
# Build the plugin
cargo build --target wasm32-unknown-unknown --release

# Run tests
cargo test

# Pack for distribution
plugin-pack ./target/wasm32-unknown-unknown/release/{{project-name}}.wasm
```

## Project Structure

```
{{project-name}}/
├── Cargo.toml          # Package configuration
├── skills.yaml         # Plugin manifest (metadata, skills, config schema)
├── src/
│   └── lib.rs          # Plugin implementation
└── README.md           # This file
```

## Configuration

The plugin accepts these configuration options:

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `greeting` | string | `"Hello"` | The greeting word to use |

## Plugin Interface

This plugin implements the `SkillPlugin` trait with these methods:

- `init(config)` → Returns `PluginInfo` with metadata
- `register()` → Returns list of provided skill IDs
- `execute(ctx)` → Main plugin logic
- `cost_estimate(ctx)` → Token cost estimation
- `shutdown()` → Cleanup

See `src/lib.rs` for the full implementation.
