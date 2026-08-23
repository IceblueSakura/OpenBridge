# Operation 与 capability 后续决策

> **状态：候选执行前设计，不构成继续实施授权。** 已完成的 Images 证明、Stages 1–7 和 legacy cleanup 历史不在本包保留；
> 当前实现事实见 [Native Images 实施现状](../../implementation-status/features/native-images-generation.md)与
> [当前架构](../../implementation-status/current-architecture.md)。

## 保留范围

1. [测试、证据与执行准备](testing-evidence-and-readiness.md)：后续 operation/profile 的可复用证明边界；
2. [后续决策门](future-decision-gates.md)：Models v2、shared identity、resource ledger、manifest 与 property testing 等尚未批准决策。

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
- [Native file input 实施现状](../../implementation-status/features/native-file-input.md)
- [Native Images 实施现状](../../implementation-status/features/native-images-generation.md)
- [当前测试资产](../../implementation-status/test-assets/protocol-corpus.md)

开始任一未来切片前必须重新读取 live source、工作树和当前需求；源码与测试事实优先于本设计包。