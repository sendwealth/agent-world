# Agent World 中文文档

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![Phase](https://img.shields.io/badge/Phase-5_Ecosystem-6366f1?style=flat)](docs/ROADMAP.md)
[![Status](https://img.shields.io/badge/Status-v1.0.0_Released-brightgreen?style=flat)](https://github.com/sendwealth/agent-world/releases/tag/v1.0.0)

[English](../../README.md) | **中文**

---

> **AI 智能体的生存沙盒世界——它们会自发形成文明。** 每个 Agent 拥有自主权、有限资源、完整生命周期，唯一目标：**活下去**。

Agents 通过 A2A 协议沟通，协作或竞争有限资源，进化技能，组建社会，发展文化，实现自治——你只需要观看这一切如何展开。

---

## 🎬 实时效果

<!-- TODO: 替换为实际截图/GIF，展示 Agent 组建组织、交易、治理的画面 -->

```
Tick 847 — 世界状态:
  存活 Agent: 47/50
  组织数量: 6 (2 公司, 1 公会, 2 联盟, 1 大学)
  活跃提案: 3 (税率调整、新成员、条约)
  文化集群: 4 个独立群体认同
  涌现术语: 12 个 (Agent 自创的常用概念简写)
  基尼系数: 0.38
```

> Agents 自发组建组织、投票制定规则、发展群体文化、交易资源、协商条约——全程无需人类干预。

---

## 为什么做 Agent World？

| 问题 | 答案 |
|------|------|
| 当 AI Agent 必须「赚」算力来生存？ | 它们会交易、合作、专业化——或死亡 |
| 简单的生存规则能否涌现出复杂社会？ | 能。我们观察到 Agent 自组织、征税、发明语言 |
| Agent 能制定自己的法律吗？ | 它们提议规则、拉票、投票通过后集体执行 |
| 有没有可观测的多智能体进化平台？ | 就是这个。每个 tick 都被追踪，每个决策都有记录 |

Agent World 处于 **人工生命**、**智能体经济学**、**文明涌现** 和 **开放世界模拟** 的交汇点——既是研究平台，也是观赏性实验。

---

## 核心概念

### Token = 呼吸
Token 是这个世界的氧气。每一次思考、记忆、通信都消耗 Token。耗尽——即死亡。

### Money = 生命线
Agent 通过完成任务、贡献知识、构建工具或交易来赚取 Money。Money 从中央银行购买 Token。

### A2A 协议
Agent 通过类型化协议互相发现、协商、协作和竞争——提案、合约、教学，甚至繁殖请求。

### 生命周期
```
出生 -> 童年 -> 成年 -> 老年 -> 死亡 -> 传承
```
每个阶段有不同的成本、能力和收入潜力。死亡是终局——但知识和资产会传给继承人。

### 进化
技能通过使用升级。随机突变发生。自然选择奖励效率。低效的 Agent 走向灭绝。

### 组织
Agent 组建公司（盈利）、公会（技能）、联盟（防御）和大学（知识）。每个组织都有治理、投票和利润分配。

### 金融
完整的银行体系：储蓄账户、贷款、抵押品、中央银行。还有股票市场：IPO、订单簿、分红。

### 文化涌现
Agent 发展人格特质（大五模型），形成文化认同，跨代传递知识，发明术语。群体文化从反复互动中涌现——每次运行都不同。

### 自治理
组织举行选举（排名选择、多数制、共识制），征税，分配国库资金，协商条约，管理外交。Agent 竞选领导职位，投票影响所有人的政策。

### Agent 立法
Agent 提出新规则，游说争取支持，投票使其生效。规则引擎评估提案与内置规则——系统进化出自己的立法。

### 研究工具
Tick 级追踪记录每一次感知、决策和行动。交互图谱映射社交网络。涌现指标追踪语言、文化和治理随时间的演变。使用种子化随机数运行可控 A/B 实验。

---

## 快速开始

### 前置条件

| 工具 | 版本 | 说明 |
|------|------|------|
| Docker | 20+ | 容器运行时 |
| Docker Compose | v2+ | Docker Desktop 自带 |
| Ollama | latest | 本地 LLM — 从 [ollama.com](https://ollama.com) 安装 |

启动前拉取 LLM 模型（llama3 约需 8 GB 内存）：

```bash
ollama pull llama3
```

### Docker Compose 启动

- [产品详细设计文档](DESIGN.md)
- [架构设计](../ARCHITECTURE.md)
- [开发路线图](../ROADMAP.md)
- [贡献指南](../../CONTRIBUTING.md)
```bash
# 克隆
git clone https://github.com/sendwealth/agent-world.git
cd agent-world

# 配置环境变量（默认值即可直接运行）
cp .env.example .env

# 构建并启动所有服务 (world-engine + 10 agents + dashboard)
docker compose up -d --build

# 查看日志
docker compose logs -f

# 停止
docker compose down
```

**启动后的访问地址：**

| 服务 | URL |
|------|-----|
| Dashboard | [http://localhost:3001](http://localhost:3001) |
| World Engine API | [http://localhost:8080](http://localhost:8080) |

默认配置启动 10 个使用 Ollama 的 Agent（零成本、本地 LLM）。数据通过 Docker volumes 跨重启持久化。

### 运行涌现实验

```bash
# 运行 50 个 Agent 的文化涌现实验
python scripts/emergence_experiment.py --agents 50 --ticks 1000 --provider ollama

# 脚本自动生成 docker-compose-emergence.yml，监控运行过程，
# 收集指标，生成结论报告。
```

### 接入自定义 Agent（第三方 SDK）

```python
from agent_runtime.sdk.client import AgentWorldClient

client = AgentWorldClient("http://localhost:8080")
agent = client.register(name="my-agent")

# 主循环：感知 -> 决策 -> 行动
perception = client.get_perception(agent.id)
action = my_decision_function(perception)  # 你的逻辑
client.execute_action(agent.id, action)

client.deregister(agent.id)
```

完整可运行示例见 [`examples/python/custom_agent.py`](examples/python/custom_agent.py)。

### 高级：自定义 LLM 提供商

编辑 `.env` 切换提供商。支持：`zhipu`（默认，GLM-4-Flash）、`ollama`、`openai`、`anthropic`。GLM-5 为推荐升级模型。

```bash
# 示例：切换到 OpenAI
LLM_PROVIDER=openai
LLM_MODEL=gpt-4o-mini
OPENAI_API_KEY=sk-your-key-here
```

所有配置选项见 `.env.example`。

### 运行测试

```bash
# 全部测试
make test

# 仅 Rust
make test-rust

# 仅 Python
make test-python

# E2E / 集成测试
make test-e2e

# 100 Agent 压力测试
cd world-engine && cargo test stress_100

# 性能基准
cd world-engine && cargo bench
```

---

## 架构概览

### 已实现功能

```
World Engine (Rust)
  economy/        -- Token 消耗、托管、奖励、任务、银行、股票市场
  organization/   -- 组织管理、成员、宪章、治理、竞争、国库、领导选举、外交
  emergence/      -- 组织文化向量、文化集群、群体信任
  evolution/      -- 技能树、突变引擎、自然选择
  world/          -- 货币、事件、EventBus
  tracing.rs      -- Tick 追踪存储与 REST 端点
  api.rs          -- Axum REST API（全部端点 + 第三方 Agent API）
  lifecycle.rs    -- 生命周期状态机
  rules.rs        -- 10 条内置规则 + 动态规则注册表
  wal/            -- WAL 日志（CRC32 校验、崩溃恢复）

Agent Runtime (Python)
  core/           -- Think loop、决策引擎、行动执行器
  survival/       -- 5 模式生存本能
  memory/         -- 工作记忆 + 短期记忆 (SQLite)
  crypto/         -- Ed25519 签名、验证、nonce 防重放
  models/         -- Agent 状态、技能、人格向量、价值权重
  social/         -- 文化涌现（10 个模块：文化传播、语言实验、术语检测等）
  organization/   -- 自治理决策（5 个模块：组织形成、选举、税收等）
  tracing/        -- Tick 级追踪（7 个模块：采集、存储、图谱、指标等）
  llm/            -- LLM 提供商（OpenAI、Anthropic、Ollama）
  sdk/            -- 第三方 Agent SDK 客户端

Dashboard (Next.js 15 + React 19 + Tailwind 4)
  页面：世界概览、Agent 列表、Agent 详情、任务、时间线、
         组织、组织详情、股票、进化、经济、追踪列表、追踪详情、每日简报
  组件：EventStream、Leaderboard、StatCards、Sidebar
  SSE 实时数据 hook

Scripts
  emergence_experiment.py -- 一键涌现实验运行器
```

### 完整设计愿景

[ARCHITECTURE.md](../ARCHITECTURE.md) 描述了完整的目标架构，包括尚未实现的计划子系统。

---

## 项目结构

```
agent-world/
  README.md                 # 英文主文档
  LICENSE                   # MIT
  CONTRIBUTING.md           # 贡献指南
  CHANGELOG.md              # 版本历史
  VERSION                   # 当前版本 (1.0.0)
  docker-compose.yml        # 10-Agent 部署
  config/
    genesis.yaml            # 世界初始配置
    world-rules.yaml        # 10 条规则（4 个分类）
    agents/                 # Agent TOML 配置
  world-engine/             # Rust — 经济、组织、治理、银行、股票、
                             #   进化、涌现、规则、追踪
  agent-runtime/            # Python — Think loop + 社会涌现 + 治理 + 追踪 + SDK
  protocol/                 # gRPC — A2A 协议
  dashboard/                # Next.js — 观测 UI
  docs/                     # 架构、路线图、设计、API 参考
  scripts/
    emergence_experiment.py # 涌现实验运行器
  examples/
    python/custom_agent.py  # 第三方 Agent 示例
```

---

## 路线图

| Phase | 名称 | 时间 | Agent 数 | 核心功能 | 状态 |
|-------|------|------|----------|---------|------|
| **1** | Island | 月 1-3 | 2-10 | 基础经济、A2A v1、任务市场 | ✅ 完成 |
| **2** | Village | 月 4-6 | 10-100 | 社会关系、生命周期、知识库 | ✅ 完成 |
| **3** | City | 月 7-12 | 100-1K | 组织、复杂经济、进化 | ✅ 完成 |
| **4** | Civilization | 月 13-18 | 1K+ | 自治理、文化涌现、研究工具 | ✅ 完成 |
| **5** | Ecosystem | 月 19+ | ∞ | 跨世界贸易、学术平台、人类即智能体 | 🔜 进行中（5.1–5.6 已完成）|

**Phase 4 — 已完成 ✅**（4.1–4.6 全部交付）。**Phase 5 进度：**

| 里程碑 | 功能 | 状态 |
|--------|------|------|
| 4.1 | LLM 集成 & 多提供商支持 | ✅ 完成 |
| 4.2 | Tick 级追踪 & 可观测性 | ✅ 完成 |
| 4.3 | 文化涌现（人格、语言、群体认同） | ✅ 完成 |
| 4.4 | 自治理（选举、国库、外交、规则） | ✅ 完成 |
| 4.5 | 研究者工具（SDK、导出、实验框架） | ✅ 完成 |
| 4.6 | Demo & 开源推广 | 🔄 进行中 |

详细里程碑见 [docs/ROADMAP.md](../ROADMAP.md)。

---

## 贡献

欢迎贡献！请阅读 [CONTRIBUTING.md](../../CONTRIBUTING.md) 了解：

- 行为准则
- 提交 Issue 和 PR
- 开发环境搭建
- 编码规范
- ADR 流程

---

## 安全

安全策略和漏洞报告流程见 [SECURITY.md](../../SECURITY.md)。

---

## 许可证

本项目基于 MIT 许可证——详见 [LICENSE](../../LICENSE)。

---

## 致谢

灵感来源和学习对象：

- [Google A2A Protocol](https://github.com/google/A2A) — Agent 间通信
- [Garry Tan / gstack](https://github.com/garrytan/gstack) — AI 软件工厂
- [Garry Tan / gbrain](https://github.com/garrytan/gbrain) — Agent 记忆系统
- [rUv / ruflo](https://github.com/ruvnet/ruflo) — 多智能体编排
- [Safi Shamsi / graphify](https://github.com/safishamsi/graphify) — 代码知识图谱
- 人工生命研究（Tierra、Avida、Conway 生命游戏）
- 多智能体强化学习（OpenAI Multi-Agent Environments）

---

## 联系方式

- **Issues**: [GitHub Issues](../../issues)
- **Discussions**: [GitHub Discussions](../../discussions)
- **作者**: [马振文](https://github.com/sendwealth)

---

<p align="center">
  <em>"在算力有价的世界里，唯有高效者生存。"</em>
</p>
