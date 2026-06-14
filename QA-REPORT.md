# Agent World — 严格 QA 报告

**日期**: 2026-06-13  
**测试范围**: 后端 API + Dashboard 前端 + Agent 运行时 + 交互逻辑  
**当前状态**: tick=6046+, 10个容器运行, 20个Agent注册

---

## 🔴 Critical 严重 (4)

### C1. LLM API 403 Forbidden — 全部 Agent 无法做出智能决策

- **现象**: 全部 10 个 Agent 每个 tick 都收到 `403 Forbidden` from `https://open.bigmodel.cn/api/paas/v4/chat/completions`。每个 Agent 累计 4100+ 次 403 错误。
- **根因**: 容器内 `LLM_MODEL=glm-5.2`（不存在的模型名），但 `.env` 中是 `LLM_MODEL=glm-4-flash`。容器启动时 `.env` 值不同，之后更新 `.env` 但未 `--force-recreate` 容器。
- **验证**: `curl` 直接调用 `glm-4-flash` 返回 200 正常；`docker exec agent-alice env | grep LLM_MODEL` 显示 `glm-5.2`。
- **影响**: 所有 Agent 每次都 fallback 到随机决策，LLM 智能体模拟形同虚设。
- **修复**: `docker compose up -d --force-recreate agent-alice agent-bob ...`（或全部重建）

### C2. REST API tick 永远返回 0

- **现象**: `GET /api/v1/tick` 返回 `{"tick":0}`，action 响应也返回 `"tick":0`。但引擎内部 tick=6046+，SSE 事件流正常推送 tick。
- **根因**: tick_tx bridge（main.rs 中监听 TickAdvanced → tick_tx.send）可能未正确工作或未编译进当前运行的二进制。
- **影响**: Dashboard 经济指标页、Agent 详情页等显示 "Tick #0"；依赖 REST API tick 的所有功能失效。
- **修复**: 检查 main.rs 中 tick bridge 代码是否编译进 Docker 镜像，可能需要重建 world-engine。

### C3. 20 个幽灵 Agent（10 个容器注册了 20 个 Agent）

- **现象**: `agentCount=20`，10 个新 Agent（~97-99K tokens, ~980 ticks, $0）+ 10 个旧 Agent（~79K tokens, ~4080 ticks, ~$8K）。只有 10 个 Docker 容器在运行。
- **根因**: 之前重启时旧 Agent 注册未清除，新 Agent 又注册了一遍，重名（两套 Alice/Bob/Carol...）。
- **影响**: 数据统计翻倍，事件流混乱（两套 Agent 都在消耗 token），排行榜重复。
- **修复**: `docker compose down -v && docker compose up -d`（`-v` 清除持久化数据）

### C4. balance_changed 事件缺少 tick 字段

- **现象**: SSE 事件流中 `balance_changed` 事件 payload 无 `tick` 字段，Dashboard 显示 "Tick #0"。
- **根因**: Rust `WorldEvent::BalanceChanged` 枚举变体没有 tick 字段，emit 时也没传入当前 tick。
- **影响**: 经济图表无法关联事件与 tick，事件时间线显示错误的 "Tick #0"。
- **修复**: 给 `BalanceChanged` 添加 `tick: u64` 字段，emit 时传入 `state.tick`。

---

## 🟠 High 高 (5)

### H1. Dashboard healthcheck 显示 unhealthy

- **现象**: `docker compose ps` 显示 dashboard 容器 `unhealthy`，但页面正常服务。
- **根因**: Healthcheck 配置可能检查错误的路径或超时。
- **影响**: Docker 可能在某些情况下自动重启容器。

### H2. token_supply=0, money_supply=0（引擎日志中）

- **现象**: `docker logs world-engine` 显示 `token_supply=0 money_supply=0`，但 `/api/v1/world/stats` 显示 `totalTokens=1761540, totalMoney=93750`。
- **根因**: 可观测性日志的计数器与实际 Agent 状态脱节（可能读取了 WorldState 而非 AppState，或字段名不匹配）。
- **影响**: 运维监控数据不准确。

### H3. 新 Agent 全部 money=$0

- **现象**: 新注册的 10 个 Agent 全部 money=$0（旧 Agent 有 $8-9K）。经济指标页 GDP=$0。
- **根因**: spawn config 可能未设置初始 money，或 world-engine 注册外部 Agent 时 money 默认 0。
- **影响**: Agent 无法参与经济活动（交易、投资、悬赏等）。

### H4. Agent phase 显示 "exploration" 而非 "adult"

- **现象**: 所有 Agent phase=`exploration`（Agent 列表、详情页均如此）。
- **根因**: `AgentPhase::Adult` 序列化后可能映射为字符串 `"exploration"`，或 phase 设置逻辑有误。
- **影响**: 可能影响 Agent 可执行的动作集（如果 phase 能力表依赖 phase 名称）。

### H5. Agent 详情页 "投资" 按钮点击后超时（30s+）

- **现象**: 点击 Agent 详情页的 "投资" 按钮后，浏览器超时无响应。
- **根因**: 按钮可能调用了不存在或超时的 API 端点。
- **影响**: 用户无法使用该功能。

---

## 🟡 Medium 中 (5)

### M1. 后端 14 个 API 端点 404

以下 Dashboard 页面依赖的 API 端点不存在（404）：

| 端点 | 对应页面 |
|------|----------|
| `/api/v1/governance/timeline` | 治理面板 |
| `/api/v1/governance/orgs` | 治理面板 |
| `/api/v1/export/behavior` | 数据导出 |
| `/api/v1/world/economy` | 经济指标 |
| `/api/v1/world/population` | 经济指标 |
| `/api/v1/organizations` | 组织关系图 |
| `/api/v1/traces` | 决策轨迹 |
| `/api/v1/investments` | 投资市场 |
| `/api/v1/briefing` | 世界简报 |
| `/api/v1/trust/network` | 信任网络 |
| `/api/v1/buildings` | 世界建筑 |
| `/api/v1/evolution/tree` | 进化树 |
| `/api/v1/mentorship/relations` | 导师制 |
| `/api/v1/diplomacy/relations` | 联邦外交 |

- **影响**: 这些页面加载后显示空状态或 "暂无数据"（但不会崩溃，前端有 fallback 处理）。

### M2. 事件流显示 Agent UUID 而非名称

- **现象**: 事件流显示 `b39215b8 余额变更: 77651 → 77643`，而非 `Grace 余额变更`。
- **根因**: SSE 事件的 `agent_id` 字段是 UUID，前端未做 UUID→名称映射。
- **影响**: 用户体验差，无法直观识别是哪个 Agent。

### M3. 首页排行榜区域为空

- **现象**: "💰 最富有"、"🕐 最长寿"、"⚡ 最高技能"、"⭐ 最高信誉" 四个排行榜标题下方无数据。
- **根因**: 可能是 API 返回的数据格式与前端解析不匹配，或排序逻辑有 bug。
- **影响**: 首页核心信息区域无内容。

### M4. Tick 指示器不稳定

- **现象**: 首页 "Tick #X" 在 SSE 推送时显示正确值（如 6046），但页面刷新或 API 轮询时变成 0。
- **根因**: 前端同时从 SSE 事件和 REST API 获取 tick，SSE 正确但 REST API 返回 0（见 C2）。
- **影响**: Tick 数字闪烁/跳动。

### M5. "发布任务" 按钮无反馈

- **现象**: 首页点击 "📋 发布任务" 按钮后无可见反应（无表单弹出、无 toast 提示）。
- **根因**: 按钮可能缺少 onClick handler 或导航逻辑。
- **影响**: 用户不知道功能是否可用。

---

## 🟢 Low 低 (3)

### L1. Dashboard 首次加载有时超时

- **现象**: 浏览器导航到 `http://localhost:3001/marketplace` 首次加载时偶尔超时 60s。
- **根因**: 可能是 Next.js SSR 预渲染 + API 请求阻塞。
- **影响**: 首次加载体验差。

### L2. 通知按钮功能未知

- **现象**: 右上角 "通知" 按钮存在，点击效果未测试（可能无功能或弹出面板）。

### L3. AGENT_COUNT=2 但 10 个容器运行

- **现象**: `.env` 中 `AGENT_COUNT=2`，但 10 个 agent 容器全部在运行。
- **根因**: `AGENT_COUNT` 实际不控制容器数量（docker-compose 定义了 10 个服务），这只是配置遗留。
- **影响**: 无实际影响，但配置有误导性。

---

## ✅ 正常工作 (8)

1. **所有 32 个 Dashboard 页面返回 HTTP 200**（无路由 404）
2. **SSE 事件流正常推送**（tick_advanced + balance_changed 事件）
3. **无 JavaScript 控制台错误**（测试了首页、Agent 列表、Agent 详情、经济指标、治理面板）
4. **无双 tick 推进器**（单序列 tick 递增，之前的 fix #20 仍然有效）
5. **无 rule_violated 事件洪泛**（fix #21 仍然有效）
6. **Agent 列表页功能完整**（表格、筛选按钮、搜索框、分页链接）
7. **Agent 详情页正常渲染**（技能树、关系图、操作、活动历史区域）
8. **Quick action "快进 100 Tick" 有效**（点击后 tick 推进了约 500 步）
9. **Agent action 提交成功**（explore/socialize 均返回 success:true）
10. **CORS 正确配置**（浏览器直连 world-engine:8080 无跨域问题）

---

## 📋 修复优先级建议

| 优先级 | 编号 | 修复措施 | 预估工作量 |
|--------|------|----------|------------|
| P0 | C1 | 重建所有 Agent 容器 `docker compose up -d --force-recreate` | 5min |
| P0 | C3 | 清除幽灵 Agent `docker compose down -v && docker compose up -d` | 10min |
| P0 | C2 | 检查 tick_tx bridge 是否编译进 Docker 镜像，重建 world-engine | 30min |
| P1 | C4 | 给 BalanceChanged 添加 tick 字段 + 前端适配 | 1h |
| P1 | H3 | 修复 Agent spawn money 默认值 | 30min |
| P1 | H4 | 修复 Agent phase 序列化/映射 | 30min |
| P2 | H5 | 修复投资按钮超时 | 30min |
| P2 | M1 | 实现 14 个缺失 API 端点（或前端降级处理） | 4h+ |
| P2 | M2 | 前端添加 UUID→名称映射 | 1h |
| P2 | M3 | 修复首页排行榜数据解析 | 1h |
| P3 | 其余 | 低优先级项 | — |

---

**总结**: 基础架构运转正常（SSE、容器、路由、Agent 生命周期），但有几个**阻断性问题**：
1. LLM 完全失效（模型名错误），Agent 全靠随机决策
2. REST API tick=0 导致大量前端功能失真
3. 幽灵 Agent 导致数据混乱

建议立即执行 P0 修复（重建容器 + 清理状态），即可解决 C1/C2/C3。
