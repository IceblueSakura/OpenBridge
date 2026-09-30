# LongCat API 协议入口

- Last reverified：2026-09-30 UTC；重新读取 Chat 与 Codex 官方页面。账号目录和 Chat wire 观察仍为 2026-09-29 的独立证据。
- Recheck trigger：Chat/Responses endpoint、认证或 reasoning wire 变化。

## 来源与范围

- [API Quick Start](https://longcat.chat/platform/docs/)
- [Chat Completions](https://longcat.chat/platform/docs/api/chat.html)
- [Codex 接入](https://longcat.chat/platform/docs/Codex.html)

本文只记录 endpoint、认证和 reasoning wire，不复制逐模型能力或 effort 结论。

## 协议事实

- OpenAI-compatible base URL 为 `https://api.longcat.chat/openai/v1`，使用 Bearer API key。
- Chat reasoning 使用 `thinking.type` 的 `enabled`/`disabled` 二态 wire。
- 官方 Codex 配置使用 `wire_api = "responses"` 与上述 base URL，当前示例选用 `LongCat-2.5-Preview`；对应原生入口为 `/openai/v1/responses`。v2 已声明该固定入口，不从配置示例推定全部 Responses wire 已兼容。
- Chat reference 仍列 `LongCat-2.0`，与 Codex 页的模型列表不一致；本次不据旧 Chat 示例重命名固定 2.5 绑定，也不把网页抽取的重复代码行当作协议事实。
- 已观察的 Chat `lastOne`、`matched_stop`、零值 usage 子字段和逐 chunk `created` 漂移，以及 Responses 的重复 usage detail view，映射合同见 [API-key profiles](../../architecture-v2/api-key-text-profiles.md)。历史 Chat 验收见 [onboarding evidence](../../implementation-status/evidence/2026-09-29-api-key-provider-onboarding.md)，新增 Responses JSON 验收与未完成的工具/SSE 边界见 [2026-09-30 evidence](../../implementation-status/evidence/2026-09-30-flash-longcat-bailian-acceptance.md)。

OpenBridge 旧运行时映射见[固定归档](https://github.com/IceblueSakura/OpenBridge/blob/4f13ecefa21265a6ec5aa967278e81f03039f585/docs/implementation-status/model-provider-mapping.md)，不是 v2 当前能力。

## 证据边界

官方文档不证明任一真实 API key、模型、JSON/SSE、Bridge、负载或长期运行行为。
