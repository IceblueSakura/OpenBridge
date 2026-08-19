# 当前开发焦点

## 状态

**进行中：Operation/capability 重构阶段 1D——selected task propagation。**

## 当前焦点

### 可观察行为

- 每个 private Public Model operation interface 显式保存 selected canonical task。
- Route/planning candidates 携带完整 `UpstreamApiKey`，forwarding 不再从 Target task 和 operation 重建 identity；客户端 wire、Route 顺序与 Models v1 不变。

### 需求与先失败测试

- 需求来源：已批准的 capability-operation-refactor 阶段 1，以及用户要求只保留必要测试并分步提交。
- 复用现有 task mismatch、MiMo task-specific wire、forwarding 与 Embeddings 测试；不增加重复测试。

### 非目标

- 不把 Public Model interface 容器改成 operation-indexed set，不改变 attempt lifecycle 或公共 schema；这些属于后续切片。
- 不增加新 operation、task、模型字段、模态或具体 Target catalog 快照测试。

### 验证边界

- 先运行 config/registry focused tests；通过后运行完整 Rust 基线与 `git diff --check`。
- 不读取私有 credential，不运行真实 Provider、外部 SDK、负载或长期测试。