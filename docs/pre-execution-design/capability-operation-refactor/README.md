# Operation 与多模态 capability 后续扩展

> **状态：保留的未完成执行前设计，不构成继续实施授权。** 阶段 0–5A 已完成，不在本包保留实施历史；当前事实以 implementation status 与 live source 为准。

## 保留范围

本包只保留尚未实施的阶段 5B 及其后续决策：

1. [阶段 5B：首个真实新 operation 与 legacy 收口](07-stage-5-proof-and-cleanup.md)
2. [测试、证据与执行准备](08-testing-evidence-and-readiness.md)
3. [后续决策门](09-open-questions.md)

阶段 5A typed file input 的当前实现见[Native 文件输入](../../implementation-status/features/native-file-input.md)，不再作为待执行计划保存。

## 固定边界

- Canonical executable profile 始终只有一个 task；请求不能按 shape 选择 task。
- Target 内 Upstream API 使用 typed `(operation, task)` key；每个 Public operation interface 显式绑定一个 task。
- Files/Uploads、异步资源、Video job 与 Realtime session 不进入 model-bound operation 架构。
- 一次只允许一个可观察切片进入 `current-focus.md`，并在该切片内直接替换旧结构，禁止长期双路径。

## 依据

- [产品范围](../../functional-requirements/product-scope/README.md)
- [模型能力域](../../functional-requirements/model-capability/README.md)
- [扩展能力共同规则](../../functional-requirements/extended-capabilities/README.md)
- [当前架构](../../implementation-status/current-architecture.md)
- [当前测试资产](../../implementation-status/test-assets/protocol-corpus.md)

开始任一切片前必须重新读取 live source、工作树和当前需求；源码与测试事实优先于本设计包。