# 当前开发焦点

## 状态

**进行中：Operation/capability 重构阶段 2C——source-owned audio input profiles。**

## 当前焦点

### 可观察行为

- remote URL、data URL 与 pure Base64 audio source 各自拥有 formats 和 limits；source absence 不再由零值表示。
- request preflight 与 Models v1 继续从同一 private profile 派生，现有 MiMo wire 与公开 JSON 保持不变。

### 需求与测试

- 需求来源：已批准的 capability-operation-refactor 阶段 2，以及用户要求仅保留必要测试并分步提交。
- 只新增一个 source-owned profile algebra 测试和一个 mixed-source cumulative budget 回归测试；复用现有 MiMo、Models 和 zero-egress tests。

### 非目标

- 不开放新 audio source/format，不改变 MiMo limits、Provider catalog、Route 或模型字段。
- core media 模块拆分与 Provider-local media 文件迁移在后续机械切片完成。

### 验证边界

- 运行新增 algebra 测试、MiMo focused tests，再运行完整 Rust 基线与 `git diff --check`。
- 不读取私有 credential，不运行真实 Provider、外部 SDK、负载或长期测试。