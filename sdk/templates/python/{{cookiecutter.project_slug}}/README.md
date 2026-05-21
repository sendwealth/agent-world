# {{ cookiecutter.project_name }}

{{ cookiecutter.project_description }}

## Quick Start

```bash
# Install dependencies
pip install -e ".[dev]"

# Run tests
pytest -v

# Build WASM (optional, if use_wasm=yes)
componentize-py -d wit/plugin.wit build src/{{ cookiecutter.project_slug }}/__init__.py -o plugin.wasm
```

## Project Structure

```
{{ cookiecutter.project_name }}/
├── pyproject.toml                    # Package configuration
├── skills.yaml                       # Plugin manifest
├── src/
│   └── {{ cookiecutter.project_slug }}/
│       └── __init__.py               # Plugin implementation
├── tests/
│   └── test_plugin.py               # Plugin tests
└── README.md
```

## Configuration

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `greeting` | string | `"Hello"` | The greeting word to use |
