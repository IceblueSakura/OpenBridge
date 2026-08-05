# 实施计划

本目录只有[当前开发焦点](current-focus.md)可以表示已获准实施的短周期行为。独立的
[测试补全计划](test-coverage-completion-plan.md)只保存测试缺口、候选顺序和验证边界，不表示其中任一条目已经
进入实施；开始工作前仍必须把单个可观察行为写入当前焦点。

除这份明确维护的测试补全清单外，当前没有获准实施的功能时，不保存其他远期设计、候选类型、阶段路线图或 已经完成的架构说明。

开始下一项工作时：

1. 先读取 live source、工作区状态、[功能需求](../functional-requirements/README.md)
   与[实施现状](../implementation-status/README.md)；
2. 只选择一个可观察行为，在 `current-focus.md` 写明需求、失败测试、不做项和验证边界；
3. 实现并验证后，将事实写入 `implementation-status/`，再把 `current-focus.md` 恢复为空焦点；
4. 没有进入当前焦点的条目不构成实施授权；测试补全清单只用于选择下一项行为，不得并行展开多个条目；
5. 测试补全结束后删除候选清单，把实际证据保留在 `implementation-status/`。
