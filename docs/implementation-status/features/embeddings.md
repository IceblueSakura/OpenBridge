# 功能：OpenAI-compatible Embeddings

## 当前行为

- `text-embedding-3-small`、`qwen3.7-text-embedding` 与 `nemotron-3-embed-1b` 分别通过独立 Public Model 和唯一
  Native Route 提供 `POST /v1/embeddings`；没有 Bridge 或跨模型 fallback。
- OpenAI 模型接受 string/string-array/token-array/token-array-array，默认 float/1536 维，公开 `encoding_format`/`user`，
  不公开显式 `dimensions`。
- Qwen 接受 string/string-array、float 和固定维度集合，默认 1024 维，批量上限 20、单输入 token 上限 128000。
- Nemotron 接受 string/string-array、float 和固定 2048 维，批量上限 20；没有已确认 tokenizer/token limit。
- 成功 JSON 在下游 commit 前有界验证 object/index/vector/usage。Bailian 可选顶层字符串 `id` 被接受但不投影；其他未知字段
  fail closed。字符串不做 tokenizer 估算，token array 只做精确本地计数。
- usage 只记录明确的 input/total token；文本、token array、user、向量与 Base64 不进入 observation。

## 所有权

Capability/analyzer 位于 `src/core/capability/embeddings.rs` 与 `src/pipeline/analysis/embeddings.rs`；registration 位于
`src/providers/catalog/embeddings.rs`，转发/校验位于 `src/ingress/forwarding/embeddings.rs`。

## 确定性与真实证据

`tests/embedding_forwarding_contract.rs` 覆盖 preflight、受信 egress、成功体、retry、cancel 和脱敏。

真实 Bailian 定向请求确认 Qwen 默认与允许维度，并确认未允许维度在 egress 前拒绝；真实 NVIDIA 定向请求以 string-array
获得两个 2048 维 float 向量。Provider 解释见 [Bailian](../providers/bailian.md)与 [NVIDIA](../providers/nvidia.md)。

## 未证明范围

真实 OpenAI、embedding 语义质量、其他账号/区域、生产配额、负载、长期网络可用性、向量转换/缓存/索引/检索未证明。

## 相关文档

- [Embeddings 需求](../../functional-requirements/extended-capabilities/embeddings.md)
- [Models 与能力预检](models-api-and-capability-preflight.md)
- [OpenTelemetry 遥测](../telemetry-metrics.md)
