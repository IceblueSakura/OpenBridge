# 执行前设计

本目录只保存用户明确要求保留、用于继续调整和执行前准备的设计包。它与功能需求、实施现状和当前开发焦点分离：

- 设计包描述候选目标结构、阶段依赖、风险和待决问题，不证明代码已实现任何行为；
- 设计包不构成实施授权，不得据此直接修改运行时行为；
- 真正实施前，必须从 live source 重新建立基线，只选择一个可观察切片进入
  [`implementation-plans/current-focus.md`](../implementation-plans/current-focus.md)；
- 实施完成后的确认事实进入 [`implementation-status/`](../implementation-status/)，而不是回写成设计完成历史；
- 功能行为与安全边界仍只由 [`functional-requirements/`](../functional-requirements/) 拥有。

## 待执行顺序

[Generation 与 operation 剩余实现计划顺序](implementation-sequence.md)是本目录唯一的顺序与依赖 owner。它把下面的详细设计拆成
一次只可提升一个的可观察切片；实际获准项仍只以
[`implementation-plans/current-focus.md`](../implementation-plans/current-focus.md)为准，前一项完成不自动授权后一项。

## 详细设计包

- [Responses `reasoning.encrypted_content` 兼容提示设计](responses-reasoning-encrypted-content-compatibility.md)：阶段 1 区分下游安全接受与
  candidate 原生转发，只为该精确 hint 定义条件转发/删除，并由 planning 在 Bridge 前统一省略。
- [Generation capability 错误定位设计](generation-capability-error-diagnostics.md)：阶段 2 保留 fail-closed 与零 egress，将泛化
  capability 400 收敛为确定性的字段级错误。
- [Operation 与多模态 capability 剩余收口](capability-operation-refactor/README.md)：阶段 4–7 只保留 timeout/lifecycle、测试证明、profile algebra 和 legacy 清理。

## 阶段实施计划

[Generation 与 operation 剩余实现计划顺序](implementation-sequence.md)是七份阶段计划的 parent index。具体计划位于
[`implementation-plans/`](implementation-plans/)，分别拥有 scope、RED、实施步骤、非目标、验证和退出门；这些文件仍属于执行前设计，
只有被复制并收窄到 `current-focus.md` 的一个阶段才构成实施授权。

只有在用户明确要求时才新增、替换或删除本目录的设计包。过时包必须标明状态或删除，不能与 live implementation status 并列作为事实来源。
