# 功能：Chat Completions 与 Responses Native 转发

## 状态

**已完成（当前 checkout）。** 网关可以对已注册且通过 Public Model 预检的 Chat/Responses 请求执行 Native JSON 或 SSE 转发，并保留目标协议
允许的原生语义。

## 已完成内容

- `POST /v1/chat/completions` 和 `POST /v1/responses` 支持当前声明范围内的非流式 JSON 与 streaming SSE。
- Upstream API 使用类型化 streaming policy：普通 API 保留下游 mode；ChatGPT Responses 声明 `stream: true` required，并启用
  bounded Responses SSE buffering。下游非流式 Responses 在合法 terminal 后返回完整 response object，非流式 Chat 再经既有
  Responses→Chat JSON Bridge 返回。
- Native Route 对已知且被接口接受的字段保留下游 canonical wire 语义；Provider adapter 在 egress 阶段绑定固定 upstream model、
  相对 path、普通固定 header 和 purpose-bound authentication。未知顶层字段不再属于 Native 透明透传范围。
- Upstream API 可以用闭合 `IgnorableGenerationParameter` 集合接受但不向上游发送已确认不兼容的普通生成字段；这些字段仍保留在
  Public Model `supported_parameters`。当前 Kimi K3 Chat 只删除 `frequency_penalty`、`presence_penalty`、`temperature`、`top_p`；
  ChatGPT GPT-5.5/5.6 Responses 只删除 `seed`。Kimi 的 `n/logprobs/top_logprobs`、MiMo V2.5/Pro Responses 的
  `top_logprobs` 和 ChatGPT 的 `include_reasoning` 改为禁用并从固定 interface 收窄，在 egress 前明确拒绝。
  stream、reasoning level/开关、tools、structured output、state、媒体和输出 token 上限同样不在忽略闭合集合内。
- 参数忽略在每个 candidate 从原始 body 独立构造之后、进入第一个 Bridge/Provider shape 转换之前执行；Native 无忽略规则时继续保留
  原始 bytes。Provider adapter 保留同一删除规则作为最终 egress 防线，前一 candidate 的删除不会改变 fallback body。
- Reasoning level 由 Canonical Model 统一定义并在同一模型的 Chat/Responses interface 中保持一致；Native Responses 保留具体
  effort，只有 thinking 开关的 Chat Provider 将 `none` 映射为关闭、其余已声明 level 映射为开启。未知 level 在 egress 前拒绝。
- 当前 Native surface 包括 OpenAI `gpt-5.6-sol`、LongCat `LongCat-2.0`、DeepSeek Chat 与 V4 Flash 无状态 Responses、
  OpenRouter 的 `deepseek-v4-flash` 与 `minimax-m3` Chat/无状态 Responses、Bailian Qwen3.7 Max/Plus，以及 Xiaomi MiMo 的
  Chat/Responses。
- Bailian Qwen3.7 Native Responses 的 reasoning output 使用官方 `reasoning.summary[]`，与 Chat 的 `reasoning_content`
  plain-text wire 分开建模；两协议仍共享同一七档 Model 能力。
- `mimo-v2.5` 的两个同协议 Native surface 还支持固定 typed contract 内的 URL/Base64 图片输入；具体边界和真实 Provider 证据由
  [Native 图片专题](native-image-input.md)记录。
- DeepSeek V4 Flash 与 OpenRouter 的 `store: true`、非空 `previous_response_id` 和 `background: true` 等未声明状态语义在 egress
  前拒绝；DeepSeek V4 Pro 仍只注册 Chat Native API。
- 上游 safe response headers、SSE framing、terminal、EOF-before-terminal 和 body failure 在统一 ingress/transport 边界处理。
- streaming-to-JSON takeover 只接受 Responses SSE，并同时受 JSON response body 与单 SSE event 上限约束；它校验标准 text lifecycle，
  从 response snapshots 与有序 `response.output_item.done` 补齐稀疏 terminal；非法 framing/UTF-8、
  非 SSE success、超限 body 或缺失 terminal 在下游 body commit 前返回安全 502。当前不实现通用 Chat SSE 聚合。
- 成功 streaming response 通过静态 Provider media profile 分类：普通 Provider 必须显式返回唯一的 `text/event-stream`；当前 ChatGPT
  Responses backend 的真实成功响应允许缺失 `Content-Type`，网关仍执行完整 SSE 校验并向下游规范化为 `text/event-stream`。已出现但错误、
  前缀相似或重复的媒体类型不会进入该特例；原生 stream 与 streaming-to-JSON takeover 均在 body commit 前 fail closed。

## 实现边界

- 请求入口位于 [`src/ingress/`](../../../src/ingress/)，请求分析/规划位于 [`src/pipeline/`](../../../src/pipeline/)，Provider adapter 位于
  [`src/provider/adapter.rs`](../../../src/provider/adapter.rs)，共享发送边界位于 [`src/transport/upstream.rs`](../../../src/transport/upstream.rs)。
- Native Route 的额外能力不会扩大 Public Model；它必须先通过公共 interface preflight。
- 这不是外部 OpenAI SDK、Codex/Hermes Agent、真实 Provider、负载或长期运行兼容性声明。

## 验证证据

- [`tests/forwarding_contract.rs`](../../../tests/forwarding_contract.rs) 覆盖 Native JSON/SSE、错误、header 和响应收口。
- [`tests/sse_contract.rs`](../../../tests/sse_contract.rs) 覆盖 SSE framing、terminal、EOF 和错误边界。
- [`tests/provider_contract.rs`](../../../tests/provider_contract.rs) 与 [`tests/provider_boundary_contract.rs`](../../../tests/provider_boundary_contract.rs)
  覆盖 Provider wire、认证和安全出站。
- [`tests/native_routing_contract.rs`](../../../tests/native_routing_contract.rs) 覆盖公共契约与候选规划。
- [`tests/config_contract.rs`](../../../tests/config_contract.rs) 覆盖 streaming policy 与 operation/capability 的启动校验；
  [`tests/forwarding_contract.rs`](../../../tests/forwarding_contract.rs) 覆盖 ChatGPT JSON/SSE、强制上游 `stream: true`、terminal takeover
  以及非法/超限流的安全失败。
- [`tests/example_config.rs`](../../../tests/example_config.rs) 与 [`tests/forwarding_contract.rs`](../../../tests/forwarding_contract.rs) 覆盖
  DeepSeek V4 Flash 的 DeepSeek→OpenRouter Responses 候选顺序、固定 `/responses` egress 与 typed SSE terminal。
- `tests/example_config.rs::minimax_m3_compiles_with_openrouter_first_and_binary_reasoning` 覆盖 MiniMax 的 OpenRouter→NVIDIA Chat
  顺序、OpenRouter Responses Native 和 `none/high` 两接口契约。

2026-08-09 ChatGPT streaming response media 修复的实际验证：

- 最小脱敏诊断确认上游 HTTP 200 Responses body 有 9 个合法事件及 `response.completed`，但没有 `Content-Type`；诊断不保存正文、ID、
  request ID 或 credential；
- 5 个 GPT 模型最终重跑 Chat/Responses × `stream:false/true` × omitted/high 共 40 个真实单元，全部得到合法 200 JSON/SSE 终态；
  0 个 HTTP、协议或传输错误，0 个单元触发 429/503 重试。

2026-08-09 严格参数处置的最终验证：

- `tests/config_contract.rs` 验证 canonical 参数必须进入类型化目录，以及 ignore rule 的声明、重复/冲突边界；
  `tests/embedding_definition_contract.rs` 验证 Embeddings 拒绝 generation ignore rule；`tests/native_routing_contract.rs` 和
  `tests/forwarding_contract.rs` 覆盖 Native/Bridge 未知参数、candidate 级删除、fallback 隔离、固定 interface 投影和 zero egress 拒绝；
- 使用真实下游 key 对 Kimi `temperature` 执行 Chat/Responses × JSON/SSE，4/4 为 HTTP 200 且终态合法；同一运行中的未知字段 2/2
  返回 `unknown_parameter`，Kimi `n/logprobs/top_logprobs` 两协议 6/6 返回带精确 `param` 的
  `unsupported_model_capability`；
- 最后使用 GPT-5.6 Luna 对照：Chat/Responses 的 `seed` 2/2 为 HTTP 200，`include_reasoning` 2/2 在 egress 前返回
  `unsupported_model_capability`。全部真实单元一次完成，没有最终 429/503 或传输错误；结果未保存 credential、请求/响应正文、
  reasoning、logprobs 或 Provider request ID。

2026-08-08 DeepSeek V4 Flash Responses Native 变更的实际验证：

- 首条 `cargo test --locked --test example_config deepseek_pro_stays_chat_only_while_flash_prefers_deepseek_responses` 在实现前按预期失败，
  原因为 Flash target 尚无 Responses API；
- `cargo test --locked --test provider_contract`、`cargo test --locked --test provider_boundary_contract`、
  `cargo test --locked --test example_config` 与 `cargo test --locked --test forwarding_contract deepseek_v4_flash`：通过；
- `cargo fmt -- --check`、`cargo test --locked`、`cargo clippy --locked -- -D warnings` 与 `git diff --check`：通过。

2026-08-09 DeepSeek V4 Flash tool-choice 收窄验证：

- DeepSeek Responses Upstream API 只保留真实确认的 `none/auto`，因此 Public Model 的 Responses interface 通过固定 DeepSeek/OpenRouter
  候选交集公开同一集合；`required/named` 不会因为后备 OpenRouter 更强而成为公共保证；
- `example_config::providers::deepseek_flash_responses_exposes_only_proven_tool_choice_modes` 覆盖 Models 投影、正向计划与
  `required/named` 计划阶段拒绝；最终 `cargo test --locked --test example_config` 通过（13 项）；
- 真实 DeepSeek 首选路径的 `none/auto/required/named` × JSON/SSE 共 8/8 符合固定契约：前四项 HTTP 200 且终态合法，后四项在
  egress 前返回 HTTP 400 `unsupported_model_capability`。当前通用能力错误不携带 `param`。

真实检查不证明 OpenRouter fallback、外部 OpenAI SDK、Codex/Hermes runtime、负载或长期运行兼容性。

## 相关文档

- [功能需求：网关 API 与客户端兼容](../../functional-requirements/gateway-api-compatibility.md)
- [协议 Bridge](protocol-bridge.md)
- [`mimo-v2.5` Native 图片输入](native-image-input.md)
- [DeepSeek API 协议入口快照](../../references/providers/deepseek/api.md)
- [重试、fallback、cooldown 与取消](resilience-retry-fallback-and-cancellation.md)
- [当前代码架构](../current-architecture.md)
