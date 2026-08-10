# 当前开发焦点：扩展 OpenBridge 能力契约以兼容 hermes 客户端

## 状态

**待审查，未获准实施。** 本文档记录扩展 OpenBridge 公共模型能力契约所需的修改点，用户审查确认后再开始实现。

## 背景与证据

hermes CLI 通过 `obr`（Responses）与 `obc`（Chat Completions）调用 `glm-5.2` / `deepseek-v4-flash` 均返回 HTTP 400。curl 直测（不带客户端特有参数）两个模型 × 双协议全部正常（含工具调用、流式），证明服务修复（`bf415be` 工具调用）本身有效，失败根因是 **hermes 发送了 OpenBridge 能力契约之外的参数**。

已通过 `9327ce9` 新增的 `[logging]` 内容日志与逐参数 curl 探测定位 5 个被拒参数：

| # | 被拒参数 | 触发路径 | 错误信息 | 原因 |
|---|---|---|---|---|
| 1 | `include: ["reasoning.encrypted_content"]` | obr（glm-5.2、deepseek-v4-flash） | `unsupported_model_capability` | bridge 模型 `response_includes` 为空，聚合时 `include` 参数被移除 |
| 2 | `reasoning: {effort: "medium"}` | obr（glm-5.2、deepseek-v4-flash） | `unsupported_model_capability` | glm-5.2 仅声明 XHigh/High/None；flash 仅 none/low/high/max，均无 Medium |
| 3 | `parallel_tool_calls: true` | obr（glm-5.2、deepseek-v4-flash） | `unsupported_model_capability` | bailian 契约 `parallel_calls: false`（bf415be 保守设置） |
| 4 | `stream_options: {include_usage: true}` | obc（所有 chat 模型） | `unsupported_model_capability` | Chat 契约未声明 `stream_options`，OpenAI SDK 2.24.0 默认发送 |
| 5 | `response_format: {type: "json_schema"}` | obc（hermes 内部 session-title 请求，model=deepseek-v4-flash） | `unsupported_model_capability` | deepseek-v4-flash Chat 仅声明 `JsonObject`，无 `JsonSchema` |

## 目标可观察行为

hermes 通过 `obr` / `obc` 使用 `glm-5.2` 与 `deepseek-v4-flash` 时，普通对话与工具调用请求不因上述参数被 400 拒绝；`/v1/models` 公开接口反映扩展后的能力。

## 修改点

### M1. reasoning 级别：glm-5.2 增加 Medium

- **文件**：`src/models/z_ai/glm_5_2.rs`
- **现状**：`reasoning: ReasoningProfile::supported([XHigh, High, None])`
- **目标**：加入 `ReasoningLevel::Medium`
- **待确认事实**：z-ai/glm-5.2 上游（bailian）是否真支持 medium effort；若支持则修改，否则将 hermes 默认 effort 调整为声明内的级别

### M2. reasoning 级别：deepseek-v4-flash 增加 Medium

- **文件**：`src/models/z_ai/deepseek_v4_flash.rs`（或等价模型定义文件）
- **现状**：公开 none/low/high/max（README 记录）
- **目标**：加入 `ReasoningLevel::Medium`
- **待确认事实**：deepseek 上游是否接受 `reasoning_effort: medium`；若支持则修改，否则保持现状并在 hermes 侧调整默认 effort

### M3. response_includes：支持 `reasoning.encrypted_content`

- **文件**：`src/providers/bailian/definition.rs`（Responses 契约 `include` 字段）、`src/registry/public_model/compiler/contribution.rs`（bridge 模型的 response_includes 聚合）、`src/bridge/conversion/request/responses_to_chat.rs`（桥接时 include 的降级处理）
- **现状**：bridge 模型 `response_includes` 为空 → 聚合移除 `include` 参数 → 带 include 的请求 400
- **目标**：接受 `include: ["reasoning.encrypted_content"]`。注意 bridge 模型（bailian 走 Chat→Responses bridge）无法真正加密内容，需确定降级策略：接受请求但返回明文 reasoning（`reasoning_text`），或拒绝 `encrypted_content` 但接受空 include/其他合法值
- **待确认事实**：hermes 客户端在何种情况下发送该 include；降级为明文 reasoning 是否可接受

### M4. parallel_tool_calls：bailian 声明 `parallel_calls: true`

- **文件**：`src/providers/bailian/definition.rs`
- **现状**：`FUNCTION_TOOLS.parallel_calls = false`（bf415be 保守设置）
- **目标**：改为 `true`，前提是 bailian 上游（百炼 OpenAI 兼容模式）接受 `parallel_tool_calls: true`
- **待确认事实**：上游实测接受度；若实测拒绝则保持 false，并在 hermes 侧禁用该参数

### M5. Chat 契约支持 `stream_options`

- **文件**：`src/providers/bailian/definition.rs`、`src/providers/deepseek/definition.rs`、`src/providers/openrouter/definition.rs`（所有 hermes 会触达的 Chat 契约）
- **现状**：`stream_options` 在 `src/core/generation_parameter.rs:228` 有定义但契约未声明支持，OpenAI SDK 2.24.0 默认携带 `stream_options: {include_usage: true}` → 所有 obc chat 请求 400
- **目标**：Chat capabilities 声明支持 `stream_options`（需确认透传语义：OpenBridge 应透传 `include_usage` 或忽略）
- **待确认事实**：上游 chat 端点对 `stream_options` 的接受度；OpenBridge 转发时是否原样透传

### M6. deepseek-v4-flash（及受影响 Chat 模型）支持 `json_schema`

- **文件**：`src/providers/deepseek/definition.rs`（`structured_outputs` 由 `JsonObject` 扩为 `JsonObjectAndJsonSchema`）或按模型 gating
- **现状**：deepseek-v4-flash Chat 仅 `JsonObject`；hermes 内部 session-title 请求用 `json_schema` strict:true → 400
- **目标**：声明支持 `json_schema`（至少非严格；strict 取决于上游）
- **待确认事实**：上游对 `response_format: json_schema` 的接受度；hermes 该内部请求是否可绕开（如改用其他模型做标题）

## 失败测试（验证手段）

每项修改后用以下方式验证：

1. `cargo test --tests`（跳过已知预存在失败的 `configuration::checked_in_examples_compile_into_a_closed_runtime_registry`，其失败源自 telemetry 配置与模板快照不一致，已在 bf415be 处理模板后恢复通过——实际当前已通过）
2. curl 逐参数回归：`include` / `reasoning effort=medium` / `parallel_tool_calls` / `stream_options` / `json_schema` 单独与组合测试
3. hermes CLI 实机回归：
   - `hermes -z "Reply with exactly: PONG" -m glm-5.2 --provider obr --cli`
   - `hermes -z "Reply with exactly: PONG" -m deepseek-v4-flash --provider obr --cli`
   - 同模型走 `--provider obc`
   - 工具调用回归（`-z` 触发工具或对话确认）
4. `/v1/models` 接口确认公开能力与契约一致

## 非目标

- 不修改 hermes 客户端默认参数（本次聚焦 OpenBridge 契约扩展；hermes 侧调整仅作为 M1/M2/M4 中上游不支持时的备选）
- 不实现 `encrypted_content` 的真实加密（bridge 模型不承诺；仅确定接受或降级策略）
- 不涉及 OpenAI API 之外的协议、多租户、公网部署
- 不重构能力聚合机制本身（`SupportState::intersection` 语义保持不变）

## 验证范围

- 修改文件：上列 `src/` 文件 + 相应 `tests/` 契约测试（`tests/capability_definition_contract.rs`、`tests/example_config/providers.rs`、`tests/provider_boundary_contract.rs` 等已有契约需同步更新）
- 回归范围：`cargo test` 全量 + hermes CLI 实机 4 模型×2 协议 + curl 逐参数
- 明确不验证：上游真实计费、并发压力、公网安全边界

## 待用户确认

1. M1/M2：glm-5.2 与 deepseek-v4-flash 上游是否真支持 `reasoning_effort: medium`？
2. M3：`reasoning.encrypted_content` 在 bridge 下降级为明文 reasoning 是否可接受？
3. M4：bailian 上游是否接受 `parallel_tool_calls: true`？
4. M6：是否一并处理 hermes 内部 session-title 的 `json_schema` 请求（而非绕开）？
