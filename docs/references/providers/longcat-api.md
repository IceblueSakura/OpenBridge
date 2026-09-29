# LongCat API 协议入口

- Last reverified：2026-09-29 UTC；重新读取 Chat 与 Codex 官方页面，并核对现有账号模型目录与选定模型的 Chat wire。
- Recheck trigger：Chat/Responses endpoint、认证或 reasoning wire 变化。

## 来源与范围

- [API Quick Start](https://longcat.chat/platform/docs/)
- [Chat Completions](https://longcat.chat/platform/docs/api/chat.html)
- [Codex 接入](https://longcat.chat/platform/docs/Codex.html)

本文只记录 endpoint、认证和 reasoning wire，不复制逐模型能力或 effort 结论。

## 协议事实

- OpenAI-compatible base URL 为 `https://api.longcat.chat/openai/v1`，使用 Bearer API key。
- Chat reasoning 使用 `thinking.type` 的 `enabled`/`disabled` 二态 wire。
- 官方 Codex 配置使用 Responses wire，其当前示例选用 `LongCat-2.5-Preview`；本轮 v2 只准入该绑定的 Chat，不从配置示例推定 Responses codec 已兼容。
- 已观察的 Chat `lastOne`、`matched_stop`、零值 usage 子字段和逐 chunk `created` 漂移，映射合同见 [API-key profiles](../../architecture-v2/api-key-text-profiles.md)，实际验收见 [onboarding evidence](../../implementation-status/evidence/2026-09-29-api-key-provider-onboarding.md)。

OpenBridge 旧运行时映射见[固定归档](https://github.com/IceblueSakura/OpenBridge/blob/4f13ecefa21265a6ec5aa967278e81f03039f585/docs/implementation-status/model-provider-mapping.md)，不是 v2 当前能力。

## 证据边界

官方文档不证明任一真实 API key、模型、JSON/SSE、Bridge、负载或长期运行行为。
