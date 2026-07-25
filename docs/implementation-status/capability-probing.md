# 上游模型发现与能力探测

## 状态与边界

本文描述当前已实现的管理员显式 probe。它是配置与未来扩展的**证据来源**，不是自动配置或真实 Provider 验收的替代品。

- 下游 `GET /v1/models` 的语义不变：只返回 `routes.toml` 中显式配置的 public alias，绝不转发或合并上游模型列表。
- probe 只可由本地命令显式发起；它使用选定 deployment 的固定 `base_url`、Provider adapter 和 credential reference，不接受命令行 URL、model、header 或 credential。
- probe 仅输出 JSON report，不写回 `routes.toml`、不改变运行中的 snapshot，也不自动修改 capability 标记。
- 每个非列表 probe 都会调用上游模型，可能消耗配额或触发限流；真实 Provider 运行前应取得服务所有者授权。

`routes.toml` 的 capability schema 现为 `schema_version = 2`。v1 的扁平字段（例如
`function_tools`、`response_store`）不再接受；升级时须按协议拆分为下方两张表。`bootstrap.toml`
仍为独立的 schema v1，因为其安全策略结构没有变化。

## 手工维护的模型限制

为 deployment 选择一个真实上游模型后，服务所有者可在 `routes.toml` 中维护已核实的 token 上限：

```toml
[[deployments]]
id = "openai-main"
upstream_model = "actual-model-id"
# 其他 deployment 字段省略

[deployments.model_limits]
context_window_tokens = 128000
max_output_tokens = 16384
```

两个字段均可省略；省略表示当前本地路由不对该维度作断言。值为 `0` 会导致配置加载失败。

`context_window_tokens` 当前仅作为已知模型元数据保存。OpenBridge 尚未集成 model-specific tokenizer，不能用 JSON 字节数安全地判断实际输入 token，因此不会伪造 context 超限检查。

`max_output_tokens` 会在请求明确携带输出上限时参与候选筛选：

- Responses 的 `max_output_tokens`；
- Chat 的 `max_completion_tokens` 或兼容字段 `max_tokens`。

若请求中多个字段同时出现，OpenBridge 以最大值比较；超出配置值的 candidate 在 egress 前被排除。未声明请求级上限时，仍由上游模型的默认行为决定实际输出。

## 原生 capability 字段

deployment capability 是服务所有者的显式、fail-closed 声明。它按端点分域，避免将
Chat Completions 的观察外推到 Responses，或反之：

```toml
[deployments.capabilities.chat_completions]
enabled = true
streaming = true
function_calling = true
parallel_tool_calls = false
image_input = false
structured_outputs = false
store = false

[deployments.capabilities.responses]
enabled = true
streaming = true
function_calling = true
parallel_tool_calls = false
image_input = false
structured_outputs = false
store = false
previous_response_id = false
background = false
```

- `function_calling` 是功能语义；实际请求使用 `tools[]`，模型返回 tool/function call。
  当前只覆盖 `type: "function"` 的 JSON-schema function tool；built-in/custom tool 尚未有独立
  capability 或 probe，因而会在上游调用前 fail-closed 拒绝。
- `parallel_tool_calls` 与请求 wire 字段同名，仅在请求同时带有非空 `tools[]` 和
  `parallel_tool_calls: true` 时被要求。模型偶然返回多个 tool calls 不会自动推断为并行能力。
- `image_input` 是语义字段；Chat 识别 `image_url` content part，Responses 识别
  `input_image` content part。
- `structured_outputs` 同时覆盖 Chat 的 `response_format`、Responses 的 `text.format`，以及两种
  function tool shape 中的 `strict: true`。
- `store` 与请求 wire 字段同名，分别在两个端点域内声明；`previous_response_id` 和 `background`
  只在 Responses 域中存在。
- 配置仍只能收窄编译期 Provider adapter 的能力上界；不能用 TOML 宣称 adapter 没有实现的功能。

## 显式 probe CLI

使用与服务进程相同的 `OPENBRIDGE_BOOTSTRAP_CONFIG`、`OPENBRIDGE_ROUTES_CONFIG` 和上游 credential 环境变量：

```powershell
cargo run --bin openbridge-probe -- --deployment openai-main --list-models

cargo run --bin openbridge-probe -- --deployment openai-main --chat --responses --function-calling
```

可选项为 `--list-models`、`--chat`、`--responses`、`--function-calling`；不传任一选择项等同
`--all`。必须显式给出已配置的 `--deployment`。标准输出为不含 secret、请求正文和上游响应正文的 JSON report。

| 探测项 | 固定上游请求 | 成功条件 |
|---|---|---|
| `list_models` | `GET /v1/models` | 返回 JSON `data[]`；报告模型 ID 和当前 `upstream_model` 是否列出。 |
| `chat` | 最小 `POST /v1/chat/completions` | 返回非空 `choices[]`。 |
| `responses` | 最小 `POST /v1/responses` | 返回 `object: "response"`。 |
| `chat_function_calling` | 强制调用一个无副作用的固定 function，再回传本地固定结果 | 初始 call 有预期名称、关联 ID 和可解析 JSON arguments，且 tool result replay 得到有效 Chat 响应。 |
| `responses_function_calling` | 同上，使用 `function_call` / `function_call_output` | call ID、名称、arguments 和 replay 均满足 Responses 形状。 |

probe report 的 `supported` 表示本次返回了有效协议形状；`unsupported` 仅表示 endpoint 明确返回 404、405 或 501。认证失败、限流、网络错误、响应超限、JSON 无效、或固定 probe 请求被 400/422 拒绝时，一律为 `unknown`，不得据此自动关闭能力。

## 当前不做的推断

- 不从 `/v1/models` 的出现推断 tool、视觉、上下文或输出能力；
- 不从一次工具调用失败推断模型不支持工具；
- 不通过递增 prompt 探测精确 context window；
- 不探测或自动标记并行工具、流式工具、视觉、audio、hosted tools、reasoning 或协议桥能力；
- 不将 probe report 自动转换为运行时 capability 配置。

这些项目可在有明确 Provider contract、固定 fixture 与真实环境授权后，作为新增 probe 项独立扩展。
