# 目标客户端契约：Codex 与 Hermes Agent

## 状态

**Working hypothesis；待固定客户端版本和真实 fixture 后接受。**

本文定义 OpenBridge 首批下游客户端的优先级和验证方式。它不把客户端当前实现细节永久固化为 OpenBridge 公共协议；每次升级目标版本时必须重跑兼容 corpus。

## 1. 结论

OpenBridge 应提供两个一等下游入口，但优先级不对称：

- **Codex：Responses-first，首版 HTTP/SSE-first。** 面向 Codex 的核心契约是 `/v1/responses`。P0 使用独立 custom Provider id，并显式配置 `supports_websockets = false`；不以 Chat Completions 或 Responses WebSocket 作为首版兼容前提。
- **Hermes Agent：多 transport。** Hermes 的常见 Provider 使用 Chat Completions，同时存在 `codex_responses`、`anthropic_messages` 等模式；OpenBridge 应同时验证 Hermes Chat 与 Responses 路径。

因此不采用“所有请求先归一成一个协议”的入口设计。原生协议应直接进入对应 native path，只有上下游协议不一致时才进入 Protocol Bridge。

## 2. 目标兼容路径

| 客户端 | 下游协议 | 上游协议 | 处理路径 | 优先级 |
|---|---|---|---|---|
| Codex | Responses over HTTP/SSE | Responses over HTTP/SSE | Native | P0 |
| Hermes | Chat Completions | Chat Completions | Native | P0 |
| Hermes | Responses | Responses | Native | P1 |
| Codex | Responses over HTTP/SSE | Chat Completions | Protocol Bridge | P1 |
| Hermes | Chat Completions | Responses | Protocol Bridge | P1 |
| Hermes | Chat/Responses | Anthropic Messages | Protocol Bridge | P2，用于检验异构抽象 |

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

实现前应记录固定的 Codex commit/version、平台和配置文件；不能只写“最新版”。

## 4. Hermes Agent 契约

### 4.1 必须验证

- `chat_completions`、`codex_responses` 与 `anthropic_messages` 的显式 transport/api mode；
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
- 本仓库源码调研：[Hermes Chat/Responses 分析](../research/hermes/chat-responses-analysis.md)

## 5. 版本化 fixture corpus

每个目标客户端版本至少保留：

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

## 7. 接受门

目标客户端契约在满足以下条件前保持 `Working hypothesis`：

- 已固定至少一个 Codex 版本和一个 Hermes 版本；Codex fixture 固定 custom Provider id、HTTP/SSE profile 与 `supports_websockets = false`；
- 两个 P0 native path 完成完整 Agent tool loop；
- 成功、取消、错误、EOF 和 partial stream 均有 fixture；
- 对 bridge 路径至少有一个真实负面样本证明 capability gate 能拒绝不可表达语义；
- 客户端升级流程和重新验证命令已记录。
