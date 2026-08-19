# Operation、多 task 与多模态 capability 重构

> **状态：执行前设计，未获准实施。** 本包整理 2026-08-19 对当前源码、需求、状态页和测试边界的分析，供继续调整与执行前准备。
> 它不表示任一阶段已经进入 `current-focus.md`，也不证明目标结构已经实现。

## 目标

把 OpenBridge 重构为长期可扩展的个人多功能 OpenAI-compatible 网关：持续增加 Provider、模型、operation 与 image/audio/file/未来
video 等多模态能力，同时保留静态可信装配、固定 Public Model 接口、保守交集、zero-egress preflight 和受控 retry/fallback。

## 阅读顺序

1. [范围、术语与不变量](00-scope-and-invariants.md)
2. [目标架构](01-target-architecture.md)
3. [阶段 0：合同与基线](02-stage-0-contract-and-baseline.md)
4. [阶段 1：Operation kernel 与多 task Model](03-stage-1-operation-and-model.md)
5. [阶段 2：Media profiles 与 Provider catalog](04-stage-2-media-and-provider.md)
6. [阶段 3：Registry compiler 与 Models v2](05-stage-3-registry-and-models.md)
7. [阶段 4：Operation-first pipeline 与 Provider adapter](06-stage-4-pipeline-and-adapters.md)
8. [阶段 5：纵向证明、清理与收口](07-stage-5-proof-and-cleanup.md)
9. [测试、证据与执行准备](08-testing-evidence-and-readiness.md)
10. [开放问题与决策门](09-open-questions.md)

## 阶段依赖

```text
阶段 0：冻结不变量和基线
    ↓
阶段 1：Operation kernel + Canonical task set
    ↓
阶段 2：Typed media + Provider-local profiles
    ↓
阶段 3：Operation-indexed registry + Models v2
    ↓
阶段 4：Operation-first pipeline + adapter/attempt
    ↓
阶段 5：首个新能力纵向证明 + legacy 清理
```

阶段表示依赖顺序，不是并行 roadmap。一次只允许一个可观察切片进入 `current-focus.md`。同一阶段可以拆成多个短周期，但不得让旧/新运行路径长期并存。

## 当前依据

- 产品范围：[`functional-requirements/product-scope/README.md`](../../functional-requirements/product-scope/README.md)
- 模型能力域：[`functional-requirements/model-capability/README.md`](../../functional-requirements/model-capability/README.md)
- 扩展能力共同规则：[`functional-requirements/extended-capabilities/README.md`](../../functional-requirements/extended-capabilities/README.md)
- 当前架构：[`implementation-status/current-architecture.md`](../../implementation-status/current-architecture.md)
- 当前测试资产：[`implementation-status/test-assets/protocol-corpus.md`](../../implementation-status/test-assets/protocol-corpus.md)

源码和测试始终优先于本设计包。开始任一阶段前必须重新读取 live source、工作树和当前需求。
