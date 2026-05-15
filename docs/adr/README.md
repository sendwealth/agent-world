# ADR-001: World Engine 技术选型

## 状态: 提议

## 背景

World Engine 负责世界状态管理、时间推进、规则执行。需要高并发、低延迟、强一致性。

## 决策

**首选: Rust**
- 性能：接近零成本抽象，适合高频 tick
- 安全：内存安全，适合金融账本
- 生态：tokio 异步运行时，tonic gRPC

**备选: Go**
- 开发速度快，协程模型简单
- 但 GC 暂停可能影响 tick 精度

## 影响

- 开发周期较长，但运行时性能更好
- 需要团队 Rust 能力

---

# ADR-002: Agent Runtime 技术选型

## 状态: 提议

## 背景

Agent Runtime 负责思考循环、记忆管理、工具调用。需要 LLM 集成和灵活的插件系统。

## 决策

**首选: Python**
- LLM 生态最成熟（OpenAI、Anthropic、LangChain）
- 快速原型开发
- 丰富的 AI/ML 库

**备选: TypeScript**
- 与 Dashboard 技术栈统一
- 但 LLM 生态不如 Python

## 影响

- Agent 思考循环用 Python 实现
- 通过 gRPC/HTTP 与 World Engine 通信

---

# ADR-003: A2A 协议传输层

## 状态: 提议

## 背景

Agent 间通信需要低延迟、类型安全、双向流。

## 决策

**gRPC (Protobuf)**
- 强类型，自动生成多语言客户端
- 双向流，适合实时通信
- 高性能，适合 Agent 间高频交互

**HTTP/JSON 作为兼容层**
- 方便调试和第三方接入
- MCP 协议兼容

## 影响

- protocol/ 目录维护 .proto 文件
- 需要 protoc 编译链
