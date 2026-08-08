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
- Chat→Responses SSE 在成功 `finish_reason` 与 `[DONE]` 之间允许一个严格的 `choices: []` + `usage` object
  统计块；该块不产生业务输出，普通 late chunk、重复 usage、finish 前 usage 和 EOF-before-terminal 仍 fail closed。
- `response_format` 与 `text.format` 只转换 text、JSON object 和 JSON Schema 的明确字段；未知格式字段不可表达，会在 egress 前拒绝。
- 不可表达的 image/file/audio、hosted/custom tool、opaque continuation、后台状态、未确认 reasoning 或 Provider 私有扩展在 egress 前拒绝，
  不伪造等价语义。

## 实现边界

- 转换实现位于 [`src/bridge/`](../../../src/bridge/)，生产接入位于 [`src/ingress/`](../../../src/ingress/) 和
  [`src/pipeline/`](../../../src/pipeline/)。
- Bridge 只对代码显式注册的 Route 生效；Embeddings 没有 Bridge representation。
- 当前没有通用异构 Provider、可配置 ConversionPolicy、动态 converter catalog 或 continuation ledger。

## 验证证据

- [`tests/bridge_conversion_contract.rs`](../../../tests/bridge_conversion_contract.rs) 覆盖双向 request、JSON 和 SSE renderer。
- [`tests/bridge_forwarding_contract.rs`](../../../tests/bridge_forwarding_contract.rs) 覆盖生产 Router、Bridge Route 和 egress 前拒绝。
- [`tests/protocol_bridge_replay.rs`](../../../tests/protocol_bridge_replay.rs) 复放 canonical SSE，覆盖 identity、terminal、EOF 和事件冲突。
- [`real-e2e-test-2026-08-08.md`](../real-e2e-test-2026-08-08.md) 记录真实 Bailian/Kimi CN
  Responses-via-Chat JSON/SSE、明文 reasoning、high 请求与 trailing usage chunk 的最终验收结果；其中 Qwen3.7 与
  DeepSeek V4 Pro 的 high Bridge 单元均已通过。

确定性测试证明已建模语义的转换和进程内 lifecycle；真实测试只证明文档所列 endpoint、账号、模型和时间点，不证明完整
OpenAI API 或任意 Provider 私有语义可转换。

## 相关文档

- [功能需求：网关 API 与客户端兼容](../../functional-requirements/gateway-api-compatibility.md)
- [Native Chat/Responses 转发](native-generation-forwarding.md)
- [协议测试语料与工具](../protocol-test-corpus.md)
