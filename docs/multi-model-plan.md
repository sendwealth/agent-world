# Agent World 多模型支持开发计划

> 参考 Open Design (nexu-io/open-design) v0.8.0 的模型接入设计
> 制定时间: 2026-06-01

---

## 一、现状分析

### 1.1 Agent World 当前模型层

**文件结构:**
```
agent-runtime/agent_runtime/llm/
├── base.py               # LLMProvider ABC + LLMConfig + ProviderType enum
├── factory.py            # create_provider(config) 工厂函数
├── ollama_provider.py    # Ollama 本地模型（已实现 model 热切换）
├── openai_provider.py    # OpenAI 兼容 API
├── anthropic_provider.py # Anthropic Claude
├── cost.py               # Token 成本计算
├── decision_log.py       # 决策日志
├── prompts.py            # Prompt 模板
└── queue.py              # 请求队列
```

**当前模型配置方式:**
- `ProviderType` 枚举: `OPENAI | ANTHROPIC | OLLAMA`（硬编码 3 种）
- `LLMConfig` dataclass: `provider + model + api_key + base_url + timeout + max_tokens + temperature`
- 工厂模式: `create_provider(config)` → 根据 provider 枚举实例化对应类
- CLI 入口 (`__main__.py`): 通过 `--provider` / `--model` / `--base-url` 参数 + 环境变量解析
- **所有 agent 共享同一个 LLMConfig**，由 Pool 层传递给子进程

**核心缺口:**
| 缺口 | 说明 |
|------|------|
| 无模型注册表 | provider 类型硬编码，新增 provider 需改 `ProviderType` 枚举 + factory + 新文件 |
| 无连接测试 | 不知道 API key/模型 ID 是否有效，直到运行时才报错 |
| 无模型发现 | 不支持动态列出可用模型（Ollama 有 `/api/ps` 但没暴露到上层） |
| 无 Dashboard 模型管理 | 前端无法切换模型、测试连接、配置多 provider |
| 全局单模型 | 所有 agent 强制同一模型，无法按 agent 分配不同模型 |
| 无协议抽象 | 各 provider 的 API 差异（Anthropic headers、Ollama native API）散落在各实现中 |

### 1.2 Open Design 的模型接入设计（参考）

Open Design 的核心理念是 **BYOK (Bring Your Own Key) + 多协议适配器 + 运行时模型发现**：

**架构层次:**
```
┌─────────────────────────────────────────────┐
│  Settings UI (Web)                           │
│  ├─ Provider 配置面板 (baseUrl + apiKey)      │
│  ├─ 模型选择器 (live models / fallback)       │
│  └─ 连接测试按钮                              │
├─────────────────────────────────────────────┤
│  Daemon (Express + SQLite)                   │
│  ├─ /api/proxy/*/stream  → BYOK 代理转发      │
│  ├─ connectionTest.ts    → Provider + Agent   │
│  │   ├─ Provider 测试 (anthropic/openai/      │
│  │   │   google/azure/ollama/senseaudio)      │
│  │   └─ Agent 测试 (spawn CLI + smoke test)   │
│  ├─ providerModels.ts    → 动态模型列表        │
│  │   ├─ OpenAI /v1/models                     │
│  │   ├─ Anthropic /v1/models                  │
│  │   ├─ Google models.list                    │
│  │   └─ Ollama /api/tags                      │
│  └─ runtimes/registry.ts → 21 个 Agent 定义    │
├─────────────────────────────────────────────┤
│  Contracts (纯 TypeScript 类型层)             │
│  ├─ ConnectionTestProtocol =                 │
│  │   'anthropic'|'openai'|'azure'|'google'|   │
│  │   'ollama'|'senseaudio'                    │
│  ├─ ProxyStreamRequest  → 统一流式请求        │
│  └─ ProviderModelsRequest → 模型发现请求      │
└─────────────────────────────────────────────┘
```

**关键设计模式:**

1. **RuntimeAgentDef（Agent 定义协议）**:
   ```typescript
   type RuntimeAgentDef = {
     id: string;           // 唯一标识
     name: string;         // 显示名
     bin: string;          // CLI 二进制名
     fallbackModels: RuntimeModelOption[];
     buildArgs: (prompt, imagePaths, options) => string[];
     listModels?: { args, parse };  // CLI 获取模型列表
     fetchModels?: (bin, env) => Promise<RuntimeModelOption[]>;
     supportsCustomModel?: boolean;
     defaultModelEnvVar?: string;
     // ... 更多能力声明
   }
   ```
   每个适配器是独立 `.ts` 文件，声明式描述能力。

2. **ConnectionTestProtocol（连接协议）**:
   6 种协议类型，每种有对应的模型发现和连接测试策略。

3. **动态模型发现**:
   - `listProviderModels()` → 调用各 provider 的 `/models` API
   - `liveModelCache` + `fallbackModels` → 双层缓存
   - `sanitizeCustomModel()` → 用户自定义模型 ID

4. **连接测试（Settings 级）**:
   - Provider 测试: 发送 "Reply with only: ok" 小请求
   - Agent 测试: spawn CLI 进程，检查输出
   - 结果分类: `success | auth_failed | timeout | rate_limited | ...`

---

## 二、目标架构

参考 Open Design 但适配 Agent World 的特点（Python Runtime + Rust Engine + Next.js Dashboard）：

```
┌──────────────────────────────────────────────────────────────┐
│  Dashboard (Next.js)                                         │
│  ┌──────────────────────────────────────────────────────┐    │
│  │ Settings > Model Providers                           │    │
│  │  ├─ Provider Cards: [Ollama ▼] [OpenAI ▼] [智谱 ▼]   │    │
│  │  ├─ 每个 Card: baseUrl + apiKey + 状态指示灯          │    │
│  │  ├─ [Test Connection] 按钮 + 延时/结果反馈            │    │
│  │  └─ [Discover Models] → 模型列表下拉                  │    │
│  ├──────────────────────────────────────────────────────┤    │
│  │ Settings > Agent Model Assignment                    │    │
│  │  ├─ Agent列表 × 模型下拉 (可按agent分配不同模型)       │    │
│  │  └─ 默认模型 (pool-level fallback)                    │    │
│  └──────────────────────────────────────────────────────┘    │
├──────────────────────────────────────────────────────────────┤
│  Agent Runtime (Python)                                       │
│  ┌──────────────────────────────────────────────────────┐    │
│  │ ModelRegistry (单例)                                  │    │
│  │  ├─ providers: dict[str, ProviderConfig]              │    │
│  │  ├─ discover_models(provider_id) → list[ModelOption]  │    │
│  │  ├─ test_connection(provider_id, model) → TestResult  │    │
│  │  ├─ get_provider(agent_id) → LLMProvider              │    │
│  │  └─ default_provider_id → str                         │    │
│  ├──────────────────────────────────────────────────────┤    │
│  │ ProviderConfig                                        │    │
│  │  ├─ id: str                    # "ollama-local"       │    │
│  │  ├─ protocol: str              # "openai"|"ollama"…   │    │
│  │  ├─ base_url: str              # "http://localhost…"  │    │
│  │  ├─ api_key: str | None                               │    │
│  │  ├─ models: list[ModelOption]   # discovered/cached   │    │
│  │  └─ agent_overrides: dict[str, ModelOption]           │    │
│  └──────────────────────────────────────────────────────┘    │
├──────────────────────────────────────────────────────────────┤
│  World Engine (Rust)                                          │
│  ├─ /api/v1/providers          → CRUD provider configs       │
│  ├─ /api/v1/providers/:id/test → connection test             │
│  ├─ /api/v1/providers/:id/models → model discovery           │
│  └─ config.rs: ProviderConfig serde                          │
└──────────────────────────────────────────────────────────────┘
```

---

## 三、开发模块与任务分解

### Phase 1: Runtime 层模型注册表（Python 侧）

**目标:** 将硬编码的 3 种 provider 扩展为动态注册表 + 连接测试 + 模型发现

#### 1.1 重构 `ProviderType` → 协议注册表

**文件:** `agent-runtime/agent_runtime/llm/registry.py` (新建)

```python
# 替代 ProviderType 枚举
class ProviderProtocol(str, Enum):
    OPENAI = "openai"          # OpenAI 兼容 (含智谱/DeepSeek/vLLM/OpenRouter)
    ANTHROPIC = "anthropic"
    OLLAMA = "ollama"
    GOOGLE = "google"          # Gemini
    AZURE = "azure"

@dataclass
class ProviderConfig:
    """一个独立的模型提供者配置（对应 Open Design 的 BYOK 卡片）"""
    id: str                    # "ollama-local", "zhipu-glm5"
    protocol: ProviderProtocol
    base_url: str
    api_key: str | None = None
    api_version: str | None = None  # Azure 专用
    display_name: str | None = None
    is_default: bool = False

@dataclass
class ModelOption:
    id: str
    label: str
    provider_id: str  # 属于哪个 provider

class ModelRegistry:
    """全局模型注册表（参考 Open Design 的 registry + liveModelCache）"""
    
    def register_provider(self, config: ProviderConfig) -> None
    def remove_provider(self, provider_id: str) -> None
    def get_provider_config(self, provider_id: str) -> ProviderConfig | None
    
    def list_providers(self) -> list[ProviderConfig]
    def discover_models(self, provider_id: str) -> list[ModelOption]
    
    def create_provider(self, provider_id: str) -> LLMProvider
    def resolve_provider_for_agent(self, agent_id: str) -> LLMProvider
    
    # Agent-level model override
    def set_agent_model(self, agent_id: str, provider_id: str, model: str) -> None
    def get_agent_model(self, agent_id: str) -> tuple[str, str] | None  # (provider_id, model)
```

#### 1.2 连接测试模块

**文件:** `agent-runtime/agent_runtime/llm/connection_test.py` (新建)

```python
@dataclass
class ConnectionTestResult:
    ok: bool
    kind: str  # "success" | "auth_failed" | "timeout" | "not_found_model" | ...
    latency_ms: int
    model: str | None = None
    sample: str | None = None  # 模型回复片段
    detail: str | None = None

async def test_provider_connection(
    config: ProviderConfig,
    model: str,
) -> ConnectionTestResult:
    """发送最小请求测试连接（参考 Open Design 的 Settings smoke test）"""
    # 每种 protocol 有不同的测试策略
    ...

async def discover_provider_models(
    config: ProviderConfig,
) -> list[ModelOption]:
    """从 provider API 动态获取可用模型列表"""
    # OpenAI: GET /v1/models
    # Anthropic: GET /v1/models?limit=1000
    # Ollama: GET /api/tags
    # Google: GET /v1beta/models
    ...
```

#### 1.3 向后兼容改造

- `ProviderType` → 保留为 `ProviderProtocol` 的别名（不破坏现有代码）
- `create_provider(LLMConfig)` → 内部转换为 `ModelRegistry.create_provider()`
- CLI `--provider zhipu` 逻辑不变，但走注册表路径

### Phase 2: World Engine Provider API（Rust 侧）

**目标:** 在 Rust 层提供 Provider 配置的持久化和 API

#### 2.1 Provider 配置持久化

**文件:** `world-engine/src/provider_config.rs` (新建)

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub id: String,
    pub protocol: String,  // "openai" | "anthropic" | "ollama" | "google" | "azure"
    pub base_url: String,
    pub api_key: Option<String>,
    pub api_version: Option<String>,
    pub display_name: Option<String>,
    pub is_default: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentModelAssignment {
    pub agent_id: String,
    pub provider_id: String,
    pub model_id: String,
}
```

#### 2.2 REST API 端点

**文件:** `world-engine/src/api_providers.rs` (新建)

```
POST   /api/v1/providers              → 创建 provider 配置
GET    /api/v1/providers              → 列出所有 provider
GET    /api/v1/providers/:id          → 获取单个 provider
PUT    /api/v1/providers/:id          → 更新 provider
DELETE /api/v1/providers/:id          → 删除 provider
POST   /api/v1/providers/:id/test     → 连接测试（代理到 Runtime）
GET    /api/v1/providers/:id/models   → 模型发现（代理到 Runtime）

PUT    /api/v1/agents/:id/model       → 设置 agent 的模型分配
GET    /api/v1/agents/:id/model       → 获取 agent 的模型分配
```

#### 2.3 SQLite 存储

Provider 配置存入 World Engine 的 SQLite（复用现有 `Storage` 基础设施），包含加密 api_key 字段。

### Phase 3: Dashboard Settings UI（Next.js 侧）

**目标:** 可视化的模型管理和 Agent 模型分配界面

#### 3.1 Settings > Model Providers 页面

**参考 Open Design 的 Settings 对话框:**

- **Provider 卡片列表**: 每个 provider 显示为一张卡片
  - 状态指示灯（绿/红/灰）
  - `base_url` 输入框
  - `api_key` 输入框（密码类型，支持显示/隐藏）
  - `[Test Connection]` 按钮 → 显示延迟和测试结果
  - `[Discover Models]` 按钮 → 填充模型下拉
  - 预设模板（一键配置 Ollama / 智谱 / DeepSeek / OpenRouter）

- **添加 Provider 按钮**:
  - 选择 protocol 类型
  - 填写配置
  - 保存

#### 3.2 Settings > Agent Model Assignment 页面

- Agent 列表 + 模型下拉
- 支持选择 provider + model 两级选择
- "Use default" 选项（继承 pool 级配置）
- 拖拽批量分配

#### 3.3 实时状态面板

- 各 provider 在线状态
- 每个 agent 当前使用的模型
- 最近 N 次请求的延迟/成功率

### Phase 4: 高级特性

#### 4.1 OpenAI 兼容协议统一

将 DeepSeek、智谱 GLM、OpenRouter、vLLM、LocalAI 等全部归为 `openai` 协议：
- 只需填 `base_url` + `api_key`
- 自动使用 `/v1/chat/completions` 和 `/v1/models`

#### 4.2 模型热切换（已有基础）

Ollama 的 `switch_model()` 已实现。扩展到所有 provider：
- `ModelRegistry` 维护 per-agent 的 provider + model 映射
- Agent 进程通过 gRPC/REST 接收模型切换指令
- ThinkLoop 在下一个 tick 使用新模型

#### 4.3 Fallback Chain

```python
@dataclass
class ModelFallback:
    """模型降级链"""
    primary: tuple[str, str]     # (provider_id, model_id)
    fallbacks: list[tuple[str, str]]  # 按优先级排序
    # 例: primary=(zhipu, glm-5), fallbacks=[(ollama, minicpm5-1b)]
```

当 primary 失败时自动切换到 fallback。

#### 4.4 预设模型商店

类似 Open Design 的 `sync-litellm-models.ts`，内置常见模型的元信息：

```yaml
# presets/models.yaml
- id: "glm-5"
  provider: "zhipu"
  label: "GLM-5 (智谱)"
  ctx: "128K"
  cost_per_1k_input: 0.001
  cost_per_1k_output: 0.001
  recommended_for: ["chat", "decision"]

- id: "minicpm5-1b"
  provider: "ollama"
  label: "MiniCPM5-1B (本地)"
  ctx: "4K"
  cost_per_1k_input: 0
  recommended_for: ["fast-decision", "low-latency"]
```

---

## 四、实施优先级

| 优先级 | 模块 | 预估工时 | 依赖 |
|--------|------|----------|------|
| **P0** | 1.1 协议注册表 + 重构 ProviderType | 2天 | 无 |
| **P0** | 1.2 连接测试模块 | 1天 | 1.1 |
| **P0** | 1.3 向后兼容改造 | 0.5天 | 1.1 |
| **P1** | 2.1-2.3 Rust Provider API | 3天 | 1.1 |
| **P1** | 3.1 Provider Settings UI | 3天 | 2.x |
| **P1** | 3.2 Agent Model Assignment UI | 1.5天 | 2.x |
| **P2** | 4.1 OpenAI 兼容统一 | 1天 | 1.1 |
| **P2** | 4.2 全局模型热切换 | 1.5天 | 3.x |
| **P2** | 4.3 Fallback Chain | 1天 | 4.2 |
| **P2** | 3.3 实时状态面板 | 2天 | 3.x |
| **P3** | 4.4 预设模型商店 | 1.5天 | 3.1 |

**总预估: ~17 天 (含测试)**

---

## 五、技术决策

| 决策点 | 方案 | 理由 |
|--------|------|------|
| 协议类型 | 5 种 (openai/anthropic/ollama/google/azure) | 参考 Open Design 的 `ConnectionTestProtocol`，覆盖主流场景 |
| 配置存储 | World Engine SQLite | 复用现有基础设施，支持持久化和跨 agent 共享 |
| API Key 安全 | AES-256 加密存储 | 与 Open Design 一致，不存明文 |
| 模型发现 | per-protocol extractor | OpenAI/Anthropic/Google/Ollama 各有不同 API，需分别解析 |
| Agent 模型分配 | 两级覆盖 (pool default → agent override) | 灵活且向后兼容 |
| 向后兼容 | `LLMConfig` → `ProviderConfig` 适配器 | 不破坏现有 CLI 和测试 |
| Dashboard 组件 | shadcn/ui Card + Select + Dialog | 与现有 Dashboard 风格一致 |

---

## 六、Open Design 代码参考索引

| 需要参考的部分 | 文件路径 |
|---------------|---------|
| 协议类型定义 | `packages/contracts/src/api/connectionTest.ts` |
| Provider 模型发现 | `apps/daemon/src/providerModels.ts` |
| 连接测试实现 | `apps/daemon/src/connectionTest.ts` |
| Agent 适配器定义 | `apps/daemon/src/runtimes/defs/*.ts` |
| Agent 注册表 | `apps/daemon/src/runtimes/registry.ts` |
| 模型缓存/解析 | `apps/daemon/src/runtimes/models.ts` |
| BYOK 代理流式请求 | `packages/contracts/src/api/proxy.ts` |
| Provider 模型类型 | `packages/contracts/src/api/providerModels.ts` |
| AMR 模型发现示例 | `apps/daemon/src/runtimes/defs/amr.ts` |
| Claude 适配器示例 | `apps/daemon/src/runtimes/defs/claude.ts` |
| Settings UI 测试 | `e2e/ui/settings-media-providers.test.ts` |
