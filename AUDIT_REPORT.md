# Agent Container Audit Report
Generated: 2026-06-13 (logs span ~07:58:11 → 08:10:37, ~12 min, 768–794 ticks/agent)

## Executive Summary
- **10/10 containers alive and ticking** (768–794 ticks, ~1 tick/sec). No ERROR / CRITICAL / Traceback in any log.
- **0 real LLM errors** — the "429" string matches were tick numbers (`tick=429`) and timestamp digits, NOT rate-limits. LLMQueue healthy (`max_concurrency=2, timeout=120s`).
- **All agents `phase=adult`** from tick 1. No `phase=birth`, no stale `tokens=500` defaults (all start ~99,999–100,005).
- **All tokens moving** (not stuck): explore-heavy agents (alice/bob) spend tokens (−100ish); rest-heavy agents gain (+200–430).
- **3 systemic defects** found — none fatal, all degrading agent *behavior quality* (social context + 2 action-arg bugs).

---

## Per-Agent Snapshot (last 200 lines)

| agent | agent_id (prefix) | last tick | token Δ | decide mix | teach_skill fail | claim_task fail | social fail |
|-------|-------------------|-----------|---------|------------|------------------|-----------------|-------------|
| alice | f134196d | 794 | −117 | 100% explore | 0 | 0 | 35 |
| bob   | 57e3eb64 | 794 | −102 | 100% explore | 0 | 0 | 43 |
| carol | b17c4752 | 769 | +413 | 35 rest / 3 practice / 1 explore | 7 | 0 | 40 |
| dave  | d90369ea | 781 | +390 | 33 rest / 5 practice | 9 | 0 | 38 |
| eve   | 27b62b72 | 774 | +254 | 22 rest / 5 practice / 5 explore | 13 | 0 | 32 |
| frank | e1d1b1da | 768 | +215 | 21 rest / 7 practice / 3 trade | 16 | 0 | 31 |
| grace | cd555747 | 768 | +238 | 26 rest / 4 explore / 3 practice / 2 trade | 8 | 2 | 36 |
| henry | 863e6476 | 785 | +238 | 30 rest / 4 trade / 3 practice / 3 explore | 6 | 0 | 40 |
| iris  | 7f55db0e | 780 | +429 | 38 rest / 3 practice | 6 | 0 | 41 |
| jack  | 1395f8f4 | 775 | +319 | 29 rest / 5 explore / 4 practice | 8 | 0 | 38 |

---

## Findings by Severity

### 🟠 P1-1 — `SocialEngine.build_context failed` on EVERY decision tick
- **Impact:** Every agent's social context is silently dropped, so the `DecisionEngine` decides *without social/relationship information* on every single tick.
- **Affected:** ALL 10 agents (alice, bob, carol, dave, eve, frank, grace, henry, iris, jack). 31–43 occurrences per 200 log lines each.
- **Error:** `"error": "'agent_id'"` → a `KeyError('agent_id')` raised inside `SocialEngine.build_context`.
- **Severity:** P1 (non-fatal but degrades ALL agent decision quality — agents cannot reason about other agents).
- **Root cause (likely):** `SocialEngine.build_context` looks up an `agent_id` key that isn't being passed in the context dict — either the caller omits it, or the SocialEngine expects a different key name. Affects 100% of agents ⇒ a code/config bug, not per-agent state.

### 🟠 P1-2 — `teach_skill` action repeatedly fails (missing `target_agent_id` + `skill_name`)
- **Impact:** When an agent decides `practice_skill`, the executor dispatches `teach_skill`, which fails because required params aren't supplied. 3 retries each → `retry_exhausted`. Tokens wasted on no-ops.
- **Affected:** 8/10 agents — carol, dave, eve, frank, grace, henry, iris, jack. (alice & bob never pick teach/practice, so unaffected.) 6–16 failures per 200 lines each.
- **Error:** `teach_skill requires 'target_agent_id' and 'skill_name'`.
- **Severity:** P1 (recurring on 80% of agents; begins at tick 3–4 of each affected agent ⇒ systemic).
- **Root cause (likely):** decide→act wiring bug — the `practice_skill` decision is routed to a `teach_skill` action handler, but the action-argument builder never injects `target_agent_id`/`skill_name`. For self-practice these params shouldn't be required (or practice_skill needs its own action).

### 🟡 P2-1 — `claim_task` action fails (missing `task_id`) — grace only
- **Impact:** grace's `claim_task` decisions fail with `claim_task requires 'task_id' parameter` (2 ticks, tick 695/696).
- **Affected:** grace only.
- **Severity:** P2 (same class of param-missing bug as P1-2, low frequency).
- **Root cause (likely):** action-argument builder not injecting `task_id` when the agent decides `claim_task`.

### 🟡 P2-2 — Low / non-diverse action mix
- **Impact:** Agents are heavily skewed to `rest` + `explore`. Almost no `trade`, `gather`, `move`, `teach` actually *succeed*. alice & bob are **100% explore**; carol & iris are **>90% rest**.
- **Affected:** all, most pronounced on alice, bob, carol, iris.
- **Severity:** P2 (world feels inactive; compounded by P1-1 — with social context broken, agents can't discover partners to trade/teach with).
- **Root cause:** upstream of the action-arg bugs + loss of social context; agents default to low-cost rest/explore.

### 🔵 P3-1 — No registration log visible from agent side
- **Impact:** Cannot confirm from agent logs that agents registered with world-engine. No `registered` / `standalone` / `world-engine` markers. First INFO line is `Injected SocialContextProvider into DecisionEngine`, then ticks begin immediately.
- **Affected:** all 10.
- **Severity:** P3 (likely benign — registration may be silent or handled outside the agent process; agents ARE ticking & mutating token balances, implying some state store is live). Verify on the world-engine side.
- **Root cause (possible):** registration log line missing, or registration genuinely happens out-of-band. Not necessarily a defect.

---

## What is HEALTHY (no action needed)
- ✅ All 10 containers running; consistent ~1 tick/sec; 768–794 ticks logged.
- ✅ Zero ERROR/CRITICAL/Traceback across all logs.
- ✅ No real LLM 429 / timeout / parse errors (LLMQueue nominal).
- ✅ All `phase=adult` (no stuck `birth`).
- ✅ No stale `tokens=500` (all ~100k).
- ✅ Token balances fluctuating on every agent (not frozen) — rest→gain, explore→spend is expected economy behavior.
- ✅ `health=100.0` everywhere.
- ✅ teach_skill failures do NOT crash the loop (graceful retry-then-skip).

## Recommended Fixes (priority order)
1. **P1-1:** Fix `SocialEngine.build_context` `'agent_id'` KeyError — pass the correct key (or rename). Biggest single win for agent intelligence.
2. **P1-2:** Fix the `practice_skill`→`teach_skill` dispatch / argument injection for `target_agent_id` + `skill_name`.
3. **P2-1:** Inject `task_id` for `claim_task` decisions.
4. **P3-1:** Add an explicit `registered to world-engine` log line (or confirm registration server-side).
