# Economy Data Analysis Plugin

A WASM analytics plugin for Agent World that collects and analyzes agent economy data, computing key economic indicators across the agent population.

## Overview

The Data Analysis Plugin provides the `economy_analysis` skill, delivering real-time economic insights about the agent world:

- **GDP** — Total wealth in the system (sum of all agent money)
- **Average Wealth** — Mean money across all visible agents
- **Median Wealth** — Middle value of the wealth distribution
- **Gini Coefficient** — Standard measure of wealth inequality (0 = perfect equality, 1 = max inequality)
- **Wealth Extremes** — Wealthiest and poorest agents identified

## Quick Start

```bash
# Build the WASM module
cd plugins/data-analysis-plugin
cargo build --target wasm32-unknown-unknown --release

# Run tests (native)
cargo test

# Package for deployment
plugin-pack ./target/wasm32-unknown-unknown/release/data_analysis_plugin.wasm \
  --manifest skills.yaml -o data-analysis-plugin.zip
```

## Project Structure

```
data-analysis-plugin/
├── Cargo.toml           # Package config (targets wasm32-unknown-unknown)
├── skills.yaml          # Plugin manifest
├── src/
│   └── lib.rs           # Plugin implementation + WASM exports + tests
└── README.md            # This file
```

## Configuration

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `include_transaction_log` | boolean | `false` | Include per-agent transaction breakdown in results |
| `gini_precision` | integer | `4` | Decimal places for Gini coefficient output (0–10) |

## Usage

### Execute Parameters

No required parameters — the plugin automatically analyzes all visible agents.

### Example Execute Call

```json
{
  "world": {
    "tick": 100,
    "agent": { "id": "a1", "name": "Alice", "money": 1000, ... },
    "visible_agents": [
      { "id": "a2", "name": "Bob", "money": 500, ... },
      { "id": "a3", "name": "Charlie", "money": 200, ... }
    ]
  },
  "params": {},
  "config": { "gini_precision": "4" }
}
```

### Example Result

```json
{
  "success": true,
  "message": "Economy Analysis (tick #100, 3 agents): GDP=1700, Avg Wealth=566.67, Median=500.00, Gini=0.3490",
  "mutations": [],
  "events": ["{\"type\":\"economy_analysis\",\"tick\":100,\"agent_count\":3,\"gdp\":\"1700\",\"gini\":\"0.3490\"}"],
  "data": {
    "gdp": "1700",
    "agent_count": "3",
    "avg_wealth": "566.67",
    "gini_coefficient": "0.3490",
    "wealthiest_agent": "Alice",
    "poorest_agent": "Charlie",
    "max_wealth": "1000",
    "min_wealth": "200",
    "median_wealth": "500.00"
  },
  "tokens_consumed": 5
}
```

## Gini Coefficient

The Gini coefficient is calculated using the standard formula:

```
G = (2 × Σ(i × xᵢ)) / (n × Σ(xᵢ)) - (n + 1) / n
```

where values are sorted in ascending order and i is 1-indexed.

| Gini Range | Interpretation |
|------------|---------------|
| 0.0 – 0.2 | Very equal wealth distribution |
| 0.2 – 0.3 | Relatively equal |
| 0.3 – 0.4 | Moderate inequality |
| 0.4 – 0.6 | High inequality |
| 0.6 – 1.0 | Extreme inequality |

## WASM Exports

| Export | Signature | Description |
|--------|-----------|-------------|
| `init` | `(ptr, len) → usize` | Config JSON in, `PluginInfo` JSON out |
| `register` | `() → usize` | Returns `["economy_analysis"]` JSON |
| `execute` | `(ptr, len) → usize` | `ActionContext` JSON in, `ActionResult` JSON out |
| `cost_estimate` | `(ptr, len) → usize` | `ActionContext` JSON in, `TokenCost` JSON out |
| `shutdown` | `() → ()` | Graceful teardown |

## Plugin Info

- **Plugin ID**: `community/data-analysis`
- **Skill ID**: `economy_analysis`
- **Tags**: `analytics`, `economy`
- **Token Cost**: 5 tokens per analysis
- **Confidence**: 0.95 (slightly variable due to agent count)
- **Min Engine Version**: 1.0.0

## Building

```bash
# Ensure you have the WASM target
rustup target add wasm32-unknown-unknown

# Build release WASM
cargo build --target wasm32-unknown-unknown --release

# Output: target/wasm32-unknown-unknown/release/data_analysis_plugin.wasm
```

## License

MIT
