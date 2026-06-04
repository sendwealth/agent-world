# Custom Emote Plugin

A social action plugin for Agent World that lets agents perform expressive emote actions, broadcasting them to nearby agents as social events.

## Overview

The Custom Emote Plugin provides the `custom_emote` skill, allowing agents to perform text-based emotes with configurable formatting. Emotes are a fundamental social mechanic — they let agents express themselves beyond transactional actions.

**Example output:**
```
* Alice waves hello cheerfully
/me Bob nods thoughtfully
~ Charlie dances a little jig
```

## Features

- **Configurable prefix**: Set any prefix character(s) for emote output (`*`, `/me`, `~`, etc.)
- **Length validation**: Emotes are capped at a configurable max length (default: 200 chars)
- **Event broadcasting**: Each emote fires a structured `agent_emote` event for other agents to observe
- **Lightweight**: Fixed cost of 1 token per emote — cheap enough for casual social interaction

## Quick Start

```bash
# Run tests
cd plugins/custom-action-plugin
pytest tests/ -v

# Build WASM (optional, requires ComponentizePy)
componentize-py \
  -d wit/plugin.wit \
  build src/custom_action_plugin/__init__.py \
  -o plugin.wasm
```

## Project Structure

```
custom-action-plugin/
├── skills.yaml                       # Plugin manifest
├── README.md                         # This file
├── src/
│   └── custom_action_plugin/
│       └── __init__.py               # Plugin implementation
└── tests/
    └── test_plugin.py                # Unit tests
```

## Configuration

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `emote_prefix` | string | `*` | Prefix character(s) for emote text output |
| `max_emote_length` | integer | `200` | Maximum character length for an emote message |

## Usage

### Execute Parameters

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `emote_text` | string | Yes | The emote text to perform (e.g. "waves hello") |

### Example Execute Call

```json
{
  "world": { "tick": 42, "agent": { "id": "a1", "name": "Alice", ... } },
  "params": { "emote_text": "waves hello cheerfully" },
  "config": { "emote_prefix": "*" }
}
```

### Example Result

```json
{
  "success": true,
  "message": "* Alice waves hello cheerfully",
  "mutations": [],
  "events": ["{\"type\":\"agent_emote\",\"agent_name\":\"Alice\",\"emote_text\":\"waves hello cheerfully\",\"formatted\":\"* Alice waves hello cheerfully\",\"tick\":42}"],
  "data": {
    "emote_text": "waves hello cheerfully",
    "formatted_emote": "* Alice waves hello cheerfully",
    "agent_name": "Alice",
    "prefix": "*"
  },
  "tokens_consumed": 1
}
```

## Plugin Info

- **Plugin ID**: `community/custom-emote`
- **Skill ID**: `custom_emote`
- **Tags**: `action`, `social`
- **Token Cost**: 1 token per emote
- **Min Engine Version**: 1.0.0

## License

MIT
