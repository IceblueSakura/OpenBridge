# 阶段契约索引

阶段按证据门推进，不按代码量或日期判断完成。阶段文档属于需求层，只保留进入条件、目标、范围、非目标、测试和退出条件；它们不是多阶段待办。

任一时刻最多只有一个核心阶段为 `Active`。既有原型可能提前覆盖后续阶段的部分能力，但不会因此改变阶段状态或进入当前计划。具体规则见[需求索引与阶段治理](../requirements/README.md)。

| 阶段 | 文档 | 当前状态 | 核心结果 |
|---|---|---|---|
| C0 | [范围与客户端契约](00-scope-and-client-contracts.md) | Active | 固定 Codex/Hermes 契约和 corpus |
| C1 | [双 Native Path](01-native-paths.md) | Blocked by C0 | 两个目标客户端原生工具循环 |
| C2 | [Provider 聚合核心](02-provider-aggregation.md) | Blocked by C1 | 多 Family、确定性路由和 conformance |
| C3 | [Responses → Chat Bridge](03-responses-to-chat.md) | Blocked by C2 | Codex 使用 Chat-only Provider |
| C4 | [Chat → Responses Bridge](04-chat-to-responses.md) | Blocked by C3 | Hermes Chat 使用 Responses-only Provider |
| C5 | [异构 Provider 验证](05-heterogeneous-provider.md) | Blocked by C4 | 非 OpenAI dialect 反证抽象 |
| C6 | [核心接受](06-core-acceptance.md) | Blocked by C1–C5 | 可发布、可复现、可回滚的核心版本 |
| E1+ | [核心后增强](enhancements.md) | Deferred | usage、health、tools、OAuth、UI |

功能职责见[功能模块索引](../modules/README.md)。跨阶段目标和证据门见[阶段交付与研究需求](../requirements/delivery-requirements.md)；可执行内容只见[当前阶段实施计划](../plans/implementation-plan.md)。
