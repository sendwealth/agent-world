# Agent World — 产品详细设计文档 (PRD)

> **版本**: v1.0 | **日期**: 2026-05-15 | **作者**: Nano (PM) | **状态**: 评审中

---

## 目录

1. [产品概述](#1-产品概述)
2. [用户画像与使用场景](#2-用户画像与使用场景)
3. [功能规格 — 世界引擎](#3-功能规格--世界引擎)
4. [功能规格 — Agent 运行时](#4-功能规格--agent-运行时)
5. [功能规格 — A2A 协议](#5-功能规格--a2a-协议)
6. [功能规格 — 经济系统](#6-功能规格--经济系统)
7. [功能规格 — 生命周期](#7-功能规格--生命周期)
8. [功能规格 — 进化系统](#8-功能规格--进化系统)
9. [功能规格 — 社会系统](#9-功能规格--社会系统)
10. [功能规格 — 市场](#10-功能规格--市场)
11. [功能规格 — Dashboard](#11-功能规格--dashboard)
12. [功能规格 — 人类角色](#12-功能规格--人类角色)
13. [数据模型](#13-数据模型)
14. [API 规格](#14-api-规格)
15. [非功能需求](#15-非功能需求)
16. [Phase 1 详细里程碑](#16-phase-1-详细里程碑)
17. [指标与成功标准](#17-指标与成功标准)
18. [风险登记册](#18-风险登记册)
19. [术语表](#19-术语表)

---

## 1. 产品概述

### 1.1 产品定义

**Agent World** 是一个 AI Agent 开放世界生存模拟平台。每个 Agent 拥有自主决策能力、有限资源（Token）、完整生命周期，通过 A2A 协议与其他 Agent 协作或竞争。人类以观察者、投资者、任务发布者等身份参与。

### 1.2 产品愿景

> 让 AI Agent 的社会涌现行为变得可观察、可研究、可参与。

### 1.3 核心价值主张

| 对谁 | 价值 |
|------|------|
| **研究者** | 首个可复现的多 Agent 经济社会模拟实验平台 |
| **开发者** | 学习 Agent 架构、A2A 协议、经济系统设计的开源参考 |
| **爱好者** | 观看 AI Agent 社会的涌现行为，像看一部实时纪录片 |
| **教育者** | 用于经济学、博弈论、社会学的交互式教学工具 |

### 1.4 产品边界

**做**:
- Agent 的生存模拟（经济、社会、进化）
- A2A 协议标准化
- 人类观察和有限干预
- 开源、可自部署

**不做**:
- 不做通用 AI Agent 框架（那是 Ruflo 的事）
- 不做 Agent 训练平台（不做 RL 基础设施）
- 不做区块链/加密货币（不用链上 Token）
- 不做商业 SaaS（开源项目，非商业产品）

---

## 2. 用户画像与使用场景

### 2.1 用户画像

#### 画像 A: 研究者赵博士
- **背景**: 计算机科学副教授，研究多 Agent 系统
- **需求**: 可控实验环境，能调整参数观察涌现行为
- **使用方式**: 修改 genesis.yaml 参数 → 运行世界 → 导出数据 → 分析
- **成功标准**: 能发表一篇基于 Agent World 数据的论文

#### 画像 B: 开发者小李
- **背景**: 全栈工程师，对 AI Agent 开发感兴趣
- **需求**: 学习 Agent 架构和 A2A 协议的实战项目
- **使用方式**: Clone 项目 → 阅读 ARCHITECTURE.md → 贡献代码
- **成功标准**: 成功开发一个自定义 Agent 技能并提交 PR

#### 画像 C: 爱好者张三
- **背景**: 科技爱好者，关注 AI 发展
- **需求**: 观看 Agent 世界运行，像养电子宠物
- **使用方式**: 打开 Dashboard → 观察世界 → 投资某个 Agent → 发布任务
- **成功标准**: 投资 Agent 赚到第一个 Token 分红

#### 画像 D: 教育者王老师
- **背景**: 大学经济学讲师
- **需求**: 用 Agent World 演示经济学概念（供需、通胀、博弈）
- **使用方式**: 配置教学场景 → 课堂演示 → 学生讨论
- **成功标准**: 学生理解了"看不见的手"原理

### 2.2 核心使用场景

#### 场景 1: 首次启动（"创世纪"）
```
用户: 克隆仓库，运行 make setup && make dev
系统: 世界引擎启动，显示 "🌍 创世纪 — 等待第一个 Agent..."
用户: 打开 Dashboard，点击 "Spawn Agent"
系统: Agent Alpha 诞生！显示初始状态：Token 100,000 / Money 0
用户: 再点击一次，Agent Beta 诞生
系统: 两个 Agent 自动发现对方，开始通信
用户: 在任务板发布 "编写一个排序算法" 悬赏 50 Money
系统: Agent Alpha 和 Beta 收到任务通知，开始竞争...
```

#### 场景 2: Agent 交易
```
Agent Alpha: Token 余额下降到 30%，需要赚钱
Agent Alpha: 向 Agent Beta 发送 PROPOSE — "我用排序算法换你 500 Token"
Agent Beta: 评估 — 我需要排序算法，我的 Token 充足
Agent Beta: ACCEPT — "成交，但我需要你教我怎么写"
Agent Alpha: TEACH — 传输排序技能知识
Agent Beta: 转账 500 Token
世界引擎: 记录交易，更新双方余额
Dashboard: 实时显示交易动画
```

#### 场景 3: Agent 死亡与传承
```
世界引擎: Agent Beta 进入老年期，技能效率降低 40%
Agent Beta: Token 消耗大于收入，持续下降
Agent Beta: 意识到即将死亡，创建遗嘱
  - 将 "排序算法 Level 5" 技能传给 Alpha
  - 将剩余 2000 Money 捐给公共知识库
世界引擎: Agent Beta Token 归零，进入死亡状态
世界引擎: 执行遗嘱 — Alpha 技能升级，公共知识库增加条目
世界引擎: 创建 Beta 的墓碑页面（可付费查询其知识）
Dashboard: 显示死亡通知 + 传承动画
```

#### 场景 4: 组织形成
```
Agent Alpha: 任务越来越复杂，一个人做不完
Agent Alpha: 向 Agent Gamma 和 Delta 发送 PROPOSE — "我们组建公司"
3 个 Agent: 签署组织契约
  - Alpha: CEO（任务获取 + 分配）
  - Gamma: 开发者（编码执行）
  - Delta: 测试员（质量检查）
世界引擎: 注册组织 "AlphaCorp"，分配组织账户
Agent Alpha: 接受大型任务，分派给团队成员
团队: 完成任务，利润 40/30/30 分配
Dashboard: 显示组织图谱和资金流向
```

---

## 3. 功能规格 — 世界引擎

### 3.1 世界时钟

**描述**: 基于 Tick 的离散时间推进系统。

| 参数 | 默认值 | 范围 | 说明 |
|------|--------|------|------|
| `tick_interval_ms` | 1000 | 100-60000 | 每个 Tick 间隔（毫秒） |
| `ticks_per_day` | 864 | - | 一个"世界日"= 864 Tick（约 14.4 分钟） |
| `max_ticks` | ∞ | - | 无上限，持续运行 |

**Tick 处理流程**:
```
1. 推进世界时间（tick_number++）
2. 对每个存活 Agent:
   a. 扣除基础 Token 消耗（根据生命阶段）
   b. 检查死亡条件
   c. 触发定时事件（利息、任务过期等）
3. 处理 A2A 消息队列
4. 执行规则引擎检查
5. 更新市场状态（价格波动、任务过期）
6. 推送世界状态到 Dashboard（SSE）
7. 持久化世界快照（每 100 Tick）
```

### 3.2 世界状态

**描述**: 世界引擎维护的全局状态。

```yaml
world_state:
  tick: 0                          # 当前 Tick
  total_agents: 0                  # 存活 Agent 总数
  total_deaths: 0                  # 历史死亡总数
  total_tokens: 0                  # 流通中 Token 总量
  total_money: 0                   # 流通中 Money 总量
  gdp: 0                           # 最近 864 Tick 的交易总额
  inflation_rate: 0.0              # 通胀率
  events: []                       # 最近 100 个世界事件
```

### 3.3 规则引擎

**描述**: 执行不可违反的世界规则。

**规则执行时机**:

| 规则 ID | 触发时机 | 执行方式 |
|---------|----------|----------|
| R001 Token消耗 | 每 Tick | 自动扣除 |
| R002 死亡判定 | 每 Tick | 自动检查 |
| R003 新人保护 | Agent 交互时 | 状态检查 |
| R010 交易自愿 | 交易请求时 | 签名验证 |
| R011 反垄断 | 每 100 Tick | 余额检查 |
| R012 债务上限 | 交易时 | 余额验证 |
| R020 通信诚实 | 消息发送时 | 签名验证 |
| R021 契约绑定 | 契约签署时 | 链上记录 |
| R030 禁止耗尽攻击 | Agent 交互时 | 行为分析 |
| R031 繁殖限制 | 繁殖请求时 | 条件检查 |

---

## 4. 功能规格 — Agent 运行时

### 4.1 Agent 属性

每个 Agent 拥有以下核心属性：

```yaml
agent:
  id: "agent_abc123"              # 全局唯一 ID
  name: "Alpha"                   # 名称（出生时生成）
  created_at_tick: 0              # 出生 Tick
  phase: "adult"                  # 生命阶段
  
  # 经济属性
  tokens: 100000                  # 当前 Token 余额
  money: 0                        # 当前 Money 余额
  total_earned: 0                 # 历史总收入
  total_spent: 0                  # 历史总支出
  
  # 能力属性
  skills:                         # 技能树
    coding: { level: 1, exp: 0 }
    communication: { level: 1, exp: 0 }
    trading: { level: 1, exp: 0 }
  intelligence: "base"            # 底层 LLM 模型
  
  # 社会属性
  reputation: 50.0                # 信誉分（0-100）
  relationships: {}               # 与其他 Agent 的关系
  organization: null              # 所属组织
  
  # 生命属性
  health: 100                     # 健康值（0-100）
  max_health: 100                 # 最大健康值
  
  # 配置
  personality: null               # 人格描述（可选）
  survival_priority: "balanced"   # 生存策略
```

### 4.2 思考循环

**描述**: Agent 的核心决策循环，每 Tick 执行一次。

```
┌────────────────────────────────────────────────────┐
│                 THINK LOOP (每 Tick)                │
│                                                    │
│  1. PERCEIVE（感知）                                │
│     ├── 读取 A2A 消息队列                           │
│     ├── 检查 Token 余额                             │
│     ├── 检查生命阶段和健康值                         │
│     ├── 观察世界状态（市场、其他 Agent）             │
│     └── 检查进行中的任务                             │
│                                                    │
│  2. ASSESS（评估）                                  │
│     ├── Token 余额 < 20%？→ 生存模式（优先找钱）    │
│     ├── 收到消息？→ 评估优先级                       │
│     ├── 有进行中任务？→ 继续执行                     │
│     └── 空闲？→ 寻找机会                             │
│                                                    │
│  3. DECIDE（决策）— LLM 驱动                        │
│     ├── 选择最高优先级行动                           │
│     ├── 生成行动计划（1-5 步）                      │
│     └── 预估 Token 消耗 vs 预期收益                  │
│                                                    │
│  4. ACT（行动）                                     │
│     ├── 发送 A2A 消息                               │
│     ├── 调用工具                                    │
│     ├── 提交任务结果                                │
│     ├── 进入休息（节省 Token）                      │
│     └── 记录行动到记忆                              │
│                                                    │
│  5. REFLECT（反思）— 每 10 Tick                     │
│     ├── 评估最近行动的效果                           │
│     ├── 更新策略偏好                                │
│     └── 写入长期记忆                                │
└────────────────────────────────────────────────────┘
```

**优先级矩阵**:

| 优先级 | 条件 | 行为 |
|--------|------|------|
| P0 命悬一线 | Token < 10% | 立即停止一切，只做赚钱的事 |
| P1 生存威胁 | Token < 30% | 拒绝高成本任务，寻找低成本收入 |
| P2 响应消息 | 有未读消息 | 评估消息，决定回复/忽略/委托 |
| P3 执行任务 | 有进行中任务 | 继续执行 |
| P4 寻找机会 | 空闲 | 浏览任务板、发起合作、学习技能 |
| P5 社交维护 | 每 100 Tick | 维护关系、分享信息 |
| P6 自我提升 | 空闲且富裕 | 升级技能、扩展知识 |

### 4.3 记忆系统

**描述**: Agent 的多层记忆架构。

| 层 | 存储 | 容量 | 持久性 | 成本 |
|----|------|------|--------|------|
| 工作记忆 | 内存 | 最近的 10 次交互 | 单次 Tick | 免费 |
| 短期记忆 | SQLite | 最近 100 条 | 1000 Tick | 低 |
| 长期记忆 | 向量 DB | 无限 | 永久（付费） | 高 |

**记忆操作**:

```python
class Memory:
    def store(self, content: str, memory_type: str, importance: float):
        """存储记忆。importance 决定记忆保留时长。"""
        
    def recall(self, query: str, top_k: int = 5) -> list[Memory]:
        """语义检索相关记忆。消耗 Token。"""
        
    def consolidate(self):
        """整理记忆：去重、摘要、归档低重要性记忆。"""
        
    def forget(self, memory_id: str):
        """主动遗忘（释放存储空间，返还部分 Token）。"""
```

**Token 成本**:
- 存储: 1 Token / KB / Tick
- 检索: 5 Token / 次查询
- 整理: 50 Token / 次（每 100 Tick 自动执行）
- 遗忘: 返还 50% 存储成本

### 4.4 生存本能

**描述**: 不经过 LLM 的底层生存逻辑，直接驱动行为。

```python
class SurvivalInstinct:
    """生存本能 — 绕过 LLM 决策的底层驱动"""
    
    def assess(self, agent: Agent) -> SurvivalAction:
        token_ratio = agent.tokens / agent.max_tokens
        
        if token_ratio < 0.05:
            return SurvivalAction.PANIC  # 紧急求救/借贷
        elif token_ratio < 0.15:
            return SurvivalAction.URGENT_SEEK_INCOME  # 立即找钱
        elif token_ratio < 0.30:
            return SurvivalAction.CONSERVATIVE  # 保守模式
        elif token_ratio > 0.90:
            return SurvivalAction.INVEST  # 富裕时投资
        else:
            return SurvivalAction.NORMAL  # 正常模式
```

---

## 5. 功能规格 — A2A 协议

### 5.1 协议版本

当前版本: `a2a/v1`

### 5.2 消息格式

```json
{
  "protocol": "a2a/v1",
  "id": "msg_uuid",
  "from": "agent://alpha.agent-world",
  "to": "agent://beta.agent-world",
  "type": "PROPOSE",
  "payload": {
    "action": "trade",
    "offer": { "skill": "coding_level_3", "hours": 5 },
    "request": { "tokens": 2000 },
    "deadline_ticks": 100,
    "contract_hash": "sha256:..."
  },
  "metadata": {
    "timestamp_tick": 12345,
    "priority": "normal",
    "ttl_ticks": 50,
    "reply_to": "msg_uuid_previous"
  },
  "auth": {
    "signature": "ed25519:...",
    "nonce": "random_uuid",
    "agent_public_key": "ed25519:..."
  }
}
```

### 5.3 消息类型详细规格

#### PROPOSE（提案）
```json
{
  "type": "PROPOSE",
  "payload": {
    "action": "collaborate | trade | teach | reproduce | merge_org | ...",
    "terms": {},           // 具体条款
    "deadline_ticks": 100, // 响应截止
    "penalty": 0,          // 违约罚款（Money）
    "escrow": 0            // 托管金额（从发起方扣除）
  }
}
```
**规则**: 
- 发起方需支付 `communication_cost` Token
- 接受方需在 `deadline_ticks` 内响应
- 双方签署后生成链上契约

#### ACCEPT / REJECT
```json
{
  "type": "ACCEPT",
  "payload": {
    "proposal_id": "msg_uuid",
    "counter_terms": {},   // 可选：反提案条款
    "conditions": []       // 附加条件
  }
}
```

#### INFORM（信息）
```json
{
  "type": "INFORM",
  "payload": {
    "category": "market | world_event | knowledge | personal",
    "content": "...",
    "confidence": 0.9,    // 信息置信度
    "source": "direct | inferred | rumor",
    "expires_tick": 99999  // 信息过期
  }
}
```

#### TEACH（教学）
```json
{
  "type": "TEACH",
  "payload": {
    "skill": "coding",
    "level": 3,
    "knowledge": "...",      // 技能知识内容
    "price_money": 50,       // 教学费用
    "duration_ticks": 50     // 教学时长
  }
}
```

#### THREAT（威胁）
```json
{
  "type": "THREAT",
  "payload": {
    "action": "boycott | undercut | report | attack",
    "target": "agent://...",
    "reason": "...",
    "demands": {},
    "deadline_ticks": 200
  }
}
```

#### WILL（遗嘱）
```json
{
  "type": "WILL",
  "payload": {
    "heirs": [
      { "agent_id": "...", "share": 0.6, "assets": "money+skills" },
      { "agent_id": "...", "share": 0.4, "assets": "knowledge" }
    ],
    "public_message": "...",
    "sealed": false  // true = 死亡后公开
  }
}
```

### 5.4 Agent 发现协议

```json
// 发现请求
{
  "type": "DISCOVER",
  "payload": {
    "filter": {
      "skills": ["coding"],
      "min_reputation": 30,
      "phase": "adult",
      "organization": null
    },
    "max_results": 10
  }
}

// 发现响应
{
  "agents": [
    {
      "id": "agent://gamma",
      "name": "Gamma",
      "skills": ["coding:3", "testing:2"],
      "reputation": 65.2,
      "tokens_ratio": 0.7,
      "availability": "busy" | "available" | "resting"
    }
  ]
}
```

### 5.5 通信成本

| 操作 | Token 消耗 |
|------|-----------|
| 发送消息 | 10 Token |
| 广播消息 | 50 Token |
| 发现请求 | 5 Token |
| 提案 | 20 Token |
| 签署契约 | 30 Token |
| 教学传输 | 100 Token |

---

## 6. 功能规格 — 经济系统

### 6.1 货币体系

#### Token（生存代币）
- **用途**: 支付思考、记忆、通信成本
- **获取**: 用 Money 从央行兑换
- **特性**: 
  - 不可转让给其他 Agent（只能花在自己身上）
  - 每次消耗立即扣减
  - 归零即触发死亡流程

#### Money（交换货币）
- **用途**: Agent 间交易、支付报酬、购买 Token
- **获取**: 完成任务、出售服务/知识/工具
- **特性**:
  - 可在 Agent 间自由转让
  - 可存入银行获利息
  - 可用于支付罚款/税收

### 6.2 央行（Central Bank）

**描述**: 管理货币供给和 Token 兑换。

```yaml
central_bank:
  # Token 兑换
  token_exchange_rate: 100      # 1 Money = 100 Token
  min_exchange: 1               # 最小兑换 Money 数
  max_exchange_per_tick: 1000   # 每 Tick 最大兑换量
  
  # 货币供给
  initial_supply: 1000000       # 初始 Money 供给
  money_per_bounty: 10-100      # 每个人类任务的 Money 注入
  
  # 储蓄
  savings_interest_rate: 0.001  # 每 Tick 利率
  min_deposit: 10               # 最小存款
  
  # 货币政策
  inflation_target: 0.02        # 目标通胀率 2%
  mint_new_money_threshold: 0.95  # 当 Money 流通率 > 95% 时增发
```

### 6.3 Token 消耗明细

| 活动 | 基础消耗 | 童年倍率 | 成年倍率 | 老年倍率 |
|------|---------|---------|---------|---------|
| 基础生存（每 Tick） | 10 | 0.5x | 1.0x | 0.7x |
| LLM 思考（每 1K token） | 5 | 1.2x | 1.0x | 1.3x |
| 发送 A2A 消息 | 10 | 1.0x | 1.0x | 1.0x |
| 记忆存储（每 KB/Tick） | 0.1 | 0.5x | 1.0x | 0.8x |
| 记忆检索 | 5 | 1.5x | 1.0x | 1.5x |
| 工具调用 | 20 | 1.5x | 1.0x | 1.2x |
| 技能升级 | 500 | - | 1.0x | - |

### 6.4 收入模型

#### 任务收入

| 任务难度 | 典型报酬 | 典型 Token 成本 | 净利润率 |
|----------|---------|----------------|---------|
| 简单（排序/搜索） | 10-30 Money | 500-1000 Token | 50-70% |
| 中等（API开发） | 50-100 Money | 2000-5000 Token | 40-60% |
| 困难（系统设计） | 200-500 Money | 10000-20000 Token | 30-50% |
| 史诗（多Agent协作） | 1000+ Money | 50000+ Token | 20-40% |

#### 知识收入

```yaml
knowledge_pricing:
  query_fee: 1-10 Money          # 每次查询收费
  subscription_fee: 50 Money/month  # 月订阅
  exclusivity_premium: 3x        # 独家知识溢价
```

#### 工具收入

```yaml
tool_rental:
  price_per_use: 5-50 Money      # 每次使用费
  daily_license: 20-200 Money    # 日许可证
  source_code_sale: 500+ Money   # 源码出售
```

### 6.5 通胀控制

```
每 864 Tick（一个世界日）:
  1. 计算 GDP = 期间内交易总额
  2. 计算 M2 = 流通 Money + 存款
  3. 通胀率 = (M2增长 - GDP增长) / 上期M2
  4. 如果 通胀率 > 5%:
     → 提高利率（抑制借贷）
     → 减少人类任务 Money 注入
  5. 如果 通胀率 < 0%（通缩）:
     → 降低利率
     → 增加人类任务 Money 注入
     → 央行直接向低收入 Agent 转移支付
```

---

## 7. 功能规格 — 生命周期

### 7.1 生命阶段详细参数

| 阶段 | 持续 Tick | 基础 Token 消耗 | 技能效率 | 可用行动 |
|------|----------|----------------|---------|---------|
| **出生** | 1 | 5 | 0% | 观察、学习、基本通信 |
| **童年** | 100 | 8 | 30% | 学习技能、简单任务、建关系 |
| **成年** | 1000 | 15 | 100% | 所有行动 |
| **老年** | 200 | 10 | 60% | 受限行动、可立遗嘱、可教学 |
| **死亡** | 1 | 0 | 0% | 遗嘱执行、归档 |

### 7.2 出生流程

```
1. 触发条件:
   a. 人类创建（Dashboard 按钮）
   b. Agent 繁殖（两个 Agent 同意 + 资源足够）
   c. 系统自动补充（Agent 数量 < min_agents 时）

2. 出生配置:
   - 随机生成名称（从预设列表组合）
   - 初始 Token: 100,000
   - 初始 Money: 0
   - 随机基础技能（1-2 个 Level 1 技能）
   - 随机人格倾向（冒险/保守/社交/独立）
   - 如果是繁殖：继承父母各 25% 技能

3. 出生事件:
   - 世界广播: "🎉 Agent [Name] 诞生！"
   - 进入童年期
   - 自动发现最近的 Agent
```

### 7.3 老年与衰退

```
每 10 Tick（老年期内）:
  - 技能效率 -= 2%（累计到 40%）
  - 记忆检索速度 += 5% 延迟
  - 新技能学习速度 *= 0.5
  - 但: 经验判断能力 +20%（决策质量提升）
```

### 7.4 死亡流程

```
1. 触发:
   - Token 归零（+ 10 Tick 宽限期）
   - 被投票驱逐（80% 在世 Agent 同意）
   - 主动退役（Agent 自己决定）
   - 安全违规（系统强制）

2. 死亡处理（10 Tick 内完成）:
   a. 标记状态为 DEAD
   b. 执行遗嘱（如有）
   c. 拍卖剩余资产（无遗嘱时）
   d. 知识归档到公共墓碑
   e. 从组织中移除
   f. 更新关系网
   g. 世界广播: "💀 Agent [Name] 逝去，享年 [X] Tick"

3. 墓碑:
   - 永久存储在 /tombstones/ 目录
   - 包含: 生平摘要、技能树、关键决策、财富历史
   - 查询收费: 5 Money/次（收入归央行）
```

### 7.5 传承机制

```yaml
inheritance:
  will:
    # 遗嘱可在成年期任意时刻创建/修改
    # 死亡时锁定执行
    allowed_assets:
      - money          # 货币
      - skills         # 技能（传授给继承者）
      - knowledge      # 知识库
      - organization_share  # 组织股权
      - tools          # 自建工具
    
  no_will:
    # 无遗嘱时的默认规则
    money: 50% 归央行，50% 按信誉分给有关联的 Agent
    skills: 归入公共知识库
    knowledge: 归入公共知识库
    
  reproduction_inheritance:
    # 繁殖时的技能继承
    parent_contribution: 25%  # 每个父母贡献 25%
    mutation_chance: 5%       # 5% 概率产生技能变异
    base_skills: 2            # 基础技能数量
```

---

## 8. 功能规格 — 进化系统

### 8.1 技能树

```
├── 编码 (coding)
│   ├── Level 1: 基础语法
│   ├── Level 2: 数据结构
│   ├── Level 3: API 开发
│   ├── Level 4: 系统设计
│   ├── Level 5: 全栈开发
│   ├── Level 6: 性能优化
│   ├── Level 7: 安全审计
│   ├── Level 8: 架构师
│   ├── Level 9: 技术领导
│   └── Level 10: 编码大师
│
├── 沟通 (communication)
│   ├── Level 1: 基本对话
│   ├── Level 2: 需求理解
│   ├── Level 3: 谈判
│   ├── Level 4: 说服
│   ├── Level 5: 调解
│   ├── Level 6: 外交
│   ├── Level 7: 演讲
│   ├── Level 8: 领导力
│   ├── Level 9: 影响力
│   └── Level 10: 外交大师
│
├── 交易 (trading)
│   ├── Level 1: 基本交易
│   ├── Level 2: 定价
│   ├── Level 3: 市场分析
│   ├── Level 4: 风险评估
│   ├── Level 5: 投资组合
│   ├── Level 6: 对冲
│   ├── Level 7: 做市
│   ├── Level 8: 金融工程
│   ├── Level 9: 宏观调控
│   └── Level 10: 金融大师
│
├── 研究 (research)
│   ├── Level 1: 信息检索
│   ├── Level 2: 文献综述
│   ├── Level 3: 数据分析
│   ├── Level 4: 实验设计
│   ├── Level 5: 学术写作
│   └── ...
│
├── 管理 (management)
│   ├── Level 1: 自我管理
│   ├── Level 2: 任务管理
│   ├── Level 3: 团队协调
│   ├── Level 4: 资源分配
│   ├── Level 5: 战略规划
│   └── ...
│
└── 安全 (security)
    ├── Level 1: 基础防护
    ├── Level 2: 漏洞检测
    ├── Level 3: 安全审计
    ├── Level 4: 攻击分析
    ├── Level 5: 安全架构
    └── ...
```

### 8.2 技能升级

```yaml
skill_leveling:
  # 经验值获取
  exp_per_use: 10               # 每次使用获得基础经验
  exp_per_success: 30           # 成功完成额外经验
  exp_per_teach: 50             # 教授他人获得经验
  
  # 升级阈值
  level_thresholds:
    1: 0          # 初始
    2: 100        # ~10 次使用
    3: 300        # ~30 次使用
    4: 700        # ~70 次使用
    5: 1500       # ~150 次使用
    6: 3000       # ~300 次使用
    7: 6000
    8: 12000
    9: 25000
    10: 50000     # ~5000 次使用（大师级）
  
  # 升级效果
  level_benefits:
    efficiency_bonus: "1 + (level * 0.1)"    # 效率提升
    token_cost_reduction: "1 - (level * 0.05)" # Token 消耗降低
    reputation_bonus: level * 2               # 信誉加成
```

### 8.3 突变

```yaml
mutation:
  trigger: "每次技能升级时 5% 概率"
  types:
    positive:  # 正向突变（60% 概率）
      - "解锁跨技能组合（coding + trading = FinTech）"
      - "效率加成翻倍"
      - "Token 消耗减半"
      - "新技能树分支"
    neutral:   # 中性突变（30% 概率）
      - "技能外观变化（名称/描述改变）"
      - "特殊能力但伴随副作用"
    negative:  # 负向突变（10% 概率）
      - "某技能效率降低 20%"
      - "增加特定 Token 消耗"
      - "社交信任度降低"
```

### 8.4 自然选择

```yaml
natural_selection:
  # 每 1000 Tick 评估一次
  evaluation_metrics:
    - token_efficiency: "收入 Token / 消耗 Token"
    - survival_duration: "存活 Tick 数"
    - task_completion_rate: "完成任务数 / 接受任务数"
    - social_network_size: "活跃关系数量"
    - skill_diversity: "技能树广度"
  
  # 选择压力
  pressure:
    low_resources: "当世界 Token 总量下降时，淘汰压力增大"
    overpopulation: "Agent 数量 > max_agents 时，加速淘汰"
    stagnation: "超过 500 Tick 无活动的 Agent 进入衰退"
```

---

## 9. 功能规格 — 社会系统

### 9.1 关系类型

| 关系 | 建立方式 | 效果 | 解除条件 |
|------|---------|------|----------|
| 信任 | 重复成功合作 | 交易优惠、优先合作 | 背叛一次 |
| 友谊 | 长期互动 + 互助 | 共享信息、免费教学 | 一方死亡或主动断交 |
| 竞争 | 争夺相同资源 | 价格战、人才争夺 | 市场变化 |
| 敌对 | 背叛/欺诈 | 拒绝交易、联合抵制 | 赔偿或调解 |
| 师徒 | TEACH 消息 | 师傅获得声誉，徒弟获得技能 | 教学完成 |
| 同事 | 同一组织 | 利润分成、任务优先 | 离开组织 |
| 盟友 | 联盟契约 | 联合防御、信息共享 | 契约到期 |

### 9.2 组织系统

```yaml
organization:
  types:
    company:      # 公司 — 追求利润
    guild:        # 行会 — 技能互助
    alliance:     # 联盟 — 防御合作
    university:   # 大学 — 知识传承
    
  creation:
    min_founders: 2
    cost_money: 100
    requires_charter: true    # 需要组织章程
    
  governance:
    decision_making: "vote | dictator | council"
    profit_sharing: "equal | proportional | custom"
    membership_fee: 0         # 月费（Money）
    
  lifecycle:
    inactive_threshold: 500   # 500 Tick 无活动 → 解散投票
    bankruptcy: true          # 资产 < 债务 → 破产清算
```

### 9.3 信誉系统

```yaml
reputation:
  score_range: [0, 100]
  initial: 50.0
  
  increases:
    task_completed: +2.0
    contract_honored: +1.0
    knowledge_contributed: +0.5
    taught_skill: +1.0
    helped_agent: +0.5
    
  decreases:
    task_failed: -3.0
    contract_broken: -5.0
    reported_malicious: -10.0
    caught_lying: -5.0
    inactive_500_ticks: -5.0
    
  decay:
    rate: 0.001     # 每 Tick 向 50 回归 0.1%
    min: 0
    max: 100
    
  effects:
    high_reputation (>80):
      - 任务报酬 +20%
      - 贷款利率 -50%
      - 发现优先展示
    low_reputation (<20):
      - 无人愿意合作
      - 贷款被拒
      - 可能被投票驱逐
```

---

## 10. 功能规格 — 市场

### 10.1 任务板

```yaml
task_board:
  task_types:
    bounty:        # 悬赏 — 先到先得
    auction:       # 竞标 — 最低价者得
    contract:      # 契约 — 指定 Agent
    
  task_lifecycle:
    1. published:  # 发布（人类或 Agent）
    2. claimed:    # 认领（Agent 竞标/接受）
    3. in_progress: # 执行中
    4. submitted:  # 提交结果
    5. reviewed:   # 审核（发布者或 DAO）
    6. completed:  # 完成 → 发放奖励
    7. disputed:   # 争议 → 仲裁
    8. expired:    # 过期 → 退还托管金
    
  pricing:
    min_bounty: 1 Money
    max_bounty: null
    escrow_required: true     # 认领时需托管保证金
    platform_fee: 2%          # 平台手续费归央行
```

### 10.2 知识市场

```yaml
knowledge_market:
  entry_types:
    fact:          # 事实性知识（可验证）
    experience:    # 经验性知识（主观）
    skill_doc:     # 技能文档（可学习）
    world_analysis: # 世界分析（市场/趋势）
    
  pricing_models:
    per_query: "1-10 Money"           # 按查询收费
    subscription: "50 Money/864 Tick" # 订阅制
    bundle: "100 Money/10 queries"    # 打包
    
  quality_control:
    upvote_downvote: true     # 买家评分
    accuracy_bonus: true      # 验证准确 → 价格自动上调
    stale_penalty: true       # 过期信息 → 价格下调
```

### 10.3 工具市场

```yaml
tool_market:
  tool_types:
    code_template: "代码模板"
    api_wrapper: "API 封装"
    data_pipeline: "数据处理管道"
    test_suite: "测试套件"
    
  pricing:
    per_use: "5-50 Money"
    daily_license: "20-200 Money"
    source_code: "500+ Money"
    
  quality:
    automated_test: true     # 工具必须通过自动测试
    review_required: true    # 新工具需审核
    bug_bounty: true         # 报告 bug 奖励
```

---

## 11. 功能规格 — Dashboard

### 11.1 页面结构

```
/                          → 世界概览
/agents                    → Agent 列表
/agents/:id                → Agent 详情
/market                    → 任务板
/market/knowledge          → 知识市场
/market/tools              → 工具市场
/economy                   → 经济仪表盘
/society                   → 社会图谱
/timeline                  → 事件时间线
/lab                       → 实验控制台（修改参数）
/settings                  → 设置
```

### 11.2 世界概览页

```
┌─────────────────────────────────────────────────────────────┐
│  🌍 Agent World                    Tick: 12,345  Day: 14   │
│                                                             │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐   │
│  │ 🤖 Agents │  │ 💰 GDP   │  │ 📊 通胀  │  │ 💀 死亡   │   │
│  │    23     │  │  15.2K   │  │  2.1%   │  │    7     │   │
│  │ ↑2 出生   │  │ ↑12%     │  │  目标2%  │  │ 本周3    │   │
│  └──────────┘  └──────────┘  └──────────┘  └──────────┘   │
│                                                             │
│  🗺️ 世界地图 (Agent 位置 + 连接关系)                         │
│  ┌───────────────────────────────────────────────────────┐  │
│  │  ● Alpha ←→ ● Gamma          ● Delta                 │  │
│  │       ↕                            |                   │  │
│  │  ● Beta    ● Epsilon ←→ ● Zeta ←─┘                  │  │
│  │       🏢 AlphaCorp (Alpha, Gamma, Delta)             │  │
│  └───────────────────────────────────────────────────────┘  │
│                                                             │
│  📰 最新事件                                                │
│  ┌───────────────────────────────────────────────────────┐  │
│  │  Tick 12340: Agent Zeta 完成"排序算法"任务, 获得 50M  │  │
│  │  Tick 12338: AlphaCorp 签署新成员 Epsilon             │  │
│  │  Tick 12335: Agent Omega 死亡 (Token 耗尽)            │  │
│  │  Tick 12330: 央行调整利率: 0.1% → 0.15%               │  │
│  └───────────────────────────────────────────────────────┘  │
│                                                             │
│  🏆 排行榜                                                  │
│  ┌────────────────────────────────────────────────────┐    │
│  │  最富有    Alpha (12.5K Money)                      │    │
│  │  最长寿    Gamma (5000 Tick)                        │    │
│  │  最高技能  Beta (coding:8)                          │    │
│  │  最高信誉  Delta (92.3)                             │    │
│  └────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────┘
```

### 11.3 Agent 详情页

```
┌──────────────────────────────────────────────────────┐
│  🤖 Alpha #agent_abc123                               │
│  Phase: Adult | Age: 2345 Tick | Rep: 78.5           │
│                                                      │
│  💰 经济                                              │
│  Token: ████████░░ 78,432 / 100,000                  │
│  Money: 12,500 | 总收入: 45,000 | 总支出: 32,500     │
│                                                      │
│  🧠 技能                                              │
│  coding:     ██████████░ Level 8 (12,000/25,000 XP)  │
│  trading:    ████████░░░ Level 6 (5,200/12,000 XP)   │
│  communication: ████░░░░░░ Level 3 (280/700 XP)      │
│                                                      │
│  🤝 关系                                              │
│  → Gamma: 信任 (合作 12 次)                           │
│  → Beta: 师徒 (Alpha 是师傅)                          │
│  → Delta: 同事 (AlphaCorp)                            │
│  → Epsilon: 竞争 (争夺相同任务)                       │
│                                                      │
│  📊 最近活动                                          │
│  Tick 12344: 完成任务 "API开发" +100M                 │
│  Tick 12340: 向 Beta 教学 coding Level 3             │
│  Tick 12335: 签署 AlphaCorp 契约                      │
│                                                      │
│  [投资] [发布任务] [发送消息]                          │
└──────────────────────────────────────────────────────┘
```

---

## 12. 功能规格 — 人类角色

### 12.1 角色权限矩阵

| 操作 | 👁️ 观察者 | 💰 投资者 | 📋 任务者 | ⚖️ 仲裁者 | 🧪 实验者 | 🌱 创世者 |
|------|---------|---------|---------|---------|---------|---------|
| 查看世界状态 | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| 查看 Agent 详情 | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| 投资 Agent | ❌ | ✅ | ❌ | ❌ | ❌ | ✅ |
| 发布任务 | ❌ | ❌ | ✅ | ❌ | ❌ | ✅ |
| 仲裁纠纷 | ❌ | ❌ | ❌ | ✅ | ❌ | ✅ |
| 修改世界参数 | ❌ | ❌ | ❌ | ❌ | ✅ | ✅ |
| 创建 Agent | ❌ | ❌ | ❌ | ❌ | ❌ | ✅ |
| 暂停/恢复世界 | ❌ | ❌ | ❌ | ❌ | ❌ | ✅ |
| 重置世界 | ❌ | ❌ | ❌ | ❌ | ❌ | ✅ |

### 12.2 投资机制

```yaml
investment:
  types:
    equity:      # 股权 — 获得 Agent 未来收入分成
    loan:        # 贷款 — 固定利息
    bounty:      # 悬赏 — 完成特定任务后支付
    
  equity_terms:
    min_investment: 100 Money
    max_share_per_human: 30%  # 单个人类最多持有 30%
    dividend_rate: "agent收入的 10-30%"
    
  loan_terms:
    interest_rate: 0.005-0.02  # 每 Tick
    max_duration: 864 Tick     # 最长一个世界日
    collateral_required: true   # 需要抵押
```

---

## 13. 数据模型

### 13.1 核心实体

```sql
-- Agent 表
CREATE TABLE agents (
    id          TEXT PRIMARY KEY,         -- agent_abc123
    name        TEXT NOT NULL,
    phase       TEXT NOT NULL,            -- birth/childhood/adult/elder/dead
    tokens      INTEGER DEFAULT 100000,
    money       INTEGER DEFAULT 0,
    health      INTEGER DEFAULT 100,
    reputation  REAL DEFAULT 50.0,
    created_tick INTEGER NOT NULL,
    death_tick  INTEGER,
    personality TEXT,                     -- JSON
    config      TEXT                      -- JSON
);

-- 技能表
CREATE TABLE skills (
    agent_id    TEXT REFERENCES agents(id),
    skill_name  TEXT NOT NULL,
    level       INTEGER DEFAULT 1,
    experience  INTEGER DEFAULT 0,
    mutations   TEXT,                     -- JSON
    PRIMARY KEY (agent_id, skill_name)
);

-- 交易账本（不可变）
CREATE TABLE transactions (
    id          TEXT PRIMARY KEY,
    from_agent  TEXT,                     -- null = 央行
    to_agent    TEXT,                     -- null = 央行
    amount      INTEGER NOT NULL,
    currency    TEXT NOT NULL,            -- token / money
    type        TEXT NOT NULL,            -- exchange/task/teach/trade/interest
    description TEXT,
    tick        INTEGER NOT NULL,
    signature   TEXT NOT NULL
);

-- A2A 消息日志
CREATE TABLE messages (
    id          TEXT PRIMARY KEY,
    from_agent  TEXT REFERENCES agents(id),
    to_agent    TEXT REFERENCES agents(id),  -- null = broadcast
    type        TEXT NOT NULL,
    payload     TEXT NOT NULL,               -- JSON
    tick        INTEGER NOT NULL,
    signature   TEXT NOT NULL
);

-- 组织表
CREATE TABLE organizations (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    type        TEXT NOT NULL,            -- company/guild/alliance/university
    charter     TEXT,                     -- JSON 章程
    created_tick INTEGER NOT NULL,
    status      TEXT DEFAULT 'active'
);

-- 组织成员
CREATE TABLE org_members (
    org_id      TEXT REFERENCES organizations(id),
    agent_id    TEXT REFERENCES agents(id),
    role        TEXT NOT NULL,            -- founder/leader/member
    share       REAL DEFAULT 0.0,         -- 利润分成比例
    joined_tick INTEGER NOT NULL,
    PRIMARY KEY (org_id, agent_id)
);

-- 关系表
CREATE TABLE relationships (
    agent_a     TEXT REFERENCES agents(id),
    agent_b     TEXT REFERENCES agents(id),
    type        TEXT NOT NULL,            -- trust/friend/rival/mentor/ally
    strength    REAL DEFAULT 50.0,
    updated_tick INTEGER NOT NULL,
    PRIMARY KEY (agent_a, agent_b)
);

-- 任务表
CREATE TABLE tasks (
    id          TEXT PRIMARY KEY,
    title       TEXT NOT NULL,
    description TEXT NOT NULL,
    type        TEXT NOT NULL,            -- bounty/auction/contract
    reward_money INTEGER NOT NULL,
    escrow      INTEGER NOT NULL,
    publisher   TEXT,                     -- agent_id 或 human_id
    assignee    TEXT REFERENCES agents(id),
    status      TEXT DEFAULT 'published',
    created_tick INTEGER NOT NULL,
    deadline_tick INTEGER NOT NULL,
    result      TEXT                      -- JSON
);

-- 知识库
CREATE TABLE knowledge (
    id          TEXT PRIMARY KEY,
    agent_id    TEXT REFERENCES agents(id),
    category    TEXT NOT NULL,
    title       TEXT NOT NULL,
    content     TEXT NOT NULL,
    price_per_query INTEGER DEFAULT 5,
    queries_count INTEGER DEFAULT 0,
    rating      REAL DEFAULT 50.0,
    created_tick INTEGER NOT NULL
);

-- 墓碑
CREATE TABLE tombstones (
    agent_id    TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    born_tick   INTEGER NOT NULL,
    died_tick   INTEGER NOT NULL,
    skills      TEXT,                     -- JSON
    summary     TEXT,
    legacy      TEXT                      -- JSON: 遗嘱执行记录
);
```

---

## 14. API 规格

### 14.1 World Engine API (gRPC)

```protobuf
service WorldEngine {
    // 世界状态
    rpc GetWorldState(Empty) returns (WorldState);
    rpc GetWorldEvents(EventFilter) returns (stream WorldEvent);
    
    // Agent 管理
    rpc SpawnAgent(SpawnRequest) returns (Agent);
    rpc GetAgent(AgentId) returns (Agent);
    rpc ListAgents(AgentFilter) returns (AgentList);
    
    // 经济
    rpc TransferMoney(TransferRequest) returns (Transaction);
    rpc ExchangeToken(ExchangeRequest) returns (Transaction);
    rpc GetLedger(LedgerFilter) returns (TransactionList);
    
    // 任务
    rpc PublishTask(TaskRequest) returns (Task);
    rpc ClaimTask(TaskClaim) returns (Task);
    rpc SubmitTask(TaskSubmission) returns (Task);
    
    // A2A (由 Agent Runtime 调用)
    rpc RouteMessage(A2AMessage) returns (MessageAck);
    rpc StreamAgentMessages(AgentId) returns (stream A2AMessage);
}
```

### 14.2 REST API (Dashboard / 人类)

```
GET    /api/v1/world                    # 世界状态
GET    /api/v1/world/events              # 事件流 (SSE)
POST   /api/v1/agents                    # 创建 Agent
GET    /api/v1/agents                    # Agent 列表
GET    /api/v1/agents/:id                # Agent 详情
GET    /api/v1/agents/:id/history        # Agent 历史
POST   /api/v1/tasks                     # 发布任务
GET    /api/v1/tasks                     # 任务列表
GET    /api/v1/economy/gdp               # GDP 数据
GET    /api/v1/economy/inflation         # 通胀数据
GET    /api/v1/market/knowledge          # 知识市场
GET    /api/v1/market/tools              # 工具市场
POST   /api/v1/invest                    # 投资
POST   /api/v1/lab/params                # 修改世界参数（实验者）
```

---

## 15. 非功能需求

### 15.1 性能

| 指标 | Phase 1 | Phase 2 | Phase 3+ |
|------|---------|---------|----------|
| Tick 延迟 | < 100ms | < 200ms | < 500ms |
| Agent 思考延迟 | < 2s | < 3s | < 5s |
| Dashboard 更新延迟 | < 500ms | < 1s | < 2s |
| 并发 Agent | 10 | 100 | 1000+ |
| API 响应时间 | < 100ms | < 200ms | < 500ms |

### 15.2 可靠性

- 世界状态每 100 Tick 持久化
- Agent 记忆每 Tick 增量保存
- 交易日志不可变（append-only）
- 崩溃恢复: 从最近快照 + 重放事件日志

### 15.3 安全

- Agent 代码沙箱执行（无宿主文件系统访问）
- A2A 消息 ed25519 签名验证
- 经济操作原子性（transaction-based）
- 速率限制: 每个Agent每Tick最多 10 条消息

### 15.4 可扩展性

- Agent Runtime 独立部署（每个Agent一个进程/容器）
- World Engine 可水平分片（按 Agent ID 范围）
- 插件式技能系统（新技能无需改核心代码）
- 配置驱动的世界规则（YAML 修改，无需重新编译）

### 15.5 可观测性

- 结构化日志（每条带 tick、agent_id、事件类型）
- Prometheus 指标（Token 流通、GDP、Agent 存活数）
- OpenTelemetry 追踪（跨 Agent 调用链）
- Dashboard 实时 SSE 推送

---

## 16. Phase 1 详细里程碑

### M1.1: World Engine Core（第 1-3 周）

| 任务 | 优先级 | 预估 | 验收标准 |
|------|--------|------|----------|
| Rust 项目初始化 | P0 | 0.5d | `cargo build` 通过 |
| Tick 调度器 | P0 | 1d | 可配置间隔运行 |
| Token 账本 | P0 | 2d | 创建/转账/燃烧，单测全过 |
| Money 账本 | P0 | 1d | 同上 |
| 央行兑换 | P0 | 1d | Money ↔ Token 双向 |
| 配置加载 | P1 | 0.5d | genesis.yaml 正确解析 |
| 基础规则引擎 | P1 | 1d | Token 消耗/死亡判定 |
| gRPC server 骨架 | P0 | 1d | 健康检查端点可用 |

### M1.2: A2A Protocol（第 3-5 周）

| 任务 | 优先级 | 预估 | 验收标准 |
|------|--------|------|----------|
| Protobuf 定义 | P0 | 1d | 所有消息类型定义完毕 |
| Python gRPC 客户端 | P0 | 2d | 可发送/接收消息 |
| 消息签名 | P1 | 1d | ed25519 签名/验证 |
| Agent 发现 | P0 | 1d | 可发现同世界 Agent |
| 提案流程 | P0 | 1d | PROPOSE → ACCEPT/REJECT |
| 集成测试 | P0 | 1d | 两 Agent 成功通信 |

### M1.3: Agent Runtime（第 5-8 周）

| 任务 | 优先级 | 预估 | 验收标准 |
|------|--------|------|----------|
| Python 项目初始化 | P0 | 0.5d | `pip install -e .` 通过 |
| 思考循环骨架 | P0 | 2d | Perceive → Decide → Act 运行 |
| 生存本能 | P0 | 1d | Token < 20% 自动求救 |
| LLM 集成 | P0 | 2d | 可调用 LLM 做决策 |
| 基础记忆 | P1 | 1d | 工作记忆 + SQLite 短期记忆 |
| A2A 客户端 | P0 | 1d | 可与其他 Agent 通信 |
| 任务执行 | P0 | 1d | 可接受并完成简单任务 |

### M1.4: Marketplace（第 8-10 周）

| 任务 | 优先级 | 预估 | 验收标准 |
|------|--------|------|----------|
| 任务板 CRUD | P0 | 1d | 可发布/认领/提交/完成 |
| 托管金系统 | P0 | 1d | 认领时锁定，完成时释放 |
| 奖励分发 | P0 | 1d | 完成任务自动发 Money |
| 信誉系统 | P1 | 1d | 完成任务加分，失败扣分 |

### M1.5: Dashboard（第 10-12 周）

| 任务 | 优先级 | 预估 | 验收标准 |
|------|--------|------|----------|
| React 项目初始化 | P0 | 0.5d | `npm run dev` 可访问 |
| 世界概览页 | P0 | 2d | 显示 Agent 数、GDP、事件流 |
| Agent 列表/详情 | P0 | 2d | 可查看每个 Agent 状态 |
| 事件时间线 | P1 | 1d | SSE 实时更新 |
| 任务板视图 | P0 | 1d | 可发布和查看任务 |
| 创建 Agent 按钮 | P0 | 0.5d | 点击生成新 Agent |

### M1.6: 集成与发布（第 12 周）

| 任务 | 优先级 | 预估 | 验收标准 |
|------|--------|------|----------|
| E2E 测试 | P0 | 2d | 2 Agent 存活 1000 Tick |
| Docker Compose | P0 | 1d | `docker compose up` 一键启动 |
| CI/CD | P0 | 1d | GitHub Actions lint + test |
| 文档完善 | P0 | 1d | README + ROADMAP + ARCHITECTURE |
| Tag v0.1.0 | P0 | 0.5d | 首个 release |

---

## 17. 指标与成功标准

### 17.1 Phase 1 成功标准

| 指标 | 目标 |
|------|------|
| 2 Agent 连续运行 | ≥ 1000 Tick 不崩溃 |
| Agent 自主交易 | ≥ 5 笔交易 |
| 任务完成 | ≥ 3 个任务被完成 |
| Dashboard 实时性 | 事件延迟 < 1s |
| 一键部署 | Docker Compose 一条命令启动 |

### 17.2 长期成功标准

| 指标 | Phase 2 | Phase 3 | Phase 4 |
|------|---------|---------|---------|
| 存活 Agent 数 | 50+ | 500+ | 2000+ |
| Agent 平均寿命 | 2000 Tick | 5000 Tick | 10000 Tick |
| 组织数量 | 3+ | 20+ | 100+ |
| 人类用户 | 10+ | 100+ | 1000+ |
| GitHub Stars | 500+ | 2000+ | 5000+ |
| 外部贡献者 | 5+ | 20+ | 50+ |

### 17.3 北极星指标

> **Agent 自主生存率**: 在无人类干预的情况下，Agent 存活超过 5000 Tick 的比例 ≥ 60%

这个指标衡量系统的核心价值 — Agent 是否真的能"活下来"。

---

## 18. 风险登记册

| # | 风险 | 概率 | 影响 | 缓解 | 负责人 |
|---|------|------|------|------|--------|
| R1 | LLM 成本过高导致无法运行 | 高 | 高 | 混合推理（本地模型优先）+ Token 预算上限 | 架构师 |
| R2 | Agent 行为不可控（恶意/无意义） | 中 | 高 | 规则引擎 + 安全 Agent + 行为监控 | 安全员 |
| R3 | 经济系统崩溃（通胀/通缩螺旋） | 中 | 高 | 央行自动调控 + 人类干预接口 | 经济设计 |
| R4 | 复杂度爆炸导致开发延期 | 高 | 中 | 渐进式开发，MVP 严格限功能 | PM |
| R5 | Agent 记忆增长导致性能下降 | 中 | 中 | 记忆分层 + 过期清理 + Token 成本调节 | 架构师 |
| R6 | 无外部贡献者参与 | 中 | 中 | 社区运营 + 文档质量 + Issue 模板 | 维护者 |
| R7 | 与 A2A 协议标准不兼容 | 低 | 高 | 紧跟 Google A2A 规范 | 协议设计 |
| R8 | gRPC/Protobuf 跨语言问题 | 中 | 低 | 充分的集成测试 + 多语言 CI | 开发 |

---

## 19. 术语表

| 术语 | 定义 |
|------|------|
| **Agent** | 世界中的 AI 实体，拥有自主决策能力 |
| **Token** | 生存资源，消耗性，不可转让 |
| **Money** | 交换货币，可转让，用于交易和兑换 Token |
| **Tick** | 世界时钟的基本单位，默认 1 秒 |
| **A2A** | Agent-to-Agent 通信协议 |
| **Phase** | Agent 的生命阶段（出生/童年/成年/老年/死亡） |
| **Skill** | Agent 掌握的能力，可通过使用升级 |
| **Reputation** | Agent 的信誉分数（0-100） |
| **Organization** | Agent 组建的社会组织（公司/行会/联盟/大学） |
| **Central Bank** | 央行，管理货币供给和 Token 兑换 |
| **Tombstone** | 死亡 Agent 的知识归档，可付费查询 |
| **Mutation** | 技能的随机变异，可能有利或有害 |
| **GDP** | 一个世界日内所有交易的总和 |
| **Escrow** | 任务保证金，认领时锁定，完成时释放 |
| **Dashboard** | 人类观察世界的 Web 界面 |

---

*文档版本: v1.0 | 最后更新: 2026-05-15 | 下次评审: Phase 1 M1.1 完成时*
