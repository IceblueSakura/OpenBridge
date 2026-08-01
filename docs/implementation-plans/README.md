# 实施计划

本目录只保留一个短周期的[当前开发焦点](current-focus.md)。当前没有获准实施的功能时，不保存远期设计、
候选类型、阶段路线图或已经完成的架构说明。

开始下一项工作时：

1. 先读取 live source、工作区状态、[功能需求](../functional-requirements/README.md)与[实施现状](../implementation-status/README.md)；
2. 只选择一个可观察行为，在 `current-focus.md` 写明需求、失败测试、不做项和验证边界；
3. 实现并验证后，将事实写入 `implementation-status/`，再把 `current-focus.md` 恢复为空焦点；
4. 没有进入当前焦点的想法不构成计划，也不单独建文档。
