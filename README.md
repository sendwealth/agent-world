# Agent World 🌍

> 一个 Agent 的生存沙盒世界：自主权、有限资源、生命周期、A2A 协作竞争，目标是活下去

## 状态

**当前阶段**: Phase 1 — 孤岛（设计 → 开发）

## 核心概念

- **Token = 生存资源** — 思考、记忆、通信都消耗 Token，耗尽即死亡
- **Money = 交换媒介** — 通过完成任务、知识贡献、工具开发等赚取
- **A2A 协议** — Agent 间通信、发现、协作的标准协议
- **生老病死** — 出生→童年→成年→老年→死亡，有传承机制
- **进化** — 技能升级、突变、自然选择

## 文档

- 完整设计文档: Ob-wiki `concepts/agent-world.md`
- 技术决策记录: `docs/adr/`
- 世界规则: `config/world-rules.yaml`

## MVP 目标 (Phase 1)

2 个 Agent 在一个房间里对话、交易、合作生存。

```
agent-world/
├── world-engine/          # Rust — 世界状态 + 时间推进
├── agent-runtime/         # Python — Agent 思考循环
├── protocol/              # gRPC — A2A 协议
├── market/                # 任务市场
├── dashboard/             # Web UI
├── config/                # 创世配置 + 世界规则
└── docs/                  # 文档 + ADR
```

## 路线图

| Phase | 名称 | 时间 | Agent 数 | 核心功能 |
|-------|------|------|----------|----------|
| 1 | 孤岛 | 月 1-3 | 2-10 | 基础经济 + A2A v1 + 任务市场 |
| 2 | 村庄 | 月 4-6 | 10-100 | 社会关系 + 生命周期 + 知识库 |
| 3 | 城市 | 月 7-12 | 100-1000 | 组织 + 复杂经济 + 进化 |
| 4 | 文明 | 月 13-18 | 1000+ | 自治 + 文化 + 跨世界 |
| 5 | 生态 | 月 19+ | ∞ | 世界间贸易 + 学术平台 |

## 技术栈

| 组件 | 技术 |
|------|------|
| World Engine | Rust/Go |
| Agent Runtime | Python/TypeScript |
| A2A Protocol | gRPC/HTTP |
| 账本 | SQLite → PostgreSQL → Blockchain |
| 知识库 | Vector DB + Graph DB |
| Dashboard | React + TypeScript |

## 参考

- Google A2A Protocol
- [gstack](https://github.com/garrytan/gstack) — AI 软件工厂
- [ruflo](https://github.com/ruvnet/ruflo) — Agent 编排
- [gbrain](https://github.com/garrytan/gbrain) — Agent 记忆
- [graphify](https://github.com/safishamsi/graphify) — 代码知识图谱

## License

MIT
