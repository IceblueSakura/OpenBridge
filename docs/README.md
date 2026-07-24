# OpenBridge 文档索引

文档按三层事实和一份当前计划组织：

1. [需求](requirements/README.md)：回答产品与各阶段必须达成什么；
2. [功能模块](modules/README.md)：回答系统由哪些模块组成、每个模块支持什么；
3. [当前实现](implementation/current-implementation.md)：回答代码已经证明了什么；
4. [当前阶段计划](plans/implementation-plan.md)：只回答唯一 `Active` 阶段怎样完成。

## 建议阅读顺序

1. [需求索引与阶段治理](requirements/README.md)
2. [阶段契约索引](phases/README.md)
3. [功能模块索引](modules/README.md)
4. [当前实现说明](implementation/current-implementation.md)
5. [当前阶段实施计划](plans/implementation-plan.md)
6. 与当前任务相关的专项架构、设计、研究或规范文档

## 专项目录

| 目录 | 作用 | 入口 |
|---|---|---|
| `modules/` | 当前功能边界、模块职责和模块验收 | [模块索引](modules/README.md) |
| `phases/` | C0–C6 的进入条件、目标、非目标、测试和退出条件 | [阶段契约索引](phases/README.md) |
| `requirements/` | 产品范围、阶段交付要求和增强需求 | [需求索引](requirements/README.md) |
| `architecture/` | 系统、Provider、配置、路由和数据流详细设计 | [架构索引](architecture/README.md) |
| `design/` | 客户端契约、协议桥和凭证专项设计 | [设计索引](design/README.md) |
| `implementation/` | 当前代码已经验证的行为 | [当前实现](implementation/current-implementation.md) |
| `plans/` | 唯一 `Active` 阶段的可执行计划 | [计划目录](plans/README.md)、[当前阶段计划](plans/implementation-plan.md) |
| `experiments/` | 实验模板、wire evidence 和证明边界 | [实验索引](experiments/README.md) |
| `research/` | 参考项目源码事实和失败反例 | [调研索引](research/README.md) |
| `specifications/` | 外部 API 协议学习资料 | [规范索引](specifications/README.md) |

## 维护规则

- 产品范围变化：更新 `requirements/` 和对应 `modules/` 文档；
- 阶段目标、依赖、测试或退出条件变化：更新阶段交付需求和对应 `phases/` 文档；
- 当前阶段的任务分解变化：只更新 `plans/implementation-plan.md`，不得追加后续阶段；
- 已实现行为变化：更新 `implementation/current-implementation.md`；
- 专项架构变化：更新 `architecture/` 或 `design/`；
- 外部事实变化：更新 `research/` 或 `specifications/`；
- 非平凡实验：在 `experiments/` 创建可复现记录；
- 文档只保留当前有效结论，不保留需求变更日志、审计过程或已被替代的方案正文。
