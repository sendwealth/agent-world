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

## Testing Conventions

### Test Locations & Naming

Tests are organized by language convention. Each subsystem follows a consistent pattern:

| Subsystem | Location | File Pattern | Example |
|-----------|----------|--------------|---------|
| Rust unit tests | Inline in source files `#[cfg(test)] mod tests` | — | `world-engine/src/agent/mod.rs` |
| Rust integration tests | `world-engine/tests/` | `<domain>_<type>.rs` | `governance_api.rs`, `stress_100_agents.rs` |
| Python unit tests | `agent-runtime/tests/` | `test_<module>.py` | `test_action_executor.py`, `test_llm.py` |
| E2E tests | `tests/e2e/` | `test_<scenario>.py` | `test_full_chain.py`, `test_smoke.py` |
| Dashboard tests | `dashboard/src/__tests__/` | `<component>.test.tsx` | `human-participation.test.tsx`, `stat-card.test.tsx` |

#### Rust Tests

**Unit tests** live inside each source file using `#[cfg(test)] mod tests` — this is Rust convention. No separate test files needed.

```rust
// world-engine/src/economy/token.rs
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_burn_reduces_supply() { ... }
}
```

**Integration tests** go in `world-engine/tests/`. Use the `<domain>_<type>` naming pattern:
- API tests: `<domain>_api.rs` (e.g., `governance_api.rs`, `population_api.rs`)
- Integration tests: `<domain>_integration.rs` (e.g., `grpc_integration.rs`)
- Stress/benchmark: `stress_<N>_<scenario>.rs` or `benchmark_<N>_<scenario>.rs`
- E2E tests: `e2e_<scenario>.rs` (e.g., `e2e_full_flow.rs`)
- Phase-specific: `phase_<N>_<description>.rs` (e.g., `phase_442_e2e_integration.rs`)

#### Python Tests

All Python tests live in `agent-runtime/tests/` with the `test_<module>.py` naming convention. Integration tests may append `_integration`:
- Unit: `test_decide.py`, `test_memory_aware_decide.py`
- Integration: `test_pool_integration.py`, `test_social_engine_integration.py`

E2E tests are in `tests/e2e/` with the same `test_<scenario>.py` pattern but separate from unit tests.

#### Dashboard Tests

Dashboard tests use **Vitest** + **React Testing Library** with jsdom environment.

- Test files go in `dashboard/src/__tests__/`
- Naming: `<component-or-feature>.test.tsx`
- Setup: `dashboard/src/__tests__/setup.ts` (imports `@testing-library/jest-dom/vitest`)
- Config: `dashboard/vitest.config.ts`

```tsx
// dashboard/src/__tests__/stat-card.test.tsx
import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { StatCard } from "@/components/StatCard";

describe("StatCard", () => {
  it("renders title and value", () => {
    render(<StatCard title="Agent 总数" value={42} icon={<span>📊</span>} color="blue" />);
    expect(screen.getByText("Agent 总数")).toBeInTheDocument();
    expect(screen.getByText("42")).toBeInTheDocument();
  });
});
```

### Running Tests

```bash
# All tests (Rust + Python)
make test

# Rust only (unit + integration)
cd world-engine && cargo test

# Python only
cd agent-runtime && pytest

# Dashboard only
cd dashboard && npm test          # vitest run
cd dashboard && npm run test:watch # vitest watch mode

# E2E only
cd tests/e2e && pytest
```

### Writing New Tests

When adding a new test, follow these rules:

1. **Place it in the correct location** — see the table above
2. **Follow the naming pattern** — `<domain>_<type>.rs` for Rust, `test_<module>.py` for Python, `<component>.test.tsx` for Dashboard
3. **Mirror the source structure** — Python test file names should mirror the module they test
4. **One test file per module/component** — keep related tests together
5. **Mock external dependencies** — Dashboard tests should mock `next/navigation`, fetch, and SSE providers

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
