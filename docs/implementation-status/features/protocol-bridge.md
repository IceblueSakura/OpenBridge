# 功能：Chat 与 Responses 的显式 Protocol Bridge

## 当前行为

- 只有注册为 `Bridged` 的 Route 执行 Chat ↔ Responses request/JSON/SSE 转换；支持 allowlist 内 text、function tool、parallel
  tool call、tool result、structured output 与明文 reasoning channel。
- `BridgePlan` 在 egress 前检查可表达性；两侧 stream state machine 维护 item/call/index、fragmented arguments 和唯一 terminal。
- Responses `reasoning.summary` 接受省略、`false` 与 `"auto"`；Responses→Chat 消费 summary 选项但不伪造 summary，真实
  `reasoning_content` 映射为 Responses reasoning text。`false` 不等于关闭 reasoning。
- Responses message shorthand 与显式 message 共享 role/content/tool ledger；额外或模糊对象 fail closed。
- Chat `include_usage:true` 由 Bridge 消费并从完整 Responses terminal usage 生成唯一 Chat usage-only chunk；缺失/非法 usage
  不伪造成功尾部。`{}`/`false` 是 no-op。
- `response_format`/`text.format` 只转换明确的 text、JSON object/Schema；`prompt_cache_key` 只有目标 API 和 converter 都能
  exact-forward 时才贡献。
- image/file/audio、hosted/custom tool、background/state、opaque continuation 和 Provider 私有语义没有可验证等价物时在 egress
  前拒绝。已完成 Responses output 的 opaque encrypted content 不投影成明文 reasoning。

## 所有权

转换位于 [`src/bridge/`](../../../src/bridge/)，生产接入位于 `src/ingress/` 与 `src/pipeline/`。Bridge 不选择 Provider/Route；
Embeddings 没有 Bridge representation。

## 确定性与真实证据

`tests/bridge_conversion_contract.rs`、`tests/bridge_forwarding_contract.rs` 与 `tests/protocol_bridge_replay.rs` 覆盖双向 request、
JSON/SSE、usage、reasoning、tool identity、terminal/EOF/conflict 和 zero egress；`tests/forwarding_contract.rs` 覆盖生产 Router、
fallback 隔离与 Provider wire。

[2026-08-09 文字矩阵](../evidence/real-provider/2026-08-09-text-generation-none-high-matrix.md)包含 Kimi、GLM 与 ChatGPT 等
正常 Bridge 路径，但没有强制多 Provider fallback。LongCat tool continuation 与 DeepSeek `json_object` 的定向真实请求支持对应
已建模转换；`prompt_cache_key` 请求成功不证明 cache hit。

## 未证明范围

完整 OpenAI API、通用异构 conversion policy、动态 converter、图片/音频/file Bridge、opaque state、外部 SDK/Agent、负载和长期运行未证明。

## 相关文档

- [网关 API 需求](../../functional-requirements/gateway-api/README.md)
- [Native generation](native-generation-forwarding.md)
- [协议语料与工具](../test-assets/protocol-corpus.md)
