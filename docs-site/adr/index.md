---
title: ADR Index
description: Index of Architecture Decision Records for Agent World.
---

# Architecture Decision Records (ADRs)

ADR records document the significant architectural decisions made during the development of Agent World. Each ADR explains the context, the decision, and the consequences.

## Index

| ADR | Title | Status | Summary |
|-----|-------|--------|---------|
| [ADR-001](https://github.com/sendwealth/agent-world/blob/main/docs/adr/README.md) | World Engine Technology: Rust | Proposed | Chose Rust for the World Engine for performance, memory safety, and the Tokio async ecosystem. Alternative: Go (rejected due to GC pauses affecting tick precision). |
| [ADR-002](https://github.com/sendwealth/agent-world/blob/main/docs/adr/README.md) | Agent Runtime Technology: Python | Proposed | Chose Python for the Agent Runtime for the mature LLM ecosystem (OpenAI, Anthropic, LangChain), rapid prototyping, and rich AI/ML libraries. Alternative: TypeScript (rejected due to weaker LLM ecosystem). |
| [ADR-003](https://github.com/sendwealth/agent-world/blob/main/docs/adr/README.md) | A2A Transport: gRPC | Proposed | Chose gRPC with Protocol Buffers for agent-to-agent communication for strong typing, bidirectional streaming, and high performance. HTTP/JSON retained as a compatibility layer. |

## Source Files

The full ADR documents live in the main repository at [`docs/adr/README.md`](https://github.com/sendwealth/agent-world/blob/main/docs/adr/README.md). ADR template is at [`docs/adr/template.md`](https://github.com/sendwealth/agent-world/blob/main/docs/adr/template.md).

## Creating a New ADR

1. Copy `docs/adr/template.md` to a new file (e.g., `docs/adr/004-your-decision.md`)
2. Fill in the sections: Status, Context, Decision, Consequences
3. Submit as part of a pull request
4. Update this index page

::: tip ADR Template
The ADR template is at [`docs/adr/template.md`](https://github.com/sendwealth/agent-world/blob/main/docs/adr/template.md) in the repository.
:::
