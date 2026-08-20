# 当前开发焦点

## 状态

**进行中：Operation/capability 重构阶段 2C——media ownership and module split。**

## 当前焦点

### 可观察行为

- Provider media constants 与 named Target profiles 由同 Provider 的 `media.rs` 拥有。
- core generation media 类型迁入专属子模块；公开 crate paths、Models v1 与 Provider wire 保持不变。

### 需求与测试

- 需求来源：已批准的 capability-operation-refactor 阶段 2，以及用户要求仅保留必要测试并分步提交。
- 纯机械所有权迁移不新增测试；复用阶段 2 已有 algebra、MiMo、Models、Bridge 和 zero-egress tests。

### 非目标

- 不开放新 media capability，不改变 limits、Provider catalog、Target 选择、Route 或模型字段。
- 不拆分 tools/reasoning 等非 media 域，不修改公共 schema。

### 验证边界

- 运行新增 algebra 测试、MiMo focused tests，再运行完整 Rust 基线与 `git diff --check`。
- 不读取私有 credential，不运行真实 Provider、外部 SDK、负载或长期测试。