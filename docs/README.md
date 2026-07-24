# OpenBridge 文档索引

文档按两条主线组织：

1. [功能模块](modules/README.md)：回答系统由哪些模块组成、每个模块支持什么；
2. [实施阶段](phases/README.md)：回答按什么顺序开发、每个阶段测试什么、何时完成。

## 建议阅读顺序

1. [功能模块索引](modules/README.md)
2. [实施阶段索引](phases/README.md)
3. [当前实现说明](implementation/current-implementation.md)
4. 与当前任务相关的专项架构、设计、研究或规范文档

## 专项目录

| 目录 | 作用 | 入口 |
|---|---|---|
| `modules/` | 当前功能边界、模块职责和模块验收 | [模块索引](modules/README.md) |
| `phases/` | C0–C6 与增强阶段的目标、测试和退出条件 | [阶段索引](phases/README.md) |
| `requirements/` | 产品范围和增强需求 | [核心需求](requirements/proxy-requirements.md)、[Hosted tool/MCP](requirements/hosted-tools-mcp.md) |
| `architecture/` | 系统、Provider、配置、路由和数据流详细设计 | [架构索引](architecture/README.md) |
| `design/` | 客户端契约、协议桥和凭证专项设计 | [设计索引](design/README.md) |
| `implementation/` | 当前代码已经验证的行为 | [当前实现](implementation/current-implementation.md) |
| `plans/` | 详细研究工作流和 gate 依赖 | [开发计划](plans/development-plan.md) |
| `experiments/` | 实验模板、wire evidence 和证明边界 | [实验索引](experiments/README.md) |
| `research/` | 参考项目源码事实和失败反例 | [调研索引](research/README.md) |
| `specifications/` | 外部 API 协议学习资料 | [规范索引](specifications/README.md) |

## 维护规则

- 功能范围变化：更新对应 `modules/` 文档和需求文档；
- 实施顺序、测试或退出条件变化：更新对应 `phases/` 文档；
- 已实现行为变化：更新 `implementation/current-implementation.md`；
- 专项架构变化：更新 `architecture/` 或 `design/`；
- 外部事实变化：更新 `research/` 或 `specifications/`；
- 非平凡实验：在 `experiments/` 创建可复现记录；
- 文档只保留当前有效结论，不保留需求变更日志、审计过程或已被替代的方案正文。
