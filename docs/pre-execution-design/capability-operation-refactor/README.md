# Operation 与多模态 capability 重构

> **状态：保留的执行前设计，不构成继续实施授权。** 本包只定义目标结构、阶段依赖和执行门，不证明代码已经实现目标结构；当前事实以 implementation status 与 live source 为准。

## 目标

让 OpenBridge 按 model-bound operation 持续增加 Provider、模型和多模态能力，同时保留静态可信装配、单 task executable profile、固定
Public Model 合同、保守交集、zero-egress preflight 和受控 retry/fallback。

## 固定边界

- Canonical executable profile 始终只有一个 task；请求不能按 shape 选择 task。
- Target 内 Upstream API 使用 typed `(operation, task)` key；每个 Public operation interface 显式绑定一个 task。
- Private registry 先改为 operation-indexed；公共扩展 Models 暂时继续输出唯一的 schema v1。
- Files/Uploads、异步资源、Video job 与 Realtime session 不进入本轮 model-bound operation 架构。
- Typed file input 与首个真实新 operation 分别作为媒体层和 operation 扩展的纵向证明。

## 阅读顺序

1. [范围、术语与不变量](00-scope-and-invariants.md)
2. [目标架构](01-target-architecture.md)
3. [阶段 0：合同与基线](02-stage-0-contract-and-baseline.md)
4. [阶段 1：Operation kernel 与 task-explicit API](03-stage-1-operation-and-model.md)
5. [阶段 2：Media profiles 与 Provider catalog](04-stage-2-media-and-provider.md)
6. [阶段 3：Operation-indexed Registry 与稳定 Models 投影](05-stage-3-registry-and-models.md)
7. [阶段 4：Operation-first pipeline 与共享 execution](06-stage-4-pipeline-and-adapters.md)
8. [阶段 5：两次纵向证明与收口](07-stage-5-proof-and-cleanup.md)
9. [测试、证据与执行准备](08-testing-evidence-and-readiness.md)
10. [后续决策门](09-open-questions.md)

## 阶段依赖

```text
阶段 0：冻结合同和基线
    ↓
阶段 1：Operation kernel + task-explicit API key
    ↓
阶段 2：Typed media + Provider-local profiles
    ↓
阶段 3：Private operation-indexed registry + Models v1 projection
    ↓
阶段 4：Operation-first pipeline + shared execution
    ↓
阶段 5A：Typed file input
    ↓
阶段 5B：首个真实新 operation + legacy 清理
```

阶段表示依赖顺序，不是并行 roadmap。一次只允许一个可观察切片进入 `current-focus.md`，并在该切片内直接替换旧结构，禁止长期双路径。

## 依据

- [产品范围](../../functional-requirements/product-scope/README.md)
- [模型能力域](../../functional-requirements/model-capability/README.md)
- [扩展能力共同规则](../../functional-requirements/extended-capabilities/README.md)
- [当前架构](../../implementation-status/current-architecture.md)
- [当前测试资产](../../implementation-status/test-assets/protocol-corpus.md)

开始任一阶段前必须重新读取 live source、工作树和当前需求；源码与测试事实优先于本设计包。
