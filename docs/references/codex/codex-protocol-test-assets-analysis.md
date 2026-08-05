# Codex Responses 与工具生命周期测试资产调研

## 状态与来源

- 在线复核日期：2026-07-26；当前源码模块级复核 commit `ee0247f95a6fe2b094ba2253d82cae2a2b4c2dff`
- 来源：[Responses helpers](https://github.com/openai/codex/blob/ee0247f95a6fe2b094ba2253d82cae2a2b4c2dff/codex-rs/core/tests/common/responses.rs)、[`tool_parallelism.rs`](https://github.com/openai/codex/blob/ee0247f95a6fe2b094ba2253d82cae2a2b4c2dff/codex-rs/core/tests/suite/tool_parallelism.rs)

## 观察事实

- `tests/common/responses.rs` 提供 Responses event/SSE 构造、request capture、function/custom tool item 和 terminal/error fixture。
- tool parallelism tests 检查多个工具是否并发启动、function calls 与 outputs 顺序，以及 output 是否按 `call_id` 匹配。
- 部分场景控制 `response.completed` 的释放时机，验证工具可以在完整响应 terminal 之前开始。
- 测试目标是 Codex client runtime，主要覆盖 Responses 消费侧和 Agent tool lifecycle。

## 覆盖与边界

这些 fixture 对 `call_id`、output item、terminal、并行执行和时序有较强确定性，但不覆盖 Chat endpoint 或双向 Chat/Responses 转换。Codex 可消费的 event 子集也不等于完整 Responses 规范。

相关源码研究见[Codex Responses SSE 与工具生命周期](codex-sse-and-tool-lifecycle-analysis.md)。

