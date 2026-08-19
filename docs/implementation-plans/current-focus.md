# 当前开发焦点

## 状态

**进行中：Operation/capability 重构阶段 0A——收紧 synthetic generation fixture。**

## 当前焦点

### 可观察行为

- 共享 synthetic Provider 的 Chat/Responses API 默认只允许普通非流式文本请求；streaming、stream usage、function tools 和 Responses
  terminal usage 必须由具体测试显式开启。
- 已有 wire、Bridge、lifecycle 和 observability 测试保持行为不变，但不再从宽松 fixture 隐式获得能力。

### 需求与先失败测试

- 需求来源：已批准的 capability-operation-refactor 阶段 0，以及用户要求只保留必要测试并分步提交。
- 一个 fixture 合同测试先证明当前宽松默认仍隐式开放 streaming/tools/terminal usage，再修改 fixture 和必要调用点。

### 非目标

- 不修改生产 Provider、Model、Target、Public Models schema 或客户端协议行为。
- 不在本切片引入 operation/task 新类型，不增加模型字段或具体 Target catalog 快照测试。

### 验证边界

- 先运行 fixture 合同测试和受影响 integration crates；通过后运行完整 Rust 基线与 `git diff --check`。
- 不读取私有 credential，不运行真实 Provider、外部 SDK、负载或长期测试。