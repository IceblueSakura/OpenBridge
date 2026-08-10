# 当前开发焦点：Hermes Chat `json_schema`

## 状态

**M4 待审查，尚未获准实施。** M1-M3 已完成并转入
[实施现状](../implementation-status/features/models-api-and-capability-preflight.md)；本文档只保留尚未实施的 M4。Reasoning level 映射已经实现，
不属于本焦点，也不在此恢复旧逻辑。

## 背景与已确认现状

Hermes 通过 `obc`（Chat Completions）发起内部 session-title 请求时携带：

```json
{"response_format":{"type":"json_schema","json_schema":{"name":"session_title","strict":true,"schema":{}}}}
```

`deepseek-v4-flash` 与 `mimo-v2.5` 的固定 Chat interface 当前只公开非 strict 的 `json_object`，因此该请求在 Provider egress 前返回
`unsupported_model_capability`。MiMo 交叉复现中，普通 Chat、stream、工具调用和 data URL 图片请求均已通过；该剩余拒绝不能归因于
reasoning level、`reasoning.encrypted_content`、`parallel_tool_calls` 或 `stream_options`。

## 需求与目标可观察行为

- Public Model 只能公开其固定 Route 完整候选交集能够执行的 structured-output 能力；未知或证据不完整的组合继续 fail closed。
- 只有在完整候选集的非 strict/strict 语义证据成立且 M4 单独获准后，目标 Chat interface 才可公开 `json_schema` 与相应 strictness。
- 响应不仅必须是合法 JSON，还必须满足对外承诺的 schema 约束，包括 required、类型、枚举和禁止额外字段等受测规则。
- `/openbridge/v1/models` 必须反映完整候选交集编译出的 structured-output 契约；标准 `/v1/models` 继续只公开模型身份。
- 不得为了接收某个参数在请求时过滤、重排或跳过固定候选。

## 当前能力边界

- `deepseek-v4-flash` 的 Chat candidate 覆盖 DeepSeek、Bailian 与 OpenRouter；`mimo-v2.5` 使用 MiMo。只有每个 Public Model 的完整
  candidate 集都能执行相同 schema 模式与 strictness，Models 才能公开对应能力。
- Provider family 上的宽泛声明可能扩大同 family 的未验证模型；必要时必须在 registration 层按 Target 收窄。
- HTTP 200 或仅返回可解析 JSON 都不足以证明 strict schema 语义。

## 实施前必须补齐的证据

对 DeepSeek、Bailian、OpenRouter 与 MiMo 分别验证：

1. 非 strict `json_schema` 请求是否被接受；
2. `strict:true` 请求是否被接受；
3. 返回内容是否为合法 JSON；
4. 返回对象是否实际满足所提交 schema，包括 required、类型、枚举和禁止额外字段等受测约束。

若任一固定 candidate 不支持目标形状，则保持当前 fail-closed 契约，或由 Hermes 绕开该内部标题请求；不得在请求时按参数过滤或重排
candidate。

## MiMo 上游 `json_schema` 实测

2026-08-10 直连 `api.xiaomimimo.com/v1/chat/completions`，模型为 `mimo-v2.5`：

| 测试 | 请求 `response_format` | 结果 | 结论 |
|---|---|---|---|
| `json_object` 对照 | `{"type":"json_object"}` | HTTP 200，返回合法 JSON `{"title":"MiMo-v2.5"}`，无额外字段 | 基线正常 |
| `json_schema` 非 strict | `{"type":"json_schema","json_schema":{"name":"session_title","strict":false,"schema":{...title...}}}` | HTTP 200，返回合法 JSON `{"title":"MiMo-v2.5"}`，满足 required、类型和禁止额外字段 | **接受** |
| `json_schema strict:true`（title） | 同上但 `strict:true` | HTTP 200（2/2），均返回合法 JSON并满足约束 | **接受** |
| `json_schema strict:true`（enum） | `{"name":"sentiment","strict":true,"schema":{...senti enum POSITIVE/NEGATIVE/NEUTRAL...}}` | HTTP 200 但不满足约束：返回字段名 `sentiment`（schema 要求 `senti`）、枚举值 `POS`（要求 `POSITIVE`），且输出截断为 `finish=abort` | **strict 语义执行不可靠** |
| `json_schema strict:true`（title 内容语义） | title schema | 字段名、类型和额外字段均满足，但内容可为长解释句（模型把 title 当说明字段） | 不违反 schema 约束 |

结论：

1. 参数层面，MiMo V2.5 接受非 strict 与 `strict:true` 的 `json_schema`；当前 OpenBridge 的 `JsonObject` 契约属于保守收窄。
2. 语义层面，简单 string + required + `additionalProperties:false` schema 能稳定满足，但 enum 与精确字段名约束并不可靠，且可能
   `finish=abort`。
3. MiMo V2.5 不能作为“`strict:true` 语义可靠”的证据。若 Hermes session-title 的 `strict:true` 是验收条件，MiMo 会收窄完整交集，
   对应 Public Model 不应公开 strict schema；只能在 Hermes 可接受时考虑非 strict，或继续 fail closed。
4. 该实测只覆盖 MiMo；DeepSeek、Bailian、OpenRouter 尚未完成同等验证，不能从 MiMo 结果外推。

## 获准后的候选修改面

- `src/providers/deepseek/definition.rs`
- `src/providers/bailian/definition.rs`
- `src/providers/openrouter/definition.rs`
- `src/providers/mimo/definition.rs`
- 必要的 per-model registration 收窄、Public Model 聚合、Models 投影、预检与 exact forwarding 测试

实际实现范围必须由证据决定：若完整候选集只支持非 strict schema，则只可公开 non-strict；只有完整候选集都满足 strict 语义时才可公开
`strict:true`。

## 失败优先测试

M4 获准后，先增加能够复现当前 `unsupported_model_capability` 的测试，再做最小实现：

| 验证层 | 重点 |
|---|---|
| `tests/capability_definition_contract.rs`、`tests/provider_boundary_contract.rs` | Provider ceiling 与精确 Target 的 structured-output profile，不扩大相邻模型 |
| `tests/example_config/providers.rs`、`tests/forwarding_contract/models.rs` | 完整 Public Model 候选交集与 `/openbridge/v1/models` 投影 |
| `tests/native_routing_contract.rs` | 参数分析、预检、固定候选顺序以及 unsupported candidate 的 fail-closed 行为 |
| `tests/forwarding_contract.rs` | Native JSON/SSE exact forwarding 和既有 fallback 顺序 |
| `tests/bridge_conversion_contract.rs`、`tests/bridge_forwarding_contract.rs` | structured-output 转换只遵守已公开的交集 |

聚焦测试通过后运行 Rust 基线：

```powershell
cargo fmt -- --check
cargo test --locked
cargo clippy --locked -- -D warnings
git diff --check
```

## 实现后的真实验收

确定性测试通过后仍需：

1. 用独立 curl 对 DeepSeek、Bailian、OpenRouter 与 MiMo 的完整固定 candidate 逐一复测获准的 schema 形状；
2. 用 Hermes `obc` 复测实际 session-title 路径；
3. 查询 `/openbridge/v1/models`，确认公开契约与完整候选交集一致；
4. 回归 MiMo 普通 Chat、stream、工具调用与图片输入。

## 非目标

- 不重新修改已完成的 M1-M3，也不恢复 reasoning level 映射逻辑。
- 不修改 Hermes 默认参数、重试或 fallback 策略。
- 不把 HTTP 200、单次成功或某个 Provider family 的结果推断为其他模型的能力。
- 不实现请求时动态能力路由、candidate 过滤、重排或隐式降级。
- 不重构 Public Model 聚合、Provider 重试/冷却、credential 或配置体系。
- 不涉及多租户、公网部署、计费确认、负载、长期运行或生产可用性承诺。

## 验证边界

- 2026-08-10 的直连结果只证明当时被测账户、端点、网络和请求形状；实现后必须重新执行真实 Provider 探测。
- Rust 确定性测试只能证明本地 capability、交集、预检、Bridge 和 wire 行为，不能替代真实 Provider、当前 SDK 或 Hermes runtime。
- HTTP 200 与 JSON 可解析不证明 schema 或 strict 语义；必须检查返回对象是否满足受测约束。
- 未运行的真实 Provider、Hermes、外部 SDK、强制 fallback、负载和长期测试不得写成已验收。

## 待用户确认

1. 是否批准实施 M4？
2. Hermes 的 `strict:true` 语义是否为必须满足的验收条件；若是，当前 MiMo 证据意味着对应 Public Model 必须继续 fail closed。
