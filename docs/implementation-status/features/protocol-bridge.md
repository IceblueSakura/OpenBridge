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
- Responses→Chat 接受显式 `type: "message"` 与只含 `role/content` 的标准 message shorthand；两种写法复用同一 content/role
  校验和 function call/output ledger，缺失 discriminator 的额外或模糊对象继续 fail closed。
- Chat→Responses SSE 在成功 `finish_reason` 与 `[DONE]` 之间允许一个严格的 `choices: []` + `usage` object
  统计块；该块不产生业务输出，普通 late chunk、重复 usage、finish 前 usage 和 EOF-before-terminal 仍 fail closed。
- `response_format` 与 `text.format` 只转换 text、JSON object 和 JSON Schema 的明确字段；未知格式字段不可表达，会在 egress 前拒绝。
- Bridge 与 Native 共用源协议顶层字段目录。未知字段先返回 `unknown_parameter`；已知但当前方向不可表示的字段只有在所选 API 对五类
  普通提示具有显式忽略规则时才能接受，并会在 Bridge request converter 之前删除。每个 fallback candidate 仍从原始 body 独立构造。
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
- [`tests/native_routing_contract.rs`](../../../tests/native_routing_contract.rs) 覆盖 Bridge 未知字段分类、转换前 candidate 参数删除和
  fallback body 隔离；[`tests/forwarding_contract.rs`](../../../tests/forwarding_contract.rs) 覆盖对应 HTTP 错误与 transport zero egress。
- [`tests/protocol_bridge_replay.rs`](../../../tests/protocol_bridge_replay.rs) 复放 canonical SSE，覆盖 identity、terminal、EOF 和事件冲突。
- `bridge_conversion_contract::responses_to_chat_non_stream_drops_completed_opaque_continuation` 覆盖真实 GPT 形状的 output-only opaque
  continuation，以及可读 summary 的保留；`forwarding_contract::chatgpt_buffers_streaming_responses_for_non_streaming_responses_and_chat`
  覆盖 streaming-only upstream 的完整 buffer 与非流式 Chat JSON 接入。
- `bridge_conversion_contract::responses_message_shorthand_preserves_a_tool_result_round_trip` 与
  `example_config::routing::longcat_responses_tool_continuation_prepares_native_and_bridge_candidates` 覆盖 shorthand 转换和 Native-first
  固定候选计划；2026-08-09 真实 LongCat Responses 非流式 call/result/final-text 续接为 2/2 HTTP 200，最终文本为 `DONE` 且没有重复 tool call。
- [`real-e2e-test-2026-08-08.md`](../real-e2e-test-2026-08-08.md) 记录真实 Bailian/Kimi CN
  Responses-via-Chat JSON/SSE，以及五个 GPT ChatGPT-source 模型的 Chat/Responses、stream on/off 与 omitted/high 最终验收结果；
  120 个文字生成单元均达到合法成功终态；同日最新聚焦复测另确认 Kimi Responses-via-Chat 在非默认 `temperature` 下的 JSON/SSE
  仍为 HTTP 200，而未知字段和已禁用输出语义参数在 Bridge 前稳定拒绝。

确定性测试证明已建模语义的转换和进程内 lifecycle；真实测试只证明文档所列 endpoint、账号、模型和时间点，不证明完整
OpenAI API 或任意 Provider 私有语义可转换。

## 相关文档

- [功能需求：网关 API 与客户端兼容](../../functional-requirements/gateway-api-compatibility.md)
- [Native Chat/Responses 转发](native-generation-forwarding.md)
- [协议测试语料与工具](../protocol-test-corpus.md)
