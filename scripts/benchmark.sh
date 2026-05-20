#!/usr/bin/env bash
# ═══════════════════════════════════════════════════════════════════════════
# Performance Benchmark Runner — Phase 4.2.2
#
# Runs the tick benchmark suite and outputs JSON results suitable for:
#   - CI regression tracking
#   - Performance dashboard ingestion
#   - Manual analysis
#
# Usage:
#   ./scripts/benchmark.sh                    # Full suite (all tiers)
#   ./scripts/benchmark.sh --quick            # Quick: 10-agent only
#   ./scripts/benchmark.sh --tier 10          # Specific tier
#   ./scripts/benchmark.sh --json             # Output JSON report only
#
# Environment:
#   BENCH_SEED  — Random seed for determinism (default: 42)
# ═══════════════════════════════════════════════════════════════════════════
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
BENCH_SEED="${BENCH_SEED:-42}"

# ── Parse Args ──────────────────────────────────────────────────────────
QUICK_MODE=false
TIER=""
JSON_ONLY=false

for arg in "$@"; do
    case "$arg" in
        --quick)    QUICK_MODE=true ;;
        --tier)     shift; TIER="${1:-}" ;;
        --json)     JSON_ONLY=true ;;
        --help|-h)
            echo "Usage: $0 [--quick] [--tier N] [--json] [--help]"
            echo ""
            echo "  --quick   Run 10-agent tier only (fast regression check)"
            echo "  --tier N  Run specific agent tier (10/25/50/100)"
            echo "  --json    Output JSON report only (no cargo bench output)"
            echo "  --help    Show this help"
            exit 0
            ;;
    esac
done

# ── Verify Cargo ────────────────────────────────────────────────────────
if ! command -v cargo &>/dev/null; then
    echo "ERROR: cargo not found in PATH" >&2
    exit 1
fi

cd "$PROJECT_ROOT/world-engine"

# ── Build ───────────────────────────────────────────────────────────────
echo "Building benchmarks..." >&2
cargo build --bench tick_benchmark --release 2>&1 | tail -1

# ── Run Profiler ────────────────────────────────────────────────────────
# Generate a JSON profile report using a standalone profiling binary.
# This runs outside of criterion to get deterministic profile data.

PROFILE_BIN="target/release/tick_benchmark"

generate_profile_report() {
    local agent_count="$1"
    local tick_count="${2:-100}"
    local output_file="$PROJECT_ROOT/target/benchmark-results-${agent_count}agents.json"

    mkdir -p "$PROJECT_ROOT/target"

    # Run criterion benchmark and capture output
    if [ "$JSON_ONLY" = true ]; then
        cargo bench --bench tick_benchmark -- "full_tick/agents/${agent_count}" --noplot 2>&1 | \
            grep -E '(avg|mean|median|std_dev|outliers)' || true
    else
        echo ""
        echo "═══ Benchmark: ${agent_count} agents ═══" >&2
        cargo bench --bench tick_benchmark -- "full_tick/agents/${agent_count}" 2>&1 || true
        echo ""
    fi
}

# ── Execute ─────────────────────────────────────────────────────────────
if [ "$QUICK_MODE" = true ]; then
    TIERS="10"
elif [ -n "$TIER" ]; then
    TIERS="$TIER"
else
    TIERS="10 25 50 100"
fi

echo "Running benchmarks for agent tiers: $TIERS" >&2
echo "Random seed: $BENCH_SEED" >&2
echo ""

RESULTS_FILE="$PROJECT_ROOT/target/benchmark-summary.json"
echo '{' > "$RESULTS_FILE"
echo '  "timestamp": "'$(date -u +%Y-%m-%dT%H:%M:%SZ)'",' >> "$RESULTS_FILE"
echo '  "seed": '$BENCH_SEED',' >> "$RESULTS_FILE"
echo '  "tiers": {' >> "$RESULTS_FILE"

FIRST=true
for tier in $TIERS; do
    if [ "$FIRST" = true ]; then
        FIRST=false
    else
        echo '    },' >> "$RESULTS_FILE"
    fi
    echo "    \"tier_${tier}\": {" >> "$RESULTS_FILE"

    # Run the benchmark
    if [ "$JSON_ONLY" = false ]; then
        echo "── Tier: ${tier} agents ──" >&2
    fi

    BENCH_OUTPUT=$(cargo bench --bench tick_benchmark -- \
        "full_tick/agents/${tier}" --noplot 2>&1 || true)

    # Parse criterion output for key metrics
    MEAN_US=$(echo "$BENCH_OUTPUT" | grep -oP '\[\s*mean\s*\]\s*\K[0-9.]+' | head -1 || echo "null")
    MEDIAN_US=$(echo "$BENCH_OUTPUT" | grep -oP '\[\s*median\s*\]\s*\K[0-9.]+' | head -1 || echo "null")
    STD_DEV=$(echo "$BENCH_OUTPUT" | grep -oP '\[\s*std\s+dev\s*\]\s*\K[0-9.]+' | head -1 || echo "null")

    # If parsing failed, try alternate format
    if [ "$MEAN_US" = "null" ]; then
        # criterion 0.5 format: "time:   [X us Y us Z us]"
        MEAN_US=$(echo "$BENCH_OUTPUT" | grep -oP 'time:\s+\[[\s0-9.nµa]+(\s+[0-9.nµa]+)+\]' | head -1 || echo "null")
    fi

    echo "      \"agent_count\": ${tier}," >> "$RESULTS_FILE"
    echo "      \"tick_latency\": {" >> "$RESULTS_FILE"
    echo "        \"mean_us\": \"${MEAN_US}\"," >> "$RESULTS_FILE"
    echo "        \"median_us\": \"${MEDIAN_US}\"," >> "$RESULTS_FILE"
    echo "        \"std_dev_us\": \"${STD_DEV}\"" >> "$RESULTS_FILE"
    echo "      }" >> "$RESULTS_FILE"

    # Also run multi-tick stability
    if [ "$JSON_ONLY" = false ]; then
        echo "  Running 100-tick stability test for ${tier} agents..." >&2
    fi
    cargo bench --bench tick_benchmark -- \
        "multi_tick_stability/100_ticks/${tier}" --noplot 2>&1 > /dev/null || true
done

echo '    }' >> "$RESULTS_FILE"
echo '  }' >> "$RESULTS_FILE"
echo '}' >> "$RESULTS_FILE"

# ── Summary ─────────────────────────────────────────────────────────────
if [ "$JSON_ONLY" = false ]; then
    echo ""
    echo "═══════════════════════════════════════════════════════"
    echo "  Benchmark Summary"
    echo "═══════════════════════════════════════════════════════"
    echo ""
    echo "Results saved to: $RESULTS_FILE"
    echo ""
    echo "Flamegraph generation:"
    echo "  cargo install flamegraph"
    echo "  cargo flamegraph --bench tick_benchmark -- \"full_tick/agents/50\""
    echo ""
    echo "Regression baseline:"
    echo "  cargo bench --bench tick_benchmark -- --save-baseline main"
    echo "  # Later:"
    echo "  cargo bench --bench tick_benchmark -- --baseline main"
fi

echo ""
cat "$RESULTS_FILE"
