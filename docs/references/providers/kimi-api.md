# Kimi CN API 协议入口

- Last reverified：2026-09-29 UTC；重读官方模型参数页并核对现有账号目录。
- Recheck trigger：base URL、认证、Chat endpoint 或官方兼容范围变化。

## 来源与范围

- [API 概述](https://platform.kimi.com/docs/api/overview)
- [Chat Completions API](https://platform.kimi.com/docs/api/chat)
- [模型与参数官方参考](https://platform.kimi.com/docs/api/models-overview)

本文只记录 Kimi 中国开放平台的 endpoint 和认证，不复制逐模型参数约束。

## 已确认协议事实

- 服务地址为 `https://api.moonshot.cn`，OpenAI-compatible SDK base URL 为 `https://api.moonshot.cn/v1`。
- 文本生成入口为 `POST /v1/chat/completions`，使用 Bearer API key。
- OpenAI-compatible 只描述请求/响应形状；具体模型参数、reasoning 和当前可用性以官方模型参考为准。当前选定 K3 的官方合同要求保留 `reasoning_content` 历史，effort 使用顶层 `reasoning_effort`，不能套用 K2.x 的 `thinking` wire。
- 当前绑定的请求投影和可读 reasoning 规则见 [API-key profiles](../../architecture-v2/api-key-text-profiles.md)，实际生成的 429 阻塞见 [onboarding evidence](../../implementation-status/evidence/2026-09-29-api-key-provider-onboarding.md)。

OpenBridge 旧运行时映射见[固定归档](https://github.com/IceblueSakura/OpenBridge/blob/4f13ecefa21265a6ec5aa967278e81f03039f585/docs/implementation-status/model-provider-mapping.md)，不是 v2 当前能力。

## 证据边界

官方文档不证明某个账户当前有模型权限，也不证明 Responses Native、负载、长期运行或未来版本行为。
