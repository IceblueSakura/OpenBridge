# Alibaba Cloud Model Studio Provider 状态

## 当前注册

- Provider family：`bailian`；
- 可信 Base URL：`https://dashscope.aliyuncs.com/compatible-mode/v1`；
- credential pool：`bailian-primary`，仅允许 API key；
- 固定注册 12 个 Target，覆盖 GLM、Qwen generation/image/audio/embedding 和 DeepSeek fallback；
- Provider 固定 `/chat/completions`、`/responses` 与 `/embeddings`；只有 `qwen3.7-max`、`qwen3.7-plus` target 注册
  Chat/Responses 双协议 Native API，其他 generation target 保持 Chat-only；
- Qwen3.7 两个 Public Model 的 Chat/Responses 都公开 `none`、`minimal`、`low`、`medium`、`high`、`xhigh`、`max`；
  Chat output 为 `PlainText`，Responses 按官方 `reasoning.summary[]` schema 为 `Summary`。Chat egress 将 `none` 映射为
  `enable_thinking=false`、其余六档映射为 `true`；Responses 原样保留 effort；
- 其他已真实确认的 `glm-5.2` 与 `bailian-deepseek-v4-pro` Chat target 保留 `PlainText`，其余 Bailian generation target
  显式收窄为 `Unknown`；
- `bailian-deepseek-v4-pro` 与 `bailian-deepseek-v4-flash` Chat target 单独公开非 strict 的 `json_object`，不把 Provider ceiling
  外推到 GLM、Qwen 或其他 Bailian target。

## 真实验证

2026-08-08 使用当前私有 credential 和真实下游用户 key 验证：

- `glm-5.2` 的 Chat/Responses × JSON/SSE × reasoning 字段省略/high 共 8 个单元全部成功；
- `qwen3.7-max` 与 `qwen3.7-plus` 的 Chat/Responses × JSON/SSE × high 共 8 个单元全部成功，终态完整且 reasoning 非空；
  当次 Responses 请求走 Responses-via-Chat，不能作为当前 Bailian Native Responses 的真实验收；
- 固定 `bailian-deepseek-v4-pro` fallback 的 direct Chat high JSON/SSE 均为 HTTP 200、终态完整且
  `reasoning_content` 非空；Public Model 的 Responses high 因此可以保留 DeepSeek/Bailian 两个 Bridge candidate；
- 2026-08-09 对 Bailian DeepSeek Pro/Flash 的 `response_format:json_object` 执行 direct Chat JSON/SSE，4/4 为 HTTP 200、终态完整、
  输出可解析且字段符合带 `json` 和字段示例的 prompt；
- 三个模型省略 reasoning 字段时会返回非空明文 `reasoning_content`，Responses Bridge 可将其保留为 reasoning item；
- 三个模型的 Chat SSE 会在 `finish_reason` 后发送 usage-only chunk；当前 Bridge 接受一个严格
  `choices: []` + `usage` object 块，随后等待 `[DONE]` 并产生唯一 `response.completed`；
- 此前标准 `reasoning_effort: "none"` 与参考 off shape 均让 GLM/Qwen 三个模型的 Chat JSON/SSE reasoning 内容为空；
  当前 high 实现与复测不加载或调用 Hermes，也不发送 Hermes custom 字段。
- `qwen3.7-text-embedding` 默认维度与官方七个显式维度共 8 个成功请求均返回结构正确的 HTTP 200；OpenBridge 接受并丢弃
  Bailian 顶层 `id`，保持 Public Model 投影。旧目录中的 `64/128` 已移除并在 egress 前精确拒绝，完整维度矩阵 10/10 通过。

最终矩阵和剩余错误边界见 [`real-e2e-test-2026-08-08.md`](../real-e2e-test-2026-08-08.md)。

## 证据边界

`tests/example_config.rs` 与 `tests/provider_contract.rs` 验证 Qwen3.7 统一七档、双协议 Native Route、Chat `PlainText`、Responses
`Summary`、Chat switch 与 Responses effort 原值；`tests/bridge_conversion_contract.rs` 与 `tests/bridge_forwarding_contract.rs`
验证既有 usage-only SSE lifecycle。

本轮没有真实复测 Qwen3.7 Native Responses 或 `minimal/low/medium/xhigh/max`，也没有把已实测 DeepSeek JSON Output 或四个
generation Target 的 reasoning 事实外推到其他 Bailian Target。确定性测试不证明其他账号、区域、未来 Provider 行为、外部 SDK、
负载或长期运行兼容性。
