# OpenBridge 文档索引

文档只按当前用途组织，不按运行时模块、阶段或目标变迁拆分：

| 分类 | 回答的问题 | 入口 |
|---|---|---|
| [实施现状](implementation-status/README.md) | 当前代码和验证实际证明了什么 | 已实现行为、能力探测与验证记录 |
| [实施计划](implementation-plans/README.md) | 一个功能接下来准备如何实现、验证和约束 | 当前焦点、架构、客户端、协议、配置与凭证方案 |
| [功能需求](functional-requirements/README.md) | 产品应提供什么行为、边界和证据要求 | 产品范围、网关 API/兼容、配置与凭证、路由韧性、观测、工具与交付要求 |
| [参考文档](references/README.md) | 外部协议和参考项目提供了什么事实 | OpenAI 协议与外部项目调研 |

## 建议阅读顺序

1. [功能需求](functional-requirements/README.md)
2. [当前代码架构](implementation-status/current-architecture.md)和[实施现状](implementation-status/README.md)
3. 与当前功能相关的[实施计划](implementation-plans/README.md)；架构演进的依赖和切片统一见[架构迁移总计划](implementation-plans/registry-architecture-migration.md)
4. 需要核验外部事实时再查阅[参考文档](references/README.md)

独立协议测试数据的日常使用与维护，直接从仓库内的 [Corpus 指南](../testdata/README.md) 和 [Testkit 指南](../tools/corpus/README.md) 开始；它们说明实际命令、数据模型、Mock Server/Client 和发布流程。设计假设仍保留在[实施计划](implementation-plans/README.md)，已验证证据仍保留在[实施现状](implementation-status/README.md)。

## 维护规则

- 产品行为、边界或非目标变化：更新 `functional-requirements/`；
- 已实现行为或已完成验证变化：更新 `implementation-status/`；
- 下一个功能的实现假设、最小边界或验证方式变化：更新 `implementation-plans/`；
- 外部协议、SDK、目标客户端或参考项目事实变化：更新 `references/`，并按影响同步前述三类文档；
- 每份文档都按功能命名；不新增按运行时模块、阶段编号、目标变迁或淘汰方案组织的文档。
