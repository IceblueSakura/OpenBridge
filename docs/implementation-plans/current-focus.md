# 当前开发焦点

## 状态

**进行中：Operation/capability 重构阶段 1C——operation-tagged Provider ceilings。**

## 当前焦点

### 可观察行为

- Provider contract 使用闭合的 operation-tagged capability set，不再把 Chat/Responses/Embeddings 固定为结构字段。
- Target subset validation 通过 `OperationKind` 读取同一 Provider ceiling；客户端 wire、Route 顺序与 Models v1 不变。

### 需求与先失败测试

- 需求来源：已批准的 capability-operation-refactor 阶段 1，以及用户要求只保留必要测试并分步提交。
- 复用现有 capability elevation、Provider contract 和完整 registry 测试；不增加 Provider catalog 快照测试。

### 非目标

- 不改变 Public Model interface 容器、pipeline/attempt lifecycle 或公共 schema；这些属于后续切片。
- 不增加新 operation、task、模型字段、模态或具体 Target catalog 快照测试。

### 验证边界

- 先运行 config/registry focused tests；通过后运行完整 Rust 基线与 `git diff --check`。
- 不读取私有 credential，不运行真实 Provider、外部 SDK、负载或长期测试。