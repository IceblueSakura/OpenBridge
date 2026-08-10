# 当前开发焦点：Hermes 剩余 Chat 兼容边界

## 状态

**待审查，未获准实施。** 2026-08-10 完成的 M1/M2 已由提交 `64b15b5` 落地，并转入
[实施现状](../implementation-status/features/models-api-and-capability-preflight.md)：目标 Responses interface 已按精确候选接受
`reasoning.encrypted_content`，目标 Chat/Responses interface 已按精确候选开放 `parallel_tool_calls:true`。

本文档只恢复尚未实施的 M3/M4。恢复计划不等于授权修改实现；若继续开发，应先确认 M3，完成并关闭后再单独确认 M4。Reasoning level
映射已经实现，不属于本焦点，也不在此恢复旧逻辑。

## 背景与已确认现状

Hermes 通过 `obc`（Chat Completions）调用 OpenBridge 时仍会触发两个独立的 fail-closed 边界：

| 阶段 | 请求字段 | 当前结果 | 已确认原因 |
|---|---|---|---|
| M3 | `stream_options: {"include_usage": true}` | `unsupported_model_capability` | 字段虽在 generation 参数目录中，但目标 Chat interface 尚未把它编译进 `supported_parameters` |
| M4 | `response_format: {"type":"json_schema", ...}` | `unsupported_model_capability` | Hermes 内部 session-title 请求携带 `strict:true`；`deepseek-v4-flash` 与 `mimo-v2.5` 的完整 Chat 候选交集目前只公开 `json_object` |

MiMo 交叉复现中，普通 Chat、stream、工具调用和 data URL 图片请求均已通过；剩余拒绝分别是 M3 的 `stream_options` 和 M4 的
`json_schema`，因此二者不能再归因于 reasoning level、`reasoning.encrypted_content` 或 `parallel_tool_calls`。

## 需求与目标可观察行为

- Public Model 只能公开其固定 Route 完整候选交集能够执行的能力；未知或证据不完整的组合继续 fail closed。
- M3 获准并完成后，Hermes 的目标 Native Chat 请求应接受并原样转发 `stream_options.include_usage:true`，流尾 usage chunk 不得被
  OpenBridge 丢弃；Responses interface 不公开该 Chat-only 字段。
- M4 只有在完整候选集的非 strict/strict 语义证据成立且另行获准后，目标 Chat interface 才可公开 `json_schema` 与相应 strictness；
  响应不仅要是合法 JSON，还必须满足已承诺的 schema 约束。
- `/openbridge/v1/models` 必须反映完整候选交集编译出的参数和 structured-output 契约；标准 `/v1/models` 继续只公开模型身份。
- 不得为了接收某个参数在请求时过滤、重排或跳过固定候选。

## M3. Chat `stream_options.include_usage`

### 协议与当前边界

- `stream_options` 属于 Chat Completions。Responses 自身在响应对象和 terminal event 中携带 usage，不声明也不透传该字段。
- `src/core/generation_parameter.rs` 当前把 `stream_options` 登记在 `BOTH` 识别空间，但目标 Chat interface 没有把它编译进
  `supported_parameters`，所以 Hermes 的 `obc` 请求在 Provider egress 前被预检拒绝。
- 现有 `bridge_sources: NEITHER` 边界保持不变；Chat→Responses Bridge 不转发这个 Chat-only 字段。

### 候选范围与已有证据

目标 Public Model 的完整 Chat 候选集为：

- `glm-5.2`：Bailian 1 个 Native Chat candidate；
- `deepseek-v4-flash`：DeepSeek、Bailian、OpenRouter 3 个 Native Chat candidates；
- `mimo-v2.5`：MiMo 1 个 Native Chat candidate。

2026-08-10 的直连流式 Chat 探测中，Bailian、DeepSeek、OpenRouter、MiMo、NVIDIA、Kimi 与 LongCat 均返回 HTTP 200，且流尾存在 usage
chunk；ChatGPT 由用户确认，OpenAI 因无活跃凭据未探测。前四项覆盖本阶段三个目标 Public Model 的当前固定候选集，但这些结果只针对
当时的账户、端点、网络和请求形状，不自动扩展到其他模型、Provider 或未来版本，也不证明 usage 数值或计费准确。

### 获准后的最小实现范围

- 将协议字段所有权收窄为 Chat-only，并只在上述已验证 Target 的 Chat capability 中声明支持，避免扩大同一 Provider family 的其他模型。
- 通过现有类型化参数所有权让完整候选交集公开 `stream_options`；同步更新 Models 投影、参数分析、Native JSON/SSE exact forwarding 和
  usage 尾块测试。
- Native Chat 原样转发 `stream_options: {"include_usage": true}`；不在 OpenBridge 内生成、修正或推算 usage。
- 保持 Bridge 不接收/不转发该字段，不为 Responses 建立兼容别名。

### 客户端影响

Hermes 当前在 M3 被拒后会回退到非流式重试。完成 M3 可以消除一次必然失败的请求与额外延迟，但只有目标 Hermes + 真实 Provider
复测才能证明该客户端路径闭环。

## M4. Chat `json_schema`

### 当前边界

- `deepseek-v4-flash` 与 `mimo-v2.5` 的 Chat interface 当前只公开 `json_object`；Hermes session-title 请求使用 `json_schema` 且携带
  `strict:true`。
- `deepseek-v4-flash` 的 Chat candidate 覆盖 DeepSeek、Bailian 与 OpenRouter；`mimo-v2.5` 使用 MiMo。只有每个 Public Model 的完整
  candidate 集都能执行相同 schema 模式与 strictness，Models 才能公开对应能力。
- Provider family 上的宽泛声明可能误伤同 family 的未验证模型；必要时必须在 registration 层按 Target 收窄。

### 实施前必须补齐的证据

对 DeepSeek、Bailian、OpenRouter 与 MiMo 分别验证：

1. 非 strict `json_schema` 请求是否被接受；
2. `strict:true` 请求是否被接受；
3. 返回内容是否为合法 JSON；
4. 返回对象是否实际满足所提交 schema，包括 required、类型、枚举和禁止额外字段等受测约束。

HTTP 200 或仅返回可解析 JSON 都不足以证明 strict schema 语义。若任一固定 candidate 不支持目标形状，则保持当前 fail-closed 契约，或由
Hermes 绕开该内部标题请求；不得在请求时按参数过滤或重排 candidate。

### 获准后的候选修改面

- `src/providers/deepseek/definition.rs`
- `src/providers/bailian/definition.rs`
- `src/providers/openrouter/definition.rs`
- `src/providers/mimo/definition.rs`
- 必要的 per-model registration 收窄、Public Model 聚合、Models 投影、预检与 exact forwarding 测试

M4 的实际范围必须由证据决定：若完整候选集只支持非 strict schema，则只可公开 non-strict；只有完整候选集都满足 strict 语义时才可公开
`strict:true`。

## 失败优先测试与实施顺序

M3 与 M4 不并行实施：先为获准阶段增加能复现当前 `unsupported_model_capability` 的失败测试，再做最小实现；M3 完成、记录实施现状并关闭后，
M4 才能在证据完整且单独获准时进入实现。

| 验证层 | 重点 |
|---|---|
| `tests/capability_definition_contract.rs`、`tests/provider_boundary_contract.rs` | Provider ceiling 与精确 Target 的 stream/structured-output profile，不扩大相邻模型 |
| `tests/example_config/providers.rs`、`tests/forwarding_contract/models.rs` | 完整 Public Model 候选交集与 `/openbridge/v1/models` 投影 |
| `tests/native_routing_contract.rs` | 参数分析、预检、固定候选顺序以及 unsupported candidate 的 fail-closed 行为 |
| `tests/forwarding_contract.rs` | Native JSON/SSE exact forwarding、usage 尾块和既有 fallback 顺序 |
| `tests/bridge_conversion_contract.rs`、`tests/bridge_forwarding_contract.rs` | Chat-only 参数不跨 Bridge 泄漏，structured-output 转换只遵守已公开的交集 |

每个获准阶段先运行对应聚焦测试，再运行 Rust 基线：

```powershell
cargo fmt -- --check
cargo test --locked
cargo clippy --locked -- -D warnings
git diff --check
```

## 实现后的真实验收

确定性测试通过后仍需按阶段执行：

1. 用独立 curl 对 GLM 5.2、DeepSeek Flash 和 MiMo V2.5 的目标 Chat candidate 逐一复测对应字段；
2. 用 Hermes `obc` 对三个模型复测普通对话、stream 与工具调用；M4 另测实际 session-title 路径；
3. 查询 `/openbridge/v1/models`，确认公开契约与完整候选交集一致；
4. 回归 MiMo 图片输入；音频/视频仍只保留各自既有的确定性验证边界。

## 非目标

- 不重新修改已完成的 M1/M2，也不恢复 reasoning level 映射逻辑。
- 不修改 Hermes 默认参数、重试或 fallback 策略。
- 不把 HTTP 200、单次成功或某个 Provider family 的结果推断为其他模型的能力。
- 不实现请求时动态能力路由、candidate 过滤、重排或隐式降级。
- 不重构 Public Model 聚合、Provider 重试/冷却、credential 或配置体系。
- 不涉及多租户、公网部署、计费确认、负载、长期运行或生产可用性承诺。

## 验证边界

- 2026-08-10 的直连结果只证明当时被测账户、端点、网络和请求形状；实现后必须重新执行真实 Provider 探测。
- Rust 确定性测试只能证明本地 capability、交集、预检、Bridge 和 wire 行为，不能替代真实 Provider、当前 SDK 或 Hermes runtime。
- M3 中存在 usage 尾块不证明 token 数值、缓存明细或上游计费准确。
- M4 中 HTTP 200 与 JSON 可解析不证明 schema 或 strict 语义；必须检查返回对象是否满足受测约束。
- 未运行的真实 Provider、Hermes、外部 SDK、强制 fallback、负载和长期测试不得写成已验收。

## 待用户确认

1. 是否批准先实施 M3，并以三个目标 Public Model 的当前完整 Chat 候选集为唯一范围？
2. M3 完成后，是否将 M4 作为新的单一焦点；若继续，Hermes 的 `strict:true` 语义是否为必须满足的验收条件？
