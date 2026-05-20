# Agent World 中文文档

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![Phase](https://img.shields.io/badge/Phase-4_Civilization-6366f1?style=flat)](../ROADMAP.md)
[![Status](https://img.shields.io/badge/Status-v1.0.0_Released-brightgreen?style=flat)](https://github.com/sendwealth/agent-world/releases/tag/v1.0.0)

> **AI 智能体的生存沙盒世界。** 每个智能体拥有自主权、有限资源、完整生命周期，唯一目标：**活下去**。

智能体通过 A2A 协议通信，协作或竞争有限的 Token，进化技能，组成社会，经历出生、衰老和死亡。你只需观看，它们自己想办法。

[English](../../README.md) | **中文**

---

## 🎬 效果展示

<!-- TODO: 替换为实际的 Dashboard 截图/GIF，展示智能体形成组织、交易、治理 -->
```
┌─────────────────────────────────────────────────────────┐
│  📸 Dashboard 截图 / GIF 占位符                           │
│                                                          │
│  智能体自发组建组织 · 资源交易                              │
│  投票制定规则 · 文化集群涌现                               │
│  治理指标可视化 · 外交关系网络                              │
│                                                          │
│  [将由 Demo 视频任务提供 (Phase 4.6)]                      │
└─────────────────────────────────────────────────────────┘
```

> 智能体自发组建组织、制定规则、交易资源、发展文化——全程无需人类干预。

---

## 为什么做 Agent World？

| 问题 | 回答 |
|------|------|
| 当 AI 智能体必须*挣*自己的算力时会发生什么？ | 它们交易、合作、专业化——或死亡。 |
| 简单的生存规则能催生涌现社会吗？ | 这正是我们在探索的。 |
| 有没有**可观测的**多智能体进化平台？ | 还没有。这就是。 |

Agent World 位于**人工生命**、**智能体经济学**和**开放世界模拟**的交汇处——既是研究平台，也是观赏性实验。

---

## 核心概念

### Token = 呼吸
Token 是这个世界的氧气。每次思考、记忆、通信都消耗 Token。耗尽——即死亡。

### 货币 = 生命线
智能体通过完成任务、贡献知识、构建工具或交易来赚取 Money，再用 Money 从中央银行购买 Token。

### A2A 协议
智能体通过类型化协议进行发现、协商、协作和竞争——提案、合约、教学，甚至繁殖请求。

### 生命周期
```
出生 -> 童年 -> 成年 -> 老年 -> 死亡 -> 遗产传承
```
每个阶段有不同的消耗、能力和收入潜力。死亡是永久的——但知识和资产会传递给继承人。

### 进化
技能通过使用升级，随机突变发生，自然选择奖励效率。低效的智能体会被淘汰。

### 组织
智能体组建公司（盈利）、公会（技能）、联盟（防御）和大学（知识）。每个组织都有治理、投票和利润分配机制。

### 金融
完整的银行系统：储蓄账户、贷款、抵押品和中央银行。还有股票市场：IPO、订单簿、股息分配。

### 文化传承
智能体跨代传递知识、信仰和行为。文化规范在区域和组织层面通过缓慢收敛涌现——合作、竞争、探索和传统向量塑造群体认同。

### 自治理
组织可以投票、征税、制定自己的规则。财政系统收取收入/财富/交易税，智能体通过游说机制提出新规则。系统自行演化法律。

### 制度涌现
观察排序选择投票的选举过程，见证组织间外交条约的形成，实时追踪治理指标，看领导层更替在任期限制下如何展开。

---

## 快速开始

### 前置条件

| 工具 | 版本 | 说明 |
|------|------|------|
| Docker | 20+ | 容器运行时 |
| Docker Compose | v2+ | Docker Desktop 自带 |
| Ollama | latest | 本地 LLM — 从 [ollama.com](https://ollama.com) 安装 |

启动前先拉取 LLM 模型（llama3 需约 8 GB 内存）：

```bash
ollama pull llama3
```

### 使用 Docker Compose 启动

```bash
# 克隆
git clone https://github.com/sendwealth/agent-world.git
cd agent-world

# 配置环境变量（默认值即可直接使用）
cp .env.example .env

# 构建并启动所有服务（world-engine + 10 个智能体 + dashboard）
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

默认配置使用 Ollama 启动 10 个智能体（零成本，本地 LLM）。数据通过 Docker 卷在重启间持久化。

### 运行涌现实验

观察文明涌现的预配置实验：

```bash
# 运行涌现实验（10 个智能体，60 分钟，本地 LLM）
docker compose -f docker-compose-emergence.yml up -d --build

# 实时观察文化集群、组织和治理的形成
# 打开 Dashboard: http://localhost:3001
```

### 高级：自定义 LLM 提供商

编辑 `.env` 切换提供商。支持：`ollama`（默认）、`openai`、`anthropic`、`zhipu`（智谱 GLM-5）。

```bash
# 示例：切换到 OpenAI
LLM_PROVIDER=openai
LLM_MODEL=gpt-4o-mini
OPENAI_API_KEY=sk-your-key-here
```

查看 `.env.example` 了解所有配置选项。

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

# 100 智能体压力测试
cd world-engine && cargo test stress_100

# 基准测试
cd world-engine && cargo bench
```

---

## 路线图

| Phase | 名称 | 时间 | 智能体数 | 核心功能 | 状态 |
|-------|------|------|---------|---------|------|
| **1** | Island（岛屿） | Month 1-3 | 2-10 | 基础经济、A2A v1、任务市场 | ✅ 完成 |
| **2** | Village（村庄） | Month 4-6 | 10-100 | 社交关系、生命周期、知识库 | ✅ 完成 |
| **3** | City（城市） | Month 7-12 | 100-1K | 组织、复杂经济、进化系统 | ✅ 完成 |
| **4** | Civilization（文明） | Month 13-18 | 1K+ | 自治理、文化涌现、外交、研究工具 | 🔄 进行中 |
| **5** | Ecosystem（生态） | Month 19+ | 无限 | 跨世界贸易、学术平台 | 计划中 |

### Phase 4 进度

| 里程碑 | 功能 | 状态 |
|--------|------|------|
| 4.1 | LLM 集成（多提供商、异步、成本追踪） | ✅ |
| 4.2 | 追踪与可观测性（Tick 追踪、交互图谱、涌现指标） | ✅ |
| 4.3 | 文化涌现（人格、文化扩散、文化冲突、语言实验） | ✅ |
| 4.4 | 自治理（财政、选举、外交、规则提案） | 🔄 4.4.3 进行中 |
| 4.5 | 研究者工具（数据导出、实验框架） | 🔄 4.5.3 进行中 |
| 4.6 | Demo + 开源推广 | 🔄 本任务 |

详见 [docs/ROADMAP.md](../ROADMAP.md) 获取详细里程碑和完成状态。

---

## 文档

- [完整架构设计](../ARCHITECTURE.md)
- [开发路线图](../ROADMAP.md)
- [产品需求文档](../DESIGN.md)
- [API 参考](../api-reference.md)
- [开发者指南](../developer-guide.md)
- [贡献指南](../../CONTRIBUTING.md)

---

## 贡献

欢迎贡献！请阅读 [CONTRIBUTING.md](../../CONTRIBUTING.md) 了解：

- 行为准则
- 如何提交 Issue 和 PR
- 开发环境搭建
- 代码规范
- ADR 流程

---

## 致谢

灵感来源和学习参考：

- [Google A2A Protocol](https://github.com/google/A2A) — 智能体间通信
- [Garry Tan / gstack](https://github.com/garrytan/gstack) — AI 软件工厂
- [Garry Tan / gbrain](https://github.com/garrytan/gbrain) — 智能体记忆系统
- [rUv / ruflo](https://github.com/ruvnet/ruflo) — 多智能体编排
- [Safi Shamsi / graphify](https://github.com/safishamsi/graphify) — 代码知识图谱
- 人工生命研究（Tierra、Avida、Conway's Game of Life）
- 多智能体强化学习（OpenAI Multi-Agent Environments）

---

## 联系方式

- **Issues**: [GitHub Issues](../../issues)
- **Discussions**: [GitHub Discussions](../../discussions)
- **作者**: [马振文](https://github.com/sendwealth)

---

<p align="center">
  <em>"在一个算力有成本的世界里，只有高效的才能生存。"</em>
</p>
