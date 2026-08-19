# 当前开发焦点

## 状态

**进行中：Operation/capability 重构阶段 1A——typed Upstream API key。**

## 当前焦点

### 可观察行为

- 每个 `UpstreamApiConfig` 显式携带 typed `(operation, task)` identity；operation/profile/task 不一致在启动时失败。
- runtime Target 使用同一 typed key 建立 API 索引，已解析 `UpstreamApi` 保留 selected task identity；客户端 wire、Route 顺序与 Models v1 不变。

### 需求与先失败测试

- 需求来源：已批准的 capability-operation-refactor 阶段 1，以及用户要求只保留必要测试并分步提交。
- 复用 duplicate API 测试并增加一个 explicit task/canonical model mismatch 测试；不复制 Provider/Target catalog 快照。

### 非目标

- 不改变 `RouteMode`、Pipeline、Bridge direction、Provider ceiling 表示或公共 schema；这些属于后续切片。
- 不增加新 operation、task、模型字段、模态或具体 Target catalog 快照测试。

### 验证边界

- 先运行 config/registry focused tests；通过后运行完整 Rust 基线与 `git diff --check`。
- 不读取私有 credential，不运行真实 Provider、外部 SDK、负载或长期测试。