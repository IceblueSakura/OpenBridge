# 当前开发焦点

## 状态

**进行中：Operation/capability 重构阶段 4B3a——Embeddings prepared-candidate runner。**

## 当前焦点

### 可观察行为

- Embeddings handler 只拥有 analysis、planning 与 trusted candidate preparation；attempt loop 移入 `ingress/forwarding/execution.rs`。
- handler 创建并传入 request-wide `AttemptCoordinator`；credential rotation、retry、backoff、commit 与 cancellation 行为不变。

### 需求与测试

- 需求来源：已批准的 capability-operation-refactor 阶段 4 Embeddings migration 与 shared execution 顺序。
- 不新增测试；先声明尚不存在的 forwarding execution owner 并确认 compile RED，再复用完整 Embeddings retry/cancel contracts。

### 非目标

- 本切片不迁移 Generation loop，不泛化 trait/driver，不改变 retry policy、health、limits、Provider adapter 或 wire。

### 验证边界

- 运行 Embeddings forwarding/config focused tests，再运行完整 Rust 基线与 `git diff --check`。
- 不读取私有 credential，不运行真实 Provider、外部 SDK、负载或长期测试。