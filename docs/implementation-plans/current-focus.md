# 当前开发焦点

## 状态

**进行中：Operation/capability 重构阶段 2C——unified registry media contract。**


## 当前焦点

### 可观察行为

- Route contribution、aggregate 与 private preflight contract 使用一个完整 media envelope；Bridge 统一贡献 empty media。
- Models v1 JSON、现有 image/audio preflight 与 Provider wire 保持不变；file 仍不进入公共 wire。

### 需求与测试

- 需求来源：已批准的 capability-operation-refactor 阶段 2，以及用户要求仅保留必要测试并分步提交。
- 复用现有 MiMo、Bridge、Models 和 zero-egress 合同，不新增具体模型或 Target 快照测试。

### 非目标

- 本切片不开放 file input、不改变 Provider catalog、不重排 Route，也不修改模型字段。
- audio source-owned payload 与模块拆分在后续 2C 切片完成。

### 验证边界

- 运行 MiMo、Bridge、Models focused tests，再运行完整 Rust 基线与 `git diff --check`。
- 不读取私有 credential，不运行真实 Provider、外部 SDK、负载或长期测试。