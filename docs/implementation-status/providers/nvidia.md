# NVIDIA Provider 状态

## 当前实现

- Provider family 为 `nvidia`，固定 origin 为 `https://integrate.api.nvidia.com/v1`，使用 `nvidia-primary` API-key pool。
- `nvidia-minimax-m3` 提供 Chat Native，并作为 `minimax-m3` 的 OpenRouter 后备；全局已有 Responses Native，因此不为该
  NVIDIA Chat source 生成冗余 Responses Bridge。
- `nvidia-nemotron-3-embed-1b` 通过唯一 Embeddings Native Route 提供 `nemotron-3-embed-1b` Public Model。
- Embeddings 接受 string/string-array、float encoding 和固定 2048 维，批量上限为 20；不公开 tokenizer 推导的 token limit。
- Provider ceiling 包含图片、工具和 structured output，但 Target 能力仍以模型级收窄和 Public Model 完整候选交集为准。

## 所有权与确定性证据

- 注册与 wire 规则：[`src/providers/nvidia/`](../../../src/providers/nvidia/)。
- `tests/provider_contract.rs`、`tests/provider_boundary_contract.rs` 保护 Chat/Embeddings 路径、模型、API-key 与安全出站。
- `tests/embedding_forwarding_contract.rs` 保护 Nemotron input/dimension/response validation 和 usage 边界。
- `tests/forwarding_contract.rs` 保护 MiniMax multi-source 与 zero-egress preflight。

## 真实 Provider 证据

2026-08-10 的定向 Nemotron 请求以 string-array 输入获得两个 2048 维 float 向量，支持当前 input/encoding/dimension 收窄；
没有从该结果推断语义质量或 token limit。

[2026-08-09 文字矩阵](../evidence/real-provider/2026-08-09-text-generation-none-high-matrix.md)中的
`minimax-m3` 正常路径优先 OpenRouter，因此不证明 NVIDIA Chat 后备。NVIDIA 外部 endpoint/model 事实见
[API 参考](../../references/providers/nvidia/api.md)与 [Models 快照](../../references/providers/nvidia/models.md)。

## 未证明边界

MiniMax 的强制 NVIDIA fallback、图片/工具/structured output、真实 reasoning、Embeddings 语义质量、其他账号/区域、外部 SDK/Agent、
配额、负载和长期运行未证明。
