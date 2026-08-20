# 当前开发焦点

## 状态

**进行中：Operation/capability 重构阶段 4A——top-level attempt coordinator。**

## 当前焦点

### 可观察行为

- bounded request/candidate attempt budget、backoff 与 retry/fallback step 由顶层 `execution::AttemptCoordinator` 唯一拥有。
- Generation 与 Embeddings 的 attempt 数、candidate 顺序、backoff、credential rotation、fallback、commit 和 cancellation 行为不变。

### 需求与测试

- 需求来源：已批准的 capability-operation-refactor 阶段 4 实施顺序第 1 项。
- 不新增测试；先让现有 attempt state-machine tests 指向新 API 并确认 RED，再复用完整 resilience/Embeddings/cancellation tests。

### 非目标

- 本切片不合并 forwarding loop，不移动 response/commit 逻辑，不改变 retry policy、limits、Provider adapter 或 wire。

### 验证边界

- 运行 attempt unit tests、Generation resilience、Embeddings retry/cancel focused tests，再运行完整 Rust 基线与 `git diff --check`。
- 不读取私有 credential，不运行真实 Provider、外部 SDK、负载或长期测试。