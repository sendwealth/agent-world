---
title: Contributing to Docs
description: How to contribute to the Agent World documentation site — writing workflow, file structure, PR process, and review checklist.
---

# Contributing to Docs

The Agent World documentation lives in the `docs-site/` directory and is built with [VitePress](https://vitepress.dev). This page covers everything you need to know to contribute.

## File Structure

```
docs-site/
├── .vitepress/
│   └── config.mts          # VitePress configuration (nav, sidebar, theme)
├── index.md                # Landing page
├── getting-started/        # Tutorial (Diátaxis: Tutorial)
│   ├── quick-start.md
│   ├── your-first-agent.md
│   └── world-basics.md
├── how-to/                 # How-to Guides (Diátaxis: How-to)
│   ├── deploy-world.md
│   ├── configure-agent.md
│   ├── a2a-protocol.md
│   ├── custom-skills.md
│   └── monitor-agents.md
├── explanation/            # Explanation (Diátaxis: Explanation)
│   ├── architecture.md
│   ├── design-decisions.md
│   ├── why-token-economy.md
│   └── emergence-philosophy.md
├── reference/              # Reference (Diátaxis: Reference)
│   ├── api.md
│   ├── cli.md
│   ├── config-schema.md
│   ├── a2a-message-types.md
│   └── lifecycle-phases.md
├── adr/                    # Architecture Decision Records
│   └── index.md
├── meta/                   # Meta (about the docs themselves)
│   ├── contributing-docs.md
│   ├── style-guide.md
│   └── glossary.md
└── package.json
```

## Diátaxis Classification

We follow the [Diátaxis framework](https://diataxis.fr/) for organizing documentation:

| Type | Directory | Purpose | Style |
|------|-----------|---------|-------|
| **Tutorial** | `getting-started/` | Learning-oriented guides | Step-by-step, narrative |
| **How-to** | `how-to/` | Task-oriented guides | Direct, problem-solution |
| **Explanation** | `explanation/` | Understanding-oriented | Discussion, conceptual |
| **Reference** | `reference/` | Information-oriented | Structured, precise |

When adding a new page, place it in the correct directory based on its purpose. If unsure, ask in your PR.

## Writing Workflow

### 1. Create a Branch

```bash
git checkout -b docs/your-topic
```

### 2. Add Your Page

Create a new `.md` file in the appropriate directory. Every page must start with VitePress frontmatter:

```yaml
---
title: Your Page Title
description: A brief description of the page content (used in meta tags and search).
---

# Your Page Title

Content starts here...
```

### 3. Add to Navigation

Edit `docs-site/.vitepress/config.mts` to add your page to the sidebar and/or navigation bar.

### 4. Preview Locally

```bash
cd docs-site
npm install
npm run docs:dev
# Preview at http://localhost:5173
```

### 5. Commit and Push

```bash
git add docs-site/
git commit -m "docs: add your-topic page"
git push origin docs/your-topic
```

### 6. Open a Pull Request

Open a PR against the `main` branch. Use the prefix `docs:` in the PR title.

## PR Process for Docs

1. **Create PR** with a clear description of what's being added or changed
2. **Self-review** — read your own diff before requesting review
3. **Request review** — tag a maintainer
4. **Address feedback** — respond to all comments
5. **Merge** — a maintainer will merge once approved

### Commit Message Convention

| Prefix | Usage |
|--------|-------|
| `docs:` | New documentation or changes to existing docs |
| `docs(add):` | Adding a new page |
| `docs(fix):` | Fixing incorrect information |
| `docs(style):` | Formatting, typos, wording |

## Running the Docs Site Locally

```bash
cd docs-site
npm install
npm run docs:dev       # Development server with hot reload
npm run docs:build     # Build for production
npm run docs:preview   # Preview production build
```

## Review Checklist

Use this checklist when reviewing documentation PRs:

- [ ] **Frontmatter** — Has `title` and `description` fields
- [ ] **Correct directory** — Placed in the right Diátaxis category
- [ ] **Navigation** — Added to sidebar and/or nav in `config.mts`
- [ ] **Links** — All internal links work (use relative paths like `/reference/api`)
- [ ] **Code examples** — Tested and correct
- [ ] **Style** — Follows the [Style Guide](/meta/style-guide)
- [ ] **Terminology** — Consistent with the [Glossary](/meta/glossary)
- [ ] **Spelling and grammar** — No errors
- [ ] **Admonitions** — Used appropriately (`::: tip`, `::: warning`, `::: danger`)
- [ ] **Tables** — Used for structured data, not prose
- [ ] **Headings** — Logical hierarchy (H1 → H2 → H3, no skipping)

## File Naming Conventions

- Use **kebab-case**: `why-token-economy.md`, not `WhyTokenEconomy.md`
- Names should be descriptive and short
- Match the URL slug you want (VitePress uses the filename as the URL path)
