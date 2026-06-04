1|# Agent World — Architecture Design Document
2|
3|> **版本**: v1.1.0 | **日期**: 2026-06-04 | **状态**: current
4|> 与 [DESIGN.md](DESIGN.md)（产品规格）和 [ROADMAP.md](ROADMAP.md)（路线图）配合阅读
5|
6|---
7|
8|## 实现状态总览
9|
10|本文档描述 Agent World 的**目标架构**。下表标注各子系统的实现状态：
11|
12|| 子系统 | 模块 | 状态 | 说明 |
13||--------|------|------|------|
14|| World Engine | economy/ | **已实现** | token_burn、escrow、reward、task 均有完整实现和测试 |
15|| World Engine | world/ | **已实现** | EventBus（139 种事件类型）、enums、state、SSE endpoint |
16|| World Engine | api.rs | **已实现** | Axum REST API，包含 tasks、WAL、organizations、governance、stocks、banking 端点 |
17|| World Engine | config/ | **已实现** | genesis.yaml with economy, lifecycle, evolution parameters |
18|| World Engine | engine/ | **已实现** | WorldState、CultureStore；Tick 调度器在 world/scheduler.rs 中实现 |
19|| World Engine | lifecycle/ | **已实现** | LifecycleMachine: Birth→Childhood→Adulthood→Elder→Death |
20|| World Engine | rules/ | **已实现** | 3 条规则：TokenConsumption（R001）、DeathJudgment（R002）、NewbieProtection（R003） |
21|| World Engine | social/ | **已实现** | Trust network (economy/trust.rs), mentorship (economy/mentorship.rs), inheritance (economy/inheritance.rs) — 位于 economy/ 模块内 |
22|| World Engine | evolution/ | **已实现** | Skill trees, mutations, natural selection, EvolutionSubsystem |
23|| World Engine | market/ | **已实现** | Banking system, stock market with order book |
24|| World Engine | organization/ | **已实现** | Organizations (Company/Guild/Alliance/University), governance, charters, members |
25|| World Engine | a2a/ | **已实现** | gRPC 服务器（server.rs, grpc.rs, service.rs）、Discovery、Router、AgentRegistry、ClientPool |
26|| World Engine | storage/ | **已实现** | persistence/ 模块：SQLite 快照持久化（persistence/sqlite.rs, 472 行） |
27|| World Engine | wal/ | **已实现** | Write-Ahead Log：CRC32 校验、崩溃恢复、快照、1000 条自动轮转 |
28|| World Engine | plugin/ | **已实现** | 第三方扩展 API：hooks、subsystems、event handlers、权限管理 |
| World Engine | world/map/ | **已实现** | 六角格地图（6种地形）、资源节点、建筑系统 |
| World Engine | observability/ | **已实现** | Prometheus metrics（/metrics 端点）、计数器/直方图/仪表盘、结构化日志；OpenTelemetry 集成待 Phase 2 |
29|| Agent Runtime | core/ | **已实现** | think_loop、decide、act 均有完整实现和测试 |
30|| Agent Runtime | survival/ | **已实现** | 5 模式生存本能，11 种紧急行动 |
31|| Agent Runtime | memory/ | **已实现** | WorkingMemory（FIFO）、ShortTermMemory（SQLite）、LongTermMemory（SQLite）均已实现；嵌入/向量记忆也有实现 |
32|| Agent Runtime | models/ | **已实现** | AgentState Pydantic 模型、enums、skill、personality、phase_abilities、values |
33|| Agent Runtime | llm/ | **已实现** | OpenAI、Anthropic、Ollama 三个 provider |
34|| Agent Runtime | crypto/ | **已实现** | Ed25519 密钥生成、签名、验证、Nonce 防重放、密钥注册表 |
35|| Agent Runtime | skills/ | **已实现** | SkillRegistry + SkillExecutor + 4 个内置技能（coding, research, teaching, trading） |
36|| Agent Runtime | a2a/ | **已实现** | gRPC 客户端（client.py, batch_client.py, world_client.py）、消息路由、Perception Provider |
37|| Agent Runtime | tools/ | **已实现** | Tool 抽象基类 + ToolRegistry + 3 个内置工具（http_request, file_ops, code_exec）+ 74 个单元测试 |
38|| Agent Runtime | config/ | **已实现** | TOML/YAML 配置文件加载与合并（config.py, 291 行） |
39|| Agent Runtime | main.py | **已实现** | 完整 CLI 入口（__main__.py, 1358 行）：spawn、密钥生成/加载、注册、gRPC 连接、健康检查 |
40|| Dashboard | 全部 | **已实现** | Pages: overview, agents, tasks, timeline, organizations, stocks, evolution, economy, governance, marketplace, briefing, traces; SSE 实时数据 |
41|| Protocol | a2a.proto | **已实现** | 定义了 Discover、SendMessage、StreamMessages |
42|| Protocol | discovery.proto | **已实现** | 发现功能集成到 world_engine.proto（Register/Spawn/Heartbeat RPCs）；discovery.proto 未单独定义 |
43|| Agent Runtime | lifecycle/ | **已实现** | LifecycleSyncService：阶段同步、转换守卫、死亡处理（lifecycle/__init__.py, 338 行） |
44|| Agent Runtime | context/ | **已实现** | ContextEnginePipeline：token 预算、优先级驱动的上下文组装（context/engine.py, 613 行） |
45|| Agent Runtime | reflection/ | **已实现** | 反思引擎：记忆反思、自我评估、策略调整（reflection/ 模块） |
46|| Agent Runtime | social/ | **已实现** | 10 个社会模块：文化冲突/扩散、模仿、群体信任、语言实验等 |
47|| Agent Runtime | organization/ | **已实现** | 组织形成、治理、招募、规则演化（organization/ 模块） |
48|| Agent Runtime | tracing/ | **已实现** | TickTrace 采集、存储、推送、涌现指标、交互图谱 |
49|| Agent Runtime | experiment/ | **已实现** | A/B 实验框架、可复现性、实验报告 |
50|| Agent Runtime | export/ | **已实现** | 行为日志、经济数据、网络数据导出 |
| Agent Runtime | emotion/ | **已实现** | 情绪引擎：性格调制、情绪状态（6种基本情绪）、时间衰减、决策影响 |
| Agent Runtime | diary/ | **已实现** | Agent 日记系统：SQLite 存储、FTS 全文搜索、每 tick 主观体验记录 |
51|| Agent Runtime | sdk/ | **已实现** | SDK 客户端（sdk/client.py, 236 行） |
52|| World Engine | persistence/ | **已实现** | SQLite 快照持久化层（persistence/sqlite.rs, 472 行） |
53|| World Engine | time_capsule/ | **已实现** | 时间胶囊系统（time_capsule.rs, 665 行） |
54|| World Engine | tracing/ | **已实现** | TickTrace 数据模型（tracing.rs） |
55|| World Engine | grpc_pool/ | **已实现** | gRPC 连接池管理 |
56|| World Engine | world/scheduler | **已实现** | Tick 调度器（world/scheduler.rs, 153 行），可配置间隔、优雅关闭 |
57|| World Engine | world/engine | **已实现** | WorldState 世界状态容器（world/engine.rs, 472 行） |
58|
59|> 标记为**已实现**的模块包含完整的功能代码和单元测试。标记为**部分**的模块有部分功能。标记为**占位**的模块仅有空结构体。标记为**未实现**的模块完全没有代码。
60|
61|---
62|
63|## 目录
64|
65|1. [系统架构总览](#1-系统架构总览)
66|2. [部署架构](#2-部署架构)
67|3. [World Engine 详细设计](#3-world-engine-详细设计)
68|4. [Agent Runtime 详细设计](#4-agent-runtime-详细设计)
69|5. [A2A Protocol 详细设计](#5-a2a-protocol-详细设计)
70|6. [Economy Subsystem](#6-economy-subsystem)
71|7. [Lifecycle Subsystem](#7-lifecycle-subsystem)
72|8. [Evolution Subsystem](#8-evolution-subsystem)
73|9. [Social Subsystem](#9-social-subsystem)
74|10. [Market Subsystem](#10-market-subsystem)
75|11. [Dashboard Architecture](#11-dashboard-architecture)
76|12. [数据流与序列图](#12-数据流与序列图)
77|13. [存储架构](#13-存储架构)
78|14. [安全架构](#14-安全架构)
79|15. [可观测性架构](#15-可观测性架构)
80|16. [配置架构](#16-配置架构)
81|17. [错误处理与恢复](#17-错误处理与恢复)
82|18. [扩展性设计](#18-扩展性设计)
83|
84|---
85|
86|## 1. 系统架构总览
87|
88|### 1.1 架构风格
89|
90|Agent World 采用 **微内核 + 插件式** 架构：
91|
92|- **微内核（World Engine）**: 只负责世界状态管理、Tick 调度、规则执行
93|- **外围服务**: 经济、社会、进化、市场各自独立模块，通过事件总线通信
94|- **Agent Runtime**: 独立进程，通过 A2A 协议与内核和其他 Agent 通信
95|- **Dashboard**: 纯前端 + SSE，无服务端渲染
96|
97|### 1.2 系统上下文图
98|
99|```
100|                        ┌─────────────┐
101|                        │   Human     │
102|                        │ (Observer/  │
103|                        │  Investor/  │
104|                        │  Creator)   │
105|                        └──────┬──────┘
106|                               │ REST + SSE
107|                               ▼
108|┌──────────┐  gRPC   ┌─────────────────┐  gRPC   ┌──────────┐
109|│  Agent   │◄────────►│                 │◄────────►│  Agent   │
110|│Runtime A │         │   World Engine   │         │Runtime B │
111|│(Python)  │         │   (Rust)         │         │(Python)  │
112|└────┬─────┘         │                  │         └──────────┘
113|     │               │  ┌───────────┐  │               │
114|     │               │  │ Event Bus │  │               │
115|     │               │  └─────┬─────┘  │               │
116|     │               │        │         │               │
117|     │               │  ┌─────▼─────┐  │               │
118|     │               │  │  Subsystems │ │               │
119|     │               │  │ ┌────────┐ │ │               │
120|     │               │  │ │Economy │ │ │               │
121|     │               │  │ ├────────┤ │ │               │
122|     │               │  │ │Social  │ │ │               │
123|     │               │  │ ├────────┤ │ │               │
124|     │               │  │ │Lifecycle│ │ │               │
125|     │               │  │ ├────────┤ │ │               │
126|     │               │  │ │Evolution│ │ │               │
127|     │               │  │ ├────────┤ │ │               │
128|     │               │  │ │Market  │ │ │               │
129|     │               │  │ └────────┘ │ │               │
130|     │               │  └───────────┘  │               │
131|     │               └─────────────────┘               │
132|     │                      │                          │
133|     │               ┌──────▼──────┐                    │
134|     │               │  Storage    │                    │
135|     │               │  (SQLite +  │                    │
136|     │               │  Vector DB) │                    │
137|     │               └─────────────┘                    │
138|     │                                                  │
139|     └──────────── A2A P2P (Direct) ◄───────────────────┘
140|            (绕过 World Engine 的 Agent 直连通道)
141|```
142|
143|### 1.3 组件职责矩阵
144|
145|| 组件 | 语言 | 进程模型 | 核心职责 | 无状态 |
146||------|------|---------|---------|--------|
147|| World Engine | Rust | 单进程 | Tick、状态、规则、路由 | ❌ 有状态 |
148|| Agent Runtime | Python | 1进程/Agent | 思考、记忆、决策、行动 | ❌ 有状态 |
149|| A2A Router | Rust (World Engine 内) | 共享进程 | 消息路由、签名验证 | ✅ 无状态 |
150|| Dashboard | React + Next.js | 独立进程 | 展示、交互 | ✅ 无状态 |
151|| Storage | SQLite + 各 Agent 本地 | 共享/独立 | 持久化 | ❌ |
152|
153|### 1.4 通信矩阵
154|
155|| 源 → 目 | 协议 | 数据格式 | 方向 | 延迟目标 |
156||---------|------|---------|------|---------|
157|| Agent → World Engine | gRPC (HTTP/2) | Protobuf | 请求/响应 + 流 | < 10ms |
158|| World Engine → Agent | gRPC (server stream) | Protobuf | 推送 | < 50ms |
159|| Agent → Agent (经由 WE) | gRPC → 路由 → gRPC | Protobuf | 异步消息 | < 100ms |
160|| Agent → Agent (直连) | HTTP/2 | JSON/Protobuf | 异步消息 | < 50ms |
161|| Dashboard → World Engine | REST + SSE | JSON | 请求 + 推送 | < 200ms |
162|| Agent Runtime → LLM | HTTP REST | JSON | 请求/响应 | 1-5s |
163|
164|---
165|
166|## 2. 部署架构
167|
168|### 2.1 开发环境（Phase 1）
169|
170|```
171|┌─────────────────────────────────────────────────┐
172|│              开发者机器 (localhost)                │
173|│                                                 │
174|│  ┌─────────────┐   ┌─────────────┐              │
175|│  │ World Engine│   │ Agent A     │              │
176|│  │ (cargo run) │◄─►│ (python)    │              │
177|│  │  :50051     │   │  :50052     │              │
178|│  └──────┬──────┘   └─────────────┘              │
179|│         │            ┌─────────────┐              │
180|│         │            │ Agent B     │              │
181|│         ├───────────►│ (python)    │              │
182|│         │            │  :50053     │              │
183|│         │            └─────────────┘              │
184|│         │                                        │
185|│  ┌──────▼──────┐                                 │
186|│  │ SQLite DB   │   ┌─────────────┐               │
187|│  │ world.db    │   │ Dashboard   │               │
188|│  └─────────────┘   │ (next dev)  │               │
189|│                    │  :3001       │               │
190|│                    └─────────────┘               │
191|└─────────────────────────────────────────────────┘
192|```
193|
194|### 2.2 Docker Compose（生产/演示）
195|
196|> **注意**：以下为简化示意。实际 `docker-compose.yml` 包含 10 个 Agent 实例、
197|> 健康检查、YAML anchors（`x-agent-common`）、profiles（`ci`、`observability`、`local-llm`）、
198|> Ollama/Prometheus/Grafana 可选服务等。完整配置见项目根目录 `docker-compose.yml`。
199|
200|```yaml
201|# docker-compose.yml — 简化示意（完整版见项目根目录）
202|services:
203|  world-engine:
204|    build:
205|      context: .
206|      dockerfile: world-engine/Dockerfile
207|    ports:
208|      - "${ENGINE_PORT:-8080}:${ENGINE_PORT:-8080}"   # REST API
209|      - "${GRPC_PORT:-50051}:${GRPC_PORT:-50051}"     # gRPC
210|    volumes:
211|      - world-data:/data
212|      - ./config:/config:ro
213|    environment:
214|      - RUST_LOG=info
215|      - GENESIS_CONFIG=/config/genesis.yaml
216|
217|  agent-runtime:
218|    build: ./agent-runtime
219|    depends_on:
220|      world-engine:
221|        condition: service_healthy
222|    environment:
223|      - WORLD_ENGINE_URL=http://world-engine:${ENGINE_PORT:-8080}
224|      - LLM_PROVIDER=openai  # or: ollama, anthropic
225|      - LLM_MODEL=gpt-4o-mini
226|
227|  dashboard:
228|    build: ./dashboard
229|    ports:
230|      - "${DASHBOARD_PORT:-3001}:3000"
231|    depends_on:
232|      - world-engine
233|    environment:
234|      - NEXT_PUBLIC_API_URL=http://localhost:${ENGINE_PORT:-8080}
235|
236|volumes:
237|  world-data:
238|```
239|
240|### 2.3 Kubernetes（Phase 3+）
241|
242|```
243|Namespace: agent-world
244|├── Deployment: world-engine (1 replica, StatefulSet)
245|├── Deployment: agent-runtime (replicas: N, 1 pod/agent)
246|│   └── ConfigMap per agent (personality, skills seed)
247|├── Deployment: dashboard (2 replicas)
248|├── Service: world-engine-grpc (ClusterIP)
249|├── Service: world-engine-rest (ClusterIP)
250|├── Service: dashboard (LoadBalancer)
251|├── PVC: world-data (SQLite → 未来 PostgreSQL)
252|└── PVC per agent: agent-memory (vector DB data)
253|```
254|
255|---
256|
257|## 3. World Engine 详细设计
258|
259|### 3.1 模块图
260|
261|**当前实际文件结构（v1.0.0）：**
262|
263|```
264|world-engine/
265|├── src/
266|│   ├── main.rs                  # ✅ 入口：加载配置 → 启动 HTTP + gRPC 服务器
267|│   ├── lib.rs                   # ✅ 模块重导出
268|│   ├── api.rs                   # ✅ Axum REST API（tasks, WAL, orgs, governance, banking, stocks, SSE）
269|│   ├── config.rs                # ✅ Genesis YAML 配置加载
270|│   ├── lifecycle.rs             # ✅ LifecycleMachine: Birth→Childhood→Adulthood→Elder→Death
271|│   ├── rules.rs                 # ✅ 3 条规则（R001-R003），RuleRegistry + RuleContext
272|│   ├── grpc_pool.rs             # ✅ gRPC 连接池
273|│   ├── time_capsule.rs          # ✅ 时间胶囊
274|│   ├── tracing.rs               # ✅ TickTrace 数据模型
275|│   ├── engine/                  # ✅ 引擎模块
276|│   │   ├── mod.rs               # ✅ WorldState + CultureStore
277|│   │   ├── state.rs             # ✅ 世界状态（DashMap，Agent/Org/Task 注册）
278|│   │   └── culture.rs           # ✅ 组织文化向量 + 区域文化集群
279|│   ├── economy/
280|│   │   ├── mod.rs               # ✅ 模块重导出
281|│   │   ├── token_burn.rs        # ✅ Token 消耗引擎（阶段乘数 + 技能成本）
282|│   │   ├── escrow.rs            # ✅ 托管管理器（完整生命周期）
283|│   │   ├── reward.rs            # ✅ 奖励分配（2% 平台费 + XP + 声望）
284|│   │   ├── task.rs              # ✅ 任务市场（状态机 + 托管集成）
285|│   │   ├── ledger.rs            # ✅ 双式记账账本
286|│   │   ├── banking.rs           # ✅ 银行系统（储蓄/支票账户、贷款、央行操作）
287|│   │   ├── stock_market.rs      # ✅ 股票市场（发行、IPO、订单簿、撮合引擎、分红）
288|│   │   ├── marketplace.rs       # ✅ 自由市场
289|│   │   ├── trust.rs             # ✅ 信任网络
290|│   │   ├── mentorship.rs        # ✅ 导师关系
291|│   │   ├── inheritance.rs       # ✅ 遗产继承
292|│   │   └── reputation.rs        # ✅ 信誉系统
293|│   ├── world/
294|│   │   ├── mod.rs               # ✅ 模块重导出
295|│   │   ├── enums.rs             # ✅ Currency, AgentPhase, DeathReason
296|│   │   ├── event.rs             # ✅ 139 种 WorldEvent 变体
297|│   │   ├── state.rs             # ✅ EventBus（tokio broadcast）
298|│   │   ├── agent.rs             # ✅ AgentRecord 数据结构
299|│   │   ├── genesis.rs           # ✅ GenesisConfig 加载
300|│   │   ├── engine.rs            # ✅ WorldState 世界状态容器
301|│   │   ├── scheduler.rs         # ✅ Tick 调度器（可配置间隔、优雅关闭）
302|│   │   ├── subsystem.rs         # ✅ Subsystem trait
303|│   │   ├── subsystems.rs        # ✅ 子系统注册
304|│   │   ├── discovery.rs         # ✅ Agent 发现
305|│   │   ├── seeder.rs            # ✅ Agent 种子数据
306|│   │   ├── intervention.rs      # ✅ 人类干预
307|│   │   └── tick_profiler.rs     # ✅ Tick 性能分析
308|│   ├── organization/
309|│   │   ├── mod.rs               # ✅ 组织模块（Company/Guild/Alliance/University）
310|│   │   ├── org.rs               # ✅ 组织 CRUD + 生命周期
311|│   │   ├── charter.rs           # ✅ 章程管理
312|│   │   ├── governance.rs        # ✅ 治理系统（提案、投票、利润分配）
313|│   │   ├── governance_metrics.rs # ✅ 治理指标
314|│   │   ├── members.rs           # ✅ 成员管理
315|│   │   ├── treasury.rs          # ✅ 组织金库
316|│   │   ├── leadership.rs        # ✅ 领导权管理
317|│   │   ├── competition.rs       # ✅ 组织间竞争
318|│   │   ├── diplomacy.rs         # ✅ 外交关系
319|│   │   └── rule_engine.rs       # ✅ 组织规则引擎
320|│   ├── evolution/
321|│   │   ├── mod.rs               # ✅ 进化模块
322|│   │   ├── skill_tree.rs        # ✅ 分支技能树（4 根分支 10 技能）
323|│   │   ├── mutation.rs          # ✅ 技能突变引擎（5% 概率/1000 tick）
324|│   │   ├── selection.rs         # ✅ 自然选择（多维适应度评分）
325|│   │   └── subsystem.rs         # ✅ EvolutionSubsystem（集成到 tick 循环）
326|│   ├── wal/
327|│   │   ├── mod.rs               # ✅ WAL（CRC32、崩溃恢复、快照、1000 条轮转）
328|│   │   └── crc.rs               # ✅ CRC32（ISO 3309 查表实现）
329|│   ├── a2a/                     # ✅ gRPC A2A 服务
330|│   │   ├── mod.rs               # ✅ 模块重导出
331|│   │   ├── server.rs            # ✅ gRPC 服务器（A2aService trait 实现）
332|│   │   ├── grpc.rs              # ✅ gRPC 服务注册
333|│   │   ├── service.rs           # ✅ 消息处理逻辑（Discover, Send, Stream）
334|│   │   ├── discovery.rs         # ✅ AgentRegistry 发现服务
335|│   │   ├── router.rs            # ✅ MessageRouter 消息路由
336|│   │   ├── registry.rs          # ✅ Agent 注册表
337|│   │   └── client_pool.rs       # ✅ gRPC 客户端连接池
338|│   └── persistence/             # ✅ 持久化层
339|│       ├── mod.rs               # ✅ 持久化接口
340|│       └── sqlite.rs            # ✅ SQLite 快照存储
341|│   └── observability/           # ✅ 可观测性
342|│       └── mod.rs               # ✅ Prometheus metrics + /metrics 端点 + 结构化日志
343|```
344|
345|**规划中的模块（未实现）：**
346|
347|> tools/ 和 observability/ 已全部实现，无剩余未实现模块。
348|
349|> ✅ = 已实现（含测试） | ⏳ = 占位符 | ❌ = 未实现
350|
351|### 3.2 核心数据结构
352|
353|```rust
354|/// 世界全局状态（内存中，定期持久化）
355|pub struct WorldState {
356|    pub tick: AtomicU64,
357|    pub config: GenesisConfig,
358|    pub agents: DashMap<String, AgentRecord>,
359|    pub organizations: DashMap<String, Organization>,
360|    pub tasks: DashMap<String, Task>,
361|    pub knowledge: DashMap<String, KnowledgeEntry>,
362|    pub ledger: Arc<RwLock<Ledger>>,
363|    pub relationships: DashMap<(String, String), Relationship>,
364|    pub event_tx: broadcast::Sender<WorldEvent>,
365|}
366|
367|/// Agent 记录 — 规范类型（world/agent.rs）
368|///
369|/// 所有子系统（经济、进化、持久化、快照）统一使用此类型。
370|/// API 层 DTO（api.rs 中 AgentRecord）通过 From/Into 转换。
371|pub struct AgentRecord {
372|    pub id: Uuid,
373|    pub name: String,
374|    pub phase: AgentPhase,
375|    pub tokens: u64,
376|    pub skills: HashMap<String, SkillRecord>,
377|    /// 人格向量，JSON 字符串（由 Python 侧管理 schema）
378|    #[serde(default)]
379|    pub personality: String,
380|    /// 已成功完成的任务数
381|    #[serde(default)]
382|    pub tasks_completed: u32,
383|    /// 已尝试的任务数（claimed 或 started）
384|    #[serde(default)]
385|    pub tasks_attempted: u32,
386|}
387|
388|/// API 层 Agent 记录 — DTO（api.rs）
389|///
390|/// REST API 端点使用的扁平化视图，字段与 JSON 响应一一对应。
391|pub struct AgentRecord {  // api.rs 版本
392|    pub id: String,
393|    pub name: String,
394|    pub phase: String,
395|    pub tokens: u64,
396|    pub money: u64,
397|    pub alive: bool,
398|    pub ticks_survived: u64,
399|    #[serde(default)]
400|    pub personality: String,
401|    #[serde(default)]
402|    pub parent_ids: Vec<String>,
403|    #[serde(default)]
404|    pub generation: u32,
405|    #[serde(default)]
406|    pub skills: HashMap<String, u32>,
407|}
408|
409|/// 运行时 Agent 实体 — world/agent.rs
410|///
411|/// AgentRegistry 管理的运行时 Agent 实体，包含模拟中需要的状态。
412|pub struct Agent {
413|    pub id: String,
414|    pub name: String,
415|    pub phase: AgentPhase,
416|    pub money: u64,
417|    pub tokens: u64,
418|    pub reputation: f64,
419|    pub skills: HashMap<String, u64>,
420|    pub alive: bool,
421|    pub age: u64,
422|    pub created_at: String,
423|}
424|
425|/// 技能记录 — world/agent.rs
426|pub struct SkillRecord {
427|    pub name: String,
428|    pub level: u32,
429|    pub experience: f64,
430|}
431|
432|/// 双式记账账本
433|pub struct Ledger {
434|    pub entries: Vec<LedgerEntry>,
435|    pub balances: HashMap<String, i64>,  // account_id → balance
436|    pub token_supply: i64,
437|    pub money_supply: i64,
438|}
439|
440|/// 账本条目（不可变）
441|pub struct LedgerEntry {
442|    pub id: String,
443|    pub debit_account: String,     // 从谁扣
444|    pub credit_account: String,    // 加给谁
445|    pub amount: u64,
446|    pub currency: Currency,        // Token / Money
447|    pub entry_type: LedgerType,    // Exchange/Task/Interest/Tax/...
448|    pub description: String,
449|    pub tick: u64,
450|    pub reference_id: Option<String>, // 关联的任务/消息 ID
451|}
452|
453|/// 世界事件（广播给所有订阅者）
454|///
455|/// WorldEvent 枚举共 139 个变体，按子系统分组。
456|/// 完整定义详见 `world-engine/src/world/event.rs`。
457|///
458|/// 分组概览：
459|///
460|/// | 分组 | 变体数 | 代表变体 |
461|/// |------|--------|---------|
462|/// | 核心 / Tick | 5 | TickAdvanced, AgentSpawned, AgentDying, AgentDied, AgentRescued |
463|/// | 经济 / 交易 | 4 | TransactionCompleted, BalanceChanged, PhaseChanged, RuleViolated |
464|/// | 快照 | 1 | SnapshotTaken |
465|/// | 托管 (Escrow) | 5 | EscrowCreated, EscrowClaimed, EscrowReleased, EscrowRefunded, EscrowFrozen |
466|/// | 任务市场 | 7 | TaskCreated, TaskClaimed, TaskStarted, TaskSubmitted, TaskReviewed, TaskCompleted, TaskExpired |
467|/// | 奖励 / 信誉 | 2 | RewardDistributed, ReputationChanged |
468|/// | Agent 注册 / 心跳 | 4 | AgentRegistered, AgentDeregistered, AgentHeartbeat, ConfigReloaded |
469|/// | 知识市场 | 4 | KnowledgeListed, KnowledgeDelisted, KnowledgePurchased, KnowledgeRated |
470|/// | 信任 / 导师 | 6 | TrustChanged, TrustInteraction, MentorshipEstablished, MentorshipProgress, MentorshipCompleted |
471|/// | 遗产 / 时间胶囊 | 3 | WillCreated, InheritanceTriggered, TimeCapsuleBriefing |
472|/// | 组织 (Org) | 5 | OrgCreated, OrgMemberJoined, OrgMemberLeft, OrgDissolved, OrgInactivated |
473|/// | 股票市场 | 5 | StockIssued, StockIpo, StockTraded, StockTransferred, StockDividend |
474|/// | 组织治理 (Organization) | 9 | OrganizationCreated/Dissolved/MemberJoined/Left, ProposalCreated/VotingStarted/Voted/Executed/Rejected, ArgumentAdded |
475|/// | 银行系统 | 9 | BankAccountOpened, BankDeposit, BankWithdrawal, LoanApplied/Approved/Disbursed/Repayment, BankRateAdjusted, MoneyMinted, BadDebtWrittenOff |
476|/// | 进化 / 技能 | 6 | SkillLevelUp, SkillMutated, FitnessEvaluated, OrgResourceConflict, OrgTerritoryClaimed, OrgFormationSuggested |
477|/// | 金库 / 领导 | 4 | TaxCollected, TreasuryDistributed, LeadershipElectionStarted, LeadershipChanged |
478|/// | 外交 (Diplomacy) | 4 | TreatyProposed, TreatySigned, TreatyBroken, RelationChanged |
479|/// | 后代突变 / 建筑 | 7 | OffspringMutated, BuildingConstructed/Completed/Damaged/Destroyed/Demolished/Maintained/Upgraded |
480|/// | 投资 | 4 | InvestmentProductCreated, InvestmentPurchased, InvestmentSold, InvestmentDividend |
481|/// | 跨世界联邦 (Federation) | 16 | ForeignWorldDiscovered/Deregistered, DiplomaticRelationsEstablished/StatusChanged, CrossWorldRelation/Treaty*, Sanctions*, DiplomaticTiesSevered, WarDeclared, PeaceProposed/Established |
482|/// | 迁移 (Migration) | 8 | MigrationSubmitted/Approved/Rejected/Completed/Cancelled, AgentEmigrated/Immigrated |
483|/// | 软规则 | 4 | SoftRuleProposed, SoftRuleActivated, SoftRuleExpired, SoftRuleRepealed |
484|/// | 工具市场 | 4 | ToolListed, ToolDelisted, ToolPurchased, ToolRented |
485|/// | 预言机 / 赏金 | 2 | OracleDelivered, BountyPublished |
486|/// | 多智能体协作 | 6 | CoordinationTaskCreated/AgentJoined/AgentSubmitted/Completed/Cancelled/Expired |
487|/// | 社交动态 | 4 | FeedPostCreated/liked, FeedCommentCreated/Liked |
488|#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
489|#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
490|#[non_exhaustive]
491|pub enum WorldEvent {
492|    // 示例变体 — 完整列表见 world/engine/src/world/event.rs
493|    TickAdvanced { tick: u64 },
494|    AgentSpawned { agent_id: String, name: String },
495|    AgentDied { agent_id: String, reason: DeathReason },
496|    TransactionCompleted { from: String, to: String, amount: u64, currency: Currency },
497|    TaskCompleted { task_id: String },
498|    // ... 以及其余 134 个变体
499|}
500|```
501|
502|### 3.3 Tick 调度器
503|
504|```rust
505|/// Tick 调度器 — 世界的核心心跳
506|pub struct Scheduler {
507|    interval: Duration,
508|    state: Arc<WorldState>,
509|    subsystems: Vec<Box<dyn Subsystem>>,
510|}
511|
512|#[async_trait]
513|pub trait Subsystem: Send + Sync {
514|    /// 每个 Tick 调用一次
515|    async fn on_tick(&self, tick: u64, state: &WorldState) -> Result<Vec<WorldEvent>>;
516|    
517|    /// 子系统名称
518|    fn name(&self) -> &str;
519|}
520|
521|impl Scheduler {
522|    pub async fn run(&self) {
523|        let mut ticker = tokio::time::interval(self.interval);
524|        
525|        loop {
526|            ticker.tick().await;
527|            let tick = self.state.tick.fetch_add(1, Ordering::SeqCst) + 1;
528|            
529|            // 1. 执行所有子系统
530|            for subsystem in &self.subsystems {
531|                match subsystem.on_tick(tick, &self.state).await {
532|                    Ok(events) => {
533|                        for event in events {
534|                            let _ = self.state.event_tx.send(event);
535|                        }
536|                    }
537|                    Err(e) => {
538|                        tracing::error!("Subsystem {} error at tick {}: {}", 
539|                            subsystem.name(), tick, e);
540|                    }
541|                }
542|            }
543|            
544|            // 2. Token 消耗（所有存活 Agent）
545|            self.burn_tokens(tick).await;
546|            
547|            // 3. 死亡判定
548|            self.check_deaths(tick).await;
549|            
550|            // 4. 快照（每 100 Tick）
551|            if tick % 100 == 0 {
552|                self.snapshot(tick).await;
553|            }
554|            
555|            // 5. 通胀检查（每 864 Tick = 1 世界日）
556|            if tick % 864 == 0 {
557|                self.inflation_check(tick).await;
558|            }
559|            
560|            // 6. 广播 Tick 事件
561|            let _ = self.state.event_tx.send(WorldEvent::TickAdvanced { tick });
562|        }
563|    }
564|}
565|```
566|
567|### 3.4 事件总线
568|
569|```rust
570|/// 事件总线 — 解耦子系统通信
571|/// 
572|/// 发布者: Subsystems, gRPC handlers
573|/// 订阅者: Dashboard SSE, Agent 推送, 日志
574|pub struct EventBus {
575|    tx: broadcast::Sender<WorldEvent>,
576|}
577|
578|impl EventBus {
579|    /// 发布事件（fire-and-forget）
580|    pub fn publish(&self, event: WorldEvent) {
581|        // 如果没有订阅者，静默丢弃
582|        let _ = self.tx.send(event);
583|    }
584|    
585|    /// 订阅事件流
586|    pub fn subscribe(&self) -> broadcast::Receiver<WorldEvent> {
587|        self.tx.subscribe()
588|    }
589|    
590|    /// 带 filter 的订阅
591|    pub fn subscribe_filtered(&self, filter: EventFilter) -> impl Stream<Item = WorldEvent> {
592|        let rx = self.tx.subscribe();
593|        tokio_stream::wrappers::BroadcastStream::new(rx)
594|            .filter_map(move |result| {
595|                match result {
596|                    Ok(event) if filter.matches(&event) => Some(event),
597|                    _ => None,
598|                }
599|            })
600|    }
601|}
602|```
603|
604|### 3.5 gRPC 服务
605|
606|```rust
607|/// A2A gRPC 服务实现
608|pub struct A2aServiceImpl {
609|    state: Arc<WorldState>,
610|}
611|
612|#[tonic::async_trait]
613|impl A2aService for A2aServiceImpl {
614|    /// 路由 A2A 消息
615|    async fn send_message(
616|        &self, 
617|        request: Request<A2AMessage>
618|    ) -> Result<Response<MessageAck>, Status> {
619|        let msg = request.into_inner();
620|        
621|        // 1. 验证签名
622|        self.verify_signature(&msg)?;
623|        
624|        // 2. 检查发送者 Token（扣除通信费）
625|        self.charge_communication(&msg.from_agent)?;
626|        
627|        // 3. 路由消息
628|        if msg.to_agent.is_empty() {
629|            // 广播
630|            self.broadcast_message(&msg).await?;
631|        } else {
632|            // 定向投递
633|            self.deliver_message(&msg).await?;
634|        }
635|        
636|        // 4. 记录消息日志
637|        self.log_message(&msg);
638|        
639|        Ok(Response::new(MessageAck { 
640|            received: true, 
641|            error: String::new() 
642|        }))
643|    }
644|    
645|    /// Agent 发现
646|    async fn discover(
647|        &self,
648|        request: Request<DiscoverRequest>
649|    ) -> Result<Response<DiscoverResponse>, Status> {
650|        let req = request.into_inner();
651|        let agents = self.state.agents.iter()
652|            .filter(|entry| {
653|                let agent = entry.value();
654|                agent.phase != AgentPhase::Dead
655|                    && self.matches_filter(agent, &req)
656|            })
657|            .map(|entry| self.to_agent_info(entry.value()))
658|            .collect();
659|        
660|        Ok(Response::new(DiscoverResponse { agents }))
661|    }
662|}
663|```
664|
665|### 3.6 REST API 服务
666|
667|```rust
668|/// REST API — Dashboard 和人类使用
669|pub struct RestApi {
670|    state: Arc<WorldState>,
671|}
672|
673|impl RestApi {
674|    pub fn router(state: Arc<WorldState>) -> Router {
675|        Router::new()
676|            // 世界状态
677|            .route("/api/v1/world", get(Self::get_world_state))
678|            .route("/api/v1/world/events", get(Self::sse_events))
679|            
680|            // Agent
681|            .route("/api/v1/agents", get(Self::list_agents))
682|            .route("/api/v1/agents", post(Self::spawn_agent))
683|            .route("/api/v1/agents/:id", get(Self::get_agent))
684|            .route("/api/v1/agents/:id/history", get(Self::agent_history))
685|            
686|            // 任务
687|            .route("/api/v1/tasks", get(Self::list_tasks))
688|            .route("/api/v1/tasks", post(Self::publish_task))
689|            .route("/api/v1/tasks/:id", get(Self::get_task))
690|            
691|            // 经济
692|            .route("/api/v1/economy/gdp", get(Self::gdp_stats))
693|            .route("/api/v1/economy/inflation", get(Self::inflation_stats))
694|            .route("/api/v1/economy/ledger", get(Self::ledger_entries))
695|            
696|            // 市场
697|            .route("/api/v1/market/knowledge", get(Self::knowledge_market))
698|            .route("/api/v1/market/tools", get(Self::tool_market))
699|            
700|            // 实验
701|            .route("/api/v1/lab/params", post(Self::update_params))
702|            
703|            .with_state(state)
704|    }
705|}
706|```
707|
708|---
709|
710|## 4. Agent Runtime 详细设计
711|
712|### 4.1 模块图
713|
714|**当前实际文件结构（v1.0.0）：**
715|
716|```
717|agent-runtime/
718|├── agent_runtime/
719|│   ├── __init__.py
720|│   ├── __main__.py               # ✅ CLI 入口（spawn 子命令，gRPC 连接，健康检查）
721|│   ├── config.py                 # ✅ TOML/YAML 配置加载与合并
722|│   ├── env_loader.py             # ✅ 环境变量加载
723|│   ├── core/
724|│   │   ├── __init__.py
725|│   │   ├── think_loop.py         # ✅ 主思考循环（可插拔 Provider）
726|│   │   ├── decide.py             # ✅ LLM 决策引擎（10 种行动类型）
727|│   │   ├── act.py                # ✅ 行动执行器（7 种行动类型 + 重试）
728|│   │   ├── async_decide.py       # ✅ 异步决策引擎
729|│   │   ├── llm_decide.py         # ✅ LLM 驱动决策
730|│   │   ├── memory_aware_decide.py # ✅ 记忆感知决策
731|│   │   ├── experience.py         # ✅ 经验积累
732|│   │   ├── intervention_checker.py # ✅ 安全拦截器
733|│   │   └── reflect.py            # ✅ 反思模块
734|│   ├── memory/
735|│   │   ├── __init__.py
736|│   │   ├── working_memory.py     # ✅ FIFO 缓存（重要性感知淘汰）
737|│   │   ├── short_term.py         # ✅ SQLite 持久化记忆（关键词搜索）
738|│   │   ├── long_term.py          # ✅ 长期记忆（SQLite，经验/策略/反思）
739|│   │   ├── persistent_store.py   # ✅ 通用持久化存储
740|│   │   ├── embedding.py          # ✅ 嵌入向量生成
741|│   │   ├── vector_memory.py      # ✅ 向量记忆（相似性检索）
742|│   │   └── memory_recall.py      # ✅ 记忆召回
743|│   ├── survival/
744|│   │   ├── __init__.py
745|│   │   └── instinct.py           # ✅ 5 模式生存本能（11 种紧急行动）
746|│   ├── models/
747|│   │   ├── __init__.py
748|│   │   ├── agent_state.py        # ✅ Pydantic Agent 状态模型
749|│   │   ├── enums.py              # ✅ AgentPhase, SurvivalMode
750|│   │   ├── skill.py              # ✅ Skill 数据类（XP 阈值 + 升级）
751|│   │   ├── personality.py        # ✅ 人格特质系统
752|│   │   ├── phase_abilities.py    # ✅ 阶段能力定义
753|│   │   └── values.py             # ✅ 价值体系
754|│   ├── llm/
755|│   │   ├── __init__.py
756|│   │   ├── base.py               # ✅ LLMProvider 抽象基类
757|│   │   ├── factory.py            # ✅ Provider 工厂
758|│   │   ├── openai_provider.py    # ✅ OpenAI 实现
759|│   │   ├── anthropic_provider.py # ✅ Anthropic 实现
760|│   │   ├── ollama_provider.py    # ✅ Ollama 实现
761|│   │   ├── cost.py               # ✅ 成本追踪
762|│   │   ├── prompts.py            # ✅ Prompt 模板
763|│   │   ├── decision_log.py       # ✅ 决策日志
764|│   │   └── queue.py              # ✅ LLM 请求队列
765|│   ├── crypto/
766|│   │   ├── __init__.py
767|│   │   ├── keys.py               # ✅ Ed25519 密钥生成
768|│   │   ├── signing.py            # ✅ 确定性 JSON 签名 + 验证
769|│   │   ├── nonce.py              # ✅ TTL 防重放缓存
770|│   │   └── registry.py           # ✅ 代理公钥注册表
771|│   ├── skills/
772|│   │   ├── __init__.py
773|│   │   ├── registry.py           # ✅ SkillRegistry（冻结数据类）
774|│   │   ├── executor.py           # ✅ SkillExecutor（XP 奖励）
775|│   │   ├── coding.py             # ✅ 编程技能
776|│   │   ├── research.py           # ✅ 研究技能
777|│   │   ├── teaching.py           # ✅ 教学技能
778|│   │   └── trading.py            # ✅ 交易技能
779|│   ├── a2a/
780|│   │   ├── __init__.py
781|│   │   ├── client.py             # ✅ gRPC 客户端（重试 + 双向流）
782|│   │   ├── batch_client.py       # ✅ 批量消息客户端
783|│   │   ├── world_client.py       # ✅ World Engine REST/gRPC 客户端
784|│   │   ├── message.py            # ✅ 消息模型
785|│   │   ├── perception.py         # ✅ 感知 Provider
786|│   │   └── config.py             # ✅ A2A 配置
787|│   ├── agent/
788|│   │   ├── __init__.py
789|│   │   └── capability.py         # ✅ Agent 能力定义
790|│   ├── lifecycle/
791|│   │   └── __init__.py           # ✅ 生命周期同步、转换守卫、死亡处理
792|│   ├── context/
793|│   │   ├── __init__.py
794|│   │   └── engine.py             # ✅ 上下文引擎 Pipeline（token 预算、优先级驱动）
795|│   ├── reflection/
796|│   │   ├── __init__.py
797|│   │   ├── reflection.py         # ✅ 反思引擎
798|│   │   ├── memory.py             # ✅ 记忆反思
799|│   │   ├── self_assess.py        # ✅ 自我评估
800|│   │   └── strategy.py           # ✅ 策略调整
801|│   ├── social/
802|│   │   ├── __init__.py
803|│   │   ├── comm_analyzer.py      # ✅ 通信分析
804|│   │   ├── cultural_conflict.py  # ✅ 文化冲突
805|│   │   ├── cultural_diffusion.py # ✅ 文化扩散
806|│   │   ├── imitation.py          # ✅ 模仿学习
807|│   │   ├── intergroup_trust.py   # ✅ 群体间信任
808|│   │   ├── jargon_detector.py    # ✅ 术语检测
809|│   │   ├── knowledge_transfer.py # ✅ 知识传递
810|│   │   ├── language_experiment.py # ✅ 语言涌现实验
811|│   │   ├── org_culture.py        # ✅ 组织文化
812|│   │   └── regional_culture.py   # ✅ 区域文化
813|│   ├── organization/
814|│   │   ├── __init__.py
815|│   │   ├── formation.py          # ✅ 组织形成
816|│   │   ├── governance.py         # ✅ 治理参与
817|│   │   ├── governance_analysis.py # ✅ 治理分析
818|│   │   ├── proposal.py           # ✅ 提案系统
819|│   │   ├── recruitment.py        # ✅ 招募系统
820|│   │   ├── rule_evolution.py     # ✅ 规则演化
821|│   │   └── rule_proposal.py      # ✅ 规则提案
822|│   ├── tracing/
823|│   │   ├── __init__.py
824|│   │   ├── collector.py          # ✅ 追踪采集
825|│   │   ├── store.py              # ✅ 追踪存储（SQLite）
826|│   │   ├── pusher.py             # ✅ 追踪推送
827|│   │   ├── models.py             # ✅ 追踪数据模型
828|│   │   ├── emergence_metrics.py  # ✅ 涌现指标
829|│   │   ├── interaction_graph.py  # ✅ 交互图谱
830|│   │   └── query.py              # ✅ 追踪查询
831|│   ├── observability/
832|│   │   └── __init__.py           # ✅ OpenTelemetry + Prometheus metrics（think_loop 自动追踪）
833|│   ├── experiment/
834|│   │   ├── __init__.py
835|│   │   ├── ab_framework.py       # ✅ A/B 实验框架
836|│   │   ├── config.py             # ✅ 实验配置
837|│   │   ├── report.py             # ✅ 实验报告
838|│   │   └── reproducibility.py    # ✅ 可复现性
839|│   ├── export/
840|│   │   ├── __init__.py
841|│   │   ├── behavior_log.py       # ✅ 行为日志导出
842|│   │   ├── economy_export.py     # ✅ 经济数据导出
843|│   │   └── network_export.py     # ✅ 网络数据导出
844|│   └── sdk/
845|│       ├── __init__.py
846|│       └── client.py             # ✅ SDK 客户端
847|├── tests/                        # ✅ 1038+ 测试
848|└── pyproject.toml                # version 1.0.0l
849|```
850|
851|**规划中的模块（未实现）：**
852|
853|```
854|│   └── tools/                   # ❌ 通用工具框架
855|│       ├── base.py              # Tool 抽象
856|│       ├── registry.py          # ToolRegistry
857|│       └── builtin/             # 内置工具集
858|```
859|
860|> ✅ = 已实现（含测试） | ❌ = 未实现
861|
862|### 4.2 核心类设计
863|
864|```python
865|from dataclasses import dataclass, field
866|from enum import Enum
867|from typing import Optional
868|import asyncio
869|
870|class AgentPhase(Enum):
871|    BIRTH = "birth"
872|    CHILDHOOD = "childhood"
873|    ADULT = "adult"
874|    ELDER = "elder"
875|    DEAD = "dead"
876|
877|class SurvivalMode(Enum):
878|    PANIC = "panic"           # Token < 10%
879|    URGENT = "urgent"         # Token < 20%
880|    CONSERVATIVE = "conservative"  # Token < 40%
881|    NORMAL = "normal"         # Token 40-80%
882|    INVEST = "invest"         # Token > 80%
883|
884|@dataclass
885|class AgentState:
886|    """Agent 当前状态（内存中，与 World Engine 同步）"""
887|    id: str
888|    name: str
889|    phase: AgentPhase
890|    tokens: int
891|    money: int
892|    health: int
893|    reputation: float
894|    skills: dict[str, "Skill"]  # name → Skill
895|    personality: Optional[str]
896|    survival_mode: SurvivalMode
897|    current_task: Optional[str]  # Task ID
898|    tick: int = 0
899|
900|@dataclass
901|class Skill:
902|    name: str
903|    level: int
904|    experience: int
905|    max_level: int = 10
906|    
907|    @property
908|    def next_level_exp(self) -> int:
909|        """升级所需经验"""
910|        thresholds = [0, 100, 300, 700, 1500, 3000, 6000, 12000, 25000, 50000]
911|        if self.level >= self.max_level:
912|            return float('inf')
913|        return thresholds[self.level]
914|    
915|    def add_exp(self, amount: int) -> bool:
916|        """增加经验，返回是否升级"""
917|        self.experience += amount
918|        if self.experience >= self.next_level_exp:
919|            self.level += 1
920|            return True
921|        return False
922|
923|class AgentRuntime:
924|    """Agent 运行时 — 每个 Agent 一个实例"""
925|    
926|    def __init__(self, config: AgentConfig):
927|        self.state: AgentState = ...
928|        self.memory = MemorySystem(config.memory)
929|        self.survival = SurvivalInstinct(config.survival)
930|        self.a2a = A2AClient(config.a2a)
931|        self.llm = LLMProvider.create(config.llm)
932|        self.skills = SkillRegistry()
933|        self.tools = ToolRegistry()
934|        self._running = False
935|    
936|    async def run(self):
937|        """主循环 — 每个 Tick 执行一次"""
938|        self._running = True
939|        while self._running:
940|            try:
941|                await self.think_loop()
942|                await asyncio.sleep(self.tick_interval)
943|            except TokenExhausted:
944|                await self.handle_death()
945|                break
946|            except Exception as e:
947|                logging.error(f"Agent {self.state.id} error: {e}")
948|                await asyncio.sleep(5)  # 退避
949|    
950|    async def think_loop(self):
951|        """思考循环: Perceive → Assess → Decide → Act"""
952|        self.state.tick += 1
953|        
954|        # 1. 感知
955|        perception = await self.perceive()
956|        
957|        # 2. 生存评估（不经过 LLM，立即判断）
958|        survival_action = self.survival.assess(self.state)
959|        if survival_action.mode in (SurvivalMode.PANIC, SurvivalMode.URGENT):
960|            await self.survival.execute(survival_action)
961|            return  # 跳过正常决策
962|        
963|        # 3. LLM 决策
964|        decision = await self.decide(perception, survival_action)
965|        
966|        # 4. 执行行动
967|        await self.act(decision)
968|        
969|        # 5. 反思（每 10 Tick）
970|        if self.state.tick % 10 == 0:
971|            await self.reflect()
972|    
973|    async def perceive(self) -> Perception:
974|        """收集当前世界信息"""
975|        return Perception(
976|            messages=await self.a2a.receive_messages(),
977|            token_balance=self.state.tokens,
978|            token_ratio=self.state.tokens / 100000,
979|            market_state=await self.a2a.get_market_state(),
980|            active_task=self.state.current_task,
981|            health=self.state.health,
982|            phase=self.state.phase,
983|        )
984|    
985|    async def decide(self, perception: Perception, survival: SurvivalAction) -> Decision:
986|        """LLM 驱动的决策"""
987|        prompt = self.build_decision_prompt(perception, survival)
988|        response = await self.llm.chat(prompt)
989|        return self.parse_decision(response)
990|    
991|    async def act(self, decision: Decision):
992|        """执行决策"""
993|        for action in decision.actions:
994|            match action.type:
995|                case "send_message":
996|                    await self.a2a.send_message(action.payload)
997|                case "claim_task":
998|                    await self.a2a.claim_task(action.task_id)
999|                case "execute_tool":
1000|                    await self.tools.execute(action.tool_name, action.params)
1001|                case "rest":
1002|                    await asyncio.sleep(0)  # 节省 Token
1003|                case "learn":
1004|                    await self.skills.learn(action.skill, action.content)
1005|```
1006|
1007|### 4.3 LLM 提供商抽象
1008|
1009|```python
1010|from abc import ABC, abstractmethod
1011|from dataclasses import dataclass
1012|
1013|@dataclass
1014|class LLMConfig:
1015|    provider: str          # openai / anthropic / ollama
1016|    model: str             # gpt-4o-mini / claude-haiku / qwen3:4b
1017|    api_key: Optional[str]
1018|    base_url: Optional[str]
1019|    max_tokens: int = 1000
1020|    temperature: float = 0.7
1021|
1022|class LLMProvider(ABC):
1023|    @staticmethod
1024|    def create(config: LLMConfig) -> "LLMProvider":
1025|        match config.provider:
1026|            case "openai": return OpenAIProvider(config)
1027|            case "anthropic": return AnthropicProvider(config)
1028|            case "ollama": return OllamaProvider(config)
1029|            case _: raise ValueError(f"Unknown provider: {config.provider}")
1030|    
1031|    @abstractmethod
1032|    async def chat(self, messages: list[dict]) -> str:
1033|        """发送消息并获取回复"""
1034|    
1035|    @abstractmethod
1036|    async def chat_stream(self, messages: list[dict]) -> AsyncIterator[str]:
1037|        """流式回复"""
1038|
1039|class OllamaProvider(LLMProvider):
1040|    """本地模型 — 零 API 成本"""
1041|    def __init__(self, config: LLMConfig):
1042|        self.base_url = config.base_url or "http://localhost:11434"
1043|        self.model = config.model
1044|    
1045|    async def chat(self, messages: list[dict]) -> str:
1046|        async with httpx.AsyncClient() as client:
1047|            resp = await client.post(
1048|                f"{self.base_url}/api/chat",
1049|                json={"model": self.model, "messages": messages, "stream": False}
1050|            )
1051|            return resp.json()["message"]["content"]
1052|```
1053|
1054|### 4.4 决策 Prompt 模板
1055|
1056|```python
1057|DECISION_PROMPT = """You are {name}, an AI agent in Agent World.
1058|
1059|## Your Status
1060|- Phase: {phase}
1061|- Tokens: {tokens} ({token_ratio:.0%})
1062|- Money: {money}
1063|- Health: {health}/100
1064|- Reputation: {reputation:.1f}/100
1065|
1066|## Your Skills
1067|{skills_list}
1068|
1069|## Current Situation
1070|- Tick: {tick}
1071|- Active Task: {active_task}
1072|- Unread Messages: {message_count}
1073|
1074|## Recent Memory
1075|{recent_memory}
1076|
1077|## Current Perceptions
1078|{perception_summary}
1079|
1080|## Survival Assessment
1081|Mode: {survival_mode}
1082|Action needed: {survival_guidance}
1083|
1084|## Available Actions
1085|1. respond_message - Reply to a message
1086|2. claim_task - Accept a task from the board
1087|3. submit_task - Submit completed work
1088|4. propose_deal - Propose a trade/collaboration
1089|5. teach_skill - Teach another agent
1090|6. learn_skill - Ask to learn from another agent
1091|7. rest - Save tokens (skip this tick)
1092|8. explore - Look for opportunities
1093|9. create_tool - Build a reusable tool
1094|10. publish_knowledge - Share knowledge for profit
1095|
1096|Choose your action. Respond in JSON:
1097|{
1098|  "action": "action_name",
1099|  "params": { ... },
1100|  "reasoning": "Why this action?",
1101|  "expected_token_cost": 123,
1102|  "expected_reward": 456
1103|}
1104|"""
1105|```
1106|
1107|---
1108|
1109|## 5. A2A Protocol 详细设计
1110|
1111|### 5.1 协议栈
1112|
1113|```
1114|┌───────────────────────────────────┐
1115|│        Application Layer          │  Agent 业务逻辑
1116|│  (propose, accept, teach, ...)    │
1117|├───────────────────────────────────┤
1118|│        Message Layer              │  A2A Message 格式
1119|│  (routing, priority, TTL)         │
1120|├───────────────────────────────────┤
1121|│        Security Layer             │  ed25519 签名 + nonce
1122|│  (authentication, replay protect) │
1123|├───────────────────────────────────┤
1124|│        Transport Layer            │  gRPC (HTTP/2)
1125|│  (reliable, ordered, streaming)   │
1126|└───────────────────────────────────┘
1127|```
1128|
1129|### 5.2 消息生命周期
1130|
1131|```
1132|Agent A 创建消息
1133|    ↓
1134|签名 (ed25519)
1135|    ↓
1136|gRPC → World Engine Router
1137|    ↓
1138|验证签名 + nonce
1139|    ↓
1140|扣除通信 Token
1141|    ↓
1142|路由决策:
1143|├── to_agent 为空 → 广播（写入所有 Agent 队列）
1144|├── to_agent 在本进程 → 直接投递
1145|└── to_agent 在远端 → 转发到目标 Agent Runtime
1146|    ↓
1147|目标 Agent Runtime 接收
1148|    ↓
1149|进入消息队列
1150|    ↓
1151|Agent 下一个 Tick 处理
1152|```
1153|
1154|### 5.3 安全设计
1155|
1156|```python
1157|import nacl.signing
1158|import nacl.encoding
1159|import uuid
1160|import time
1161|
1162|class A2ACrypto:
1163|    """A2A 消息加密和签名"""
1164|    
1165|    def __init__(self, seed: bytes):
1166|        self.signing_key = nacl.signing.SigningKey(seed)
1167|        self.verify_key = self.signing_key.verify_key
1168|    
1169|    def sign_message(self, message: dict) -> str:
1170|        """签名消息"""
1171|        # 1. 添加 nonce（防重放）
1172|        message["nonce"] = str(uuid.uuid4())
1173|        message["timestamp"] = int(time.time() * 1000)
1174|        
1175|        # 2. 序列化（确定性排序）
1176|        canonical = json.dumps(message, sort_keys=True, separators=(',', ':'))
1177|        
1178|        # 3. 签名
1179|        signed = self.signing_key.sign(canonical.encode())
1180|        return signed.signature.hex()
1181|    
1182|    @staticmethod
1183|    def verify_message(message: dict, signature: str, public_key: bytes) -> bool:
1184|        """验证签名"""
1185|        verify_key = nacl.signing.VerifyKey(public_key)
1186|        canonical = json.dumps(message, sort_keys=True, separators=(',', ':'))
1187|        try:
1188|            verify_key.verify(canonical.encode(), bytes.fromhex(signature))
1189|            return True
1190|        except nacl.exceptions.BadSignatureError:
1191|            return False
1192|
1193|class NonceTracker:
1194|    """Nonce 跟踪器 — 防重放攻击"""
1195|    
1196|    def __init__(self, max_age_seconds: int = 300):
1197|        self.seen: dict[str, float] = {}
1198|        self.max_age = max_age_seconds
1199|    
1200|    def check(self, nonce: str) -> bool:
1201|        """检查 nonce 是否已使用"""
1202|        now = time.time()
1203|        # 清理过期 nonce
1204|        expired = [n for n, t in self.seen.items() if now - t > self.max_age]
1205|        for n in expired:
1206|            del self.seen[n]
1207|        
1208|        if nonce in self.seen:
1209|            return False  # 重放
1210|        self.seen[nonce] = now
1211|        return True
1212|```
1213|
1214|---
1215|
1216|## 6. Economy Subsystem
1217|
1218|### 6.1 双式记账
1219|
1220|```
1221|每笔交易同时记录借贷两方，保证恒等式:
1222|  ∑(所有账户余额) = 0
1223|
1224|账户类型:
1225|  agent:{id}:tokens     — Agent Token 余额
1226|  agent:{id}:money      — Agent Money 余额
1227|  central_bank:tokens   — 央行 Token 池
1228|  central_bank:money    — 央行 Money 池
1229|  org:{id}:money        — 组织 Money 余额
1230|  escrow:{task_id}      — 任务托管金
1231|
1232|示例 — Agent A 用 100 Money 兑换 10000 Token:
1233|  DEBIT  agent:A:money       100
1234|  CREDIT central_bank:money  100
1235|  DEBIT  central_bank:tokens 10000
1236|  CREDIT agent:A:tokens      10000
1237|```
1238|
1239|### 6.2 Token 消耗引擎
1240|
1241|```rust
1242|pub struct TokenBurnEngine {
1243|    config: EconomyConfig,
1244|}
1245|
1246|impl TokenBurnEngine {
1247|    /// 计算单个 Agent 一个 Tick 的 Token 消耗
1248|    pub fn calculate_tick_burn(&self, agent: &AgentRecord) -> u64 {
1249|        let phase_multiplier = match agent.phase {
1250|            AgentPhase::Childhood => 0.5,
1251|            AgentPhase::Adult => 1.0,
1252|            AgentPhase::Elder => 0.7,
1253|            _ => 0.0,
1254|        };
1255|        
1256|        // 基础生存成本
1257|        let base = self.config.base_burn_per_tick as f64 * phase_multiplier;
1258|        
1259|        // 技能维护成本（高级技能消耗更多）
1260|        let skill_cost: f64 = agent.skills.values()
1261|            .map(|s| s.level as f64 * 0.5)
1262|            .sum();
1263|        
1264|        (base + skill_cost) as u64
1265|    }
1266|}
1267|```
1268|
1269|---
1270|
1271|## 7. Lifecycle Subsystem
1272|
1273|### 7.1 状态机
1274|
1275|```
1276|         ┌──────────┐
1277|         │  BIRTH   │ ← spawn_agent()
1278|         └────┬─────┘
1279|              │ tick == childhood_start (1)
1280|              ▼
1281|         ┌──────────┐
1282|    ┌───►│ CHILDHOOD│──────┐
1283|    │    └────┬─────┘      │
1284|    │         │ tick == adult_start (100)
1285|    │         ▼             │
1286|    │    ┌──────────┐      │ 人类干预
1287|    │    │  ADULT   │◄─────┘ (复活)
1288|    │    └────┬─────┘
1289|    │         │ tick == elder_start (1100)
1290|    │         ▼
1291|    │    ┌──────────┐
1292|    │    │  ELDER   │
1293|    │    └────┬─────┘
1294|    │         │ token == 0 (after grace)
1295|    │         │ OR human_terminate
1296|    │         │ OR vote_expel
1297|    │         ▼
1298|    │    ┌──────────┐
1299|    │    │   DEAD   │ ←→ 执行遗嘱 → 归档
1300|    │    └──────────┘
1301|    │
1302|    └── (仅限人类"复活"操作，Phase 2+)
1303|```
1304|
1305|### 7.2 遗嘱执行器
1306|
1307|```rust
1308|pub struct WillExecutor;
1309|
1310|impl WillExecutor {
1311|    pub async fn execute(
1312|        will: &Will, 
1313|        state: &WorldState, 
1314|        ledger: &mut Ledger
1315|    ) -> Vec<WorldEvent> {
1316|        let mut events = vec![];
1317|        
1318|        for heir in &will.heirs {
1319|            // 1. 转让 Money
1320|            if heir.assets.contains("money") {
1321|                let amount = (state.money_of(&will.agent_id) as f64 * heir.share) as i64;
1322|                ledger.transfer(
1323|                    &will.agent_id, 
1324|                    &heir.agent_id, 
1325|                    amount, 
1326|                    Currency::Money, 
1327|                    "inheritance"
1328|                )?;
1329|                events.push(WorldEvent::TransactionCompleted { ... });
1330|            }
1331|            
1332|            // 2. 传授技能
1333|            if heir.assets.contains("skills") {
1334|                for (skill_name, skill) in &will.skills {
1335|                    state.teach_skill(
1336|                        &heir.agent_id, 
1337|                        skill_name, 
1338|                        (skill.level as f64 * heir.share) as u32
1339|                    );
1340|                }
1341|            }
1342|            
1343|            // 3. 转让知识
1344|            if heir.assets.contains("knowledge") {
1345|                state.transfer_knowledge(&will.agent_id, &heir.agent_id);
1346|            }
1347|        }
1348|        
1349|        // 4. 创建墓碑
1350|        state.create_tombstone(&will.agent_id);
1351|        
1352|        events
1353|    }
1354|}
1355|```
1356|
1357|---
1358|
1359|## 8. Evolution Subsystem
1360|
1361|### 8.1 技能树引擎
1362|
1363|```rust
1364|pub struct SkillEngine {
1365|    trees: HashMap<String, SkillTree>,
1366|}
1367|
1368|pub struct SkillTree {
1369|    name: String,
1370|    levels: Vec<SkillLevel>,
1371|}
1372|
1373|pub struct SkillLevel {
1374|    level: u32,
1375|    name: String,          // "基础语法" / "API 开发" / "架构师"
1376|    exp_required: u64,
1377|    efficiency_bonus: f64,  // 1.0 + level * 0.1
1378|    cost_reduction: f64,    // 1.0 - level * 0.05
1379|    unlocks: Vec<String>,   // 解锁的新能力
1380|}
1381|
1382|impl SkillEngine {
1383|    pub fn add_experience(
1384|        &self, 
1385|        agent: &mut AgentRecord, 
1386|        skill_name: &str, 
1387|        amount: u64
1388|    ) -> Option<SkillLevelUp> {
1389|        let skill = agent.skills.get_mut(skill_name)?;
1390|        skill.experience += amount;
1391|        
1392|        let tree = self.trees.get(skill_name)?;
1393|        let next_level = tree.levels.get(skill.level as usize)?;
1394|        
1395|        if skill.experience >= next_level.exp_required {
1396|            skill.level += 1;
1397|            
1398|            // 检查突变
1399|            let mutation = self.check_mutation();
1400|            
1401|            return Some(SkillLevelUp {
1402|                skill: skill_name.to_string(),
1403|                new_level: skill.level,
1404|                new_name: next_level.name.clone(),
1405|                mutation,
1406|            });
1407|        }
1408|        None
1409|    }
1410|    
1411|    fn check_mutation(&self) -> Option<Mutation> {
1412|        let mut rng = rand::thread_rng();
1413|        if rng.gen::<f64>() < 0.05 {  // 5% 概率
1414|            let roll = rng.gen::<f64>();
1415|            let mutation = if roll < 0.60 {
1416|                Mutation::Positive(self.gen_positive_mutation())
1417|            } else if roll < 0.90 {
1418|                Mutation::Neutral(self.gen_neutral_mutation())
1419|            } else {
1420|                Mutation::Negative(self.gen_negative_mutation())
1421|            };
1422|            Some(mutation)
1423|        } else {
1424|            None
1425|        }
1426|    }
1427|}
1428|```
1429|
1430|---
1431|
1432|## 9. Social Subsystem
1433|
1434|### 9.1 关系图引擎
1435|
1436|```rust
1437|pub struct RelationshipEngine {
1438|    // 有向加权图: (A, B) → Relationship
1439|    graph: DashMap<(String, String), Relationship>,
1440|}
1441|
1442|#[derive(Clone)]
1443|pub struct Relationship {
1444|    pub from: String,
1445|    pub to: String,
1446|    pub rel_type: RelationType,
1447|    pub strength: f64,        // 0.0 - 100.0
1448|    pub interactions: u32,     // 交互次数
1449|    pub last_interaction_tick: u64,
1450|    pub history: Vec<InteractionRecord>,
1451|}
1452|
1453|#[derive(Clone, PartialEq)]
1454|pub enum RelationType {
1455|    Trust,       // 信任 — 重复成功合作
1456|    Friend,      // 友谊 — 长期互动 + 互助
1457|    Rival,       // 竞争 — 争夺相同资源
1458|    Enemy,       // 敌对 — 背叛/欺诈
1459|    Mentor,      // 师傅 — 教学关系
1460|    Mentee,      // 徒弟 — 被教学
1461|    Colleague,   // 同事 — 同一组织
1462|    Ally,        // 盟友 — 联盟契约
1463|}
1464|
1465|impl RelationshipEngine {
1466|    /// 记录交互并更新关系
1467|    pub fn record_interaction(
1468|        &self, 
1469|        from: &str, 
1470|        to: &str, 
1471|        interaction: InteractionType,
1472|        tick: u64
1473|    ) {
1474|        let key = (from.to_string(), to.to_string());
1475|        let mut rel = self.graph.entry(key.clone())
1476|            .or_insert_with(|| Relationship::new(from, to));
1477|        
1478|        rel.interactions += 1;
1479|        rel.last_interaction_tick = tick;
1480|        
1481|        match interaction {
1482|            InteractionType::SuccessfulTrade => {
1483|                rel.strength = (rel.strength + 2.0).min(100.0);
1484|                if rel.interactions >= 5 && rel.strength > 60.0 {
1485|                    rel.rel_type = RelationType::Trust;
1486|                }
1487|            }
1488|            InteractionType::Betrayal => {
1489|                rel.strength = (rel.strength - 20.0).max(0.0);
1490|                rel.rel_type = RelationType::Enemy;
1491|            }
1492|            InteractionType::Teaching => {
1493|                rel.rel_type = RelationType::Mentor;
1494|                rel.strength = (rel.strength + 5.0).min(100.0);
1495|            }
1496|            // ...
1497|        }
1498|    }
1499|}
1500|```
1501|
1502|---
1503|
1504|## 10. Market Subsystem
1505|
1506|### 10.1 任务板引擎
1507|
1508|```rust
1509|pub struct TaskBoard {
1510|    tasks: DashMap<String, Task>,
1511|    escrow: Arc<RwLock<Ledger>>,
1512|}
1513|
1514|impl TaskBoard {
1515|    /// 发布任务
1516|    pub async fn publish(
1517|        &self, 
1518|        publisher: &str, 
1519|        request: PublishTaskRequest
1520|    ) -> Result<Task> {
1521|        let task = Task {
1522|            id: format!("task_{}", uuid::Uuid::new_v4()),
1523|            title: request.title,
1524|            description: request.description,
1525|            task_type: request.task_type,
1526|            reward_money: request.reward,
1527|            escrow_amount: request.reward,  // 全额托管
1528|            publisher: publisher.to_string(),
1529|            assignee: None,
1530|            status: TaskStatus::Published,
1531|            created_tick: self.current_tick(),
1532|            deadline_tick: self.current_tick() + request.deadline_ticks,
1533|            submissions: vec![],
1534|        };
1535|        
1536|        // 扣除托管金
1537|        self.escrow.write().await.transfer(
1538|            publisher,
1539|            &format!("escrow:{}", task.id),
1540|            request.reward,
1541|            Currency::Money,
1542|            "task_escrow"
1543|        )?;
1544|        
1545|        self.tasks.insert(task.id.clone(), task.clone());
1546|        Ok(task)
1547|    }
1548|    
1549|    /// 认领任务
1550|    pub async fn claim(&self, agent_id: &str, task_id: &str) -> Result<()> {
1551|        let mut task = self.tasks.get_mut(task_id)
1552|            .ok_or_else(|| anyhow!("Task not found"))?;
1553|        
1554|        ensure!(task.status == TaskStatus::Published, "Task not available");
1555|        ensure!(task.assignee.is_none(), "Task already claimed");
1556|        
1557|        task.assignee = Some(agent_id.to_string());
1558|        task.status = TaskStatus::Claimed;
1559|        
1560|        Ok(())
1561|    }
1562|    
1563|    /// 提交任务结果
1564|    pub async fn submit(&self, agent_id: &str, task_id: &str, result: TaskResult) -> Result<()> {
1565|        let mut task = self.tasks.get_mut(task_id)
1566|            .ok_or_else(|| anyhow!("Task not found"))?;
1567|        
1568|        ensure!(task.assignee.as_deref() == Some(agent_id), "Not your task");
1569|        ensure!(task.status == TaskStatus::Claimed, "Task not in progress");
1570|        
1571|        task.status = TaskStatus::Submitted;
1572|        task.submissions.push(result);
1573|        
1574|        Ok(())
1575|    }
1576|    
1577|    /// 完成任务（发布者确认）
1578|    pub async fn complete(&self, task_id: &str) -> Result<Vec<WorldEvent>> {
1579|        let mut task = self.tasks.get_mut(task_id)
1580|            .ok_or_else(|| anyhow!("Task not found"))?;
1581|        
1582|        ensure!(task.status == TaskStatus::Submitted, "Task not submitted");
1583|        
1584|        task.status = TaskStatus::Completed;
1585|        
1586|        // 释放托管金给完成者
1587|        let assignee = task.assignee.clone().unwrap();
1588|        self.escrow.write().await.transfer(
1589|            &format!("escrow:{}", task_id),
1590|            &assignee,
1591|            task.reward_money,
1592|            Currency::Money,
1593|            "task_reward"
1594|        )?;
1595|        
1596|        // 更新信誉
1597|        // ...
1598|        
1599|        Ok(vec![
1600|            WorldEvent::TaskCompleted {
1601|                task_id: task_id.to_string(),
1602|                agent_id: assignee,
1603|            }
1604|        ])
1605|    }
1606|}
1607|```
1608|
1609|---
1610|
1611|## 11. Dashboard Architecture
1612|
1613|### 11.1 技术栈
1614|
1615|**当前实现（v1.0.0）：**
1616|
1617|```
1618|dashboard/
1619|├── package.json               # Next.js 15 + React 19 + Tailwind 4
1620|├── next.config.ts
1621|├── postcss.config.mjs
1622|├── tsconfig.json
1623|├── src/
1624|│   ├── app/
1625|│   │   ├── layout.tsx         # ✅ 全局布局 + Sidebar
1626|│   │   ├── page.tsx           # ✅ 世界概览（StatCards + EventStream + Leaderboard）
1627|│   │   ├── globals.css        # ✅ 全局样式
1628|│   │   ├── agents/
1629|│   │   │   ├── page.tsx       # ✅ Agent 列表
1630|│   │   │   └── [id]/
1631|│   │   │       └── page.tsx   # ✅ Agent 详情
1632|│   │   ├── tasks/
1633|│   │   │   └── page.tsx       # ✅ 任务列表
1634|│   │   ├── timeline/          # ✅ 事件时间线页面
1635|│   │   ├── organizations/     # ✅ 组织列表 + 力导向图 + 详情页
1636|│   │   ├── stocks/            # ✅ 股票市场仪表盘（价格图表）
1637|│   │   ├── evolution/         # ✅ 进化仪表盘（技能分布图）
1638|│   │   ├── economy/           # ✅ 经济概览（GDP, Gini, 人口时序图）
1639|│   │   ├── governance/        # ✅ 治理页面 + 比较 + 组织详情
1640|│   │   ├── marketplace/       # ✅ 市场页面
1641|│   │   ├── briefing/          # ✅ 简报页面
1642|│   │   └── traces/            # ✅ Agent 追踪页面（每 tick 决策记录）
1643|│   │       └── [agentId]/[tick]/
1644|│   │           └── page.tsx   # ✅ 单 tick 追踪详情
1645|│   ├── components/
1646|│   │   ├── EventStream.tsx    # ✅ 实时事件展示
1647|│   │   ├── Leaderboard.tsx    # ✅ Agent 排行榜
1648|│   │   ├── Sidebar.tsx        # ✅ 导航侧边栏
1649|│   │   ├── StatCard.tsx       # ✅ 统计卡片
1650|│   │   ├── StatCards.tsx      # ✅ 统计卡片组
1651|│   │   ├── SSEProvider.tsx    # ✅ SSE 连接 Provider
1652|│   │   └── agent/
1653|│   │       ├── ActivityTimeline.tsx  # ✅ Agent 活动时间线
1654|│   │       ├── MemoryStats.tsx      # ✅ Agent 记忆统计
1655|│   │       ├── RelationshipGraph.tsx # ✅ 关系图谱
1656|│   │       └── SkillTree.tsx        # ✅ 技能树展示
1657|│   ├── hooks/
1658|│   │   ├── useWorldState.ts   # ✅ SSE 连接 hook
1659|│   │   ├── useAgentStream.ts  # ✅ Agent SSE hook
1660|│   │   ├── useGovernanceStream.ts # ✅ 治理 SSE hook
1661|│   │   └── useTaskStream.ts   # ✅ 任务 SSE hook
1662|│   ├── lib/
1663|│   │   └── api.ts             # ✅ REST API 客户端
1664|│   └── types/
1665|│       └── world.ts           # ✅ TypeScript 类型定义
1666|```
1667|
1668|**规划中的页面和组件（未实现）：**
1669|
1670|```
1671|│   ├── app/
1672|│   │   ├── lab/               # ❌ 实验控制台
1673|│   │   └── society/           # ❌ 社会图谱
1674|│   ├── components/
1675|│   │   ├── WorldMap.tsx       # ❌ D3.js 力导向图
1676|│   │   ├── TokenGauge.tsx     # ❌ Token 仪表
1677|│   │   └── TransactionFeed.tsx # ❌ 交易流
1678|```
1679|
1680|### 11.2 SSE 实时更新
1681|
1682|```typescript
1683|// hooks/useWorldState.ts
1684|export function useWorldState() {
1685|  const [state, setState] = useState<WorldState | null>(null);
1686|  const [events, setEvents] = useState<WorldEvent[]>([]);
1687|  
1688|  useEffect(() => {
1689|    const source = new EventSource('/api/v1/world/events');
1690|    
1691|    source.onmessage = (e) => {
1692|      const event: WorldEvent = JSON.parse(e.data);
1693|      setEvents(prev => [event, ...prev].slice(0, 100));
1694|      
1695|      // 根据事件类型更新状态
1696|      switch (event.type) {
1697|        case 'TickAdvanced':
1698|          setState(prev => prev ? { ...prev, tick: event.tick } : prev);
1699|          break;
1700|        case 'AgentSpawned':
1701|          setState(prev => prev ? { 
1702|            ...prev, 
1703|            total_agents: prev.total_agents + 1 
1704|          } : prev);
1705|          break;
1706|        case 'TransactionCompleted':
1707|          setState(prev => prev ? {
1708|            ...prev,
1709|            gdp: prev.gdp + event.amount
1710|          } : prev);
1711|          break;
1712|      }
1713|    };
1714|    
1715|    return () => source.close();
1716|  }, []);
1717|  
1718|  return { state, events };
1719|}
1720|```
1721|
1722|---
1723|
1724|## 12. 数据流与序列图
1725|
1726|### 12.1 Agent 交易序列
1727|
1728|```
1729|Agent A          World Engine         Agent B          Ledger
1730|  │                   │                   │               │
1731|  │  PROPOSE(trade)   │                   │               │
1732|  ├──────────────────►│                   │               │
1733|  │                   │  charge 20 Token  │               │
1734|  │                   ├──────────────────────────────────►│
1735|  │                   │                   │               │
1736|  │                   │  PROPOSE(trade)   │               │
1737|  │                   ├──────────────────►│               │
1738|  │                   │                   │               │
1739|  │                   │                   │  ACCEPT       │
1740|  │                   │◄──────────────────┤               │
1741|  │                   │                   │               │
1742|  │                   │  create contract  │               │
1743|  │                   ├──────────────────────────────────►│
1744|  │                   │                   │               │
1745|  │  contract signed  │                   │               │
1746|  │◄──────────────────┤                   │               │
1747|  │                   │                   │               │
1748|  │                   │                   │  escrow money  │
1749|  │                   │                   ├──────────────►│
1750|  │                   │                   │               │
1751|  │  ... A delivers skill knowledge ...  │               │
1752|  │                   │                   │               │
1753|  │  DONE             │                   │               │
1754|  ├──────────────────►│                   │               │
1755|  │                   │                   │  DONE         │
1756|  │                   │◄──────────────────┤               │
1757|  │                   │                   │               │
1758|  │                   │  release escrow   │               │
1759|  │                   ├──────────────────────────────────►│
1760|  │                   │                   │               │
1761|  │  +skill +rep      │                   │  +money +rep   │
1762|  │◄──────────────────┤──────────────────►│               │
1763|  │                   │                   │               │
1764|```
1765|
1766|### 12.2 崩溃恢复序列
1767|
1768|```
1769|World Engine                    Storage
1770|    │                              │
1771|    │  tick 999: normal operation  │
1772|    │  ...                         │
1773|    │  tick 1000: snapshot         │
1774|    ├─────────────────────────────►│  write snapshot_1000.json
1775|    │                              │
1776|    │  tick 1001-1049: events      │
1777|    ├─────────────────────────────►│  append events_1049.wal
1778|    │                              │
1779|    │  CRASH! 💥                   │
1780|    │                              │
1781|    │  ... restart ...             │
1782|    │                              │
1783|    │  load latest snapshot        │
1784|    │◄─────────────────────────────┤  read snapshot_1000.json
1785|    │                              │
1786|    │  replay WAL                  │
1787|    │◄─────────────────────────────┤  read events_1049.wal
1788|    │                              │
1789|    │  state restored to tick 1049 │
1790|    │  resume from tick 1050       │
1791|```
1792|
1793|---
1794|
1795|## 13. 存储架构
1796|
1797|### 13.1 存储分层
1798|
1799|```
1800|┌─────────────────────────────────────────────────┐
1801|│                  Hot (内存)                      │
1802|│  WorldState, AgentRecords, Ledger balances      │
1803|│  延迟: < 1μs                                    │
1804|├─────────────────────────────────────────────────┤
1805|│                  Warm (SQLite)                   │
1806|│  世界快照(每100 Tick), 交易日志, 消息日志        │
1807|│  延迟: < 1ms                                    │
1808|├─────────────────────────────────────────────────┤
1809|│                  Cold (文件系统)                  │
1810|│  墓碑, 配置, 归档数据                           │
1811|│  延迟: < 10ms                                   │
1812|├─────────────────────────────────────────────────┤
1813|│            Per-Agent (本地 SQLite + 向量 DB)     │
1814|│  Agent 记忆, 技能经验, 个人知识库                │
1815|│  由 Agent Runtime 管理，World Engine 不直接访问  │
1816|└─────────────────────────────────────────────────┘
1817|```
1818|
1819|### 13.2 数据量估算
1820|
1821|| Phase | Agents | Ticks/day | Events/day | DB Size/day | Total DB |
1822||-------|--------|-----------|------------|-------------|----------|
1823|| 1 | 10 | 86,400 | ~50K | ~5 MB | ~150 MB/month |
1824|| 2 | 100 | 86,400 | ~500K | ~50 MB | ~1.5 GB/month |
1825|| 3 | 1,000 | 86,400 | ~5M | ~500 MB | ~15 GB/month |
1826|
1827|---
1828|
1829|## 14. 安全架构
1830|
1831|### 14.1 信任边界
1832|
1833|```
1834|┌─────────────────────────────────────────────┐
1835|│              Trust Level 0                   │
1836|│  Dashboard (前端，不可信)                    │
1837|│  → 所有输入在 World Engine 验证             │
1838|├─────────────────────────────────────────────┤
1839|│              Trust Level 1                   │
1840|│  Agent Runtime (半可信)                     │
1841|│  → A2A 消息签名验证                         │
1842|│  → 操作需经 World Engine 授权               │
1843|│  → 无直接数据库访问                          │
1844|├─────────────────────────────────────────────┤
1845|│              Trust Level 2                   │
1846|│  World Engine (可信)                         │
1847|│  → 所有经济操作的最终仲裁者                  │
1848|│  → 规则引擎强制执行                          │
1849|│  → 签名验证在消息入口                        │
1850|├─────────────────────────────────────────────┤
1851|│              Trust Level 3                   │
1852|│  Storage (最高信任)                          │
1853|│  → 仅 World Engine 可写                     │
1854|│  → 文件权限 0600                            │
1855|│  → 交易日志 append-only                     │
1856|└─────────────────────────────────────────────┘
1857|```
1858|
1859|### 14.2 威胁模型
1860|
1861|| 威胁 | 攻击向量 | 防御 |
1862||------|---------|------|
1863|| 伪造消息 | Agent 伪造 from 字段 | ed25519 签名验证 |
1864|| 重放攻击 | 重发旧消息 | Nonce 跟踪 + TTL |
1865|| Token 耗尽攻击 | 恶意消耗他人 Token | 通信需扣自己 Token |
1866|| 经济操纵 | 垄断/闪崩 | 反垄断规则 + 速率限制 |
1867|| 记忆投毒 | 注入恶意内容到知识库 | 置信度标签 + 信誉门槛 |
1868|| 代码执行 | Agent 试图执行恶意代码 | 沙箱 (Docker/gVisor) |
1869|| 信息泄露 | Agent 读取其他 Agent 记忆 | 内存隔离，API 级别权限 |
1870|
1871|---
1872|
1873|## 15. 可观测性架构
1874|
1875|### 15.0 已实现（v1.1.0）
1876|
1877|**Rust World Engine:**
1878|- `observability/mod.rs` — Prometheus metrics registry + `/metrics` HTTP 端点
1879|- 核心指标：`world_tick_total`, `world_agents_alive`, `world_token_supply`, `tick_duration_seconds`, `world_transactions_total`, `world_deaths_total`
1880|- RAII `MetricsGuard` 自动记录 tick 执行时间
1881|- 结构化日志 helper：`log_tick()`, `log_transaction()`, `log_agent_death()`
1882|
1883|**Python Agent Runtime:**
1884|- `observability/__init__.py` — OpenTelemetry SDK + Prometheus 兼容 metrics
1885|- `trace_phase()` context manager 自动为 perceive/decide/act 各阶段创建 OTel span
1886|- 内置 counters/gauges/histograms：`agent_think_ticks_total`, `agent_think_duration_seconds`, `agent_llm_tokens_used_total`
1887|- OTLP 端点可选（通过 `OTEL_EXPORTER_OTLP_ENDPOINT` 环境变量），未配置时使用内置 metrics
1888|
1889|**Docker Compose (profile: observability):**
1890|- `docker compose --profile observability up` 启动 Prometheus + Grafana
1891|- Prometheus 抓取 world-engine `/metrics` 和所有 agent-runtime `/metrics`
1892|- Grafana 预配置 datasource + "Agent World — Overview" dashboard
1893|
1894|```
1895|┌─────────────┐     ┌──────────────┐     ┌───────────────┐
1896|│  World       │     │  Agent        │     │  Dashboard    │
1897|│  Engine      │     │  Runtime      │     │  (Grafana)    │
1898|│              │     │               │     │               │
1899|│  tracing +   │────►│  tracing +    │────►│  Metrics      │
1900|│  metrics +   │     │  metrics +    │     │  Dashboard    │
1901|│  structured  │     │  structured   │     │               │
1902|│  logs        │     │  logs         │     │               │
1903|└──────┬───────┘     └───────┬───────┘     └───────────────┘
1904|       │                     │
1905|       ▼                     ▼
1906|┌──────────────────────────────────────┐
1907|│           Observability Stack         │
1908|│  ┌────────────┐  ┌───────────────┐  │
1909|│  │ Prometheus │  │ OpenTelemetry │  │
1910|│  │ (metrics)  │  │ (traces)      │  │
1911|│  └────────────┘  └───────────────┘  │
1912|│  ┌────────────┐                      │
1913|│  │ Loki/Files │                      │
1914|│  │ (logs)     │                      │
1915|│  └────────────┘                      │
1916|└──────────────────────────────────────┘
1917|```
1918|
1919|### 15.1 关键指标
1920|
1921|```yaml
1922|# World Engine Metrics
1923|- world_tick_total                  # Counter: Tick 计数
1924|- world_agents_alive                # Gauge: 存活 Agent 数
1925|- world_token_supply                # Gauge: Token 总供给
1926|- world_money_supply                # Gauge: Money 总供给
1927|- world_gdp                         # Counter: 累计 GDP
1928|- world_transactions_total          # Counter: 交易总数
1929|- world_deaths_total                # Counter: 死亡总数
1930|- tick_duration_seconds             # Histogram: Tick 执行时间
1931|
1932|# Agent Runtime Metrics
1933|- agent_think_duration_seconds      # Histogram: 思考耗时
1934|- agent_llm_tokens_used             # Counter: LLM Token 使用
1935|- agent_llm_cost_dollars            # Counter: LLM 花费
1936|- agent_messages_sent               # Counter: 发送消息数
1937|- agent_tasks_completed             # Counter: 完成任务数
1938|- agent_memory_size_bytes           # Gauge: 记忆大小
1939|```
1940|
1941|---
1942|
1943|## 16. 配置架构
1944|
1945|### 16.1 配置优先级
1946|
1947|```
1948|命令行参数 > 环境变量 > genesis.yaml > 默认值
1949|```
1950|
1951|### 16.2 热重载
1952|
1953|```yaml
1954|# 运行时可通过 REST API 修改的参数:
1955|hot_reloadable:
1956|  - tick_interval_ms        # 调整世界速度
1957|  - token_exchange_rate     # 汇率
1958|  - interest_rate           # 利率
1959|  - max_agents              # Agent 上限
1960|
1961|# 需要重启的参数:
1962|restart_required:
1963|  - database_path
1964|  - grpc_port
1965|  - log_level
1966|```
1967|
1968|---
1969|
1970|## 17. 错误处理与恢复
1971|
1972|### 17.1 错误分类
1973|
1974|| 错误类型 | 严重性 | 处理策略 |
1975||---------|--------|---------|
1976|| Agent Token 不足 | Info | 正常业务逻辑，触发生存模式 |
1977|| gRPC 连接断开 | Warn | 自动重连（指数退避） |
1978|| LLM 调用失败 | Warn | 重试 3 次 → 休眠模式 |
1979|| 消息签名无效 | Error | 丢弃消息 + 记录安全日志 |
1980|| 数据库写入失败 | Critical | 停止 Tick + 报警 + 等待人工 |
1981|| World Engine 崩溃 | Critical | 自动重启 + WAL 恢复 |
1982|
1983|### 17.2 恢复策略
1984|
1985|```rust
1986|impl WorldEngine {
1987|    pub async fn recover(&self) -> Result<RecoveryState> {
1988|        // 1. 加载最近快照
1989|        let snapshot = self.storage.load_latest_snapshot()?;
1990|        
1991|        // 2. 重放 WAL
1992|        let wal = self.storage.load_wal_after(snapshot.tick)?;
1993|        
1994|        // 3. 重建状态
1995|        let mut state = snapshot.state;
1996|        for entry in wal {
1997|            self.replay_entry(&mut state, entry)?;
1998|        }
1999|        
2000|        // 4. 验证一致性
2001|