# LongCat API 协议入口

- Last reverified：外部来源最后复核 2026-08-08；2026-08-24 仅整理本地文档，未刷新外部来源。
- Recheck trigger：Chat/Responses endpoint、认证或 reasoning wire 变化。

## 来源与范围

- [API Quick Start](https://longcat.chat/platform/docs/)
- [Chat Completions](https://longcat.chat/platform/docs/api/chat.html)
- [Codex 接入](https://longcat.chat/platform/docs/Codex.html)

本文只记录 endpoint、认证和 reasoning wire，不复制逐模型能力或 effort 结论。

## 协议事实

- OpenAI-compatible base URL 为 `https://api.longcat.chat/openai/v1`，使用 Bearer API key。
- Chat reasoning 使用 `thinking.type` 的 `enabled`/`disabled` 二态 wire。
- 官方 Codex 配置使用 Responses wire；具体模型、effort、context 和当前可用性应直接读取 LongCat 官方文档。

OpenBridge 当前映射见[Model 与 Provider 映射](../../implementation-status/model-provider-mapping.md)。

## 证据边界

官方文档不证明任一真实 API key、模型、JSON/SSE、Bridge、负载或长期运行行为。
