# 功能：OpenAI-compatible Embeddings

## 状态

**已完成（当前 checkout）。** `text-embedding-3-small` 通过独立 Public Model 和唯一 Native Route 提供受限的
`POST /v1/embeddings` JSON 链路。

## 已完成内容

- 输入 union 接受 string、string array、token array 和 token-array array；请求预检同时执行 JSON、输入数量、单输入和 token 限制。
- 当前公共 interface 公开 `encoding_format` 与 `user`，默认 encoding 为 float、默认维度为 1536；显式 `dimensions` 当前不公开并在 egress
  前拒绝。
- 请求固定转发到 OpenAI `text-embedding-3-small` target；client 不能覆盖 upstream model、endpoint、credential 或 header。
- 成功体在下游 response commit 前一次性执行有界 JSON 校验，验证 object/index/embedding/usage 等结构；非法成功体不进入 retry。
- Embeddings usage 只记录明确返回的 input/total token；原始文本、token array、user、向量和 base64 不进入观测字段。
- 当前没有 Bridge、多 candidate、向量转换、缓存、索引、检索或独立 tokenizer；字符串不做 tokenizer 估算，token array 只做本地精确计数。

## 实现边界

- 独立 capability 和 request analyzer 位于 [`src/core/capability/embeddings.rs`](../../../src/core/capability/embeddings.rs) 与
  [`src/pipeline/analysis/embeddings.rs`](../../../src/pipeline/analysis/embeddings.rs)。
- Route 注册位于 [`src/providers/catalog/routing.rs`](../../../src/providers/catalog/routing.rs)，转发与成功体校验位于
  [`src/ingress/forwarding/embeddings.rs`](../../../src/ingress/forwarding/embeddings.rs)。
- 该功能的 JSON compatibility 不代表其他 OpenAI 扩展资源或 Native multimodal 已实现。

## 验证证据

- [`tests/embedding_definition_contract.rs`](../../../tests/embedding_definition_contract.rs) 覆盖 Embeddings capability 和编译约束。
- [`tests/embedding_registry_contract.rs`](../../../tests/embedding_registry_contract.rs) 覆盖 Public Model、唯一 candidate 和公开接口。
- [`tests/embedding_forwarding_contract.rs`](../../../tests/embedding_forwarding_contract.rs) 覆盖受信 egress、成功体边界、retry、cancel 和脱敏。
- [`tests/embedding_client_contract.rs`](../../../tests/embedding_client_contract.rs) 覆盖客户端可见 JSON contract。

这些测试证明本地 JSON/HTTP contract 和 fake upstream 行为，不证明真实 OpenAI embedding 数值、模型配额或生产网络可用性。

## 相关文档

- [功能需求：Embeddings 与 Native 多模态扩展](../../functional-requirements/embedding-and-native-multimodal.md)
- [Models 接口与能力预检](models-api-and-capability-preflight.md)
- [运行时指标与遥测](../telemetry-metrics.md)
