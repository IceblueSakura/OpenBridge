# OpenBridge 文档索引

文档只按当前用途组织，不按运行时模块、阶段或目标变迁拆分：

| 分类 | 回答的问题 | 入口 |
|---|---|---|
| [实施现状](implementation-status/README.md) | 当前代码和验证实际证明了什么 | 已实现行为、能力探测与验证记录 |
| [实施计划](implementation-plans/README.md) | 当前是否有一个获准实施的短周期行为 | 仅保留当前开发焦点 |
| [功能需求](functional-requirements/README.md) | 产品当前应保持什么行为和边界 | 产品范围、网关 API、配置凭证、路由韧性与交付证据 |
| [参考文档](references/README.md) | 外部协议和参考项目提供了什么事实 | OpenAI 协议与外部项目调研 |

## 建议阅读顺序

1. [功能需求](functional-requirements/README.md)
2. [当前代码架构](implementation-status/current-architecture.md)和[实施现状](implementation-status/README.md)
3. 开始实现前核对[当前开发焦点](implementation-plans/current-focus.md)
4. 需要核验外部事实时再查阅[参考文档](references/README.md)

独立协议测试数据的日常使用与维护，直接从仓库内的 [Corpus 指南](../testdata/README.md) 和 [Testkit 指南](../tools/corpus/README.md) 开始；已验证证据与尚未接入边界记录在[实施现状](implementation-status/protocol-test-corpus.md)。

## 维护规则

- 产品行为、边界或非目标变化：更新 `functional-requirements/`；
- 已实现行为或已完成验证变化：更新 `implementation-status/`；
- 下一个功能获准实施：只更新 `implementation-plans/current-focus.md`；完成后将它恢复为空焦点；
- 外部协议、SDK、目标客户端或参考项目事实变化：更新 `references/`，并按影响同步前述三类文档；
- 不保留远期设计、阶段路线图、目标变迁或淘汰方案；需要实施时再从当前源码建立焦点。
