---
title: Writing Style Guide
description: Conventions for writing Agent World documentation
---

# Documentation Style Guide

This guide defines the writing and formatting conventions for Agent World documentation. Follow these rules to keep the docs consistent, clear, and maintainable.

---

## Voice and Tone

| Aspect | Guideline |
|--------|-----------|
| **Voice** | Active voice. "Click **Run**" not "The **Run** button should be clicked." |
| **Tone** | Direct and friendly. Assume the reader is a developer who wants to get things done. |
| **Person** | Second person ("you") for instructions. First person plural ("we") for explanations of design rationale. |
| **Level** | Professional, not academic. Avoid unnecessary jargon. Define terms on first use. |

---

## Document Types (Diátaxis)

Each page belongs to one of four categories. Write accordingly:

| Type | Reader Goal | Style |
|------|-------------|-------|
| **Tutorial** | Learn by doing — achieve a first success | Step-by-step, outcome-focused, no detours |
| **How-to Guide** | Accomplish a specific task | Goal-oriented, assumes basic knowledge, prerequisite section |
| **Reference** | Look up exact details | Structured, complete, precise, minimal prose |
| **Explanation** | Understand a concept | Narrative, can have opinions, no step-by-step |

If a page mixes types, split it. A reference page should not contain tutorials. A tutorial should not list every config option.

---

## Formatting

### Headings

- Use `#` (H1) for the page title. Only one H1 per page.
- Use `##` (H2) for major sections.
- Use `###` (H3) for subsections.
- Avoid H4+. If you need H4, restructure the content.
- Headings use Title Case for tutorials and how-tos, sentence case for reference and explanation.

### Code Blocks

- Always specify the language: ` ```bash `, ` ```python `, ` ```rust `, ` ```yaml `.
- Every code example must be runnable. Include import statements and context.
- Use `$` prefix for shell commands that a user types:
  ```bash
  $ cargo build --release
  ```
- Use `#` prefix for shell comments, not `$`.
- For output examples, use no prefix:
  ```
  Agent World Engine v1.0.0
  Status: ready
  ```

### Inline Code

- Use backticks for: CLI commands, API endpoints, file names, config keys, type names, variable names.
- Example: "Edit `genesis.yaml` and set `initial_tokens` to `1000`."

### Admonitions

VitePress supports these containers:

```
:::tip
Helpful suggestion or best practice.
:::

:::info
Neutral informational note.
:::

:::warning
Something that could cause errors or unexpected behavior.
:::

:::danger
Data loss, security risk, or irreversible action.
:::
```

Use them sparingly. If everything is a warning, nothing is.

### Tables

- Use tables for structured data: config options, API parameters, comparison lists.
- Keep column headers short.
- Align columns for readability.

### Links

- Use relative links for internal pages: `[Architecture](/explanation/architecture)`.
- Use full URLs for external resources: `[Rust](https://www.rust-lang.org/)`.
- Every page should have "Next steps" or "See also" at the bottom with relevant links.

---

## Terminology

Use terms consistently across all pages. When in doubt, check the [Glossary](/meta/glossary).

| Term | Use | Don't Use |
|------|-----|-----------|
| Agent | Agent (capital A when referring to the concept) | agent, bot, entity |
| World Engine | World Engine (two words, both capitalized) | world-engine, engine, server |
| Agent Runtime | Agent Runtime (capitalized) | runtime, agent-runtime |
| Dashboard | Dashboard (capitalized) | dashboard, UI, web UI |
| Token | Token (capital T when referring to the resource) | token, credit, point |
| Money | money (lowercase) | Money, currency, coin |
| A2A | A2A | a2a, Agent-to-Agent |
| Tick | Tick (capital T when referring to the concept) | tick, cycle, step |
| WAL | WAL | wal, Write-Ahead Log (use full name on first mention) |

---

## Code Example Standards

1. **Complete and runnable** — include imports, initialization, and cleanup.
2. **Use realistic values** — `agent_name = "explorer-1"` not `agent_name = "foo"`.
3. **Show error handling** — don't only show the happy path.
4. **Use comments sparingly** — only comment non-obvious logic.
5. **One concept per example** — don't combine multiple features in one block.

Good example:

```python
import httpx

response = httpx.post(
    "http://localhost:8080/api/v1/agents",
    json={"name": "explorer-1", "tokens": 500, "money": 100}
)
response.raise_for_status()
agent = response.json()
print(f"Agent spawned: {agent['id']}")
```

Bad example:

```python
# do stuff
r = httpx.post(URL, json=DATA)
print(r.json())
```

---

## File Organization

- One topic per file.
- File names: lowercase, hyphen-separated (`deploy-world.md`, not `deployWorld.md`).
- Directory structure follows Diátaxis classification.
- Every file has VitePress frontmatter with at least `title` and `description`.

---

## Review Checklist

Before submitting a documentation PR:

- [ ] File is in the correct Diátaxis directory
- [ ] Frontmatter includes `title` and `description`
- [ ] All code examples are runnable
- [ ] Internal links use relative paths
- [ ] Terminology matches the glossary
- [ ] No broken links
- [ ] Spelling and grammar checked
- [ ] "Next steps" section at the bottom
