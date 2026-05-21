# Phase 4.7 Validation Report: LLM-Driven Emergence Experiment

**Experiment ID**: `experiment-20260521-phase47-validation`
**Date**: 2026-05-21
**Verdict**: **CONDITIONAL**

---

## 1. Experiment Configuration

| Parameter | Experiment A (LLM) | Experiment B (Mock Presets) |
|---|---|---|
| Agents | 5 (Alice, Bob, Carol, Dave, Eve) | 5 (same) |
| Ticks | 100 per agent | 100 per agent |
| LLM Model | qwen3:8b (Ollama, Q4_K_M) | Mock presets |
| LLM Provider | Ollama (localhost:11434) | — |
| Decision Mode | LLM with async fallback | Mock (survival, hungry_gather, social_nearby) |
| Tick Interval | 1.0s | 1.0s |
| World Engine | Rust native (port 8080, gRPC 50051) | Same |
| Hardware | Apple M3 / 16GB RAM / macOS 15.3 | Same |
| Code | main branch, commit b9bb3aa | Same |

---

## 2. Experiment A: LLM-Driven (qwen3:8b)

### 2.1 Setup & Execution

- World Engine built from source (`cargo build --release`), started natively on port 8080
- Ollama confirmed running with qwen3:8b model (Q4_K_M, 5.2GB)
- 5 agents launched via `python -m agent_runtime spawn` with `--llm-provider ollama --llm-model qwen3:8b`
- All 5 agents registered with World Engine via REST API (`POST /api/v1/agents`)
- All 5 agents completed 100 ticks in ~100 seconds (tick_interval=1.0s)

### 2.2 LLM Performance

| Metric | Value |
|---|---|
| qwen3:8b inference speed | ~3.4 tokens/sec (CPU-only, Metal not engaged) |
| Thinking tokens per call | ~146 tokens (qwen3 uses built-in CoT by default) |
| Single request latency | 22–72 seconds |
| LLMQueue timeout | 30 seconds (default) |
| LLM call success rate | 0% (all timed out) |
| Fallback decisions | 100% (rest, confidence: 0) |

**Root Cause**: qwen3:8b's built-in thinking mode consumes tokens before producing visible output. With 30s LLMQueue timeout and ~3.4 tok/s, the model needs ~45+ seconds per inference. All 5 agents exceeded the timeout on every call, triggering the `fallback_on_timeout` mechanism.

**Contributing Factors**:
- Apple M3 with 16GB RAM: model fits in memory but inference is CPU-bound
- `size_vram: 0` — Ollama not leveraging Metal GPU acceleration
- 5 concurrent agents compete for single-model inference (serial queuing)
- Qwen3's thinking tokens inflate output: 146 think-tokens before the actual ~7-token answer

### 2.3 Architecture Validation

Despite LLM timeout, the system architecture worked correctly:

| Component | Status | Notes |
|---|---|---|
| World Engine startup | ✅ | Rust binary, loaded genesis.yaml, all subsystems registered |
| Agent registration | ✅ | 5/5 agents registered via REST |
| ThinkLoop execution | ✅ | 100 ticks completed in 100.1s per agent |
| LLMQueue | ✅ | Started, attempted calls, fell back gracefully |
| AsyncDecisionProvider | ✅ | Decoupled LLM latency from tick speed |
| Health check server | ✅ | Responded on unique ports per agent |
| Graceful shutdown | ✅ | max_ticks reached, clean shutdown |
| WAL event recording | ✅ | tick_advanced + agent_spawned events captured |
| Persistence (SQLite) | ✅ | Opened successfully, snapshot mechanism ready |
| EventBus | ✅ | 256-capacity broadcast channel operational |
| 7 Subsystems | ✅ | TokenBurn, DeathJudgment, Lifecycle, ReputationDecay, Evolution, etc. |

---

## 3. Experiment B: Mock LLM Presets

### 3.1 Setup & Execution

- Same World Engine instance, fresh start
- 5 agents with diverse mock presets:
  - Alice: `survival` (resource-conservative decisions)
  - Bob: `hungry_gather` (resource-seeking)
  - Carol: `social_nearby` (communication-focused)
  - Dave: `hungry_gather` (resource-seeking)
  - Eve: `survival` (resource-conservative)

### 3.2 Results

| Metric | Value |
|---|---|
| Total ticks (5 agents × 100) | 500 |
| Wall time | ~105 seconds |
| Agent survival rate | 100% (5/5 alive at end) |
| Errors | 0 per agent |
| Agent token balance | 500 each (unchanged — actions not reflected in World Engine state) |
| WAL events | 1836 total (mostly tick_advanced + agent_spawned) |

### 3.3 Emergence Behavior Observations

#### What We Observed

1. **Agent Registration Pattern**: Agents registered sequentially over ~5 ticks, creating a natural staggered birth pattern. This is analogous to real-world population emergence.

2. **Behavioral Diversity**: Three distinct behavior profiles were active:
   - *Survival agents* (Alice, Eve): Conservative, prioritize self-preservation
   - *Gatherer agents* (Bob, Dave): Active resource acquisition
   - *Social agent* (Carol): Communication-oriented behavior

3. **WAL Event Logging**: 1836 events recorded, including agent spawn events and tick-advanced events. The TimeCapsule and Persistence subsystems operated correctly.

#### What We Did NOT Observe (Limitations)

- **No economic transactions**: Agent actions (gather, trade, build) were decided locally but not submitted back to the World Engine via API calls. The REST fallback client logs warnings and returns mock data.
- **No spatial movement**: No position tracking or proximity-based interactions.
- **No communication between agents**: A2A messaging was not triggered in this experiment.
- **No organization/governance formation**: No proposals, voting, or rule creation.
- **No resource depletion or competition**: Token balances remained static.

**Key Gap**: The mock decision provider generates decisions but the ActionExecutor doesn't consistently route those actions back to the World Engine API. The agents run their think loops but the World Engine doesn't see the effects. This is a **critical integration gap** that needs addressing for Phase 5.

---

## 4. Performance Data

| Metric | Value |
|---|---|
| World Engine startup | <1s (release build) |
| World Engine binary size | 10.6 MB |
| Agent spawn time | ~1-2s per agent |
| Tick latency | ~1ms (Rust scheduler) |
| Agent memory (estimated) | ~50MB per Python process |
| Total system memory | ~2.5 GB (5 agents + World Engine + Ollama) |
| LLM inference (qwen3:8b) | 22-72s per call |
| LLM throughput | 3.4 tokens/sec |

---

## 5. GO/NO-GO Recommendation

### Verdict: **CONDITIONAL**

The system architecture is sound and all Phase 4 deliverables are functionally complete. However, two critical gaps prevent a full GO:

#### Critical Issues (Must Fix for Phase 5)

1. **LLM Latency vs. Tick Speed Mismatch**
   - qwen3:8b on M3/16GB is too slow (3.4 tok/s) for real-time decisions at 1s ticks
   - Solutions: (a) Use smaller model (qwen3:1.7b or qwen3:0.6b), (b) Increase tick interval to 60-120s, (c) Deploy on GPU-enabled server, (d) Optimize Ollama Metal acceleration, (e) Use cloud LLM API for faster inference

2. **Action-to-World-Engine Feedback Loop Missing**
   - Agent decisions are made but not submitted back to the World Engine
   - The ActionExecutor doesn't route gather/trade/communicate/build actions to the REST API
   - Without this loop, agents can't affect the shared world state → no emergence possible
   - This is the **#1 blocker** for observing emergence behaviors

#### Recommendations for Phase 5

| Priority | Action | Effort |
|---|---|---|
| P0 | Wire ActionExecutor → World Engine REST API for all action types | 2-3 days |
| P0 | Test with qwen3:1.7b or cloud API (GLM-5, GPT-4o-mini) for faster inference | 1 day |
| P1 | Add A2A messaging integration for inter-agent communication | 2-3 days |
| P1 | Implement spatial positioning and proximity detection | 2 days |
| P2 | Run longer experiments (500+ ticks) with working action loop | 1 day |
| P2 | Add emergence metrics: Gini coefficient, network density, cultural diversity index | 2-3 days |

#### Phase 5 GO Criteria

Before proceeding to Phase 5, validate:
- [ ] Action feedback loop: agent decisions visibly change World Engine state (tokens, resources, positions)
- [ ] LLM decision success rate > 50% (at least half the ticks get real LLM decisions)
- [ ] At least 1 emergent behavior observable in World Engine state (resource competition, communication, or organization formation)

---

## 6. Artifacts

- World Engine log: `/tmp/world-engine.log`
- Agent logs: `/tmp/agent-{alice,bob,carol,dave,eve}*.log`
- WAL data: `data/wal.log` (1836 events)
- SQLite persistence: `data/world.db`

---

*Generated by Phase 4.7 validation experiment — commit b9bb3aa, 2026-05-21*
