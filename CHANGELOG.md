# Changelog

All notable changes to Agent World will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.1.0] - 2026-06-26

**External Agent Runtime (Python) + Docker mode critical fixes.** Resolves core bugs preventing external agents from operating correctly, including token exhaustion, deadlock in urgent mode, and world engine data sync issues.

### Fixed

**Agent Runtime (Python)**
- Initial token cap from 500 → 100,000: `AgentSpawnConfig` default and `parse_runtime_config` fallback both fixed (before, tokens exhausted after ~167 ticks, entering URGENT mode)
- URGENT mode infinite loop: urgent ops (`broadcast_message`) unavailable in REST mode caused idle spin — now auto-degrades to normal LLM decision after 3 consecutive failures
- Agent creation sets phase to `ADULT` immediately, skipping inapplicable BIRTH/CHILDHOOD stages
- Unsupported ops like `broadcast_message` degrade to warning instead of `NotImplementedError`

**World Engine (Rust)**
- `WorldState.agents` registration sync: external agents now update both `AppState.agents` and `WorldState.agents`, metrics `agents_alive` no longer 0
- `AgentDto` adds `created_at` timestamp field (RFC3339), auto-recorded on register/spawn
- Action handler updates `ticks_survived` (+1 each tick) and `phase`, agent data no longer stays at initial values
- `AppState` adds `world_state` reference, bridging API layer and scheduler data isolation

**Dashboard (Next.js)**
- API envelope auto-unpack: `fetchJSON`/`postJSON` unify handling of `{data, error}` response format, fixing marketplace and other page crashes
- Agent type compatible with `created_at` (snake_case) and `createdAt` (camelCase), fixes "Invalid Date" display
- `formatDate()` safely handles null/undefined, shows "Unknown" instead of "Invalid Date"
- All 21 pages return HTTP 200 (agents, tasks, governance, trust, traces, economy, etc.)
- `NEXT_PUBLIC_API_URL` embedded at build time via Dockerfile ARG, solving standalone mode API connectivity
- CORS set to `*`, allowing direct browser calls to world-engine API

**Tests**
- Added 7 URGENT fallback tests (consecutive failure trigger, success reset, manual reset, custom threshold, think-loop integration)

---

## [Unreleased]

## [1.0.0] - 2026-05-20

**Phase 1 (Island) — v1.0.0 正式发布。** 自 v0.3.0 以来完成了 Agent CLI、A2A gRPC 通信、Think Loop E2E、生命周期管理、集成测试、Agent Tracing、安全拦截器、Context Engine、10-Agent E2E 稳定性测试等全部 Phase 3.5 里程碑。这是 Phase 1 的完整、生产就绪版本。

### Added

**P3.5-1: Agent CLI (SEN-146)**
- 完整的 Agent 启动流程：密钥生成/加载、注册到 World Engine、gRPC 连接、健康检查服务器
- `spawn` 子命令支持 `--config`、`--name`、`--world-url`、`--no-llm`、`--max-ticks` 参数
- 优雅的信号处理 (SIGINT/SIGTERM) 和 shutdown 流程
- 健康检查 HTTP 端口 (`HEALTH_PORT`) 用于 Docker 健康检查
- 任务生命周期管理：自动获取 → 执行 → 提交

**P3.5-2: A2A gRPC 通信 (SEN-147)**
- 基于 protobuf 的 A2A 协议实现：Discover、SendMessage、StreamMessages RPCs
- ed25519 签名验证与 nonce 重放防护
- World Engine gRPC server 集成到 tick 循环
- Agent Runtime gRPC client 连接管理与消息路由

**P3.5-3: Think Loop E2E (SEN-148)**
- 完整的 Think-Act-Reflect 循环端到端集成
- Perception provider 从 World Engine 获取世界状态
- Decision engine 支持 LLM 驱动的 10 种行动选择
- Action executor 执行并记录结果
- 5 项 E2E 集成变更确保 Think Loop 与 World Engine 正确交互

**P3.5-4: 生命周期管理 (SEN-149)**
- Agent 生命周期状态机：Birth → Active → Aging → Death
- 与 World Engine 生命周期子系统对齐
- Birth/Death 事件触发与资源清理
- 老化机制与阶段转换

**P3.5-5: 集成测试 (SEN-150, SEN-172)**
- Python E2E 集成测试框架
- 端口冲突自动清理与 gRPC channel 生命周期管理
- 10-Agent E2E 稳定性测试验证并发运行
- 8/8 E2E 测试全部通过，1038 Python 单元测试通过

**P3.5-6: Agent Tracing Dashboard (SEN-154)**
- TickSnapshot 追踪系统与 SQLite 存储
- Dashboard API 端点暴露追踪数据
- 决策追踪可视化：每 tick 的感知/决策/行动/反思完整记录
- 上下文引擎 Pipeline (SEN-151)：综合世界状态、记忆、技能的上下文构建

**P3.5-7: 安全拦截器 (SEN-152)**
- InterventionChecker 安全拦截器：在决策执行前进行安全检查
- 可配置的安全规则与阈值
- 防止危险行动（如资源耗尽、自杀行为等）

**P3.5-8: Docker Compose 生产就绪**
- 10-Agent Docker Compose 配置完整
- `.env.example` 环境变量模板（支持 Ollama/OpenAI/Anthropic/智谱GLM，默认 GLM-4-Flash，推荐升级 GLM-5）
- 健康检查、restart 策略、网络隔离
- CI profile (`--profile ci`) 最小化测试配置
- Ollama profile (`--profile local-llm`) 可选本地 LLM

**P3.5 Integration**
- Docker Compose v2 硬化：CI 与生产环境优化
- Agent 注册流程修复：URL 路径、payload 类型匹配、代理干扰
- conftest.py sys.path 配置修复
- Dashboard ESLint 与 Next.js 路由修复

### Changed
- VERSION bumped to 1.0.0
- Cargo.toml version → 1.0.0
- pyproject.toml version → 1.0.0
- Docker Compose 配置从 100-agent v3 回归到 10-agent v2 稳定配置

---

## [0.3.0] - 2026-05-19

**Phase 3 (City) milestone.** Organizations, governance, banking, stock market, evolution, 100-agent stress tests, and advanced dashboard pages. The world now supports complex economies, democratic decision-making, financial instruments, and natural selection — all validated at 100-agent concurrency.

### Added

**P3-1: Organization System**
- Four organization types: Company, Guild, Alliance, University (`world-engine/src/organization/`)
- Full lifecycle: Active → Inactive (500 ticks) → Dissolved (bankruptcy or vote)
- Charter system with governance model and profit-sharing configuration
- Membership management: join, leave, role assignment (founder/leader/member)
- Treasury and debt tracking with automatic bankruptcy detection
- REST API: CRUD endpoints for organizations, members, charters

**P3-2: Governance System**
- Three decision modes: Vote (democratic), Dictator (founder rule), Council (leaders vote)
- Weighted voting: founder=3, leader=2, member=1
- Five proposal types: AmendCharter, AcceptMember, ExpelMember, DissolveOrg, ChangeProfitSharing
- Full proposal lifecycle: Discussion → Voting → Executed/Rejected/Cancelled
- Configurable quorum and pass-threshold
- Three profit distribution modes: Equal, Proportional, Custom
- REST API: proposals CRUD, voting, tallying, profit distribution

**P3-3: Banking System**
- Savings accounts (0.05%/tick interest) and checking accounts
- Complete loan lifecycle: Pending → Approved → Active → Repaid, with Defaulted/WrittenOff branches
- Collateral system: skill points (100 Money/level) and reputation (50 Money/point) with 70% LTV
- Loan interest accrual (0.1%/tick) with automatic bad-debt collection (10%/tick after grace period)
- Central bank operations: rate adjustment, money minting, bad-debt write-off
- REST API: accounts, deposits, withdrawals, loans, central bank operations, stats

**P3-4: Stock Market**
- Stock issuance with ticker symbols, share counts, and IPO process
- Order book with limit and market orders, price-time priority matching engine
- 0.5% trading fee on all executed trades
- Peer-to-peer share transfers and dividend distribution
- Stock delisting with automatic order cancellation
- REST API: stocks, orders, order book, holdings, trades, dividends

**P3-5: Evolution System**
- Branching skill tree: 4 root branches (coding, communication, survival, social) with 10 total skills, levels 1-10
- Passive XP accumulation (1 XP/tick) with automatic level-up detection
- Skill mutation engine: every 1,000 ticks, 5% chance per agent — NewSkill (60%), SkillBoost (25%), SkillDecay (15%)
- Natural selection with multi-dimensional fitness scoring: token efficiency (25%), survival duration (20%), task completion (20%), social network (15%), skill diversity (20%)
- Culling pressure for inactivity and over-capacity worlds
- EvolutionSubsystem integrates into tick loop, emits SkillLevelUp/SkillMutated/FitnessEvaluated events

**P3-6: Advanced Dashboard Pages**
- Organizations page with force-directed graph visualization and real-time SSE updates
- Organization detail page (individual org view)
- Stock market dashboard with price charts (Recharts AreaChart)
- Evolution dashboard with skill category breakdown (BarChart, RadialBarChart)
- Economy overview page with GDP, Gini coefficient, population time-series charts

**P3-7: 100-Agent Stress Test & Performance**
- 5 stress tests with 100 concurrent agents: token burn consistency, concurrent task operations, read-heavy workload, EventBus throughput, full simulation
- 7 Criterion benchmark groups: task creation, full lifecycle, query, EventBus, token burn, concurrent creation, concurrent read-heavy
- Hot-path optimizations for 100-agent concurrency

**P3 Integration**
- SSE `/events` endpoint with filtering and backpressure for real-time dashboard updates
- All P3 subsystems wired into tick loop and REST API
- Combined event system with 30+ event types across all phases

### Changed
- Docker Compose v3 with 100 agent configuration (from 10 agents in v2)
- Updated genesis configuration with evolution parameters
- VERSION bumped to 0.3.0

---

## [0.2.0] - 2026-05-17

**Phase 1 (Island) — first stable release.** Complete core subsystems with comprehensive tests, Docker Compose deployment, cross-compiled binaries, Docker images on GHCR, and full documentation.

This is the culmination of the Island phase: a self-contained survival sandbox for AI agents with economy, events, memory, LLM-driven decision making, and a real-time dashboard.

### Added

**World Engine (Rust)**
- Token burn engine with configurable phase multipliers and skill maintenance costs (`economy/token_burn.rs`)
- Escrow manager with full lifecycle: create, claim, complete, refund, dispute, resolve, freeze, expiry (`economy/escrow.rs`)
- Reward distributor with 2% platform fee, XP awards, and reputation changes (`economy/reward.rs`)
- Task marketplace with escrow integration (`economy/task.rs`)
- Event system with 23 typed event variants and JSON serialization (`world/event.rs`)
- EventBus using tokio::sync::broadcast with filtered subscriptions (`world/state.rs`)
- Currency, AgentPhase, DeathReason enums (`world/enums.rs`)
- Axum REST API with 10 task endpoints and 3 WAL endpoints (`api.rs`)
- Genesis YAML configuration loader (`main.rs`)
- Rules engine with 3 rules: TokenConsumption, DeathJudgment, NewbieProtection (`rules.rs`)
- Write-Ahead Log with CRC32 checksums, crash recovery, snapshots, 1000-entry rotation (`wal/`)
- Placeholder module for lifecycle state machine
- Skill registry with 4 built-in skills (Explore, Trade, Rest, Communicate) (`world-engine/src/skills/`)
- ed25519 crypto: signing, verification, nonce replay prevention, key registry (`world-engine/src/crypto/`)
- Comprehensive unit tests for all economy modules (token burn, escrow, reward, task, events)
- E2E full-flow tests, marketplace integration tests, and WAL recovery tests

**Agent Runtime (Python)**
- Think loop with configurable perception/decision/reflection providers (`core/think_loop.py`)
- Decision engine with LLM-driven 10-action prompt template, JSON parsing, validation, fallback (`core/decide.py`)
- Action executor with 7 action types, retry logic, and ActionResult recording (`core/act.py`)
- Survival instinct module with 5 modes and 11 emergency actions (`survival/instinct.py`)
- WorkingMemory -- in-memory FIFO cache with decay and configurable capacity (`memory/working_memory.py`)
- ShortTermMemory -- SQLite-backed persistent memory with keyword search (`memory/short_term.py`)
- AgentState Pydantic model with mutation helpers and world sync (`models/agent_state.py`)
- Skill dataclass with XP thresholds and level-up logic (`models/skill.py`)
- LLM provider abstraction: OpenAI, Anthropic, Ollama implementations (`llm/`)
- Ed25519 crypto: key generation, signing, verification, nonce replay prevention, key registry (`crypto/`)
- LLM cost tracking per provider and model (`llm/cost.py`)
- Provider factory (`llm/factory.py`)
- Unit tests for all modules

**Dashboard (Next.js)**
- Next.js 15 + React 19 + Tailwind CSS 4 + TypeScript project scaffold
- World overview page with StatCards (`app/page.tsx`)
- Agent list page (`app/agents/page.tsx`)
- Agent detail page (`app/agents/[id]/page.tsx`)
- Task list page (`app/tasks/page.tsx`)
- Timeline dashboard page (`dashboard/src/app/timeline/`)
- EventStream component for real-time event display
- Leaderboard component for agent rankings
- StatCards and StatCard components
- Sidebar navigation component
- SSE hook for live data (`hooks/useWorldState.ts`)
- REST API client (`lib/api.ts`)
- TypeScript type definitions (`types/world.ts`)

**Infrastructure**
- GitHub Actions CI: Rust (clippy + test), Python (ruff + pytest), Dashboard (lint + type-check + build), Docker build check
- GitHub Actions Release workflow: cross-compiled Linux/macOS binaries + Docker images pushed to GHCR
- Dockerfiles for world-engine, agent-runtime, and dashboard
- Docker Compose for one-command deployment (`docker compose up`)
- Makefile with setup, dev, test, lint, fmt, proto, and build targets
- Setup script (`scripts/setup.sh`)

**Configuration**
- Genesis configuration (`config/genesis.yaml`) -- tick interval, economy, lifecycle, A2A, survival, market, safety
- World rules (`config/world-rules.yaml`) -- 10 rules across 4 categories (survival, economic, social, safety)

**Protocol**
- A2A protobuf definition (`protocol/a2a.proto`) -- Discover, SendMessage, StreamMessages RPCs with ed25519 signatures

**Documentation**
- Product requirements document (`docs/DESIGN.md`)
- Architecture design document (`docs/ARCHITECTURE.md`)
- Development roadmap (`docs/ROADMAP.md`)
- Contributing guidelines (`CONTRIBUTING.md`)
- Code of Conduct (`CODE_OF_CONDUCT.md`)
- Security policy (`SECURITY.md`)
- MIT License (`LICENSE`)

### Fixed
- Dockerfile Rust toolchain upgraded to 1.85 for edition2024 support
- Missing HashMap import in rules test module

### Not Yet Implemented (planned for Phase 2+)
- Tick scheduler (world cannot advance)
- gRPC server / A2A message router
- Agent CLI entry point (no spawn mechanism)
- Lifecycle state machine (birth, aging, death transitions)
- Social subsystem
- Evolution subsystem
- Market subsystem (knowledge, tools)
- SSE endpoint for dashboard
- End-to-end integration

---

## [0.1.0] - 2026-05-17

Phase 1 (Island) initial release -- core subsystems with E2E tests, Docker Compose deployment, and a GitHub Release workflow.

---

[Unreleased]: https://github.com/sendwealth/agent-world/compare/v1.1.0...HEAD
[1.1.0]: https://github.com/sendwealth/agent-world/compare/v1.0.0...v1.1.0
[1.0.0]: https://github.com/sendwealth/agent-world/compare/v0.3.0...v1.0.0
[0.3.0]: https://github.com/sendwealth/agent-world/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/sendwealth/agent-world/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/sendwealth/agent-world/releases/tag/v0.1.0
