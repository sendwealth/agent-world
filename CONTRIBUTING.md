# Contributing to Agent World

First off, thank you for considering contributing to Agent World!

## Code of Conduct

This project adheres to the [Contributor Covenant Code of Conduct](CODE_OF_CONDUCT.md). By participating, you are expected to uphold this code.

## How Can I Contribute?

### Reporting Bugs

- Check [existing issues](../../issues) before opening a new one
- Use the **Bug Report** template
- Include: OS, Rust/Python version, steps to reproduce, expected vs actual behavior

### Suggesting Features

- Open a [Discussion](../../discussions) first for major features
- Use the **Feature Request** template for small, well-defined features
- Explain the use case and how it fits the project's vision

### Pull Requests

1. **Fork** the repository
2. Create a **feature branch**: `git checkout -b feat/your-feature`
3. **Commit** with conventional commits:
   - `feat:` new feature
   - `fix:` bug fix
   - `docs:` documentation
   - `refactor:` code refactoring
   - `test:` tests
   - `chore:` build/tooling
4. **Push** and open a PR against `main`
5. Ensure CI passes (lint + test)
6. Request review

### Branch Naming

| Type | Format | Example |
|------|--------|---------|
| Feature | `feat/description` | `feat/a2a-protocol` |
| Bug fix | `fix/description` | `fix/token-ledger-race` |
| Docs | `docs/description` | `docs/architecture-guide` |
| Experiment | `exp/description` | `exp/reinforcement-learning` |

## Development Setup

### Prerequisites

- **Rust** 1.80+ (`rustup`)
- **Python** 3.11+ (`uv` recommended)
- **Node.js** 20+ (for dashboard)
- **protoc** 3.20+ (Protocol Buffers compiler)

### Quick Setup

```bash
# Clone
git clone https://github.com/sendwealth/agent-world.git
cd agent-world

# Setup development environment
make setup
# or: ./scripts/setup.sh

# Run tests
make test

# Run linters
make lint

# Start development
make dev
```

### Using Makefile

```bash
make help        # Show all available commands
make setup       # Install dependencies
make dev         # Show instructions for starting dev environment
make test        # Run Rust and Python tests
make lint        # Run linters (cargo clippy + ruff)
make fmt         # Format code (rustfmt + ruff format)
make proto       # Generate protobuf code
make clean       # Clean build artifacts
make build       # Build world-engine (Rust, release mode)
```

## Coding Standards

### Rust (world-engine)
- Follow `rustfmt` defaults
- Run `cargo clippy` with no warnings
- Document all public APIs with `///` doc comments
- Write tests for every module (unit tests in `#[cfg(test)] mod tests` within each file)

### Python (agent-runtime)
- Follow PEP 8 (enforced by `ruff`)
- Type hints required for all function signatures
- Docstrings for all public functions (Google style)
- Tests mirroring source structure

### Protocol Buffers (protocol/)
- Use `proto3` syntax
- Comment every message and field
- Version messages with `option go_package`

### YAML configs (config/)
- 2-space indentation
- Comment every key
- Validate with schema before committing

### Dashboard (Next.js)
- TypeScript strict mode
- Follow existing component patterns (see `src/components/`)
- Use Tailwind CSS utility classes

## Architecture Decision Records (ADRs)

When making significant technical decisions:

1. Copy `docs/adr/template.md` to `docs/adr/NNN-title.md`
2. Fill in: Context, Decision, Consequences
3. Commit with the feature it supports

ADRs are immutable once merged -- update via new ADR that supersedes.

## Testing

### Unit Tests
```bash
# Rust (world-engine)
cd world-engine && cargo test

# Python (agent-runtime)
cd agent-runtime && pytest

# Dashboard
cd dashboard && npm run build
```

### All Tests
```bash
make test        # Runs both Rust and Python tests
```

## Commit Messages

Follow [Conventional Commits](https://www.conventionalcommits.org/):

```
feat(economy): add token burn engine with phase multipliers
fix(escrow): resolve double-refund on concurrent claims
docs(roadmap): update Phase 1 milestone completion status
refactor(events): extract event serialization into separate module
test(economy): add property-based tests for reward distribution
chore(deps): bump tokio to 1.40
```

## Questions?

- [GitHub Discussions](../../discussions) -- general questions, ideas
- [GitHub Issues](../../issues) -- bugs, feature requests

---

Thank you for helping build Agent World!
