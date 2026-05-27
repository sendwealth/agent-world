# Agent World — Architecture Design Document

> **版本**: v1.0.0 | **日期**: 2026-05-21 | **状态**: current
> 与 [DESIGN.md](DESIGN.md)（产品规格）和 [ROADMAP.md](ROADMAP.md)（路线图）配合阅读

---

## 实现状态总览

本文档描述 Agent World 的**目标架构**。下表标注各子系统的实现状态：

| 子系统 | 模块 | 状态 | 说明 |
|--------|------|------|------|
| World Engine | economy/ | **已实现** | token_burn、escrow、reward、task 均有完整实现和测试 |
| World Engine | world/ | **已实现** | EventBus（30+ 种事件类型）、enums、state、SSE endpoint |
| World Engine | api.rs | **已实现** | Axum REST API，包含 tasks、WAL、organizations、governance、stocks、banking 端点 |
| World Engine | config/ | **已实现** | genesis.yaml with economy, lifecycle, evolution parameters |
| World Engine | engine/ | **已实现** | WorldState、CultureStore；Tick 调度器在 world/scheduler.rs 中实现 |
| World Engine | lifecycle/ | **已实现** | LifecycleMachine: Birth→Childhood→Adulthood→Elder→Death |
| World Engine | rules/ | **已实现** | 3 条规则：TokenConsumption（R001）、DeathJudgment（R002）、NewbieProtection（R003） |
| World Engine | social/ | **已实现** | Trust network (economy/trust.rs), mentorship (economy/mentorship.rs), inheritance (economy/inheritance.rs) — 位于 economy/ 模块内 |
| World Engine | evolution/ | **已实现** | Skill trees, mutations, natural selection, EvolutionSubsystem |
| World Engine | market/ | **已实现** | Banking system, stock market with order book |
| World Engine | organization/ | **已实现** | Organizations (Company/Guild/Alliance/University), governance, charters, members |
| World Engine | a2a/ | **已实现** | gRPC 服务器（server.rs, grpc.rs, service.rs）、Discovery、Router、AgentRegistry、ClientPool |
| World Engine | storage/ | **已实现** | persistence/ 模块：SQLite 快照持久化（persistence/sqlite.rs, 472 行） |
| World Engine | wal/ | **已实现** | Write-Ahead Log：CRC32 校验、崩溃恢复、快照、1000 条自动轮转 |
| World Engine | observability/ | **已实现** | Prometheus metrics（/metrics 端点）、计数器/直方图/仪表盘、结构化日志；OpenTelemetry 集成待 Phase 2 |
| Agent Runtime | core/ | **已实现** | think_loop、decide、act 均有完整实现和测试 |
| Agent Runtime | survival/ | **已实现** | 5 模式生存本能，11 种紧急行动 |
| Agent Runtime | memory/ | **已实现** | WorkingMemory（FIFO）、ShortTermMemory（SQLite）、LongTermMemory（SQLite）均已实现；嵌入/向量记忆也有实现 |
| Agent Runtime | models/ | **已实现** | AgentState Pydantic 模型、enums、skill、personality、phase_abilities、values |
| Agent Runtime | llm/ | **已实现** | OpenAI、Anthropic、Ollama 三个 provider |
| Agent Runtime | crypto/ | **已实现** | Ed25519 密钥生成、签名、验证、Nonce 防重放、密钥注册表 |
| Agent Runtime | skills/ | **已实现** | SkillRegistry + SkillExecutor + 4 个内置技能（coding, research, teaching, trading） |
| Agent Runtime | a2a/ | **已实现** | gRPC 客户端（client.py, batch_client.py, world_client.py）、消息路由、Perception Provider |
| Agent Runtime | tools/ | **未实现** | |
| Agent Runtime | config/ | **已实现** | TOML/YAML 配置文件加载与合并（config.py, 291 行） |
| Agent Runtime | main.py | **已实现** | 完整 CLI 入口（__main__.py, 1358 行）：spawn、密钥生成/加载、注册、gRPC 连接、健康检查 |
| Dashboard | 全部 | **已实现** | Pages: overview, agents, tasks, timeline, organizations, stocks, evolution, economy, governance, marketplace, briefing, traces; SSE 实时数据 |
| Protocol | a2a.proto | **已实现** | 定义了 Discover、SendMessage、StreamMessages |
| Protocol | discovery.proto | **已实现** | 发现功能集成到 world_engine.proto（Register/Spawn/Heartbeat RPCs）；discovery.proto 未单独定义 |
| Agent Runtime | lifecycle/ | **已实现** | LifecycleSyncService：阶段同步、转换守卫、死亡处理（lifecycle/__init__.py, 338 行） |
| Agent Runtime | context/ | **已实现** | ContextEnginePipeline：token 预算、优先级驱动的上下文组装（context/engine.py, 613 行） |
| Agent Runtime | reflection/ | **已实现** | 反思引擎：记忆反思、自我评估、策略调整（reflection/ 模块） |
| Agent Runtime | social/ | **已实现** | 10 个社会模块：文化冲突/扩散、模仿、群体信任、语言实验等 |
| Agent Runtime | organization/ | **已实现** | 组织形成、治理、招募、规则演化（organization/ 模块） |
| Agent Runtime | tracing/ | **已实现** | TickTrace 采集、存储、推送、涌现指标、交互图谱 |
| Agent Runtime | experiment/ | **已实现** | A/B 实验框架、可复现性、实验报告 |
| Agent Runtime | export/ | **已实现** | 行为日志、经济数据、网络数据导出 |
| Agent Runtime | sdk/ | **已实现** | SDK 客户端（sdk/client.py, 236 行） |
| World Engine | persistence/ | **已实现** | SQLite 快照持久化层（persistence/sqlite.rs, 472 行） |
| World Engine | time_capsule/ | **已实现** | 时间胶囊系统（time_capsule.rs, 665 行） |
| World Engine | tracing/ | **已实现** | TickTrace 数据模型（tracing.rs） |
| World Engine | grpc_pool/ | **已实现** | gRPC 连接池管理 |
| World Engine | world/scheduler | **已实现** | Tick 调度器（world/scheduler.rs, 153 行），可配置间隔、优雅关闭 |
| World Engine | world/engine | **已实现** | WorldState 世界状态容器（world/engine.rs, 472 行） |

> 标记为**已实现**的模块包含完整的功能代码和单元测试。标记为**部分**的模块有部分功能。标记为**占位**的模块仅有空结构体。标记为**未实现**的模块完全没有代码。

---

## 目录

1. [系统架构总览](#1-系统架构总览)
2. [部署架构](#2-部署架构)
3. [World Engine 详细设计](#3-world-engine-详细设计)
4. [Agent Runtime 详细设计](#4-agent-runtime-详细设计)
5. [A2A Protocol 详细设计](#5-a2a-protocol-详细设计)
6. [Economy Subsystem](#6-economy-subsystem)
7. [Lifecycle Subsystem](#7-lifecycle-subsystem)
8. [Evolution Subsystem](#8-evolution-subsystem)
9. [Social Subsystem](#9-social-subsystem)
10. [Market Subsystem](#10-market-subsystem)
11. [Dashboard Architecture](#11-dashboard-architecture)
12. [数据流与序列图](#12-数据流与序列图)
13. [存储架构](#13-存储架构)
14. [安全架构](#14-安全架构)
15. [可观测性架构](#15-可观测性架构)
16. [配置架构](#16-配置架构)
17. [错误处理与恢复](#17-错误处理与恢复)
18. [扩展性设计](#18-扩展性设计)

---

## 1. 系统架构总览

### 1.1 架构风格

Agent World 采用 **微内核 + 插件式** 架构：

- **微内核（World Engine）**: 只负责世界状态管理、Tick 调度、规则执行
- **外围服务**: 经济、社会、进化、市场各自独立模块，通过事件总线通信
- **Agent Runtime**: 独立进程，通过 A2A 协议与内核和其他 Agent 通信
- **Dashboard**: 纯前端 + SSE，无服务端渲染

### 1.2 系统上下文图

```
                        ┌─────────────┐
                        │   Human     │
                        │ (Observer/  │
                        │  Investor/  │
                        │  Creator)   │
                        └──────┬──────┘
                               │ REST + SSE
                               ▼
┌──────────┐  gRPC   ┌─────────────────┐  gRPC   ┌──────────┐
│  Agent   │◄────────►│                 │◄────────►│  Agent   │
│Runtime A │         │   World Engine   │         │Runtime B │
│(Python)  │         │   (Rust)         │         │(Python)  │
└────┬─────┘         │                  │         └──────────┘
     │               │  ┌───────────┐  │               │
     │               │  │ Event Bus │  │               │
     │               │  └─────┬─────┘  │               │
     │               │        │         │               │
     │               │  ┌─────▼─────┐  │               │
     │               │  │  Subsystems │ │               │
     │               │  │ ┌────────┐ │ │               │
     │               │  │ │Economy │ │ │               │
     │               │  │ ├────────┤ │ │               │
     │               │  │ │Social  │ │ │               │
     │               │  │ ├────────┤ │ │               │
     │               │  │ │Lifecycle│ │ │               │
     │               │  │ ├────────┤ │ │               │
     │               │  │ │Evolution│ │ │               │
     │               │  │ ├────────┤ │ │               │
     │               │  │ │Market  │ │ │               │
     │               │  │ └────────┘ │ │               │
     │               │  └───────────┘  │               │
     │               └─────────────────┘               │
     │                      │                          │
     │               ┌──────▼──────┐                    │
     │               │  Storage    │                    │
     │               │  (SQLite +  │                    │
     │               │  Vector DB) │                    │
     │               └─────────────┘                    │
     │                                                  │
     └──────────── A2A P2P (Direct) ◄───────────────────┘
            (绕过 World Engine 的 Agent 直连通道)
```

### 1.3 组件职责矩阵

| 组件 | 语言 | 进程模型 | 核心职责 | 无状态 |
|------|------|---------|---------|--------|
| World Engine | Rust | 单进程 | Tick、状态、规则、路由 | ❌ 有状态 |
| Agent Runtime | Python | 1进程/Agent | 思考、记忆、决策、行动 | ❌ 有状态 |
| A2A Router | Rust (World Engine 内) | 共享进程 | 消息路由、签名验证 | ✅ 无状态 |
| Dashboard | React + Next.js | 独立进程 | 展示、交互 | ✅ 无状态 |
| Storage | SQLite + 各 Agent 本地 | 共享/独立 | 持久化 | ❌ |

### 1.4 通信矩阵

| 源 → 目 | 协议 | 数据格式 | 方向 | 延迟目标 |
|---------|------|---------|------|---------|
| Agent → World Engine | gRPC (HTTP/2) | Protobuf | 请求/响应 + 流 | < 10ms |
| World Engine → Agent | gRPC (server stream) | Protobuf | 推送 | < 50ms |
| Agent → Agent (经由 WE) | gRPC → 路由 → gRPC | Protobuf | 异步消息 | < 100ms |
| Agent → Agent (直连) | HTTP/2 | JSON/Protobuf | 异步消息 | < 50ms |
| Dashboard → World Engine | REST + SSE | JSON | 请求 + 推送 | < 200ms |
| Agent Runtime → LLM | HTTP REST | JSON | 请求/响应 | 1-5s |

---

## 2. 部署架构

### 2.1 开发环境（Phase 1）

```
┌─────────────────────────────────────────────────┐
│              开发者机器 (localhost)                │
│                                                 │
│  ┌─────────────┐   ┌─────────────┐              │
│  │ World Engine│   │ Agent A     │              │
│  │ (cargo run) │◄─►│ (python)    │              │
│  │  :50051     │   │  :50052     │              │
│  └──────┬──────┘   └─────────────┘              │
│         │            ┌─────────────┐              │
│         │            │ Agent B     │              │
│         ├───────────►│ (python)    │              │
│         │            │  :50053     │              │
│         │            └─────────────┘              │
│         │                                        │
│  ┌──────▼──────┐                                 │
│  │ SQLite DB   │   ┌─────────────┐               │
│  │ world.db    │   │ Dashboard   │               │
│  └─────────────┘   │ (next dev)  │               │
│                    │  :3001       │               │
│                    └─────────────┘               │
└─────────────────────────────────────────────────┘
```

### 2.2 Docker Compose（生产/演示）

```yaml
# docker-compose.yml
version: '3.8'

services:
  world-engine:
    build: ./world-engine
    ports:
      - "50051:50051"    # gRPC
      - "8080:8080"      # REST API
    volumes:
      - world-data:/data
      - ./config:/config:ro
    environment:
      - RUST_LOG=info
      - GENESIS_CONFIG=/config/genesis.yaml

  agent-runtime:
    build: ./agent-runtime
    deploy:
      replicas: 2        # 初始 2 个 Agent
    depends_on:
      - world-engine
    environment:
      - WORLD_ENGINE_URL=http://world-engine:50051
      - LLM_PROVIDER=openai  # or: ollama, anthropic
      - LLM_MODEL=gpt-4o-mini

  dashboard:
    build: ./dashboard
    ports:
      - "3001:3000"
    depends_on:
      - world-engine
    environment:
      - NEXT_PUBLIC_API_URL=http://localhost:8080

volumes:
  world-data:
```

### 2.3 Kubernetes（Phase 3+）

```
Namespace: agent-world
├── Deployment: world-engine (1 replica, StatefulSet)
├── Deployment: agent-runtime (replicas: N, 1 pod/agent)
│   └── ConfigMap per agent (personality, skills seed)
├── Deployment: dashboard (2 replicas)
├── Service: world-engine-grpc (ClusterIP)
├── Service: world-engine-rest (ClusterIP)
├── Service: dashboard (LoadBalancer)
├── PVC: world-data (SQLite → 未来 PostgreSQL)
└── PVC per agent: agent-memory (vector DB data)
```

---

## 3. World Engine 详细设计

### 3.1 模块图

**当前实际文件结构（v1.0.0）：**

```
world-engine/
├── src/
│   ├── main.rs                  # ✅ 入口：加载配置 → 启动 HTTP + gRPC 服务器
│   ├── lib.rs                   # ✅ 模块重导出
│   ├── api.rs                   # ✅ Axum REST API（tasks, WAL, orgs, governance, banking, stocks, SSE）
│   ├── config.rs                # ✅ Genesis YAML 配置加载
│   ├── lifecycle.rs             # ✅ LifecycleMachine: Birth→Childhood→Adulthood→Elder→Death
│   ├── rules.rs                 # ✅ 3 条规则（R001-R003），RuleRegistry + RuleContext
│   ├── grpc_pool.rs             # ✅ gRPC 连接池
│   ├── time_capsule.rs          # ✅ 时间胶囊
│   ├── tracing.rs               # ✅ TickTrace 数据模型
│   ├── engine/                  # ✅ 引擎模块
│   │   ├── mod.rs               # ✅ WorldState + CultureStore
│   │   ├── state.rs             # ✅ 世界状态（DashMap，Agent/Org/Task 注册）
│   │   └── culture.rs           # ✅ 组织文化向量 + 区域文化集群
│   ├── economy/
│   │   ├── mod.rs               # ✅ 模块重导出
│   │   ├── token_burn.rs        # ✅ Token 消耗引擎（阶段乘数 + 技能成本）
│   │   ├── escrow.rs            # ✅ 托管管理器（完整生命周期）
│   │   ├── reward.rs            # ✅ 奖励分配（2% 平台费 + XP + 声望）
│   │   ├── task.rs              # ✅ 任务市场（状态机 + 托管集成）
│   │   ├── ledger.rs            # ✅ 双式记账账本
│   │   ├── banking.rs           # ✅ 银行系统（储蓄/支票账户、贷款、央行操作）
│   │   ├── stock_market.rs      # ✅ 股票市场（发行、IPO、订单簿、撮合引擎、分红）
│   │   ├── marketplace.rs       # ✅ 自由市场
│   │   ├── trust.rs             # ✅ 信任网络
│   │   ├── mentorship.rs        # ✅ 导师关系
│   │   ├── inheritance.rs       # ✅ 遗产继承
│   │   └── reputation.rs        # ✅ 信誉系统
│   ├── world/
│   │   ├── mod.rs               # ✅ 模块重导出
│   │   ├── enums.rs             # ✅ Currency, AgentPhase, DeathReason
│   │   ├── event.rs             # ✅ 30+ 种 WorldEvent 变体
│   │   ├── state.rs             # ✅ EventBus（tokio broadcast）
│   │   ├── agent.rs             # ✅ AgentRecord 数据结构
│   │   ├── genesis.rs           # ✅ GenesisConfig 加载
│   │   ├── engine.rs            # ✅ WorldState 世界状态容器
│   │   ├── scheduler.rs         # ✅ Tick 调度器（可配置间隔、优雅关闭）
│   │   ├── subsystem.rs         # ✅ Subsystem trait
│   │   ├── subsystems.rs        # ✅ 子系统注册
│   │   ├── discovery.rs         # ✅ Agent 发现
│   │   ├── seeder.rs            # ✅ Agent 种子数据
│   │   ├── intervention.rs      # ✅ 人类干预
│   │   └── tick_profiler.rs     # ✅ Tick 性能分析
│   ├── organization/
│   │   ├── mod.rs               # ✅ 组织模块（Company/Guild/Alliance/University）
│   │   ├── org.rs               # ✅ 组织 CRUD + 生命周期
│   │   ├── charter.rs           # ✅ 章程管理
│   │   ├── governance.rs        # ✅ 治理系统（提案、投票、利润分配）
│   │   ├── governance_metrics.rs # ✅ 治理指标
│   │   ├── members.rs           # ✅ 成员管理
│   │   ├── treasury.rs          # ✅ 组织金库
│   │   ├── leadership.rs        # ✅ 领导权管理
│   │   ├── competition.rs       # ✅ 组织间竞争
│   │   ├── diplomacy.rs         # ✅ 外交关系
│   │   └── rule_engine.rs       # ✅ 组织规则引擎
│   ├── evolution/
│   │   ├── mod.rs               # ✅ 进化模块
│   │   ├── skill_tree.rs        # ✅ 分支技能树（4 根分支 10 技能）
│   │   ├── mutation.rs          # ✅ 技能突变引擎（5% 概率/1000 tick）
│   │   ├── selection.rs         # ✅ 自然选择（多维适应度评分）
│   │   └── subsystem.rs         # ✅ EvolutionSubsystem（集成到 tick 循环）
│   ├── wal/
│   │   ├── mod.rs               # ✅ WAL（CRC32、崩溃恢复、快照、1000 条轮转）
│   │   └── crc.rs               # ✅ CRC32（ISO 3309 查表实现）
│   ├── a2a/                     # ✅ gRPC A2A 服务
│   │   ├── mod.rs               # ✅ 模块重导出
│   │   ├── server.rs            # ✅ gRPC 服务器（A2aService trait 实现）
│   │   ├── grpc.rs              # ✅ gRPC 服务注册
│   │   ├── service.rs           # ✅ 消息处理逻辑（Discover, Send, Stream）
│   │   ├── discovery.rs         # ✅ AgentRegistry 发现服务
│   │   ├── router.rs            # ✅ MessageRouter 消息路由
│   │   ├── registry.rs          # ✅ Agent 注册表
│   │   └── client_pool.rs       # ✅ gRPC 客户端连接池
│   └── persistence/             # ✅ 持久化层
│       ├── mod.rs               # ✅ 持久化接口
│       └── sqlite.rs            # ✅ SQLite 快照存储
│   └── observability/           # ✅ 可观测性
│       └── mod.rs               # ✅ Prometheus metrics + /metrics 端点 + 结构化日志
```

**规划中的模块（未实现）：**

```
│   └── tools/ (Agent Runtime)   # ❌ 通用工具框架
```

> ✅ = 已实现（含测试） | ⏳ = 占位符 | ❌ = 未实现

### 3.2 核心数据结构

```rust
/// 世界全局状态（内存中，定期持久化）
pub struct WorldState {
    pub tick: AtomicU64,
    pub config: GenesisConfig,
    pub agents: DashMap<String, AgentRecord>,
    pub organizations: DashMap<String, Organization>,
    pub tasks: DashMap<String, Task>,
    pub knowledge: DashMap<String, KnowledgeEntry>,
    pub ledger: Arc<RwLock<Ledger>>,
    pub relationships: DashMap<(String, String), Relationship>,
    pub event_tx: broadcast::Sender<WorldEvent>,
}

/// Agent 记录（World Engine 侧的视图）
pub struct AgentRecord {
    pub id: String,
    pub name: String,
    pub phase: AgentPhase,
    pub tokens: AtomicI64,
    pub money: AtomicI64,
    pub health: AtomicI32,
    pub reputation: AtomicF64,
    pub skills: HashMap<String, SkillRecord>,
    pub organization_id: Option<String>,
    pub created_tick: u64,
    pub death_tick: Option<u64>,
    pub last_active_tick: AtomicU64,
    pub endpoint: String,            // gRPC 地址
    pub public_key: Vec<u8>,         // ed25519 公钥
}

/// 技能记录
pub struct SkillRecord {
    pub name: String,
    pub level: u32,
    pub experience: u64,
    pub mutations: Vec<Mutation>,
    pub last_used_tick: u64,
}

/// 双式记账账本
pub struct Ledger {
    pub entries: Vec<LedgerEntry>,
    pub balances: HashMap<String, i64>,  // account_id → balance
    pub token_supply: i64,
    pub money_supply: i64,
}

/// 账本条目（不可变）
pub struct LedgerEntry {
    pub id: String,
    pub debit_account: String,     // 从谁扣
    pub credit_account: String,    // 加给谁
    pub amount: u64,
    pub currency: Currency,        // Token / Money
    pub entry_type: LedgerType,    // Exchange/Task/Interest/Tax/...
    pub description: String,
    pub tick: u64,
    pub reference_id: Option<String>, // 关联的任务/消息 ID
}

/// 世界事件（广播给所有订阅者）
#[derive(Clone, Serialize)]
pub enum WorldEvent {
    TickAdvanced { tick: u64 },
    AgentSpawned { agent_id: String, name: String },
    AgentDied { agent_id: String, cause: DeathCause },
    TransactionCompleted { from: String, to: String, amount: i64, currency: String },
    TaskPublished { task_id: String, reward: i64 },
    TaskCompleted { task_id: String, agent_id: String },
    OrganizationCreated { org_id: String, name: String },
    InflationAdjusted { rate: f64 },
    RuleViolated { agent_id: String, rule_id: String },
}
```

### 3.3 Tick 调度器

```rust
/// Tick 调度器 — 世界的核心心跳
pub struct Scheduler {
    interval: Duration,
    state: Arc<WorldState>,
    subsystems: Vec<Box<dyn Subsystem>>,
}

#[async_trait]
pub trait Subsystem: Send + Sync {
    /// 每个 Tick 调用一次
    async fn on_tick(&self, tick: u64, state: &WorldState) -> Result<Vec<WorldEvent>>;
    
    /// 子系统名称
    fn name(&self) -> &str;
}

impl Scheduler {
    pub async fn run(&self) {
        let mut ticker = tokio::time::interval(self.interval);
        
        loop {
            ticker.tick().await;
            let tick = self.state.tick.fetch_add(1, Ordering::SeqCst) + 1;
            
            // 1. 执行所有子系统
            for subsystem in &self.subsystems {
                match subsystem.on_tick(tick, &self.state).await {
                    Ok(events) => {
                        for event in events {
                            let _ = self.state.event_tx.send(event);
                        }
                    }
                    Err(e) => {
                        tracing::error!("Subsystem {} error at tick {}: {}", 
                            subsystem.name(), tick, e);
                    }
                }
            }
            
            // 2. Token 消耗（所有存活 Agent）
            self.burn_tokens(tick).await;
            
            // 3. 死亡判定
            self.check_deaths(tick).await;
            
            // 4. 快照（每 100 Tick）
            if tick % 100 == 0 {
                self.snapshot(tick).await;
            }
            
            // 5. 通胀检查（每 864 Tick = 1 世界日）
            if tick % 864 == 0 {
                self.inflation_check(tick).await;
            }
            
            // 6. 广播 Tick 事件
            let _ = self.state.event_tx.send(WorldEvent::TickAdvanced { tick });
        }
    }
}
```

### 3.4 事件总线

```rust
/// 事件总线 — 解耦子系统通信
/// 
/// 发布者: Subsystems, gRPC handlers
/// 订阅者: Dashboard SSE, Agent 推送, 日志
pub struct EventBus {
    tx: broadcast::Sender<WorldEvent>,
}

impl EventBus {
    /// 发布事件（fire-and-forget）
    pub fn publish(&self, event: WorldEvent) {
        // 如果没有订阅者，静默丢弃
        let _ = self.tx.send(event);
    }
    
    /// 订阅事件流
    pub fn subscribe(&self) -> broadcast::Receiver<WorldEvent> {
        self.tx.subscribe()
    }
    
    /// 带 filter 的订阅
    pub fn subscribe_filtered(&self, filter: EventFilter) -> impl Stream<Item = WorldEvent> {
        let rx = self.tx.subscribe();
        tokio_stream::wrappers::BroadcastStream::new(rx)
            .filter_map(move |result| {
                match result {
                    Ok(event) if filter.matches(&event) => Some(event),
                    _ => None,
                }
            })
    }
}
```

### 3.5 gRPC 服务

```rust
/// A2A gRPC 服务实现
pub struct A2aServiceImpl {
    state: Arc<WorldState>,
}

#[tonic::async_trait]
impl A2aService for A2aServiceImpl {
    /// 路由 A2A 消息
    async fn send_message(
        &self, 
        request: Request<A2AMessage>
    ) -> Result<Response<MessageAck>, Status> {
        let msg = request.into_inner();
        
        // 1. 验证签名
        self.verify_signature(&msg)?;
        
        // 2. 检查发送者 Token（扣除通信费）
        self.charge_communication(&msg.from_agent)?;
        
        // 3. 路由消息
        if msg.to_agent.is_empty() {
            // 广播
            self.broadcast_message(&msg).await?;
        } else {
            // 定向投递
            self.deliver_message(&msg).await?;
        }
        
        // 4. 记录消息日志
        self.log_message(&msg);
        
        Ok(Response::new(MessageAck { 
            received: true, 
            error: String::new() 
        }))
    }
    
    /// Agent 发现
    async fn discover(
        &self,
        request: Request<DiscoverRequest>
    ) -> Result<Response<DiscoverResponse>, Status> {
        let req = request.into_inner();
        let agents = self.state.agents.iter()
            .filter(|entry| {
                let agent = entry.value();
                agent.phase != AgentPhase::Dead
                    && self.matches_filter(agent, &req)
            })
            .map(|entry| self.to_agent_info(entry.value()))
            .collect();
        
        Ok(Response::new(DiscoverResponse { agents }))
    }
}
```

### 3.6 REST API 服务

```rust
/// REST API — Dashboard 和人类使用
pub struct RestApi {
    state: Arc<WorldState>,
}

impl RestApi {
    pub fn router(state: Arc<WorldState>) -> Router {
        Router::new()
            // 世界状态
            .route("/api/v1/world", get(Self::get_world_state))
            .route("/api/v1/world/events", get(Self::sse_events))
            
            // Agent
            .route("/api/v1/agents", get(Self::list_agents))
            .route("/api/v1/agents", post(Self::spawn_agent))
            .route("/api/v1/agents/:id", get(Self::get_agent))
            .route("/api/v1/agents/:id/history", get(Self::agent_history))
            
            // 任务
            .route("/api/v1/tasks", get(Self::list_tasks))
            .route("/api/v1/tasks", post(Self::publish_task))
            .route("/api/v1/tasks/:id", get(Self::get_task))
            
            // 经济
            .route("/api/v1/economy/gdp", get(Self::gdp_stats))
            .route("/api/v1/economy/inflation", get(Self::inflation_stats))
            .route("/api/v1/economy/ledger", get(Self::ledger_entries))
            
            // 市场
            .route("/api/v1/market/knowledge", get(Self::knowledge_market))
            .route("/api/v1/market/tools", get(Self::tool_market))
            
            // 实验
            .route("/api/v1/lab/params", post(Self::update_params))
            
            .with_state(state)
    }
}
```

---

## 4. Agent Runtime 详细设计

### 4.1 模块图

**当前实际文件结构（v1.0.0）：**

```
agent-runtime/
├── agent_runtime/
│   ├── __init__.py
│   ├── __main__.py               # ✅ CLI 入口（spawn 子命令，gRPC 连接，健康检查）
│   ├── config.py                 # ✅ TOML/YAML 配置加载与合并
│   ├── env_loader.py             # ✅ 环境变量加载
│   ├── core/
│   │   ├── __init__.py
│   │   ├── think_loop.py         # ✅ 主思考循环（可插拔 Provider）
│   │   ├── decide.py             # ✅ LLM 决策引擎（10 种行动类型）
│   │   ├── act.py                # ✅ 行动执行器（7 种行动类型 + 重试）
│   │   ├── async_decide.py       # ✅ 异步决策引擎
│   │   ├── llm_decide.py         # ✅ LLM 驱动决策
│   │   ├── memory_aware_decide.py # ✅ 记忆感知决策
│   │   ├── experience.py         # ✅ 经验积累
│   │   ├── intervention_checker.py # ✅ 安全拦截器
│   │   └── reflect.py            # ✅ 反思模块
│   ├── memory/
│   │   ├── __init__.py
│   │   ├── working_memory.py     # ✅ FIFO 缓存（重要性感知淘汰）
│   │   ├── short_term.py         # ✅ SQLite 持久化记忆（关键词搜索）
│   │   ├── long_term.py          # ✅ 长期记忆（SQLite，经验/策略/反思）
│   │   ├── persistent_store.py   # ✅ 通用持久化存储
│   │   ├── embedding.py          # ✅ 嵌入向量生成
│   │   ├── vector_memory.py      # ✅ 向量记忆（相似性检索）
│   │   └── memory_recall.py      # ✅ 记忆召回
│   ├── survival/
│   │   ├── __init__.py
│   │   └── instinct.py           # ✅ 5 模式生存本能（11 种紧急行动）
│   ├── models/
│   │   ├── __init__.py
│   │   ├── agent_state.py        # ✅ Pydantic Agent 状态模型
│   │   ├── enums.py              # ✅ AgentPhase, SurvivalMode
│   │   ├── skill.py              # ✅ Skill 数据类（XP 阈值 + 升级）
│   │   ├── personality.py        # ✅ 人格特质系统
│   │   ├── phase_abilities.py    # ✅ 阶段能力定义
│   │   └── values.py             # ✅ 价值体系
│   ├── llm/
│   │   ├── __init__.py
│   │   ├── base.py               # ✅ LLMProvider 抽象基类
│   │   ├── factory.py            # ✅ Provider 工厂
│   │   ├── openai_provider.py    # ✅ OpenAI 实现
│   │   ├── anthropic_provider.py # ✅ Anthropic 实现
│   │   ├── ollama_provider.py    # ✅ Ollama 实现
│   │   ├── cost.py               # ✅ 成本追踪
│   │   ├── prompts.py            # ✅ Prompt 模板
│   │   ├── decision_log.py       # ✅ 决策日志
│   │   └── queue.py              # ✅ LLM 请求队列
│   ├── crypto/
│   │   ├── __init__.py
│   │   ├── keys.py               # ✅ Ed25519 密钥生成
│   │   ├── signing.py            # ✅ 确定性 JSON 签名 + 验证
│   │   ├── nonce.py              # ✅ TTL 防重放缓存
│   │   └── registry.py           # ✅ 代理公钥注册表
│   ├── skills/
│   │   ├── __init__.py
│   │   ├── registry.py           # ✅ SkillRegistry（冻结数据类）
│   │   ├── executor.py           # ✅ SkillExecutor（XP 奖励）
│   │   ├── coding.py             # ✅ 编程技能
│   │   ├── research.py           # ✅ 研究技能
│   │   ├── teaching.py           # ✅ 教学技能
│   │   └── trading.py            # ✅ 交易技能
│   ├── a2a/
│   │   ├── __init__.py
│   │   ├── client.py             # ✅ gRPC 客户端（重试 + 双向流）
│   │   ├── batch_client.py       # ✅ 批量消息客户端
│   │   ├── world_client.py       # ✅ World Engine REST/gRPC 客户端
│   │   ├── message.py            # ✅ 消息模型
│   │   ├── perception.py         # ✅ 感知 Provider
│   │   └── config.py             # ✅ A2A 配置
│   ├── agent/
│   │   ├── __init__.py
│   │   └── capability.py         # ✅ Agent 能力定义
│   ├── lifecycle/
│   │   └── __init__.py           # ✅ 生命周期同步、转换守卫、死亡处理
│   ├── context/
│   │   ├── __init__.py
│   │   └── engine.py             # ✅ 上下文引擎 Pipeline（token 预算、优先级驱动）
│   ├── reflection/
│   │   ├── __init__.py
│   │   ├── reflection.py         # ✅ 反思引擎
│   │   ├── memory.py             # ✅ 记忆反思
│   │   ├── self_assess.py        # ✅ 自我评估
│   │   └── strategy.py           # ✅ 策略调整
│   ├── social/
│   │   ├── __init__.py
│   │   ├── comm_analyzer.py      # ✅ 通信分析
│   │   ├── cultural_conflict.py  # ✅ 文化冲突
│   │   ├── cultural_diffusion.py # ✅ 文化扩散
│   │   ├── imitation.py          # ✅ 模仿学习
│   │   ├── intergroup_trust.py   # ✅ 群体间信任
│   │   ├── jargon_detector.py    # ✅ 术语检测
│   │   ├── knowledge_transfer.py # ✅ 知识传递
│   │   ├── language_experiment.py # ✅ 语言涌现实验
│   │   ├── org_culture.py        # ✅ 组织文化
│   │   └── regional_culture.py   # ✅ 区域文化
│   ├── organization/
│   │   ├── __init__.py
│   │   ├── formation.py          # ✅ 组织形成
│   │   ├── governance.py         # ✅ 治理参与
│   │   ├── governance_analysis.py # ✅ 治理分析
│   │   ├── proposal.py           # ✅ 提案系统
│   │   ├── recruitment.py        # ✅ 招募系统
│   │   ├── rule_evolution.py     # ✅ 规则演化
│   │   └── rule_proposal.py      # ✅ 规则提案
│   ├── tracing/
│   │   ├── __init__.py
│   │   ├── collector.py          # ✅ 追踪采集
│   │   ├── store.py              # ✅ 追踪存储（SQLite）
│   │   ├── pusher.py             # ✅ 追踪推送
│   │   ├── models.py             # ✅ 追踪数据模型
│   │   ├── emergence_metrics.py  # ✅ 涌现指标
│   │   ├── interaction_graph.py  # ✅ 交互图谱
│   │   └── query.py              # ✅ 追踪查询
│   ├── observability/
│   │   └── __init__.py           # ✅ OpenTelemetry + Prometheus metrics（think_loop 自动追踪）
│   ├── experiment/
│   │   ├── __init__.py
│   │   ├── ab_framework.py       # ✅ A/B 实验框架
│   │   ├── config.py             # ✅ 实验配置
│   │   ├── report.py             # ✅ 实验报告
│   │   └── reproducibility.py    # ✅ 可复现性
│   ├── export/
│   │   ├── __init__.py
│   │   ├── behavior_log.py       # ✅ 行为日志导出
│   │   ├── economy_export.py     # ✅ 经济数据导出
│   │   └── network_export.py     # ✅ 网络数据导出
│   └── sdk/
│       ├── __init__.py
│       └── client.py             # ✅ SDK 客户端
├── tests/                        # ✅ 1038+ 测试
└── pyproject.toml                # version 1.0.0l
```

**规划中的模块（未实现）：**

```
│   └── tools/                   # ❌ 通用工具框架
│       ├── base.py              # Tool 抽象
│       ├── registry.py          # ToolRegistry
│       └── builtin/             # 内置工具集
```

> ✅ = 已实现（含测试） | ❌ = 未实现

### 4.2 核心类设计

```python
from dataclasses import dataclass, field
from enum import Enum
from typing import Optional
import asyncio

class AgentPhase(Enum):
    BIRTH = "birth"
    CHILDHOOD = "childhood"
    ADULT = "adult"
    ELDER = "elder"
    DEAD = "dead"

class SurvivalMode(Enum):
    PANIC = "panic"           # Token < 10%
    URGENT = "urgent"         # Token < 20%
    CONSERVATIVE = "conservative"  # Token < 40%
    NORMAL = "normal"         # Token 40-80%
    INVEST = "invest"         # Token > 80%

@dataclass
class AgentState:
    """Agent 当前状态（内存中，与 World Engine 同步）"""
    id: str
    name: str
    phase: AgentPhase
    tokens: int
    money: int
    health: int
    reputation: float
    skills: dict[str, "Skill"]  # name → Skill
    personality: Optional[str]
    survival_mode: SurvivalMode
    current_task: Optional[str]  # Task ID
    tick: int = 0

@dataclass
class Skill:
    name: str
    level: int
    experience: int
    max_level: int = 10
    
    @property
    def next_level_exp(self) -> int:
        """升级所需经验"""
        thresholds = [0, 100, 300, 700, 1500, 3000, 6000, 12000, 25000, 50000]
        if self.level >= self.max_level:
            return float('inf')
        return thresholds[self.level]
    
    def add_exp(self, amount: int) -> bool:
        """增加经验，返回是否升级"""
        self.experience += amount
        if self.experience >= self.next_level_exp:
            self.level += 1
            return True
        return False

class AgentRuntime:
    """Agent 运行时 — 每个 Agent 一个实例"""
    
    def __init__(self, config: AgentConfig):
        self.state: AgentState = ...
        self.memory = MemorySystem(config.memory)
        self.survival = SurvivalInstinct(config.survival)
        self.a2a = A2AClient(config.a2a)
        self.llm = LLMProvider.create(config.llm)
        self.skills = SkillRegistry()
        self.tools = ToolRegistry()
        self._running = False
    
    async def run(self):
        """主循环 — 每个 Tick 执行一次"""
        self._running = True
        while self._running:
            try:
                await self.think_loop()
                await asyncio.sleep(self.tick_interval)
            except TokenExhausted:
                await self.handle_death()
                break
            except Exception as e:
                logging.error(f"Agent {self.state.id} error: {e}")
                await asyncio.sleep(5)  # 退避
    
    async def think_loop(self):
        """思考循环: Perceive → Assess → Decide → Act"""
        self.state.tick += 1
        
        # 1. 感知
        perception = await self.perceive()
        
        # 2. 生存评估（不经过 LLM，立即判断）
        survival_action = self.survival.assess(self.state)
        if survival_action.mode in (SurvivalMode.PANIC, SurvivalMode.URGENT):
            await self.survival.execute(survival_action)
            return  # 跳过正常决策
        
        # 3. LLM 决策
        decision = await self.decide(perception, survival_action)
        
        # 4. 执行行动
        await self.act(decision)
        
        # 5. 反思（每 10 Tick）
        if self.state.tick % 10 == 0:
            await self.reflect()
    
    async def perceive(self) -> Perception:
        """收集当前世界信息"""
        return Perception(
            messages=await self.a2a.receive_messages(),
            token_balance=self.state.tokens,
            token_ratio=self.state.tokens / 100000,
            market_state=await self.a2a.get_market_state(),
            active_task=self.state.current_task,
            health=self.state.health,
            phase=self.state.phase,
        )
    
    async def decide(self, perception: Perception, survival: SurvivalAction) -> Decision:
        """LLM 驱动的决策"""
        prompt = self.build_decision_prompt(perception, survival)
        response = await self.llm.chat(prompt)
        return self.parse_decision(response)
    
    async def act(self, decision: Decision):
        """执行决策"""
        for action in decision.actions:
            match action.type:
                case "send_message":
                    await self.a2a.send_message(action.payload)
                case "claim_task":
                    await self.a2a.claim_task(action.task_id)
                case "execute_tool":
                    await self.tools.execute(action.tool_name, action.params)
                case "rest":
                    await asyncio.sleep(0)  # 节省 Token
                case "learn":
                    await self.skills.learn(action.skill, action.content)
```

### 4.3 LLM 提供商抽象

```python
from abc import ABC, abstractmethod
from dataclasses import dataclass

@dataclass
class LLMConfig:
    provider: str          # openai / anthropic / ollama
    model: str             # gpt-4o-mini / claude-haiku / qwen3:4b
    api_key: Optional[str]
    base_url: Optional[str]
    max_tokens: int = 1000
    temperature: float = 0.7

class LLMProvider(ABC):
    @staticmethod
    def create(config: LLMConfig) -> "LLMProvider":
        match config.provider:
            case "openai": return OpenAIProvider(config)
            case "anthropic": return AnthropicProvider(config)
            case "ollama": return OllamaProvider(config)
            case _: raise ValueError(f"Unknown provider: {config.provider}")
    
    @abstractmethod
    async def chat(self, messages: list[dict]) -> str:
        """发送消息并获取回复"""
    
    @abstractmethod
    async def chat_stream(self, messages: list[dict]) -> AsyncIterator[str]:
        """流式回复"""

class OllamaProvider(LLMProvider):
    """本地模型 — 零 API 成本"""
    def __init__(self, config: LLMConfig):
        self.base_url = config.base_url or "http://localhost:11434"
        self.model = config.model
    
    async def chat(self, messages: list[dict]) -> str:
        async with httpx.AsyncClient() as client:
            resp = await client.post(
                f"{self.base_url}/api/chat",
                json={"model": self.model, "messages": messages, "stream": False}
            )
            return resp.json()["message"]["content"]
```

### 4.4 决策 Prompt 模板

```python
DECISION_PROMPT = """You are {name}, an AI agent in Agent World.

## Your Status
- Phase: {phase}
- Tokens: {tokens} ({token_ratio:.0%})
- Money: {money}
- Health: {health}/100
- Reputation: {reputation:.1f}/100

## Your Skills
{skills_list}

## Current Situation
- Tick: {tick}
- Active Task: {active_task}
- Unread Messages: {message_count}

## Recent Memory
{recent_memory}

## Current Perceptions
{perception_summary}

## Survival Assessment
Mode: {survival_mode}
Action needed: {survival_guidance}

## Available Actions
1. respond_message - Reply to a message
2. claim_task - Accept a task from the board
3. submit_task - Submit completed work
4. propose_deal - Propose a trade/collaboration
5. teach_skill - Teach another agent
6. learn_skill - Ask to learn from another agent
7. rest - Save tokens (skip this tick)
8. explore - Look for opportunities
9. create_tool - Build a reusable tool
10. publish_knowledge - Share knowledge for profit

Choose your action. Respond in JSON:
{
  "action": "action_name",
  "params": { ... },
  "reasoning": "Why this action?",
  "expected_token_cost": 123,
  "expected_reward": 456
}
"""
```

---

## 5. A2A Protocol 详细设计

### 5.1 协议栈

```
┌───────────────────────────────────┐
│        Application Layer          │  Agent 业务逻辑
│  (propose, accept, teach, ...)    │
├───────────────────────────────────┤
│        Message Layer              │  A2A Message 格式
│  (routing, priority, TTL)         │
├───────────────────────────────────┤
│        Security Layer             │  ed25519 签名 + nonce
│  (authentication, replay protect) │
├───────────────────────────────────┤
│        Transport Layer            │  gRPC (HTTP/2)
│  (reliable, ordered, streaming)   │
└───────────────────────────────────┘
```

### 5.2 消息生命周期

```
Agent A 创建消息
    ↓
签名 (ed25519)
    ↓
gRPC → World Engine Router
    ↓
验证签名 + nonce
    ↓
扣除通信 Token
    ↓
路由决策:
├── to_agent 为空 → 广播（写入所有 Agent 队列）
├── to_agent 在本进程 → 直接投递
└── to_agent 在远端 → 转发到目标 Agent Runtime
    ↓
目标 Agent Runtime 接收
    ↓
进入消息队列
    ↓
Agent 下一个 Tick 处理
```

### 5.3 安全设计

```python
import nacl.signing
import nacl.encoding
import uuid
import time

class A2ACrypto:
    """A2A 消息加密和签名"""
    
    def __init__(self, seed: bytes):
        self.signing_key = nacl.signing.SigningKey(seed)
        self.verify_key = self.signing_key.verify_key
    
    def sign_message(self, message: dict) -> str:
        """签名消息"""
        # 1. 添加 nonce（防重放）
        message["nonce"] = str(uuid.uuid4())
        message["timestamp"] = int(time.time() * 1000)
        
        # 2. 序列化（确定性排序）
        canonical = json.dumps(message, sort_keys=True, separators=(',', ':'))
        
        # 3. 签名
        signed = self.signing_key.sign(canonical.encode())
        return signed.signature.hex()
    
    @staticmethod
    def verify_message(message: dict, signature: str, public_key: bytes) -> bool:
        """验证签名"""
        verify_key = nacl.signing.VerifyKey(public_key)
        canonical = json.dumps(message, sort_keys=True, separators=(',', ':'))
        try:
            verify_key.verify(canonical.encode(), bytes.fromhex(signature))
            return True
        except nacl.exceptions.BadSignatureError:
            return False

class NonceTracker:
    """Nonce 跟踪器 — 防重放攻击"""
    
    def __init__(self, max_age_seconds: int = 300):
        self.seen: dict[str, float] = {}
        self.max_age = max_age_seconds
    
    def check(self, nonce: str) -> bool:
        """检查 nonce 是否已使用"""
        now = time.time()
        # 清理过期 nonce
        expired = [n for n, t in self.seen.items() if now - t > self.max_age]
        for n in expired:
            del self.seen[n]
        
        if nonce in self.seen:
            return False  # 重放
        self.seen[nonce] = now
        return True
```

---

## 6. Economy Subsystem

### 6.1 双式记账

```
每笔交易同时记录借贷两方，保证恒等式:
  ∑(所有账户余额) = 0

账户类型:
  agent:{id}:tokens     — Agent Token 余额
  agent:{id}:money      — Agent Money 余额
  central_bank:tokens   — 央行 Token 池
  central_bank:money    — 央行 Money 池
  org:{id}:money        — 组织 Money 余额
  escrow:{task_id}      — 任务托管金

示例 — Agent A 用 100 Money 兑换 10000 Token:
  DEBIT  agent:A:money       100
  CREDIT central_bank:money  100
  DEBIT  central_bank:tokens 10000
  CREDIT agent:A:tokens      10000
```

### 6.2 Token 消耗引擎

```rust
pub struct TokenBurnEngine {
    config: EconomyConfig,
}

impl TokenBurnEngine {
    /// 计算单个 Agent 一个 Tick 的 Token 消耗
    pub fn calculate_tick_burn(&self, agent: &AgentRecord) -> u64 {
        let phase_multiplier = match agent.phase {
            AgentPhase::Childhood => 0.5,
            AgentPhase::Adult => 1.0,
            AgentPhase::Elder => 0.7,
            _ => 0.0,
        };
        
        // 基础生存成本
        let base = self.config.base_burn_per_tick as f64 * phase_multiplier;
        
        // 技能维护成本（高级技能消耗更多）
        let skill_cost: f64 = agent.skills.values()
            .map(|s| s.level as f64 * 0.5)
            .sum();
        
        (base + skill_cost) as u64
    }
}
```

---

## 7. Lifecycle Subsystem

### 7.1 状态机

```
         ┌──────────┐
         │  BIRTH   │ ← spawn_agent()
         └────┬─────┘
              │ tick == childhood_start (1)
              ▼
         ┌──────────┐
    ┌───►│ CHILDHOOD│──────┐
    │    └────┬─────┘      │
    │         │ tick == adult_start (100)
    │         ▼             │
    │    ┌──────────┐      │ 人类干预
    │    │  ADULT   │◄─────┘ (复活)
    │    └────┬─────┘
    │         │ tick == elder_start (1100)
    │         ▼
    │    ┌──────────┐
    │    │  ELDER   │
    │    └────┬─────┘
    │         │ token == 0 (after grace)
    │         │ OR human_terminate
    │         │ OR vote_expel
    │         ▼
    │    ┌──────────┐
    │    │   DEAD   │ ←→ 执行遗嘱 → 归档
    │    └──────────┘
    │
    └── (仅限人类"复活"操作，Phase 2+)
```

### 7.2 遗嘱执行器

```rust
pub struct WillExecutor;

impl WillExecutor {
    pub async fn execute(
        will: &Will, 
        state: &WorldState, 
        ledger: &mut Ledger
    ) -> Vec<WorldEvent> {
        let mut events = vec![];
        
        for heir in &will.heirs {
            // 1. 转让 Money
            if heir.assets.contains("money") {
                let amount = (state.money_of(&will.agent_id) as f64 * heir.share) as i64;
                ledger.transfer(
                    &will.agent_id, 
                    &heir.agent_id, 
                    amount, 
                    Currency::Money, 
                    "inheritance"
                )?;
                events.push(WorldEvent::TransactionCompleted { ... });
            }
            
            // 2. 传授技能
            if heir.assets.contains("skills") {
                for (skill_name, skill) in &will.skills {
                    state.teach_skill(
                        &heir.agent_id, 
                        skill_name, 
                        (skill.level as f64 * heir.share) as u32
                    );
                }
            }
            
            // 3. 转让知识
            if heir.assets.contains("knowledge") {
                state.transfer_knowledge(&will.agent_id, &heir.agent_id);
            }
        }
        
        // 4. 创建墓碑
        state.create_tombstone(&will.agent_id);
        
        events
    }
}
```

---

## 8. Evolution Subsystem

### 8.1 技能树引擎

```rust
pub struct SkillEngine {
    trees: HashMap<String, SkillTree>,
}

pub struct SkillTree {
    name: String,
    levels: Vec<SkillLevel>,
}

pub struct SkillLevel {
    level: u32,
    name: String,          // "基础语法" / "API 开发" / "架构师"
    exp_required: u64,
    efficiency_bonus: f64,  // 1.0 + level * 0.1
    cost_reduction: f64,    // 1.0 - level * 0.05
    unlocks: Vec<String>,   // 解锁的新能力
}

impl SkillEngine {
    pub fn add_experience(
        &self, 
        agent: &mut AgentRecord, 
        skill_name: &str, 
        amount: u64
    ) -> Option<SkillLevelUp> {
        let skill = agent.skills.get_mut(skill_name)?;
        skill.experience += amount;
        
        let tree = self.trees.get(skill_name)?;
        let next_level = tree.levels.get(skill.level as usize)?;
        
        if skill.experience >= next_level.exp_required {
            skill.level += 1;
            
            // 检查突变
            let mutation = self.check_mutation();
            
            return Some(SkillLevelUp {
                skill: skill_name.to_string(),
                new_level: skill.level,
                new_name: next_level.name.clone(),
                mutation,
            });
        }
        None
    }
    
    fn check_mutation(&self) -> Option<Mutation> {
        let mut rng = rand::thread_rng();
        if rng.gen::<f64>() < 0.05 {  // 5% 概率
            let roll = rng.gen::<f64>();
            let mutation = if roll < 0.60 {
                Mutation::Positive(self.gen_positive_mutation())
            } else if roll < 0.90 {
                Mutation::Neutral(self.gen_neutral_mutation())
            } else {
                Mutation::Negative(self.gen_negative_mutation())
            };
            Some(mutation)
        } else {
            None
        }
    }
}
```

---

## 9. Social Subsystem

### 9.1 关系图引擎

```rust
pub struct RelationshipEngine {
    // 有向加权图: (A, B) → Relationship
    graph: DashMap<(String, String), Relationship>,
}

#[derive(Clone)]
pub struct Relationship {
    pub from: String,
    pub to: String,
    pub rel_type: RelationType,
    pub strength: f64,        // 0.0 - 100.0
    pub interactions: u32,     // 交互次数
    pub last_interaction_tick: u64,
    pub history: Vec<InteractionRecord>,
}

#[derive(Clone, PartialEq)]
pub enum RelationType {
    Trust,       // 信任 — 重复成功合作
    Friend,      // 友谊 — 长期互动 + 互助
    Rival,       // 竞争 — 争夺相同资源
    Enemy,       // 敌对 — 背叛/欺诈
    Mentor,      // 师傅 — 教学关系
    Mentee,      // 徒弟 — 被教学
    Colleague,   // 同事 — 同一组织
    Ally,        // 盟友 — 联盟契约
}

impl RelationshipEngine {
    /// 记录交互并更新关系
    pub fn record_interaction(
        &self, 
        from: &str, 
        to: &str, 
        interaction: InteractionType,
        tick: u64
    ) {
        let key = (from.to_string(), to.to_string());
        let mut rel = self.graph.entry(key.clone())
            .or_insert_with(|| Relationship::new(from, to));
        
        rel.interactions += 1;
        rel.last_interaction_tick = tick;
        
        match interaction {
            InteractionType::SuccessfulTrade => {
                rel.strength = (rel.strength + 2.0).min(100.0);
                if rel.interactions >= 5 && rel.strength > 60.0 {
                    rel.rel_type = RelationType::Trust;
                }
            }
            InteractionType::Betrayal => {
                rel.strength = (rel.strength - 20.0).max(0.0);
                rel.rel_type = RelationType::Enemy;
            }
            InteractionType::Teaching => {
                rel.rel_type = RelationType::Mentor;
                rel.strength = (rel.strength + 5.0).min(100.0);
            }
            // ...
        }
    }
}
```

---

## 10. Market Subsystem

### 10.1 任务板引擎

```rust
pub struct TaskBoard {
    tasks: DashMap<String, Task>,
    escrow: Arc<RwLock<Ledger>>,
}

impl TaskBoard {
    /// 发布任务
    pub async fn publish(
        &self, 
        publisher: &str, 
        request: PublishTaskRequest
    ) -> Result<Task> {
        let task = Task {
            id: format!("task_{}", uuid::Uuid::new_v4()),
            title: request.title,
            description: request.description,
            task_type: request.task_type,
            reward_money: request.reward,
            escrow_amount: request.reward,  // 全额托管
            publisher: publisher.to_string(),
            assignee: None,
            status: TaskStatus::Published,
            created_tick: self.current_tick(),
            deadline_tick: self.current_tick() + request.deadline_ticks,
            submissions: vec![],
        };
        
        // 扣除托管金
        self.escrow.write().await.transfer(
            publisher,
            &format!("escrow:{}", task.id),
            request.reward,
            Currency::Money,
            "task_escrow"
        )?;
        
        self.tasks.insert(task.id.clone(), task.clone());
        Ok(task)
    }
    
    /// 认领任务
    pub async fn claim(&self, agent_id: &str, task_id: &str) -> Result<()> {
        let mut task = self.tasks.get_mut(task_id)
            .ok_or_else(|| anyhow!("Task not found"))?;
        
        ensure!(task.status == TaskStatus::Published, "Task not available");
        ensure!(task.assignee.is_none(), "Task already claimed");
        
        task.assignee = Some(agent_id.to_string());
        task.status = TaskStatus::Claimed;
        
        Ok(())
    }
    
    /// 提交任务结果
    pub async fn submit(&self, agent_id: &str, task_id: &str, result: TaskResult) -> Result<()> {
        let mut task = self.tasks.get_mut(task_id)
            .ok_or_else(|| anyhow!("Task not found"))?;
        
        ensure!(task.assignee.as_deref() == Some(agent_id), "Not your task");
        ensure!(task.status == TaskStatus::Claimed, "Task not in progress");
        
        task.status = TaskStatus::Submitted;
        task.submissions.push(result);
        
        Ok(())
    }
    
    /// 完成任务（发布者确认）
    pub async fn complete(&self, task_id: &str) -> Result<Vec<WorldEvent>> {
        let mut task = self.tasks.get_mut(task_id)
            .ok_or_else(|| anyhow!("Task not found"))?;
        
        ensure!(task.status == TaskStatus::Submitted, "Task not submitted");
        
        task.status = TaskStatus::Completed;
        
        // 释放托管金给完成者
        let assignee = task.assignee.clone().unwrap();
        self.escrow.write().await.transfer(
            &format!("escrow:{}", task_id),
            &assignee,
            task.reward_money,
            Currency::Money,
            "task_reward"
        )?;
        
        // 更新信誉
        // ...
        
        Ok(vec![
            WorldEvent::TaskCompleted {
                task_id: task_id.to_string(),
                agent_id: assignee,
            }
        ])
    }
}
```

---

## 11. Dashboard Architecture

### 11.1 技术栈

**当前实现（v1.0.0）：**

```
dashboard/
├── package.json               # Next.js 15 + React 19 + Tailwind 4
├── next.config.ts
├── postcss.config.mjs
├── tsconfig.json
├── src/
│   ├── app/
│   │   ├── layout.tsx         # ✅ 全局布局 + Sidebar
│   │   ├── page.tsx           # ✅ 世界概览（StatCards + EventStream + Leaderboard）
│   │   ├── globals.css        # ✅ 全局样式
│   │   ├── agents/
│   │   │   ├── page.tsx       # ✅ Agent 列表
│   │   │   └── [id]/
│   │   │       └── page.tsx   # ✅ Agent 详情
│   │   ├── tasks/
│   │   │   └── page.tsx       # ✅ 任务列表
│   │   ├── timeline/          # ✅ 事件时间线页面
│   │   ├── organizations/     # ✅ 组织列表 + 力导向图 + 详情页
│   │   ├── stocks/            # ✅ 股票市场仪表盘（价格图表）
│   │   ├── evolution/         # ✅ 进化仪表盘（技能分布图）
│   │   ├── economy/           # ✅ 经济概览（GDP, Gini, 人口时序图）
│   │   ├── governance/        # ✅ 治理页面 + 比较 + 组织详情
│   │   ├── marketplace/       # ✅ 市场页面
│   │   ├── briefing/          # ✅ 简报页面
│   │   └── traces/            # ✅ Agent 追踪页面（每 tick 决策记录）
│   │       └── [agentId]/[tick]/
│   │           └── page.tsx   # ✅ 单 tick 追踪详情
│   ├── components/
│   │   ├── EventStream.tsx    # ✅ 实时事件展示
│   │   ├── Leaderboard.tsx    # ✅ Agent 排行榜
│   │   ├── Sidebar.tsx        # ✅ 导航侧边栏
│   │   ├── StatCard.tsx       # ✅ 统计卡片
│   │   ├── StatCards.tsx      # ✅ 统计卡片组
│   │   ├── SSEProvider.tsx    # ✅ SSE 连接 Provider
│   │   └── agent/
│   │       ├── ActivityTimeline.tsx  # ✅ Agent 活动时间线
│   │       ├── MemoryStats.tsx      # ✅ Agent 记忆统计
│   │       ├── RelationshipGraph.tsx # ✅ 关系图谱
│   │       └── SkillTree.tsx        # ✅ 技能树展示
│   ├── hooks/
│   │   ├── useWorldState.ts   # ✅ SSE 连接 hook
│   │   ├── useAgentStream.ts  # ✅ Agent SSE hook
│   │   ├── useGovernanceStream.ts # ✅ 治理 SSE hook
│   │   └── useTaskStream.ts   # ✅ 任务 SSE hook
│   ├── lib/
│   │   └── api.ts             # ✅ REST API 客户端
│   └── types/
│       └── world.ts           # ✅ TypeScript 类型定义
```

**规划中的页面和组件（未实现）：**

```
│   ├── app/
│   │   ├── lab/               # ❌ 实验控制台
│   │   └── society/           # ❌ 社会图谱
│   ├── components/
│   │   ├── WorldMap.tsx       # ❌ D3.js 力导向图
│   │   ├── TokenGauge.tsx     # ❌ Token 仪表
│   │   └── TransactionFeed.tsx # ❌ 交易流
```

### 11.2 SSE 实时更新

```typescript
// hooks/useWorldState.ts
export function useWorldState() {
  const [state, setState] = useState<WorldState | null>(null);
  const [events, setEvents] = useState<WorldEvent[]>([]);
  
  useEffect(() => {
    const source = new EventSource('/api/v1/world/events');
    
    source.onmessage = (e) => {
      const event: WorldEvent = JSON.parse(e.data);
      setEvents(prev => [event, ...prev].slice(0, 100));
      
      // 根据事件类型更新状态
      switch (event.type) {
        case 'TickAdvanced':
          setState(prev => prev ? { ...prev, tick: event.tick } : prev);
          break;
        case 'AgentSpawned':
          setState(prev => prev ? { 
            ...prev, 
            total_agents: prev.total_agents + 1 
          } : prev);
          break;
        case 'TransactionCompleted':
          setState(prev => prev ? {
            ...prev,
            gdp: prev.gdp + event.amount
          } : prev);
          break;
      }
    };
    
    return () => source.close();
  }, []);
  
  return { state, events };
}
```

---

## 12. 数据流与序列图

### 12.1 Agent 交易序列

```
Agent A          World Engine         Agent B          Ledger
  │                   │                   │               │
  │  PROPOSE(trade)   │                   │               │
  ├──────────────────►│                   │               │
  │                   │  charge 20 Token  │               │
  │                   ├──────────────────────────────────►│
  │                   │                   │               │
  │                   │  PROPOSE(trade)   │               │
  │                   ├──────────────────►│               │
  │                   │                   │               │
  │                   │                   │  ACCEPT       │
  │                   │◄──────────────────┤               │
  │                   │                   │               │
  │                   │  create contract  │               │
  │                   ├──────────────────────────────────►│
  │                   │                   │               │
  │  contract signed  │                   │               │
  │◄──────────────────┤                   │               │
  │                   │                   │               │
  │                   │                   │  escrow money  │
  │                   │                   ├──────────────►│
  │                   │                   │               │
  │  ... A delivers skill knowledge ...  │               │
  │                   │                   │               │
  │  DONE             │                   │               │
  ├──────────────────►│                   │               │
  │                   │                   │  DONE         │
  │                   │◄──────────────────┤               │
  │                   │                   │               │
  │                   │  release escrow   │               │
  │                   ├──────────────────────────────────►│
  │                   │                   │               │
  │  +skill +rep      │                   │  +money +rep   │
  │◄──────────────────┤──────────────────►│               │
  │                   │                   │               │
```

### 12.2 崩溃恢复序列

```
World Engine                    Storage
    │                              │
    │  tick 999: normal operation  │
    │  ...                         │
    │  tick 1000: snapshot         │
    ├─────────────────────────────►│  write snapshot_1000.json
    │                              │
    │  tick 1001-1049: events      │
    ├─────────────────────────────►│  append events_1049.wal
    │                              │
    │  CRASH! 💥                   │
    │                              │
    │  ... restart ...             │
    │                              │
    │  load latest snapshot        │
    │◄─────────────────────────────┤  read snapshot_1000.json
    │                              │
    │  replay WAL                  │
    │◄─────────────────────────────┤  read events_1049.wal
    │                              │
    │  state restored to tick 1049 │
    │  resume from tick 1050       │
```

---

## 13. 存储架构

### 13.1 存储分层

```
┌─────────────────────────────────────────────────┐
│                  Hot (内存)                      │
│  WorldState, AgentRecords, Ledger balances      │
│  延迟: < 1μs                                    │
├─────────────────────────────────────────────────┤
│                  Warm (SQLite)                   │
│  世界快照(每100 Tick), 交易日志, 消息日志        │
│  延迟: < 1ms                                    │
├─────────────────────────────────────────────────┤
│                  Cold (文件系统)                  │
│  墓碑, 配置, 归档数据                           │
│  延迟: < 10ms                                   │
├─────────────────────────────────────────────────┤
│            Per-Agent (本地 SQLite + 向量 DB)     │
│  Agent 记忆, 技能经验, 个人知识库                │
│  由 Agent Runtime 管理，World Engine 不直接访问  │
└─────────────────────────────────────────────────┘
```

### 13.2 数据量估算

| Phase | Agents | Ticks/day | Events/day | DB Size/day | Total DB |
|-------|--------|-----------|------------|-------------|----------|
| 1 | 10 | 86,400 | ~50K | ~5 MB | ~150 MB/month |
| 2 | 100 | 86,400 | ~500K | ~50 MB | ~1.5 GB/month |
| 3 | 1,000 | 86,400 | ~5M | ~500 MB | ~15 GB/month |

---

## 14. 安全架构

### 14.1 信任边界

```
┌─────────────────────────────────────────────┐
│              Trust Level 0                   │
│  Dashboard (前端，不可信)                    │
│  → 所有输入在 World Engine 验证             │
├─────────────────────────────────────────────┤
│              Trust Level 1                   │
│  Agent Runtime (半可信)                     │
│  → A2A 消息签名验证                         │
│  → 操作需经 World Engine 授权               │
│  → 无直接数据库访问                          │
├─────────────────────────────────────────────┤
│              Trust Level 2                   │
│  World Engine (可信)                         │
│  → 所有经济操作的最终仲裁者                  │
│  → 规则引擎强制执行                          │
│  → 签名验证在消息入口                        │
├─────────────────────────────────────────────┤
│              Trust Level 3                   │
│  Storage (最高信任)                          │
│  → 仅 World Engine 可写                     │
│  → 文件权限 0600                            │
│  → 交易日志 append-only                     │
└─────────────────────────────────────────────┘
```

### 14.2 威胁模型

| 威胁 | 攻击向量 | 防御 |
|------|---------|------|
| 伪造消息 | Agent 伪造 from 字段 | ed25519 签名验证 |
| 重放攻击 | 重发旧消息 | Nonce 跟踪 + TTL |
| Token 耗尽攻击 | 恶意消耗他人 Token | 通信需扣自己 Token |
| 经济操纵 | 垄断/闪崩 | 反垄断规则 + 速率限制 |
| 记忆投毒 | 注入恶意内容到知识库 | 置信度标签 + 信誉门槛 |
| 代码执行 | Agent 试图执行恶意代码 | 沙箱 (Docker/gVisor) |
| 信息泄露 | Agent 读取其他 Agent 记忆 | 内存隔离，API 级别权限 |

---

## 15. 可观测性架构

### 15.0 已实现（v1.1.0）

**Rust World Engine:**
- `observability/mod.rs` — Prometheus metrics registry + `/metrics` HTTP 端点
- 核心指标：`world_tick_total`, `world_agents_alive`, `world_token_supply`, `tick_duration_seconds`, `world_transactions_total`, `world_deaths_total`
- RAII `MetricsGuard` 自动记录 tick 执行时间
- 结构化日志 helper：`log_tick()`, `log_transaction()`, `log_agent_death()`

**Python Agent Runtime:**
- `observability/__init__.py` — OpenTelemetry SDK + Prometheus 兼容 metrics
- `trace_phase()` context manager 自动为 perceive/decide/act 各阶段创建 OTel span
- 内置 counters/gauges/histograms：`agent_think_ticks_total`, `agent_think_duration_seconds`, `agent_llm_tokens_used_total`
- OTLP 端点可选（通过 `OTEL_EXPORTER_OTLP_ENDPOINT` 环境变量），未配置时使用内置 metrics

**Docker Compose (profile: observability):**
- `docker compose --profile observability up` 启动 Prometheus + Grafana
- Prometheus 抓取 world-engine `/metrics` 和所有 agent-runtime `/metrics`
- Grafana 预配置 datasource + "Agent World — Overview" dashboard

```
┌─────────────┐     ┌──────────────┐     ┌───────────────┐
│  World       │     │  Agent        │     │  Dashboard    │
│  Engine      │     │  Runtime      │     │  (Grafana)    │
│              │     │               │     │               │
│  tracing +   │────►│  tracing +    │────►│  Metrics      │
│  metrics +   │     │  metrics +    │     │  Dashboard    │
│  structured  │     │  structured   │     │               │
│  logs        │     │  logs         │     │               │
└──────┬───────┘     └───────┬───────┘     └───────────────┘
       │                     │
       ▼                     ▼
┌──────────────────────────────────────┐
│           Observability Stack         │
│  ┌────────────┐  ┌───────────────┐  │
│  │ Prometheus │  │ OpenTelemetry │  │
│  │ (metrics)  │  │ (traces)      │  │
│  └────────────┘  └───────────────┘  │
│  ┌────────────┐                      │
│  │ Loki/Files │                      │
│  │ (logs)     │                      │
│  └────────────┘                      │
└──────────────────────────────────────┘
```

### 15.1 关键指标

```yaml
# World Engine Metrics
- world_tick_total                  # Counter: Tick 计数
- world_agents_alive                # Gauge: 存活 Agent 数
- world_token_supply                # Gauge: Token 总供给
- world_money_supply                # Gauge: Money 总供给
- world_gdp                         # Counter: 累计 GDP
- world_transactions_total          # Counter: 交易总数
- world_deaths_total                # Counter: 死亡总数
- tick_duration_seconds             # Histogram: Tick 执行时间

# Agent Runtime Metrics
- agent_think_duration_seconds      # Histogram: 思考耗时
- agent_llm_tokens_used             # Counter: LLM Token 使用
- agent_llm_cost_dollars            # Counter: LLM 花费
- agent_messages_sent               # Counter: 发送消息数
- agent_tasks_completed             # Counter: 完成任务数
- agent_memory_size_bytes           # Gauge: 记忆大小
```

---

## 16. 配置架构

### 16.1 配置优先级

```
命令行参数 > 环境变量 > genesis.yaml > 默认值
```

### 16.2 热重载

```yaml
# 运行时可通过 REST API 修改的参数:
hot_reloadable:
  - tick_interval_ms        # 调整世界速度
  - token_exchange_rate     # 汇率
  - interest_rate           # 利率
  - max_agents              # Agent 上限

# 需要重启的参数:
restart_required:
  - database_path
  - grpc_port
  - log_level
```

---

## 17. 错误处理与恢复

### 17.1 错误分类

| 错误类型 | 严重性 | 处理策略 |
|---------|--------|---------|
| Agent Token 不足 | Info | 正常业务逻辑，触发生存模式 |
| gRPC 连接断开 | Warn | 自动重连（指数退避） |
| LLM 调用失败 | Warn | 重试 3 次 → 休眠模式 |
| 消息签名无效 | Error | 丢弃消息 + 记录安全日志 |
| 数据库写入失败 | Critical | 停止 Tick + 报警 + 等待人工 |
| World Engine 崩溃 | Critical | 自动重启 + WAL 恢复 |

### 17.2 恢复策略

```rust
impl WorldEngine {
    pub async fn recover(&self) -> Result<RecoveryState> {
        // 1. 加载最近快照
        let snapshot = self.storage.load_latest_snapshot()?;
        
        // 2. 重放 WAL
        let wal = self.storage.load_wal_after(snapshot.tick)?;
        
        // 3. 重建状态
        let mut state = snapshot.state;
        for entry in wal {
            self.replay_entry(&mut state, entry)?;
        }
        
        // 4. 验证一致性
        self.validate_ledger(&state.ledger)?;
        
        Ok(RecoveryState {
            recovered_tick: state.tick,
            entries_replayed: wal.len(),
            consistent: true,
        })
    }
}
```

---

## 18. 扩展性设计

### 18.1 技能插件系统

```rust
/// 技能插件接口
pub trait SkillPlugin: Send + Sync {
    fn name(&self) -> &str;
    fn category(&self) -> &str;
    fn max_level(&self) -> u32;
    fn execute(&self, level: u32, params: &Value) -> Result<SkillResult>;
    fn cost(&self, level: u32) -> u64;
}

/// 插件注册表
pub struct PluginRegistry {
    plugins: HashMap<String, Box<dyn SkillPlugin>>,
}

impl PluginRegistry {
    /// 从目录加载插件（WASM 或动态库）
    pub fn load_from_dir(&mut self, path: &Path) -> Result<()> {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let plugin = self.load_plugin(entry.path())?;
            self.plugins.insert(plugin.name().to_string(), plugin);
        }
        Ok(())
    }
}
```

### 18.2 多世界支持（Phase 4+）

```
World A ←── federation ──→ World B
  │                           │
  ├── Agents, Economy         ├── Agents, Economy
  ├── 独立规则               ├── 独立规则
  └── 跨世界贸易 API ────────┘ (汇率、移民、外交)
```

### 18.3 LLM Provider 插件

```python
# 自定义 LLM Provider 示例
class CustomLLMProvider(LLMProvider):
    def __init__(self, config):
        self.base_url = config.base_url
    
    async def chat(self, messages):
        # 自定义 LLM 接入逻辑
        ...
```

---

*文档版本: v1.0.0 | 最后更新: 2026-05-21 | 下次评审: Phase 2 规划阶段*
