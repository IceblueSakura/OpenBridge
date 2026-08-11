# 功能：Chat 与 Responses 的显式 Protocol Bridge

## 状态

**已完成（当前 checkout）。** 对注册为 `Bridged` 的 Route，网关可以在明确可表达的语义范围内执行 Chat Completions ↔ Responses 双向请求和
响应转换，包括 JSON 与 SSE。

## 已完成内容

- 支持 allowlist 内的 text、function tool、parallel tool call、tool result、structured output、明文 reasoning channel、非流式 JSON 和流式
  SSE 转换。
- `BridgePlan` 在上游调用前检查可表达性；tool-call identity、fragmented arguments、response/project index 和 Responses terminal 由独立 stream
  state machine 维护。
- Chat→Responses 与 Responses→Chat 均使用显式的 request converter、stream renderer 和 terminal lifecycle，不把两种协议简单当作字段别名。
- Responses 请求分析已类型化区分 `reasoning.summary` 的省略、兼容 `false`、标准 `"auto"` 与非法值。Native Responses 精确保留
  `false`/`"auto"`；Responses→Chat 只消费该字段并继续把 `reasoning.effort` 映射为 `reasoning_effort`，不向 Chat wire 添加
  summary 或私有 reasoning 开关。Chat 上游实际返回的 `reasoning_content` 映射为 Responses reasoning item 的
  `content[].type:"reasoning_text"` 与流式 `response.reasoning_text.delta/done`，`summary` 保持空数组且不生成
  `response.reasoning_summary_*` 事件。兼容值 `false` 不关闭 reasoning；其他 summary shape 以及显式 `effort:"none"` +
  `summary:"auto"` 在 Provider egress 前返回稳定无效请求。
- Responses→Chat 接受显式 `type: "message"` 与只含 `role/content` 的标准 message shorthand；两种写法复用同一 content/role
  校验和 function call/output ledger，缺失 discriminator 的额外或模糊对象继续 fail closed。
- Chat→Responses SSE 在成功 `finish_reason` 与 `[DONE]` 之间允许一个严格的 `choices: []` + `usage` object
  统计块；该块不产生业务输出，普通 late chunk、重复 usage、finish 前 usage 和 EOF-before-terminal 仍 fail closed。
- `response_format` 与 `text.format` 只转换 text、JSON object 和 JSON Schema 的明确字段；未知格式字段不可表达，会在 egress 前拒绝。
- Bridge 与 Native 共用源协议顶层字段目录。未知字段先返回 `unknown_parameter`；已知但当前方向不可表示的字段只有在所选 API 对五类
  普通提示具有显式忽略规则时才能接受，并会在 Bridge request converter 之前删除。每个 fallback candidate 仍从原始 body 独立构造。
- `prompt_cache_key` 是显式的双向 shared request field；只有目标 Upstream API 的静态 profile 声明 exact forwarding 时，Bridge Route 才把
  它贡献到固定接口并原样复制。`include: []` 先作为 no-op 移除。Responses→Chat 当前只对具有可读 reasoning channel 的 Route 接受
  `reasoning.encrypted_content`：该值没有 Chat wire 对应物，方向转换器验证后显式消费，继续保留上游真实明文 reasoning，但不保证
  reasoning item 存在，也不把明文重新标记为 opaque `encrypted_content`。其他非空 include 仍不贡献。
- 不可表达的 image/file/audio、hosted/custom tool、后台状态、未确认 reasoning 或 Provider 私有扩展在 egress 前拒绝，不伪造等价语义。
  下游 request/history 中的 opaque continuation 仍拒绝；已完成 Responses 输出转为无状态 Chat response 时，验证后丢弃
  `encrypted_content`，保留可读 summary/content、text 与 tool call，且绝不把 opaque 值投影为 `reasoning_content`。

## 实现边界

- 转换实现位于 [`src/bridge/`](../../../src/bridge/)，生产接入位于 [`src/ingress/`](../../../src/ingress/) 和
  [`src/pipeline/`](../../../src/pipeline/)。
- Bridge 只对代码显式注册的 Route 生效；Embeddings 没有 Bridge representation。
- 当前没有通用异构 Provider、可配置 ConversionPolicy、动态 converter catalog 或 continuation ledger。

## 验证证据

- [`tests/bridge_conversion_contract.rs`](../../../tests/bridge_conversion_contract.rs) 覆盖双向 request、JSON 和 SSE renderer。
- [`tests/bridge_forwarding_contract.rs`](../../../tests/bridge_forwarding_contract.rs) 覆盖生产 Router、Bridge Route 和 egress 前拒绝。
  其中缓存键用例检查 Responses→DeepSeek Chat post-adapter exact egress 与空 include 移除；reasoning include 用例检查固定 interface
  接受该值、Chat egress 显式移除且不合成 opaque output。
- [`tests/forwarding_contract.rs`](../../../tests/forwarding_contract.rs) 覆盖 Bridge 相关 HTTP 错误、参数处置、fallback 隔离与 transport
  zero egress；默认测试不再单独锁定内部 candidate body 或 Route 顺序。
- 2026-08-11 的 M6 失败优先测试先确认旧 Bridge 对 `false`/`"auto"` 返回 `UnsupportedSemantics`；实现后，转换与生产 Router
  测试覆盖两个值的 Chat exact egress、JSON/SSE reasoning content 与零伪造 summary，Native 测试覆盖两个值的 exact forwarding，
  并确认 `"auto"` 在凭据轮换重试时复用完全相同的请求 body。非法 string、`true`、`null`、object 和 `none+auto` 均覆盖 typed
  HTTP 400 与 zero Provider egress。
- 同日 `cargo fmt -- --check`、`cargo test --locked`、`cargo clippy --locked -- -D warnings` 与 `git diff --check` 全部通过；其中
  `bridge_conversion_contract` 20 项、`bridge_forwarding_contract` 12 项、`forwarding_contract` 70 项、`ingress_contract` 6 项通过。
  本轮没有运行 Hermes、外部 OpenAI SDK、真实 Provider、强制多 Provider fallback、负载或长期运行验收。
- [`tests/protocol_bridge_replay.rs`](../../../tests/protocol_bridge_replay.rs) 复放 canonical SSE，覆盖 identity、terminal、EOF 和事件冲突。
- `bridge_conversion_contract::responses_to_chat_non_stream_drops_completed_opaque_continuation` 覆盖真实 GPT 形状的 output-only opaque
  continuation，以及可读 summary 的保留；`forwarding_contract::chatgpt_buffers_streaming_responses_for_non_streaming_responses_and_chat`
  覆盖 streaming-only upstream 的完整 buffer 与非流式 Chat JSON 接入。
- `bridge_conversion_contract::responses_message_shorthand_preserves_a_tool_result_round_trip` 与
  `example_config::routing::longcat_responses_tool_continuation_prepares_native_and_bridge_candidates` 覆盖 shorthand 转换和 Native-first
  固定候选计划；2026-08-09 真实 LongCat Responses 非流式 call/result/final-text 续接为 2/2 HTTP 200，最终文本为 `DONE` 且没有重复 tool call。
- `forwarding_contract::native::deepseek_json_object_is_preserved_by_native_and_bridge_egress` 覆盖 DeepSeek V4 Pro
  Responses `text.format:json_object` 到 Chat `response_format:json_object` 的生产 Bridge；同日真实 JSON/SSE 聚焦请求 2/2 返回可解析的
  预期 JSON。
- [`real-e2e-test-2026-08-08.md`](../real-e2e-test-2026-08-08.md) 只保留最新的 16 个可见文字模型
  `none/high × Chat/Responses × JSON/SSE` 矩阵；128 个请求中 124 个返回完整 HTTP 200 终态，另外 4 个均为 Spark `none`
  的已记录 HTTP 400。矩阵覆盖 Kimi 与 GLM 的 Responses-via-Chat JSON/SSE，但不证明强制 fallback 或未纳入该矩阵的参数组合。

2026-08-10 对 6 个真实 Responses-via-Chat 候选分别执行 baseline 与 `prompt_cache_key` 脱敏请求，12/12 得到 HTTP 200 和可识别 Chat
completion：LongCat、DeepSeek V4 Pro、Bailian DeepSeek V4 Pro、Kimi K3、Bailian GLM-5.2 与 Qwen3.6 27B。该结果只支持 exact
forwarding 声明；未执行 cache-hit 因果验收，也未证明 Bridge 能返回任何非空 Responses include 投影。

确定性测试证明已建模语义的转换和进程内 lifecycle；真实测试只证明文档所列 endpoint、账号、模型和时间点，不证明完整
OpenAI API 或任意 Provider 私有语义可转换。

## 相关文档

- [功能需求：网关 API 与客户端兼容](../../functional-requirements/gateway-api-compatibility.md)
- [Native Chat/Responses 转发](native-generation-forwarding.md)
- [协议测试语料与工具](../protocol-test-corpus.md)
