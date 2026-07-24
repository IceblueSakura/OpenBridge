# 实施阶段索引

实施阶段按证据门推进，不按代码量或日期判断完成。阶段文档只保留当前目标、工作范围、测试和退出条件。

| 阶段 | 文档 | 当前状态 | 核心结果 |
|---|---|---|---|
| C0 | [范围与客户端契约](00-scope-and-client-contracts.md) | In progress | 固定 Codex/Hermes 契约和 corpus |
| C1 | [双 Native Path](01-native-paths.md) | In progress | 两个目标客户端原生工具循环 |
| C2 | [Provider 聚合核心](02-provider-aggregation.md) | In progress | 多 Family、确定性路由和 conformance |
| C3 | [Responses → Chat Bridge](03-responses-to-chat.md) | Not started | Codex 使用 Chat-only Provider |
| C4 | [Chat → Responses Bridge](04-chat-to-responses.md) | Not started | Hermes Chat 使用 Responses-only Provider |
| C5 | [异构 Provider 验证](05-heterogeneous-provider.md) | Not started | 非 OpenAI dialect 反证抽象 |
| C6 | [核心接受](06-core-acceptance.md) | Blocked | 可发布、可复现、可回滚的核心版本 |
| E1+ | [核心后增强](enhancements.md) | Deferred | usage、health、tools、OAuth、UI |

功能职责见[功能模块索引](../modules/README.md)。详细研究工作流仍见[开发与调研收敛计划](../plans/development-plan.md)。
