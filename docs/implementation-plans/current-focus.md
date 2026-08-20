# 当前开发焦点

## 状态

**进行中：Operation/capability 重构阶段 4C1——Generation prepared-candidate runner。**

## 当前焦点

### 可观察行为

- Generation handler 保留 analysis、planning、Route order、cross-candidate health skip 与 final failure；candidate-internal loop 移入 `ingress/forwarding/execution.rs`。
- request-wide `AttemptCoordinator` 继续由外层 handler 创建并跨 candidates 共享；OAuth、rotation、retry、fallback、commit 与 cancellation 行为不变。

### 需求与测试

- 需求来源：已批准的 capability-operation-refactor 阶段 4 Generation migration 顺序。
- 不新增测试；先引用尚不存在的 generation runner API 并确认 compile RED，再复用 Generation resilience/OAuth/Bridge contracts。

### 非目标

- 本切片只机械共置两个 operation loop，不合并其 policy 分支，不改变 health、limits、Provider adapter 或 wire。

### 验证边界

- 运行 Generation resilience、OAuth、Bridge focused tests，再运行完整 Rust 基线与 `git diff --check`。
- 不读取私有 credential，不运行真实 Provider、外部 SDK、负载或长期测试。