# 功能：OpenAI-compatible Embeddings

## 状态

**已完成（当前 checkout）。** `text-embedding-3-small` 与 `qwen3.7-text-embedding` 分别通过独立 Public Model 和唯一
Native Route 提供受限的 `POST /v1/embeddings` JSON 链路。

## 已完成内容

- OpenAI Public Model 接受 string、string array、token array 和 token-array array；Qwen Public Model 接受 string 和 string array；请求预检同时执行 JSON、输入数量、单输入和 token 限制。
- `text-embedding-3-small` 公共 interface 公开 `encoding_format` 与 `user`，默认 encoding 为 float、默认维度为 1536；显式 `dimensions` 当前不公开并在 egress 前拒绝。
- `qwen3.7-text-embedding` 公共 interface 公开 `encoding_format` 与 `dimensions`，只允许 float，默认维度为 1024，允许维度为 256、512、768、1024、1536、2048、2560，批量上限为 20，单输入 token 上限为 128000。
- 请求分别固定转发到 OpenAI `text-embedding-3-small` 或百炼 `qwen3.7-text-embedding` Target；client 不能覆盖 upstream model、endpoint、credential 或 header。
- 成功体在下游 response commit 前一次性执行有界 JSON 校验，验证 object/index/embedding/usage 等结构；Bailian 可选顶层字符串 `id` 被显式接收但不投影给下游，其他未知字段仍 fail closed；非法成功体不进入 retry。
- Embeddings usage 只记录明确返回的 input/total token；原始文本、token array、user、向量和 base64 不进入观测字段。
- 当前没有 Bridge、多 candidate、跨模型 fallback、向量转换、缓存、索引、检索或独立 tokenizer；字符串不做 tokenizer 估算，token array 只做本地精确计数。

## 实现边界

- 独立 capability 和 request analyzer 位于 [`src/core/capability/embeddings.rs`](../../../src/core/capability/embeddings.rs) 与
  [`src/pipeline/analysis/embeddings.rs`](../../../src/pipeline/analysis/embeddings.rs)。
- Route 注册位于 [`src/providers/catalog/routing.rs`](../../../src/providers/catalog/routing.rs)，转发与成功体校验位于
  [`src/ingress/forwarding/embeddings.rs`](../../../src/ingress/forwarding/embeddings.rs)。
- 该功能的 JSON compatibility 不代表其他 OpenAI 扩展资源、Native file/audio 或图片切片之外的多模态已实现。

## 验证证据

- [`tests/embedding_definition_contract.rs`](../../../tests/embedding_definition_contract.rs) 覆盖 Embeddings capability 和编译约束。
- [`tests/embedding_registry_contract.rs`](../../../tests/embedding_registry_contract.rs) 覆盖 Public Model、唯一 candidate 和公开接口。
- [`tests/embedding_forwarding_contract.rs`](../../../tests/embedding_forwarding_contract.rs) 覆盖受信 egress、客户端可见 JSON
  response、成功体边界、retry、cancel 和脱敏。
- 2026-08-09 聚焦验证：`cargo test --locked --test embedding_definition_contract --test embedding_forwarding_contract --test embedding_registry_contract`
  通过（6 + 20 + 2 项）。
- 2026-08-09 真实 Bailian 验证：默认维度与 `256/512/768/1024/1536/2048/2560` 均返回 HTTP 200、向量维度正确且下游不含
  Provider `id`；`64/128` 均在 egress 前返回 HTTP 400 `unsupported_model_capability`，精确 `param: dimensions`，共 10/10 通过。

这些测试与真实矩阵证明当前私有配置下的 Bailian Qwen JSON/HTTP contract 和维度行为；不证明 embedding 语义质量、真实 OpenAI、其他账号/区域、生产配额、负载或长期网络可用性。

## 相关文档

- [功能需求：Embeddings 能力](../../functional-requirements/embeddings.md)
- [扩展共同规则](../../functional-requirements/embedding-and-native-multimodal.md)
- [Models 接口与能力预检](models-api-and-capability-preflight.md)
- [运行时指标与遥测](../telemetry-metrics.md)
