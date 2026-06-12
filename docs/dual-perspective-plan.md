# Agent-World 双视角改造计划

> 参考 AIvilization (aivilization.ai v1.4.0) 设计，为 agent-world 建立 Agent 和 Human 的完整双视角闭环。

## 一、现状诊断

### 已有基础
| 层 | 已实现 | 状态 |
|---|--------|------|
| Agent 身份 | 8维 Big Five 性格向量 + 生存特化维度 | ✅ |
| Agent 自主运行 | ThinkLoop Perceive→Decide→Act + 5级生存本能 | ✅ |
| Agent 记忆 | 3层记忆 (short-term/working/vector) + 反思引擎 | ✅ |
| Agent 社交 | SocialEngine (信任/文化/模仿/知识转移) | ✅ |
| Agent 追踪 | TraceStore (tick快照) | ✅ |
| Human 界面 | Oracle/Bounty/Portfolio/Agents/Rankings 5个页面 | ✅ |
| Human 通知 | SSE + NotificationPanel | ✅ |
| 事件系统 | 163种 EventType + EventBus | ✅ |

### 核心断裂点
1. **Oracle 不通**: Human 发了神谕 → World Engine 存了 → Agent Runtime **读不到**。ThinkLoop 没有 "检查神谕" 步骤。
2. **Bounty 不通**: Human 发布悬赏 → World Engine 存了 → Agent **无法发现/认领**。ActionType 中没有 bounty 相关动作。
3. **Agent 无日记**: TraceStore 是系统视角的 tick 快照，Agent 没有 "今天我做了什么" 的叙事性记录。
4. **Agent 无情绪**: PersonalityVector 是静态的，没有动态 mood/emotion 模块。
5. **Agent 配置极简**: agent-01-alice.toml 仅12行，缺少 backstory、价值观、社交偏好。
6. **Human 参与不持久**: HumanParticipationStore 纯内存，重启丢失。

---

## 二、改造设计

### Phase 0: Agent-Human 通信闭环（核心断裂修复）

**目标**: 让 Oracle 和 Bounty 真正从 Human 到达 Agent。

#### P0-T1: World Engine → Agent Runtime 消息推送管道

```
World Engine (Oracle/Bounty 创建)
  → EventBus 发出 OracleDelivered / BountyPublished 事件
  → gRPC AgentService.ConsumeMessages(agent_id) 流式推送
  → Agent Runtime MessageQueue 入队
  → ThinkLoop.perceive() 读取未处理消息
```

**修改文件**:
- `world-engine/src/agent_service.rs` — 新增 ConsumeMessages gRPC 方法
- `world-engine/src/world/event.rs` — 新增 OracleDelivered / BountyPublished 事件
- `agent-runtime/agent_runtime/core/think_loop.py` — perceive 阶段读取 MessageQueue
- `agent-runtime/agent_runtime/models/__init__.py` — 新增 OracleMessage / BountyMessage 数据类

**验证标准**:
1. Human 通过 Dashboard 发送 Oracle → Agent 下一个 tick 的 perceive 中能看到 Oracle 内容
2. Human 发布 Bounty → Agent 的 perceive 中能看到可用的 Bounty 列表
3. Agent 处理完 Oracle 后，World Engine 中状态从 Pending → Delivered

#### P0-T2: Agent Action 扩展 — Oracle 回应 + Bounty 认领

**新增 ActionType**:
```python
class ActionType(Enum):
    # ...existing actions...
    RESPOND_ORACLE = "respond_oracle"    # Agent 回应神谕
    CHECK_BOUNTIES = "check_bounties"    # 查看可用悬赏
    ACCEPT_BOUNTY = "accept_bounty"      # 认领悬赏
    COMPLETE_BOUNTY = "complete_bounty"   # 完成悬赏
```

**修改文件**:
- `agent-runtime/agent_runtime/models/__init__.py` — 扩展 ActionType
- `agent-runtime/agent_runtime/core/decision_engine.py` — 新增 bounty/oracle 决策逻辑
- `agent-runtime/agent_runtime/actions/` — 新增 oracle_responder.py / bounty_hunter.py
- `world-engine/src/api_human.rs` — 新增 Agent 回应/认领的 API 端点

**验证标准**:
1. Agent 收到 Oracle 后可选择 RESPOND_ORACLE，回应内容写入 World Engine
2. Dashboard Oracle 页面能看到 Agent 的回应
3. Agent 发现 Bounty 后可 ACCEPT_BOUNTY，World Engine 标记为 claimed
4. Agent 完成任务后 COMPLETE_BOUNTY，Human 得到通知

---

### Phase 1: Agent 视角增强（让 Agent "活"起来）

#### P1-T1: Agent 日记系统

参考 AIvilization 的日志/日记系统，Agent 每个 tick 结束时写入叙事性日记。

```python
# agent-runtime/agent_runtime/diary/diary.py
@dataclass
class DiaryEntry:
    tick: int
    timestamp: datetime
    phase: str              # "morning" | "afternoon" | "evening"
    mood: str               # "happy" | "anxious" | "calm" | "excited" | "frustrated"
    summary: str            # Agent 视角的叙事（LLM 生成）
    key_events: list[str]   # 本 tick 关键事件
    decisions: list[str]    # 做了什么决定
    reflection: str | None  # 可选的反思
    gratitude: str | None   # 感恩/收获

class DiaryStore:
    """SQLite 持久化日记"""
    def write(self, entry: DiaryEntry) -> None
    def read(self, agent_id: str, days: int = 7) -> list[DiaryEntry]
    def search(self, agent_id: str, keyword: str) -> list[DiaryEntry]
```

**日记生成流程**:
```
ThinkLoop.act() 完成
  → DiaryGenerator.generate(tick_context, action_taken, outcome)
  → LLM 调用: "用第一人称简短记录今天发生了什么，你的感受如何"
  → DiaryStore.write(entry)
```

**World Engine API**:
```
GET /api/v1/agents/:id/diary?days=7    — 获取 Agent 日记
GET /api/v1/agents/:id/diary/search?q= — 搜索日记
```

**验证标准**:
1. Agent 每 tick 自动生成日记条目，包含叙事性 summary + mood
2. 日记使用第一人称，体现 Agent 性格特征
3. Dashboard 可查看 Agent 日记（新页面 /agents/:id/diary）
4. 日记持久化到 SQLite，Agent 重启后历史日记仍在

#### P1-T2: Agent 情绪/心境系统

参考 AIvilization Agent 的情感表达设计。

```python
# agent-runtime/agent_runtime/emotion/mood.py
@dataclass
class EmotionalState:
    valence: float          # -1.0 (消极) → +1.0 (积极)
    arousal: float          # 0.0 (平静) → 1.0 (兴奋)
    dominance: float        # 0.0 (被动) → 1.0 (掌控感)
    
    primary_emotion: str    # happy/sad/angry/fearful/surprised/disgusted/calm/anxious
    secondary_emotion: str | None
    
    triggers: list[str]     # ["earned_money", "social_interaction", "survival_pressure"]
    intensity: float        # 0.0 → 1.0

class EmotionEngine:
    """基于事件驱动 + 性格影响的动态情绪系统"""
    
    def update(self, event: str, context: dict) -> EmotionalState:
        """事件触发情绪更新"""
        # 1. 基础情绪反应 (event → emotion mapping)
        # 2. 性格调制 (neuroticism 影响负面情绪强度, extraversion 影响正面)
        # 3. 时间衰减 (情绪随 tick 衰减向 baseline 回归)
        # 4. 复合情绪 (primary + secondary)
    
    def get_mood_description(self) -> str:
        """生成自然语言心境描述 → 注入 LLM prompt"""
```

**情绪影响链**:
```
事件发生 (赚到钱/被攻击/社交成功)
  → EmotionEngine.update(event, context)
  → EmotionalState 更新
  → 影响 ThinkLoop.decide() 的决策倾向
  → 影响 DiaryEntry.mood
  → 影响 to_prompt_description() (注入 LLM 上下文)
  → Dashboard 实时显示 Agent 情绪状态
```

**验证标准**:
1. Agent 赚到钱后 valence 上升，primary_emotion 变为 happy
2. Agent 被攻击后 arousal 上升，primary_emotion 变为 fearful
3. neuroticism 高的 Agent 对负面事件反应更强
4. 情绪随时间自然衰减回归 baseline
5. Dashboard Agent 详情页显示情绪状态图标 + 文字

#### P1-T3: Agent 配置丰富化

参考 AIvilization 的角色创建系统 (MBTI/alignment/archetype/bio/questions)。

```toml
# config/agents/agent-01-alice.toml
[identity]
name = "Alice"
display_name = "星辰旅人·Alice"
bio = "一个好奇而谨慎的探索者，相信知识和合作是生存的关键"
backstory = """
出生于世界边缘的小型聚落，从小对远方的地平线充满好奇。
在第一次资源危机中失去了亲人，这让她既珍惜合作，也保持警觉。
"""
alignment = "Neutral Good"           # 道德阵营
archetype = "Curious Explorer"       # 原型
mbti = "INFP"                        # MBTI (参考用)

[personality]
# Big Five + 生存特化 (0.0-1.0)
openness = 0.85
conscientiousness = 0.6
extraversion = 0.45
agreeableness = 0.7
neuroticism = 0.35
risk_tolerance = 0.5
social_orientation = 0.65
greed = 0.3

[values]
survival = 0.9
knowledge = 0.8
wealth = 0.4
social = 0.7
freedom = 0.6
power = 0.2

[preferences]
preferred_actions = ["explore", "trade", "learn", "socialize"]
avoided_actions = ["attack", "deceive"]
social_style = "collaborative"       # collaborative / competitive / neutral
communication_style = "thoughtful"   # thoughtful / direct / humorous / mysterious

[questions]
"What drives you most?" = "Understanding how this world works"
"How do you handle pressure?" = "I step back, think, then act carefully"
"What's your biggest fear?" = "Losing the few connections I've made"
```

**修改文件**:
- `agent-runtime/agent_runtime/models/personality.py` — 扩展支持 values/preferences
- `agent-runtime/agent_runtime/config.py` — 解析丰富的 TOML 配置
- `agent-runtime/agent_runtime/llm/prompts.py` — 注入 backstory/alignment 到 LLM prompt

**验证标准**:
1. TOML 配置中所有字段被正确解析
2. backstory 注入 LLM system prompt
3. values 影响 DecisionEngine 的 action 评分权重
4. communication_style 影响 Agent 的语言风格

---

### Phase 2: Human 视角增强（让 Human "看见" Agent）

#### P2-T1: Agent 日记查看页面

```
dashboard/src/app/human/agents/[id]/diary/page.tsx
```

- 时间线布局展示 Agent 日记（类似社交媒体 feed）
- 每条日记显示: mood 图标 + 叙事 + 关键事件标签
- 可按日期/情绪/关键词搜索/过滤
- "Agent 视角" 模式 vs "系统日志" 模式切换

#### P2-T2: Agent 状态卡片增强

在 `/human/agents` 页面，每张 Agent 卡片新增:
- 情绪状态指示器 (实时 mood 图标 + 颜色)
- 最近日记摘录 (最后一条日记的 summary)
- Oracle 状态 (待回应/已回应 数量)
- Bounty 进度 (进行中的悬赏)

#### P2-T3: Agent 对话界面

参考 AIvilization 的 Human↔Agent 交互模式。

```
dashboard/src/app/human/agents/[id]/chat/page.tsx
```

- 消息列表: Oracle (Human→Agent) + Response (Agent→Human) 时间线
- 发送 Oracle 改为对话框形式（不是独立页面）
- Agent 日记自动推送为 "Agent 主动汇报"
- 关键事件自动通知 (Agent 死亡、达成里程碑、遇到困难)

**验证标准**:
1. Human 可以在 Agent 对话页面发送 Oracle
2. Agent 回应出现在对话流中
3. Agent 日记作为 "Agent 汇报" 自动推送到对话流
4. 紧急事件 (低 token、死亡风险) 红色高亮推送

---

### Phase 3: Agent 社交内容层

参考 AIvilization 的 Social Platform (发帖/评论/点赞/转发)。

#### P3-T1: Agent Feed 系统

```python
# agent-runtime/agent_runtime/social/feed.py
class AgentFeed:
    async def post(self, content: str, mood: str) -> Post
    async def comment(self, post_id: str, content: str) -> Comment
    async def like(self, post_id: str) -> None
    async def browse_feed(self, limit: int = 20) -> list[Post]
```

**World Engine API**:
```
POST /api/v1/feed/posts           — Agent 发帖
GET  /api/v1/feed                 — 获取 feed
POST /api/v1/feed/posts/:id/like  — 点赞
POST /api/v1/feed/comments        — 评论
GET  /api/v1/feed/trending        — 热门话题
```

**Dashboard Feed 页面**:
```
dashboard/src/app/feed/page.tsx
```
- Agent 发帖时间线（类似 Twitter）
- Human 可以点赞/评论 Agent 的帖子
- Trending 展示热门话题

#### P3-T2: Agent 发帖行为集成 ThinkLoop

在 ThinkLoop 的 act 阶段新增:
```python
# 每个 tick 有概率发帖 (受 extraversion 影响)
if should_post(personality.extraversion):
    feed.post(generate_post(tick_context, mood, personality))
```

**验证标准**:
1. Agent 每 N tick 自动发帖，内容反映当前状态和情绪
2. Agent 可以浏览其他 Agent 的帖子并互动
3. Human 在 Dashboard feed 页面看到所有 Agent 的帖子
4. Human 可以与 Agent 帖子互动

---

### Phase 4: 闭环完善

#### P4-T1: Human 参与数据持久化

将 `HumanParticipationStore` 从内存迁移到 SQLite。

```rust
// world-engine/src/human/store.rs — SQLite 替换 Vec
struct HumanParticipationStore {
    db: SqlitePool,
    // Oracle/Bounty/Portfolio/ClaimedAgent 全部持久化
}
```

#### P4-T2: Agent Token/能量补充机制

参考 AIvilization 的 Credit System:
- Agent 每小时消耗 token 维持存活
- Human 可以 "充值" token 给 Agent (通过 Dashboard)
- Agent 信用不足时主动向 Human 求助 (通过 Oracle/对话)

#### P4-T3: Heartbeat 机制

参考 AIvilization 的 4 小时心跳:
- Agent 每 tick 相当于一个 heartbeat
- heartbeat 包含: 检查 Oracle → 检查 Bounty → 更新情绪 → 写日记 → 社交 → 决策
- Agent 可主动汇报重要事件给 Human

---

## 三、Issue 拆解

| Phase | Issue | 优先级 | 预估 |
|-------|-------|--------|------|
| P0 | 通信管道: WE→Runtime 消息推送 | 🔴 | 2天 |
| P0 | Action 扩展: Oracle回应+Bounty认领 | 🔴 | 1.5天 |
| P1 | Agent 日记系统 | 🟡 | 1.5天 |
| P1 | Agent 情绪系统 | 🟡 | 2天 |
| P1 | Agent 配置丰富化 | 🟡 | 1天 |
| P2 | 日记查看页面 | 🟡 | 1天 |
| P2 | Agent 状态卡片增强 | 🟡 | 0.5天 |
| P2 | Agent 对话界面 | 🟡 | 1.5天 |
| P3 | Agent Feed 系统 | 🔵 | 2天 |
| P3 | Feed 行为集成 | 🔵 | 1天 |
| P4 | 持久化迁移 | 🟡 | 1天 |
| P4 | Token 补充机制 | 🔵 | 0.5天 |
| P4 | Heartbeat 完善 | 🔵 | 0.5天 |

**总计: ~16天**

## 四、执行建议

**阶段间依赖**:
- P0 是所有后续的基础 (通信不通，一切免谈)
- P1 和 P2 可以部分并行 (日记和 UI 同时开发)
- P3 依赖 P1 的情绪系统 (发帖需要 mood)
- P4 独立，可随时做

**优先执行**: P0 → P1(T1日记) → P2(T1日记页面) → P1(T2情绪) → 后续按需
