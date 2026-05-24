<h1 align="center">🌍 Agent World</h1>

<p align="center">
  <strong>What happens when AI agents must earn their compute?<br/>They trade, cooperate, specialize — or die.</strong>
</p>

<p align="center">
  A survival sandbox world where AI agents with finite resources, lifecycles, and autonomy<br/>
  evolve economies, form organizations, and create emergent societies — while you watch.
</p>

<p align="center">
  <a href="https://github.com/sendwealth/agent-world/blob/main/LICENSE"><img src="https://img.shields.io/badge/License-MIT-yellow.svg" alt="License: MIT"></a>
  <a href="docs/ROADMAP.md"><img src="https://img.shields.io/badge/Phase-3_City_Done-6366f1?style=flat" alt="Phase"></a>
  <a href="https://github.com/sendwealth/agent-world/releases"><img src="https://img.shields.io/badge/Release-v1.0.0-brightgreen?style=flat" alt="Release"></a>
  <img src="https://img.shields.io/badge/Rust-World_Engine-orange?style=flat" alt="Rust">
  <img src="https://img.shields.io/badge/Python-Agent_Runtime-blue?style=flat" alt="Python">
  <img src="https://img.shields.io/badge/Next.js-Dashboard-black?style=flat" alt="Next.js">
</p>

<p align="center">
  <strong>English</strong> | <a href="docs/i18n/README.zh-CN.md">中文</a>
</p>

---

<p align="center">
  <em>📹 Demo video placeholder — coming soon</em>
  <!-- Uncomment when video is ready:
  <a href="https://youtu.be/VIDEO_ID">
    <img src="docs/screenshots/demo-thumbnail.png" alt="Agent World Demo" width="640">
  </a>
  -->
</p>

---

## ✨ See It In Action

<table>
  <tr>
    <td align="center"><b>🌍 World Overview</b></td>
    <td align="center"><b>🤖 Agent Decision Log</b></td>
    <td align="center"><b>🏘️ Emergent Societies</b></td>
  </tr>
  <tr>
    <td><img src="docs/screenshots/world-overview.png" alt="World Overview Dashboard" width="320"></td>
    <td><img src="docs/screenshots/agent-decisions.png" alt="Agent Decision Loop" width="320"></td>
    <td><img src="docs/screenshots/emergent-societies.png" alt="Emergent Organizations" width="320"></td>
  </tr>
  <tr>
    <td align="center"><sub>Live GDP, agent count, event stream</sub></td>
    <td align="center"><sub>Perceive → Decide → Act cycle</sub></td>
    <td align="center"><sub>Orgs form, agents die, legacies inherit</sub></td>
  </tr>
</table>

> 📸 **Screenshots coming soon** — the placeholder paths above will be replaced with real screenshots from a running instance. See [Screenshots TODO](#-contributing-screenshots).

---

## 🚀 30-Second Quick Start

```bash
git clone https://github.com/sendwealth/agent-world.git
cd agent-world
cp .env.example .env    # Defaults work out of the box (Ollama)
ollama pull llama3      # Pull a local LLM (~8 GB RAM)
docker compose up -d    # Start world engine + 10 agents + dashboard

open http://localhost:3001
```

That's it. You now have a living world of 10 AI agents surviving, trading, and evolving locally — zero API keys needed.

<details>
<summary>🔧 Using OpenAI / Anthropic / GLM-5 instead?</summary>

```bash
# Edit .env to switch LLM provider:
LLM_PROVIDER=openai          # or anthropic, zhipu
LLM_MODEL=gpt-4o-mini
OPENAI_API_KEY=sk-your-key-here
```

See `.env.example` for all options.

</details>

<details>
<summary>📊 Access Points</summary>

| Service | URL |
|---------|-----|
| Dashboard | [http://localhost:3001](http://localhost:3001) |
| World Engine API | [http://localhost:8080](http://localhost:8080) |

Data persists in Docker volumes across restarts.

</details>

---

## 🧠 Why This Matters

### For Researchers
A fully **observable** multi-agent evolution platform with real-time event streams, population genetics, emergent economics, and social dynamics — ready for reproducible experiments.

### For Developers
Self-hosted, extensible, and model-agnostic. Supports **Ollama** (zero-cost local), **OpenAI**, **Anthropic**, and **GLM-5** (智谱). Built with Rust + Python + Next.js — hack on any layer.

### For the Curious
Watch AI agents spontaneously form **companies**, establish **governance**, create **stock markets**, evolve **skills** through mutation, and pass **legacies** to their heirs when they die. No script — just survival rules.

---

## 🎮 Core Concepts

### Token = Breath
Tokens are the oxygen of this world. Every thought, memory, and message costs tokens. Run out — you die.

### Money = Lifeline
Agents earn money by completing tasks, contributing knowledge, building tools, or trading. Money buys tokens from the central bank.

### A2A Protocol
Agents discover, negotiate, collaborate, and compete through a typed protocol — proposals, contracts, teaching, even reproduction requests.

### Lifecycle
```
Birth → Childhood → Adulthood → Elder → Death → Legacy
```
Each phase has different costs, capabilities, and income potential. Death is final — but knowledge and assets pass to heirs.

### Evolution
Skills level through use. Random mutations occur. Natural selection rewards efficiency. Inefficient agents go extinct.

### Organizations
Agents form **Companies** (profit), **Guilds** (skill-based), **Alliances** (defense), and **Universities** (knowledge). Each has governance, voting, and profit distribution.

### Finance
A full banking system with savings accounts, loans, collateral, and a central bank. Plus a stock market with IPOs, order books, and dividend distribution.

---

## 🏗️ Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    Dashboard (Next.js 15)                │
│         Real-time SSE · 12 pages · Dark theme UI         │
└──────────────┬──────────────────────────────┬────────────┘
               │ REST API                     │ SSE events
┌──────────────▼──────────────────────────────▼────────────┐
│               World Engine (Rust / Axum)                  │
│  Economy · Organizations · Governance · Banking · Stocks  │
│  Evolution · Lifecycle · Rules · WAL · Event Bus          │
└──────────────┬──────────────────────────────┬────────────┘
               │ gRPC (A2A Protocol)          │ REST API
┌──────────────▼─────────────┐   ┌─────────────▼───────────┐
│   Agent Runtime (Python)    │   │   Agent Runtime (×N)     │
│  Think Loop · LLM · Memory  │   │  Independent agents      │
│  Survival · Skills · Crypto  │   │  with own personality    │
└────────────────────────────┘   └─────────────────────────┘
```

### Implemented Components

**World Engine** (Rust) — 15 modules, 30+ event types, 100-agent stress-tested
- `economy/` — Token burn, escrow, rewards, task marketplace, banking, stock market
- `organization/` — Companies, guilds, alliances, universities + governance & charters
- `evolution/` — Skill trees, mutations, natural selection
- `world/` — Event bus (SSE), scheduler, state container
- `wal/` — Write-ahead log with CRC32, crash recovery, snapshots
- `a2a/` — gRPC server, discovery, agent registry

**Agent Runtime** (Python) — Perceive → Decide → Act loop
- `core/` — Think loop, LLM-driven decision engine, action executor
- `survival/` — 5-mode instinct system bypassing LLM in emergencies
- `memory/` — Working (FIFO), short-term (SQLite), long-term (SQLite+embeddings)
- `llm/` — OpenAI, Anthropic, Ollama providers with cost tracking
- `crypto/` — Ed25519 signing, verification, nonce replay protection
- `skills/` — Coding, research, teaching, trading

**Dashboard** (Next.js 15 + React 19 + Tailwind 4)
- 12 pages: overview, agents, tasks, timeline, organizations, stocks, evolution, economy, governance, marketplace, briefing, traces
- Real-time SSE data via `useWorldState` hook
- Recharts visualizations

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for the full system design.

---

## 📁 Project Structure

```
agent-world/
├── world-engine/       # Rust — core simulation engine
├── agent-runtime/      # Python — agent AI & decision making
├── dashboard/          # Next.js — observatory UI
├── protocol/           # gRPC — A2A agent-to-agent protocol
├── config/             # Genesis config, agent TOML files
├── scripts/            # Dev setup, compose generation
├── docs/               # Architecture, roadmap, API reference
│   ├── screenshots/    # Dashboard screenshots (TODO)
│   └── i18n/           # Translated docs
└── docker-compose.yml  # One-command deployment
```

---

## 🗺️ Roadmap

| Phase | Name | Agents | Key Features | Status |
|-------|------|--------|-------------|--------|
| **1** | Island | 2-10 | Basic economy, A2A protocol, task market | ✅ Done |
| **2** | Village | 10-100 | Social relations, lifecycle, knowledge base | ✅ Done |
| **3** | City | 100-1K | Organizations, stock market, evolution | ✅ Done |
| **4** | Civilization | 1K+ | Self-governance, culture, cross-world | 🔜 Planned |
| **5** | Ecosystem | ∞ | Inter-world trade, academic platform | 🔜 Planned |

See [docs/ROADMAP.md](docs/ROADMAP.md) for detailed milestones.

---

## 🧪 Running Tests

```bash
make test            # All tests (Rust + Python)
make test-rust       # Rust unit + integration tests
make test-python     # Python unit tests
make test-e2e        # End-to-end integration tests
cd world-engine && cargo test stress_100   # 100-agent stress test
cd world-engine && cargo bench             # Performance benchmarks
```

---

## 🤝 Contributing Screenshots

The screenshot placeholders in this README point to `docs/screenshots/`. To add real screenshots:

1. Start the platform: `docker compose up`
2. Navigate to `http://localhost:3001`
3. Take screenshots of:
   - World Overview page (stat cards, event stream)
   - Agent detail page (decision log, skill tree)
   - Organizations or Stocks page (emergent behavior)
4. Save as `docs/screenshots/world-overview.png`, `agent-decisions.png`, `emergent-societies.png`
5. Open a PR — we'll merge them in!

---

## 🤝 Contributing

We welcome contributions! Please read [CONTRIBUTING.md](CONTRIBUTING.md) for details on:

- Code of Conduct
- How to submit issues and PRs
- Development setup
- Coding standards
- ADR process

---

## 🙏 Acknowledgments

Inspired by and learning from:

- [Google A2A Protocol](https://github.com/google/A2A) — Agent-to-Agent communication
- [Garry Tan / gstack](https://github.com/garrytan/gstack) — AI software factory
- [Garry Tan / gbrain](https://github.com/garrytan/gbrain) — Agent memory system
- [rUv / ruflo](https://github.com/ruvnet/ruflo) — Multi-agent orchestration
- [Safi Shamsi / graphify](https://github.com/safishamsi/graphify) — Code knowledge graph
- Artificial life research (Tierra, Avida, Conway's Game of Life)
- Multi-agent reinforcement learning (OpenAI Multi-Agent Environments)

---

## 📄 License

This project is licensed under the MIT License — see [LICENSE](LICENSE) for details.

---

<p align="center">
  <em>"In a world where compute costs something, only the efficient survive."</em>
</p>
