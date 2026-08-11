# OpenAI Provider 状态

## 当前实现

- Provider family 为 `openai`，固定 origin 为 `https://api.openai.com`，使用 `openai-primary` API-key pool。
- `openai-main` 绑定 `gpt-5.6-sol` 的 Chat/Responses Native surface，并作为同名 Public Model 的后备 source。
- GPT-5.5、GPT-5.6 Luna/Terra 也有固定 Chat/Responses Target，但当前不单独贡献 Public Model source；它们只是可信
  canonical/Target 绑定。
- `openai-text-embedding-3-small` 通过唯一 Embeddings Native Route 提供 `text-embedding-3-small` Public Model。
- Generation Target 保守关闭未验证的图片、strict tool、structured output 与持久状态；function tool 只保留共同的非 strict、
  non-parallel 合同。Embeddings 固定 float/base64、输入形状、批量/token budget 与 1536 默认维度，不公开显式 dimensions。
- 缺失或空 `openai-primary` 只让这些 Target 运行时不可执行，不删除编译期 Provider/Model 注册。

## 所有权与确定性证据

- Provider ceiling、endpoint 与 Target 收窄：[`src/providers/openai/`](../../../src/providers/openai/)。
- `tests/provider_contract.rs`、`tests/provider_boundary_contract.rs` 保护相对路径、认证、模型替换与安全 header。
- `tests/embedding_forwarding_contract.rs` 保护 Embeddings preflight、Native request/response 和有界 JSON 校验。
- `tests/upstream_credential_config.rs` 保护 active pool 对运行时可执行性的单向收窄。

## 真实 Provider 证据

当前没有成功的真实 OpenAI Provider 记录。此前 Models probe 得到认证失败，只能记为 `unknown`，不能证明 endpoint 或模型不支持。
[2026-08-09 文字矩阵](../evidence/real-provider/2026-08-09-text-generation-none-high-matrix.md)中的 `gpt-5.6-sol`
使用 ChatGPT 首选 source，不能作为 OpenAI generation 或 fallback 证据；当前也没有真实 `text-embedding-3-small` 请求记录。

## 未证明边界

真实账号权限、Models、Chat/Responses、Embeddings、强制 fallback、外部 SDK、图片、strict/parallel tool、structured output、
状态资源、配额、负载和长期运行均未证明。Provider ceiling 不等于 checked-in Target 已开放能力。

## 相关文档

- [Embeddings 功能状态](../features/embeddings.md)
- [OpenAI API 参考](../../references/openai/README.md)
