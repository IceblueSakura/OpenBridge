# 执行前设计

本目录只保存用户明确要求继续保留、用于未来决策和执行准备的设计资料。它与功能需求、实施现状和当前开发焦点分离：

- 设计资料不证明代码已实现任何行为，也不构成实施授权；
- 真正实施前必须从 live source 重新建立基线，并将单一可观察切片写入
  [`implementation-plans/current-focus.md`](../implementation-plans/current-focus.md)；
- 已完成切片的事实只进入 [`implementation-status/`](../implementation-status/)，不在本目录保留完成历史；
- 产品行为与安全边界仍只由 [`functional-requirements/`](../functional-requirements/) 拥有。

## 当前保留资料

- [Operation 与 capability 后续决策](capability-operation-refactor/README.md)：保留未来 decision gates 与可复用的测试、证据和执行准备边界。

Stages 1–7 的执行顺序、详细计划和已实现设计已在完成后删除；对应当前行为、owner 与验证证据见
[`implementation-status/`](../implementation-status/README.md)。新增、替换或删除本目录资料仍须用户明确授权。
