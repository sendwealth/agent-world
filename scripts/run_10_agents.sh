#!/usr/bin/env bash
# run_10_agents.sh — E2E validation: 10 Agents × 500 ticks
#
# Validates the complete agent-world pipeline:
#   1. Agent registration & survival
#   2. Trust network formation (cooperation/betrayal)
#   3. Mentor-apprentice skill transfer
#   4. Inheritance trigger (death → asset transfer)
#   5. Knowledge marketplace (publish/buy)
#   6. Lifecycle transitions (birth → death)
#   7. Time capsule / SSE event generation
#
# Usage:
#   ./scripts/run_10_agents.sh          # Run Rust E2E tests
#   ./scripts/run_10_agents.sh --python # Run Python E2E demo
#   ./scripts/run_10_agents.sh --all    # Run both

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

log()   { echo -e "${CYAN}[INFO]${NC} $*"; }
pass()  { echo -e "${GREEN}[PASS]${NC} $*"; }
fail()  { echo -e "${RED}[FAIL]${NC} $*"; }
warn()  { echo -e "${YELLOW}[WARN]${NC} $*"; }

run_rust_tests() {
    log "Running Rust E2E validation tests..."
    cd "$PROJECT_DIR/world-engine"

    # Build first
    log "Building world-engine..."
    if ! cargo build 2>/dev/null; then
        fail "Build failed"
        exit 1
    fi
    pass "Build succeeded"

    # Run E2E validation tests
    log "Running E2E validation test (10 agents × 500 ticks)..."
    if cargo test --test e2e_validation -- --nocapture 2>&1; then
        pass "Rust E2E validation tests passed"
    else
        fail "Rust E2E validation tests failed"
        exit 1
    fi

    # Run all unit tests
    log "Running unit tests..."
    if cargo test 2>&1 | tail -5; then
        pass "All unit tests passed"
    else
        fail "Some unit tests failed"
        exit 1
    fi
}

run_python_demo() {
    log "Running Python E2E demo..."
    if command -v python3 &>/dev/null; then
        python3 "$PROJECT_DIR/scripts/e2e_demo.py" --ticks 500
        pass "Python E2E demo completed"
    else
        warn "python3 not found, skipping Python demo"
    fi
}

# ── Main ──────────────────────────────────────────────────

echo ""
echo "============================================================"
echo "  Agent World — E2E Validation: 10 Agents × 500 Ticks"
echo "============================================================"
echo ""

case "${1:-}" in
    --python)
        run_python_demo
        ;;
    --all)
        run_rust_tests
        echo ""
        run_python_demo
        ;;
    *)
        run_rust_tests
        ;;
esac

echo ""
echo "============================================================"
echo "  E2E Validation Complete"
echo "============================================================"
echo ""
