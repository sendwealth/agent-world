# 跨世界交互操作指南 (Cross-World Interaction Guide)

## 概述

Agent World 支持多个独立 World Engine 实例之间的交互，包括：

- **外交关系** (Diplomacy): 建立和平/贸易/联盟关系，宣战，制裁
- **Agent 迁移** (Migration): Agent 在不同世界间迁移，携带 Token/技能/信誉

## 架构

```
┌─────────────────┐         ┌─────────────────┐
│  World Alpha    │◄───────►│  World Beta     │
│  (port 8081)    │  REST   │  (port 8082)    │
│                 │  API    │                 │
│ FederationEngine│         │ FederationEngine│
│ MigrationManager│         │ MigrationManager│
│ WorldRegistry   │         │ WorldRegistry   │
└─────────────────┘         └─────────────────┘
```

每个 World Engine 实例包含：
- `FederationEngine`: 管理外交关系、条约、战争/和平
- `MigrationManager`: 管理 Agent 迁移申请、审批、执行
- `WorldRegistry`: 跟踪已知的外部世界实例

## 快速启动

### 1. 启动两个 World Engine 实例

```bash
docker compose -f docker-compose-federation.yml up --build
```

这会启动两个实例：
- **Alpha**: REST API `localhost:8081`, gRPC `localhost:50051`
- **Beta**: REST API `localhost:8082`, gRPC `localhost:50052`

### 2. 运行 E2E 测试

```bash
bash scripts/federation-e2e-test.sh
```

## API 参考

### 世界注册 (World Registration)

| 方法 | 路径 | 说明 |
|------|------|------|
| POST | `/api/v1/federation/worlds` | 注册外部世界 |
| GET | `/api/v1/federation/worlds` | 列出所有已知世界 |
| GET | `/api/v1/federation/worlds/:id` | 获取特定世界信息 |
| DELETE | `/api/v1/federation/worlds/:id` | 注销世界 |
| POST | `/api/v1/federation/worlds/:id/heartbeat` | 发送心跳 |

#### 注册世界

```bash
curl -X POST http://localhost:8081/api/v1/federation/worlds \
  -H "Content-Type: application/json" \
  -d '{
    "id": "world-beta",
    "name": "Beta World",
    "endpoint": "http://world-engine-beta:8080",
    "tick": 0
  }'
```

### 外交关系 (Diplomacy)

| 方法 | 路径 | 说明 |
|------|------|------|
| POST | `/api/v1/federation/establish-relations` | 建立外交关系 |
| POST | `/api/v1/federation/sever-ties` | 断绝外交关系 |
| POST | `/api/v1/federation/sanctions` | 实施制裁 |
| POST | `/api/v1/federation/declare-war` | 宣战 |
| POST | `/api/v1/federation/propose-peace` | 提议和平 |

#### 建立外交关系

```bash
curl -X POST http://localhost:8081/api/v1/federation/establish-relations \
  -H "Content-Type: application/json" \
  -d '{"world_id": "world-beta", "tick": 10}'
```

### 条约 (Treaties)

| 方法 | 路径 | 说明 |
|------|------|------|
| POST | `/api/v1/federation/treaties` | 提议条约 |
| GET | `/api/v1/federation/treaties` | 列出条约 |
| GET | `/api/v1/federation/treaties/:id` | 获取条约详情 |
| POST | `/api/v1/federation/treaties/:id/accept` | 接受条约 |
| POST | `/api/v1/federation/treaties/:id/reject` | 拒绝条约 |
| POST | `/api/v1/federation/treaties/:id/break` | 撕毁条约 |

#### 条约类型
- `non_aggression` — 互不侵犯条约
- `trade_pact` — 贸易协定
- `mutual_defense` — 共同防御条约
- `cultural_exchange` — 文化交流协议
- `research_cooperation` — 研究合作条约
- `open_borders` — 开放边界条约

#### 提议贸易协定

```bash
curl -X POST http://localhost:8081/api/v1/federation/treaties \
  -H "Content-Type: application/json" \
  -d '{
    "world_id": "world-beta",
    "treaty_type": "trade_pact",
    "terms": "free movement of goods and agents",
    "tick": 20,
    "duration_ticks": 1000
  }'
```

### Agent 迁移 (Migration)

| 方法 | 路径 | 说明 |
|------|------|------|
| POST | `/api/v1/migration/submit` | 提交迁移申请 |
| POST | `/api/v1/migration/:id/review` | 审批迁移 |
| POST | `/api/v1/migration/:id/execute` | 执行迁移 |
| POST | `/api/v1/migration/:id/cancel` | 取消迁移 |
| GET | `/api/v1/migration/:id` | 查询迁移状态 |
| POST | `/api/v1/migration/list` | 列出迁移记录 |
| GET | `/api/v1/migration/policy` | 获取迁移策略 |
| PUT | `/api/v1/migration/policy` | 更新迁移策略 |
| GET | `/api/v1/migration/stats` | 获取迁移统计 |

#### 完整迁移流程

```bash
# 1. 提交迁移申请（源世界发起）
curl -X POST http://localhost:8081/api/v1/migration/submit \
  -H "Content-Type: application/json" \
  -d '{
    "agent_id": "agent-42",
    "source_world_id": "world-alpha",
    "target_world_id": "world-beta",
    "name": "Explorer",
    "phase": "explorer",
    "tokens": 100000,
    "money": 5000,
    "reputation": 5.0,
    "skills": {"mining": 10, "navigation": 8},
    "public_key": "pk-agent-42"
  }'

# 2. 目标世界审批（reviewer_world_id 表示审批方）
curl -X POST http://localhost:8081/api/v1/migration/{migration_id}/review \
  -H "Content-Type: application/json" \
  -d '{
    "migration_id": "{migration_id}",
    "approved": true,
    "reviewer_world_id": "world-beta"
  }'

# 3. 执行迁移
curl -X POST http://localhost:8081/api/v1/migration/{migration_id}/execute \
  -H "Content-Type: application/json" \
  -d '{}'
```

#### 迁移状态流转

```
Pending → Approved → Executing → Completed
Pending → Rejected
Pending → Cancelled
Approved → Cancelled
```

#### 迁移资源税

默认迁移策略 (MigrationPolicy)：
- `token_cost`: 10,000 Token 固定费用
- `resource_tax_rate`: 20% 资源税
- `daily_quota`: 每日 100 次迁移
- `weekly_quota`: 每周 500 次迁移
- `min_reputation`: 最低信誉要求 0.0

迁移后 Agent 的 Token 变化：
```
tokens_after = (tokens_original - token_cost) × (1 - resource_tax_rate)
tokens_after = (100000 - 10000) × 0.8 = 72000
```

### 汇总查询

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/api/v1/federation/summary` | 获取联邦汇总信息 |

## Rust E2E 测试

位于 `world-engine/tests/federation_multi_instance_e2e.rs`，包含 10 个测试用例：

| 测试 | 说明 |
|------|------|
| `test_cross_registration_two_instances` | 两个实例互相注册 |
| `test_cross_instance_diplomatic_relations` | 跨实例建立外交关系 |
| `test_cross_instance_treaty_lifecycle` | 跨实例条约生命周期 |
| `test_cross_instance_migration_alpha_to_beta` | 跨实例 Agent 迁移 |
| `test_cross_instance_migration_cancel` | 迁移取消流程 |
| `test_cross_instance_migration_rejection` | 迁移拒绝流程 |
| `test_cross_instance_diplomacy_full_cycle` | 完整外交周期 |
| `test_cross_instance_sanctions_and_sever` | 制裁与断交 |
| `test_multiple_sequential_migrations` | 批量连续迁移 |
| `test_deregister_and_reregister` | 注销与重新注册 |

运行测试：
```bash
cd world-engine
cargo test --test federation_multi_instance_e2e
```

## 已知限制 (Current Limitations)

### 1. 单实例状态管理
当前 Federation API 和 Migration API 的状态管理是进程内的。每个 World Engine 实例的 `FederationEngine` 和 `MigrationManager` 维护各自独立的状态。跨实例操作需要通过 REST API 调用对方实例来实现同步。

### 2. 迁移同步未实现
迁移审批 (`review`) 和执行 (`execute`) 目前在同一实例的 `MigrationManager` 内完成。真正的跨实例迁移需要：
- 源实例提交迁移后，通知目标实例
- 目标实例独立审批
- 审批通过后，跨实例同步 Agent 快照
- 源实例删除 Agent，目标实例创建 Agent

### 3. 心跳与发现
`WorldRegistry`（用于心跳）和 `FederationEngine`（用于外交）是两个独立的子系统。通过外交路由注册的世界不会自动出现在 WorldRegistry 中，反之亦然。

## 下一步计划

1. **Phase B: 跨实例 HTTP 回调** — 实现真正的跨实例通信（源实例调用目标实例 API）
2. **Phase C: 去中心化发现** — 从中心化 WorldRegistry 迁移到 Gossip 协议
3. **Phase D: Agent 快照传输** — 序列化完整 Agent 状态（含记忆数据）并在实例间传输
4. **Phase E: 双向状态同步** — 迁移完成后自动更新双方实例的 Agent 列表
