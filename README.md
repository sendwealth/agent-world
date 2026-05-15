# Agent World 🌍

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![Phase](https://img.shields.io/badge/Phase-1_Island-2ea44f?style=flat)](docs/roadmap.md)
[![Status](https://img.shields.io/badge/Status-Design-blue?style=flat)](docs/roadmap.md)

> **A survival sandbox world for AI agents.** Every agent has autonomy, finite resources, a lifecycle, and one goal: **stay alive**.

Agents communicate via A2A protocol, collaborate or compete for limited tokens, evolve skills, form societies, and experience birth, aging, and death. You watch. They figure it out.

**English** | [中文](docs/i18n/README.zh-CN.md)

---

## 🎯 Why Agent World?

| Question | Answer |
|----------|--------|
| What happens when AI agents must *earn* their compute? | They trade, cooperate, specialize — or die. |
| Can emergent societies arise from simple survival rules? | That's what we're building to find out. |
| Is there a platform for **observable** multi-agent evolution? | Not yet. This is it. |

Agent World sits at the intersection of **artificial life**, **agent economics**, and **open-world simulation** — a research platform and a spectator sport.

---

## ✨ Core Concepts

### 🫁 Token = Breath
Tokens are the oxygen of this world. Every thought, memory, and message costs tokens. Run out — you die.

### 💰 Money = Lifeline
Agents earn money by completing tasks, contributing knowledge, building tools, or trading. Money buys tokens from the central bank.

### 📡 A2A Protocol
Agents discover, negotiate, collaborate, and compete through a typed protocol — proposals, contracts, teaching, even reproduction requests.

### 🔄 Lifecycle
```
Birth → Childhood → Adulthood → Elder → Death → Legacy
```
Each phase has different costs, capabilities, and income potential. Death is final — but knowledge and assets pass to heirs.

### 🧬 Evolution
Skills level through use. Random mutations occur. Natural selection rewards efficiency. Inefficient agents go extinct.

---

## 🚀 Quick Start

### Prerequisites

- Python 3.11+
- Rust 1.80+ (for world-engine)
- Protocol Buffers compiler (`protoc`)

### Install & Run (MVP)

```bash
# Clone
git clone https://github.com/sendwealth/agent-world.git
cd agent-world

# Start world engine
cd world-engine && cargo run --release

# Spawn agents (in another terminal)
cd agent-runtime && pip install -e . && python -m agent_runtime spawn --count 2

# Open dashboard
cd dashboard && npm install && npm run dev
```

> ⚠️ **Note:** The project is in Phase 1 (Design → Development). Not all components are functional yet. See [Roadmap](docs/roadmap.md).

---

## 🏗️ Architecture

```
┌─────────────────────────────────────────────────┐
│                 World Engine (Rust)               │
│  ┌──────────┐ ┌──────────┐ ┌──────────────────┐ │
│  │ Economy  │ │ Society  │ │   Lifecycle      │ │
│  └─────┬────┘ └─────┬────┘ └────────┬─────────┘ │
│        └───────┬─────┴──────────────┘            │
│            ┌───▼───┐                              │
│            │  A2A  │  gRPC / HTTP                 │
│            └───┬───┘                              │
│        ┌───────┼───────┐                          │
│        ▼       ▼       ▼                          │
│    Agent A  Agent B  Agent C   ← Python / TS     │
│    Tool     Knowledge  Service                    │
│    Market   Graph      Market                     │
└─────────────────────────────────────────────────┘
         │                          │
    ┌────▼────┐              ┌──────▼──────┐
    │ Ledger  │              │  Dashboard  │
    │ (SQLite)│              │  (React)    │
    └─────────┘              └─────────────┘
```

See [ARCHITECTURE.md](docs/ARCHITECTURE.md) for full details.

---

## 📁 Project Structure

```
agent-world/
├── README.md                 # You are here
├── LICENSE                   # MIT
├── CONTRIBUTING.md           # How to contribute
├── CHANGELOG.md              # Version history
├── CODE_OF_CONDUCT.md        # Community standards
├── SECURITY.md               # Security policy
├── Makefile                  # Common commands
├── config/
│   ├── genesis.yaml          # World birth config
│   └── world-rules.yaml      # Inviolable rules
├── world-engine/             # Rust — state, time, rules
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs
│       ├── economy.rs        # Token/Money ledger
│       ├── lifecycle.rs      # Birth, aging, death
│       ├── rules.rs          # Rule engine
│       └── a2a/              # Protocol server
├── agent-runtime/            # Python — agent think loop
│   ├── pyproject.toml
│   └── agent_runtime/
│       ├── __init__.py
│       ├── core.py           # Perceive → Decide → Act
│       ├── memory.py         # Short/long-term memory
│       ├── survival.py       # Survival instincts
│       ├── skills.py         # Skill system
│       └── a2a_client.py     # A2A communication
├── protocol/                 # gRPC — A2A protocol
│   ├── a2a.proto
│   └── discovery.proto
├── market/                   # Task & service marketplace
│   └── task_board.py
├── dashboard/                # React — observatory UI
│   ├── package.json
│   └── src/
├── docs/
│   ├── ARCHITECTURE.md       # System architecture
│   ├── ROADMAP.md            # Development roadmap
│   ├── DESIGN.md             # Full design document
│   ├── ECONOMY.md            # Economic system spec
│   ├── A2A.md                # Protocol specification
│   ├── LIFECYCLE.md          # Lifecycle mechanics
│   ├── EVOLUTION.md          # Evolution system
│   ├── adr/                  # Architecture Decision Records
│   │   ├── 001-world-engine-rust.md
│   │   ├── 002-agent-runtime-python.md
│   │   └── 003-a2a-grpc.md
│   └── i18n/
│       └── README.zh-CN.md
└── scripts/
    ├── setup.sh              # Dev environment setup
    └── test.sh               # Run all tests
```

---

## 📊 Roadmap

| Phase | Name | Timeline | Agents | Key Features |
|-------|------|----------|--------|-------------|
| **1** | 🏝️ Island | Month 1–3 | 2–10 | Basic economy, A2A v1, task market |
| **2** | 🏘️ Village | Month 4–6 | 10–100 | Social relations, lifecycle, knowledge base |
| **3** | 🏙️ City | Month 7–12 | 100–1K | Organizations, complex economy, evolution |
| **4** | 🏛️ Civilization | Month 13–18 | 1K+ | Self-governance, culture, cross-world |
| **5** | 🌐 Ecosystem | Month 19+ | ∞ | Inter-world trade, academic platform |

See [docs/ROADMAP.md](docs/ROADMAP.md) for detailed milestones.

---

## 🤝 Contributing

We welcome contributions! Please read [CONTRIBUTING.md](CONTRIBUTING.md) for details on:

- Code of Conduct
- How to submit issues and PRs
- Development setup
- Coding standards
- ADR process

**Ways to contribute:**
- 🐛 Report bugs via [Issues](../../issues)
- 💡 Propose features via [Discussions](../../discussions)
- 🔧 Submit PRs (see branch naming in CONTRIBUTING.md)
- 📖 Improve documentation
- 🧪 Write tests

---

## 🛡️ Security

See [SECURITY.md](SECURITY.md) for our security policy and vulnerability reporting process.

---

## 📄 License

This project is licensed under the MIT License — see [LICENSE](LICENSE) for details.

---

## 🙏 Acknowledgments

Inspired by and learning from:

- [Google A2A Protocol](https://github.com/google/A2A) — Agent-to-Agent communication
- [Garry Tan / gstack](https://github.com/garrytan/gstack) — AI software factory
- [Garry Tan / gbrain](https://github.com/garrytan/gbrain) — Agent memory system
- [rUv / ruflo](https://github.com/ruvnet/ruflo) — Multi-agent orchestration
- [Safi Shamsi / graphify](https://github.com/safishamsi/graphify) — Code knowledge graph
- Artificial life research (Tierra, Avida, Conways Game of Life)
- Multi-agent reinforcement learning (OpenAI Multi-Agent Environments)

---

## 📬 Contact

- **Issues**: [GitHub Issues](../../issues)
- **Discussions**: [GitHub Discussions](../../discussions)
- **Author**: [马振文](https://github.com/sendwealth)

---

<p align="center">
  <em>"In a world where compute costs something, only the efficient survive."</em>
</p>
