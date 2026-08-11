# Alibaba Cloud Model Studio Provider 状态

## 当前注册

- Provider family：`bailian`；
- 可信 Base URL：`https://dashscope.aliyuncs.com/compatible-mode/v1`；
- credential pool：`bailian-primary`，仅允许 API key；
- 固定注册 11 个 Target，覆盖 GLM、Qwen generation/image/embedding 和 DeepSeek fallback；
- Provider 固定 `/chat/completions`、`/responses` 与 `/embeddings`；`qwen3.8-max`、`qwen3.7-max`、`qwen3.7-plus` target 注册
  Chat/Responses 双协议 Native API，其他 generation target 保持 Chat-only；
- 三个 Qwen Public Model 的 Chat/Responses 都公开 `none`、`minimal`、`low`、`medium`、`high`、`xhigh`、`max`；
  Chat output 为 `PlainText`，Responses 按官方 `reasoning.summary[]` schema 为 `Summary`。Chat egress 将 `none` 映射为
  `enable_thinking=false`、其余六档映射为 `true`；Responses 原样保留 effort；
- `qwen3.6-27b` Public Model 只使用固定 Bailian Chat API：Chat 为 Native，Responses 通过 Chat Bridge；两个接口只公开
  `none/high`，Chat egress 分别映射为 `enable_thinking=false/true`，reasoning output 为 `PlainText`；
- `glm-5.2` Public Model 的 Chat/Responses 公开 `none`、`high`、`xhigh`，并在 Chat egress 保留标准
  `reasoning_effort`；
- DeepSeek V4 Pro/Flash 的公共接口分别公开 `none/high/max` 与 `none/low/high/max`。两个 Bailian DeepSeek Chat target
  只把 `none` 转为 `enable_thinking=false`，其他 effort 原样保留；
- 其他已真实确认的 `glm-5.2` 与 Bailian DeepSeek Chat target 保留 `PlainText`，其余 Bailian generation target
  显式收窄为 `Unknown`；
- `bailian-deepseek-v4-pro` 与 `bailian-deepseek-v4-flash` Chat target 单独公开非 strict 的 `json_object`，不把 Provider ceiling
  外推到 GLM、Qwen 或其他 Bailian target。
- `bailian-glm-5-2` 与 `bailian-deepseek-v4-flash` Chat target 接受并转发 `parallel_tool_calls:true`；其他 Bailian generation target
  仍在 registration 层收窄为 unsupported。GLM 没有上游 Responses endpoint，OpenBridge 的 Responses Bridge 只消费
  `reasoning.encrypted_content` include 提示并保留真实明文 reasoning，不向 Chat wire 泄漏 `include` 或伪造 opaque item。
- `qwen/qwen-audio-3.0-asr-flash` 只保留为 canonical `SpeechRecognition` Model；当前没有 Bailian executable Target、Route 或
  Public Model，不计入上述 11 个 Target，也不属于运行时可调用模型。

## 真实验证

2026-08-08 使用当前私有 credential 和真实下游用户 key 验证：

- `glm-5.2` 的 Chat/Responses × JSON/SSE × reasoning 字段省略/high 共 8 个单元全部成功；
- `qwen3.7-max` 与 `qwen3.7-plus` 的 Chat/Responses × JSON/SSE × high 共 8 个单元全部成功，终态完整且 reasoning 非空；
  当次 Responses 请求走 Responses-via-Chat，不能作为当前 Bailian Native Responses 的真实验收；
- 固定 `bailian-deepseek-v4-pro` fallback 的 direct Chat high JSON/SSE 均为 HTTP 200、终态完整且
  `reasoning_content` 非空；Public Model 的 Responses high 因此可以保留 DeepSeek/Bailian 两个 Bridge candidate；
- Bailian DeepSeek Pro/Flash 的 direct Chat off probe 证明原样 `reasoning_effort:none` 不是可用关闭形状，而
  `enable_thinking:false` 可以成功关闭 reasoning；当前 adapter 按此证据只转换 `none`；
- 2026-08-09 对 Bailian DeepSeek Pro/Flash 的 `response_format:json_object` 执行 direct Chat JSON/SSE，4/4 为 HTTP 200、终态完整、
  输出可解析且字段符合带 `json` 和字段示例的 prompt；
- 三个模型省略 reasoning 字段时会返回非空明文 `reasoning_content`，Responses Bridge 可将其保留为 reasoning item；
- 三个模型的 Chat SSE 会在 `finish_reason` 后发送 usage-only chunk；当前 Bridge 接受一个严格
  `choices: []` + `usage` object 块，随后等待 `[DONE]` 并产生唯一 `response.completed`；
- 此前标准 `reasoning_effort: "none"` 与参考 off shape 均让 GLM/Qwen 三个模型的 Chat JSON/SSE reasoning 内容为空；
  当前 high 实现与复测不加载或调用 Hermes，也不发送 Hermes custom 字段。
- `qwen3.7-text-embedding` 默认维度与官方七个显式维度共 8 个成功请求均返回结构正确的 HTTP 200；OpenBridge 接受并丢弃
  Bailian 顶层 `id`，保持 Public Model 投影。旧目录中的 `64/128` 已移除并在 egress 前精确拒绝，完整维度矩阵 10/10 通过。
- 2026-08-09 对 `bailian-qwen3-8-max` 执行 Models/Chat/Responses probe：三项均为 HTTP 200，远端 Models 列表包含
  `qwen3.8-max`；随后使用真实下游用户 key 运行 Chat/Responses × JSON/SSE × none/high，8/8 HTTP 200、终态完整且文本非空，
  none 的 reasoning 为空、high 的 reasoning 非空；
- Qwen3.8 Responses 的 `minimal/low/medium/xhigh/max` 额外非流式请求 5/5 HTTP 200、终态完整且 reasoning 非空；结合上一矩阵的
  none/high，当前北京 endpoint 的七档均已真实接受；
- 在接入 Public Model 前，对 `qwen3.6-27b` 直接执行 `enable_thinking=false/true` 两项 Chat 请求，2/2 HTTP 200 且分别无/有
  `reasoning_content`，确认模型只有开关证据时可归一化为 `none/high`，不外推中间强度。
- 2026-08-10 接入后，扩展 Models 单模型查询返回 HTTP 200，Chat/Responses 均公开 `none/high` 和 `plain_text`；真实下游
  Chat/Responses × JSON/SSE × none/high 为 8/8 HTTP 200、文字非空且终态完整。四个 none 单元均无 reasoning，四个 high
  单元均有可读 reasoning；其中 Responses-via-Chat high 同时产生 reasoning item，但没有 reasoning token 计数。
- 2026-08-10 直连 Chat 证明 GLM 5.2 与 `deepseek-v4-flash-0731` 接受 `parallel_tool_calls:true`；单次结果未证明多 tool call。
  同日 Qwen3.8 Native Responses 的带/不带 `reasoning.encrypted_content` 请求都返回相同明文 `summary_text`，而 GLM 5.2 的直连
  Responses 返回 `Unsupported model`，与当前 Chat Bridge 边界一致。

最终矩阵和剩余错误边界见 [`real-e2e-test-2026-08-08.md`](../real-e2e-test-2026-08-08.md)。

## 证据边界

`tests/provider_contract.rs` 验证 Bailian Chat switch、Responses effort 原值和 DeepSeek `none` 转换；
`tests/forwarding_contract.rs` 验证 Qwen3.8 标准/扩展 Models HTTP 投影及客户端转发结果；`tests/bridge_conversion_contract.rs` 与
`tests/bridge_forwarding_contract.rs` 验证既有 usage-only SSE lifecycle。默认测试不再固定 Qwen Route ID、候选数量/顺序或完整能力快照。

Qwen3.6 当前已接成 Chat Native/Responses Bridge Public Model，并完成上述正常首选 Route 的真实 E2E；本轮没有把 Qwen3.8
多模态/工具/结构化输出或其他 generation Target 的 reasoning 事实外推到未验证能力。当前单账号、单区域请求不证明其他账号、
区域、未来 Provider 行为、外部 SDK、fallback、负载或长期运行兼容性。
