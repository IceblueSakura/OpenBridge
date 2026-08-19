# 执行前设计

本目录只保存用户明确要求保留、用于继续调整和执行前准备的设计包。它与功能需求、实施现状和当前开发焦点分离：

- 设计包描述候选目标结构、阶段依赖、风险和待决问题，不证明代码已实现任何行为；
- 设计包不构成实施授权，不得据此直接修改运行时行为；
- 真正实施前，必须从 live source 重新建立基线，只选择一个可观察切片进入
  [`implementation-plans/current-focus.md`](../implementation-plans/current-focus.md)；
- 实施完成后的确认事实进入 [`implementation-status/`](../implementation-status/)，而不是回写成设计完成历史；
- 功能行为与安全边界仍只由 [`functional-requirements/`](../functional-requirements/) 拥有。

## 当前设计包

- [Operation、多 task 与多模态 capability 重构](capability-operation-refactor/README.md)：面向个人多功能 OpenAI-compatible
  网关的破坏性架构迁移分析。

只有在用户明确要求时才新增、替换或删除本目录的设计包。过时包必须标明状态或删除，不能与 live implementation status 并列作为事实来源。
