# Kimi CN API 协议入口

- Last reverified：外部来源最后复核 2026-08-09；2026-08-24 仅整理本地文档，未刷新外部来源。
- Recheck trigger：base URL、认证、Chat endpoint 或官方兼容范围变化。

## 来源与范围

- [API 概述](https://platform.kimi.com/docs/api/overview)
- [Chat Completions API](https://platform.kimi.com/docs/api/chat)
- [模型与参数官方参考](https://platform.kimi.com/docs/api/models-overview)

本文只记录 Kimi 中国开放平台的 endpoint 和认证，不复制逐模型参数约束。

## 已确认协议事实

- 服务地址为 `https://api.moonshot.cn`，OpenAI-compatible SDK base URL 为 `https://api.moonshot.cn/v1`。
- 文本生成入口为 `POST /v1/chat/completions`，使用 Bearer API key。
- OpenAI-compatible 只描述请求/响应形状；具体模型参数、reasoning 和当前可用性以官方模型参考为准。

OpenBridge 当前映射见[Model 与 Provider 映射](../../implementation-status/model-provider-mapping.md)。

## 证据边界

官方文档不证明某个账户当前有模型权限，也不证明 Responses Native、负载、长期运行或未来版本行为。
