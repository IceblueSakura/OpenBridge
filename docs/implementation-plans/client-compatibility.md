# 目标客户端契约：Codex 与 Hermes Agent

## 状态

**Working hypothesis。** 客户端行为以每次滚动记录的实际版本和真实 fixture 为准；本文不固定长期客户端版本，也不定义整体接受条件。

本文定义本地正在使用的 Codex 与 Hermes Agent 的兼容优先级和验证方式。选择它们是为了验证实际本地工作流，不表示 OpenBridge 提供客户端管理、客户端产品适配承诺或通用 Agent 平台；每次升级目标版本时必须重跑兼容 corpus。

## 1. 结论

OpenBridge 为两个本地 Agent 保持兼容路径，但优先级不对称：

- **Codex：Responses-first，初期 HTTP/SSE-first。** 面向 Codex 的首要契约是 `/v1/responses`。使用独立 custom Provider id，并显式配置 `supports_websockets = false`；不以 Chat Completions 或 Responses WebSocket 作为初期兼容前提。
- **Hermes Agent：按声明验证。** Hermes 还存在 `anthropic_messages` 等模式；Anthropic Messages 兼容是与 hosted tool facade 同级的后续方向，不作为当前客户端契约。

因此不采用“所有请求先归一成一个协议”的入口设计。原生协议应直接进入对应 native path，只有上下游协议不一致时才进入 Protocol Bridge。

### 1.1 开发验证优先级

每次影响 HTTP/JSON/SSE、tool loop、错误或取消语义的开发改动，先运行当时可用的 OpenAI SDK Chat/Responses regression；影响 Codex Responses profile 时，再运行当时可用的 Codex CLI custom Provider E2E。SDK 验证标准 wire contract，Codex CLI 验证目标客户端的实际 transport 与 tool-loop 行为。

SDK/CLI 不固定为长期精确版本。每次验证 artifact 必须记录实际解析到的版本、安装来源、平台、运行日期和无 secret 配置快照；版本滚动后重跑对应 regression。一次通过只证明该次记录的运行环境，不推断前后版本等价。

Hermes 仍是兼容目标，但只在版本、文档或发布声明包含 Hermes 支持时运行对应 E2E；它不替代 SDK/Codex CLI 的日常验证，也不能只凭 SDK 通过而被宣称已兼容。

## 2. 目标兼容路径

| 客户端 | 下游协议 | 上游协议 | 处理路径 | 优先级 |
|---|---|---|---|---|
| Codex | Responses over HTTP/SSE | Responses over HTTP/SSE | Native | 初期方向 |
| Hermes | Chat Completions | Chat Completions | Native | 按兼容声明选择 |
| Hermes | Responses | Responses | Native | 后续可选 |
| Codex | Responses over HTTP/SSE | Chat Completions | Protocol Bridge | 后续可选 |
| Hermes | Chat Completions | Responses | Protocol Bridge | 后续可选 |
| Hermes | Chat/Responses | Anthropic Messages | Protocol Bridge | 后续方向，与 hosted tool facade 同级 |

## 3. Codex 契约

### 3.1 首版 transport profile

Codex 当前 custom model Provider 可配置 `base_url`、认证、Responses wire protocol 和 `supports_websockets`。OpenBridge 首版只承诺 HTTP JSON/SSE，因此必须使用非保留的 custom Provider id，而不是尝试覆盖内置 `openai` Provider：

```toml
model = "code-primary"
model_provider = "openbridge"

[model_providers.openbridge]
name = "OpenBridge"
base_url = "http://127.0.0.1:8080/v1"
env_key = "OPENBRIDGE_DOWNSTREAM_TOKEN"
wire_api = "responses"
supports_websockets = false
```

`supports_websockets = false` 应显式写入 fixture，不能只依赖默认值。测试还应记录 Codex 的诊断/日志，确认 active provider 的 WebSocket capability 为 false，且实际请求走 HTTP/SSE。

Responses WebSocket 是独立候选 transport。以下任一条件成立时必须重新打开范围决策：

- 固定目标 Codex 版本忽略 custom Provider 的 `supports_websockets = false`；
- Codex 移除或实质降级 HTTP/SSE Responses path；
- 目标模型/工作流必须依赖连接级增量输入或 WebSocket-only state；
- HTTP/SSE 的兼容性或性能无法满足核心使用场景。

### 3.2 必须验证

- custom Provider 的 `base_url`、认证环境变量、Responses wire mode 和 WebSocket capability；
- `/v1/responses` 请求 body 与模型字段；
- Responses SSE event framing、terminal event 和错误事件；
- function tool schema、`call_id`、arguments delta 与 tool output 回传；
- reasoning item、usage、取消和 incomplete/failed outcome；
- `previous_response_id` 或其他 continuation state 的 deployment affinity；
- Codex 版本升级后新增或收紧的 header、event、HTTP/SSE/WebSocket transport 行为。

### 3.3 不从 Codex 推导

- Codex 本地 `auth.json` 不构成 OpenBridge 可复用的上游 credential contract；
- Codex 能消费某个 fixture，不证明其他 Responses SDK 或 Provider 等价；
- Codex 内部未公开 endpoint、client identity 或 subscription OAuth 不构成稳定依赖。

### 3.4 主要一手参考

- Codex repository：https://github.com/openai/codex
- Codex configuration：https://developers.openai.com/codex/config-advanced
- Codex configuration reference（含 `supports_websockets`）：https://developers.openai.com/codex/config-reference
- Codex Provider model source：https://github.com/openai/codex/blob/main/codex-rs/model-provider-info/src/lib.rs
- Chat wire deprecation discussion：https://github.com/openai/codex/discussions/7782

作为证据运行时应记录实际 Codex commit/version、平台和配置文件；不能只写“最新版”，但也不把该记录固化为后续运行的版本要求。

## 4. Hermes Agent 契约

### 4.1 宣称 Hermes 兼容时必须验证

- `chat_completions` 与 `codex_responses` 的显式 transport/api mode；`anthropic_messages` 仅在选择 Anthropic Messages 行为时单独验证；
- 自定义 Provider 的 base URL、模型名与认证配置；
- Chat 流式文本、tool call arguments 分片、并行 tool calls 和 tool result replay；
- Responses tool loop、reasoning、usage 与 continuation；
- Provider 切换时 transport mode 不被错误继承；
- 严格 OpenAI-compatible endpoint 对未知字段的拒绝行为；
- Agent 主循环以及至少一个辅助任务路径，而不只验证单次 SDK 调用。

### 4.2 主要一手参考

- Hermes Agent repository：https://github.com/NousResearch/hermes-agent
- Adding providers：https://github.com/NousResearch/hermes-agent/blob/main/website/docs/developer-guide/adding-providers.md
- Providers overview：https://github.com/NousResearch/hermes-agent/blob/main/website/docs/integrations/providers.md
- 本仓库源码调研：[Hermes Chat/Responses 分析](../references/hermes/hermes-chat-responses-analysis.md)

## 5. 版本化 fixture corpus

每次需要保存的目标客户端运行证据采用：

```text
fixtures/clients/<client>/<version>/
  environment.md
  config/
  requests/
  upstream-responses/
  expected-client-events/
  negative-cases/
  tool-loop.md
```

每个 case 必须记录：

- 客户端版本、commit、平台和启动参数；
- OpenBridge 配置快照；
- 原始下游请求；
- 原始或脱敏上游 JSON/SSE byte stream；
- 客户端实际观察到的事件；
- 预期 terminal outcome；
- 该实验**证明什么**；
- 该实验**不证明什么**。

## 6. 最小测试矩阵

### Codex

1. 非流式文本；
2. 流式文本；
3. 单 function call；
4. 并行 function calls；
5. arguments delta 跨多个 SSE event；
6. tool output 回传；
7. 多轮 continuation；
8. reasoning item；
9. usage；
10. incomplete/failed/error；
11. client cancel；
12. unknown event/新增字段；
13. custom Provider 诊断确认 `supports_websockets = false`，且没有隐式 WebSocket 尝试。

### Hermes

1. Chat 非流式和流式；
2. Chat tool call arguments 分片；
3. 并行 tool calls 与 tool result replay；
4. Responses mode 的同等 tool loop；
5. reasoning 与 usage；
6. usage-only final chunk；
7. HTTP 200 内嵌 stream error；
8. strict endpoint 拒绝 source-specific 字段；
9. Provider/transport 切换；
10. 一个辅助模型/辅助任务路径。

## 7. 证据解释

本文保持 `Working hypothesis`。每次客户端行为验证应记录实际 SDK/Codex CLI 版本、安装来源、平台和运行日期；Codex fixture 记录 custom Provider id、HTTP/SSE profile 与 `supports_websockets = false`。如宣称 Hermes 兼容，也记录其实际运行环境。

可按当前行为选择以下观察，而不把它们组合成全局接受门：

- 原生路径的文本、tool loop、成功、取消、错误、EOF 与 partial stream fixture；
- bridge 路径拒绝不可表达语义的负面样本；
- 目标客户端升级后的重新运行命令和脱敏结果。
