# Agent World Plugin SDK

Tools and templates for developing skill plugins for the Agent World simulation engine.

## Structure

```
sdk/
├── docs/
│   └── (see /docs/plugin-interface-spec.md)
├── templates/
│   ├── rust/                    # Rust cargo-generate template
│   │   ├── Cargo.toml
│   │   ├── skills.yaml
│   │   ├── src/lib.rs           # Hello World plugin implementation
│   │   └── README.md
│   └── python/                  # Python cookiecutter template
│       ├── cookiecutter.json
│       └── {{cookiecutter.project_slug}}/
│           ├── pyproject.toml
│           ├── skills.yaml
│           ├── src/{{cookiecutter.project_slug}}/__init__.py
│           ├── tests/test_plugin.py
│           └── README.md
├── plugin-test-runner/          # Local test framework
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs
│       ├── mock.rs              # Mock WorldContext & ActionContext builders
│       ├── runner.rs            # Test execution engine
│       └── report.rs            # Test report formatting
├── plugin-pack/                 # Pack CLI: compile → WASM → .awp bundle
│   ├── Cargo.toml
│   └── src/main.rs
└── plugin-publish/              # Publish CLI: upload to marketplace
    ├── Cargo.toml
    └── src/main.rs
```

## Quick Start

### 1. Create a Plugin (Rust)

```bash
# Install cargo-generate if you haven't
cargo install cargo-generate

# Generate from template
cd sdk/templates/rust
cargo generate --path . --name my-plugin

# Build and test
cd my-plugin
cargo test
cargo build --target wasm32-unknown-unknown --release
```

### 2. Create a Plugin (Python)

```bash
# Install cookiecutter
pip install cookiecutter

# Generate from template
cd sdk/templates/python
cookiecutter .

# Test
cd my-plugin
pip install -e ".[dev]"
pytest -v
```

### 3. Test Your Plugin

```bash
# Build the test runner
cd sdk/plugin-test-runner
cargo build --release

# Run tests against a compiled WASM plugin
../target/release/plugin-test-runner path/to/plugin.wasm --verbose
```

### 4. Pack Your Plugin

```bash
# Build the pack tool
cd sdk/plugin-pack
cargo build --release

# Pack a compiled plugin into a .awp bundle
../target/release/plugin-pack path/to/plugin.wasm --manifest skills.yaml
```

### 5. Publish

```bash
# Build the publish tool
cd sdk/plugin-publish
cargo build --release

# Publish (requires API key)
../target/release/plugin-publish my_plugin.awp --api-key YOUR_KEY
```

## Plugin Interface

See the full specification: [docs/plugin-interface-spec.md](../../docs/plugin-interface-spec.md)

### Core Trait Methods

| Method | Called When | Returns |
|--------|-----------|---------|
| `init(config)` | Plugin loaded | `PluginInfo` — metadata |
| `register()` | After init | `Vec<SkillId>` — provided skills |
| `execute(ctx)` | Each tick / event | `ActionResult` — output + mutations |
| `cost_estimate(ctx)` | Before execute | `TokenCost` — budget check |
| `shutdown()` | Engine stopping | `()` — cleanup |
| `on_event(event, ctx)` | Subscribed event | `Option<ActionResult>` |

### Plugin Manifest (skills.yaml)

Every plugin ships with a `skills.yaml` that declares:

- Plugin ID, name, version, author
- Skills provided
- Required skill dependencies
- Configuration schema (JSON Schema)
- Event subscriptions
- Resource requirements

## WASM Target

Plugins compile to `wasm32-unknown-unknown` and run in a wasmtime sandbox:

- No filesystem or network access
- Memory limit: 64 MB (configurable)
- Execution timeout: 30s (configurable)
- JSON-based ABI for all data crossing the WASM boundary
