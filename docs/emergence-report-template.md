# Emergence Observation Report — Template

This template is auto-populated by `scripts/emergence_experiment.py` after each
experiment run. Sections 1–3 are generated automatically; sections 4–6 require
manual analysis and should be completed by the researcher.

---

## 1. Experiment Configuration

| Parameter | Value |
|---|---|
| Experiment ID | `experiment-YYYYMMDD-HHMMSS` |
| Date | ISO 8601 timestamp |
| Agent count | N |
| Duration | X minutes |
| LLM model | e.g. `llama3` |
| LLM provider | `ollama` / `openai` / `anthropic` / `zhipu` |
| Ollama parallelism | `OLLAMA_NUM_PARALLEL` |
| Tick interval | seconds between ticks |
| Genesis config | link to `config/genesis.yaml` |

---

## 2. Running Statistics

| Metric | Value |
|---|---|
| Total ticks (all agents) | N |
| Agents alive at end | N / N |
| Survival rate | X% |
| Error rate | X% |
| LLM calls (estimated) | N |
| Avg ticks per agent | N |
| Wall time | Xs |

### Per-Agent Summary

| Agent | Container | Healthy | Ticks | Uptime | Errors | LLM Calls |
|---|---|---|---|---|---|---|
| Alice | agent-alice | Yes/No | N | Xs | N | N |
| Bob | agent-bob | Yes/No | N | Xs | N | N |
| ... | ... | ... | ... | ... | ... | ... |

---

## 3. Economy Summary

Token economy statistics collected from the world engine.

| Metric | Value |
|---|---|
| Total tokens burned | N |
| Total tasks created | N |
| Total tasks completed | N |
| Total trades | N |
| Platform fees collected | N |
| Money supply | N |

> This section is populated from World Engine API data when available.

---

## 4. Emergence Behavior Observations

> **Manual analysis section** — complete after reviewing dashboard data,
> interaction logs, and agent behavior patterns.

### 4.1 Clustering & Social Structure

- Did agents form spatial or social clusters?
- Were there leader-follower dynamics?
- Did agents specialize in specific roles (traders, teachers, builders)?

**Observations**:

(Describe observed patterns here)

### 4.2 Cooperation Patterns

- Reciprocal trading relationships
- Teaching / mentorship events
- Organization formation (companies, guilds, alliances)
- Collective problem-solving

**Observations**:

(Describe observed patterns here)

### 4.3 Emergence Events

Document any unexpected behaviors not explicitly programmed:

- [ ] Novel communication strategies
- [ ] Economic phenomena (bubbles, crashes, equilibria)
- [ ] Cultural transmission or norm formation
- [ ] Trust network formation or betrayal events
- [ ] Skill specialization or division of labor
- [ ] Self-organized governance or voting patterns
- [ ] Emergent hierarchies or social classes

**Events log**:

| Tick range | Event type | Description | Agents involved |
|---|---|---|---|
| N–M | e.g. trade_cluster | Description | Alice, Bob, Carol |
| ... | ... | ... | ... |

### 4.4 Interaction Graph Analysis

- Attach screenshots from the dashboard
- Describe the communication topology:
  - Centralization index
  - Clustering coefficient
  - Number of connected components
  - Diameter of the interaction network

**Graph observations**:

(Describe here)

---

## 5. Verdict

**Decision**: GO / NO-GO / NEED-MORE-DATA

### Decision Criteria

| Criterion | Threshold | Actual | Pass? |
|---|---|---|---|
| Survival rate | >= 80% | X% | Yes/No |
| Total ticks | > 1000 | N | Yes/No |
| Error rate | < 5% | X% | Yes/No |
| Emergence events observed | >= 1 | N | Yes/No |

### Reasoning

(Explain the verdict — why the data is or isn't sufficient for emergence analysis)

### Verdict Definitions

- **GO**: System is stable, data quality is sufficient, emergence behaviors are observable. Proceed to deeper analysis and Phase 4 design.
- **NO-GO**: System is not stable enough (low survival, high errors). Fix infrastructure issues before re-running.
- **NEED-MORE-DATA**: System is functioning but the experiment didn't run long enough or produce clear emergence patterns. Extend duration or adjust parameters.

---

## 6. Next Steps

Based on the verdict:

### If GO
- [ ] Deep-dive analysis of emergence events
- [ ] Extract and classify agent communication patterns
- [ ] Map economic network topology
- [ ] Design Phase 4 emergence detection metrics
- [ ] Run follow-up experiments with varied parameters

### If NO-GO
- [ ] Investigate root cause of low survival / high errors
- [ ] Check LLM performance (latency, throughput, model quality)
- [ ] Adjust token economy parameters in `config/genesis.yaml`
- [ ] Consider shorter tick intervals or more initial tokens
- [ ] Review Ollama resource requirements (RAM, GPU)

### If NEED-MORE-DATA
- [ ] Extend experiment duration (2x, 4x)
- [ ] Run multiple experiments and aggregate results
- [ ] Vary agent count (5, 20, 50)
- [ ] Try different LLM models
- [ ] Monitor for patterns that emerge over longer time scales

---

## Appendix

### A. Raw Data Locations

- JSON metrics: `./logs/experiment-YYYYMMDD-HHMMSS/metrics.json`
- Docker logs: `./logs/experiment-YYYYMMDD-HHMMSS/docker-logs/`
- Dashboard screenshots: (attach manually)

### B. Reproduction

To reproduce this experiment:

```bash
python scripts/emergence_experiment.py \
  --agents N \
  --duration Xm \
  --model MODEL_NAME \
  --ollama-parallel N
```

### C. Environment

- Docker version:
- Ollama version:
- Host OS:
- Host RAM:
- GPU (if applicable):

---

*Template version: 1.0 — Generated by `scripts/emergence_experiment.py`*
