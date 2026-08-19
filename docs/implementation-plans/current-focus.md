# 当前开发焦点

## 状态

**进行中：Operation/capability 重构阶段 1B——typed Generation Bridge direction。**

## 当前焦点

### 可观察行为

- `RouteMode` 直接携带 Generation Bridge direction，不再由 downstream/upstream operation pair 在请求路径隐式推断方向。
- registry 启动编译验证 direction 与两端 operation 完全一致；客户端 wire、Route 顺序与 Models v1 不变。

### 需求与先失败测试

- 需求来源：已批准的 capability-operation-refactor 阶段 1，以及用户要求只保留必要测试并分步提交。
- 复用现有 invalid Bridge Route 测试验证显式 direction mismatch；不增加重复 Bridge wire 测试。

### 非目标

- 不改变 Provider ceiling、Public Model interface 容器、attempt lifecycle 或公共 schema；这些属于后续切片。
- 不增加新 operation、task、模型字段、模态或具体 Target catalog 快照测试。

### 验证边界

- 先运行 config/registry focused tests；通过后运行完整 Rust 基线与 `git diff --check`。
- 不读取私有 credential，不运行真实 Provider、外部 SDK、负载或长期测试。