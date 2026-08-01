# CLIProxyAPI 状态与 Protocol Bridge 负面案例调研

## 状态与范围

**外部实现调研；用作 translator/stateful routing 的失败与约束材料，不是 OpenBridge 的实现模板。**

| 项目 | 值 |
|---|---|
| 调研仓库 | `https://github.com/router-for-me/CLIProxyAPI` |
| 固定证据快照 | `F:/codespace/CLIProxyAPI`，`main` @ `a14dfc779f43aed588e68b31fb34ab5ced700851` |
| 快照日期 | 2026-07-25 |
| 阅读范围 | Responses translator 及其 tests、Codex/xAI executor 的 `previous_response_id`、reasoning replay、WebSocket ID state |
| 矩阵角色 | state affinity、ID mapping、SSE terminal 与多轮 continuation 的负面案例库 |
| 不在范围 | 多账号/订阅 credential、OAuth/client identity、WebSocket 初期实现、管理 API、账号轮转与负载均衡 |

**2026-08-01 当前模块级复核。** 本地 `main` 已 fast-forward 至 `bc71c77f5cc42f3fbe1bf040cf14d4f166894835`。`previous_response_not_found` 保留错误测试、`previous_response_id` 处理，以及 `response.output_item.done`/`response.completed` 的 Responses translator 和 executor 测试仍可定位；executor 已拆分为多文件，因此下文细粒度行号继续只属于固定证据快照。

## 1. `previous_response_id` 是绑定状态，不是可自由转发的字段

`codex_executor_retry_test.go:156-162` 将上游 `previous_response_not_found` 视为保留的 400 错误，而不是普通可恢复失败。xAI WebSocket executor 则维护 downstream response id 到 upstream response id 的会话内映射；若无法映射 previous id，会删除 `previous_response_id` 并以本地 transcript 补入输入（`xai_websockets_executor.go:106-114`、`:285-301`）。

这暴露的风险比“能否续接”更重要：opaque response id 只在原 issuer、deployment、route 与会话上下文中有意义。CLIProxyAPI 的 transcript 补偿是其特定产品的恢复策略，不能被 OpenBridge 当成通用 fallback。

OpenBridge 规则应为：

- Native Path 只在同 issuer/deployment 的明确 binding 下保留 continuation；
- bridge 需要 state 时，ledger 必须绑定 issuer、deployment、route snapshot、protocol、创建/过期时间和容量；
- 无 binding、过期、歧义或跨 deployment 的 `previous_response_id` 必须拒绝或要求客户端提供完整无状态 input，不得静默转发、重写或猜测恢复；
- 初期 HTTP/SSE 不因 CLIProxyAPI 有 WebSocket state mapper 而扩大到 Responses WebSocket。

## 2. replay state 的最小隔离仍不足以成为通用契约

`xai_reasoning_replay.go` 的 replay scope 由 `modelName` 和 `sessionKey` 组成（:18-27），对客户端可控 session key 会再加下游 API key 的 hash 前缀以避免调用者共享 state（:70-88）。它遇到重复 reasoning、缺失匹配 tool output 或 assistant 历史歧义时会跳过注入（:101-176），并在 completed output 中缓存 reasoning/message/function/custom-tool items（:250-293）。

这些实现说明 replay 需要隔离、去重和歧义拒绝；但本次阅读范围没有证明其 scope 等同 OpenBridge 所需的 issuer、deployment、route snapshot、credential binding、TTL 和容量契约。因此它应生成如下 OpenBridge fixture，而不是复用缓存键：

1. 不同 deployment/issuer 的 continuation 一定不共享；
2. caller 传入与缓存 assistant history 不一致时拒绝，不回填旧 item；
3. function/custom tool call 只有在匹配 output 的同一 call group 内恢复；
4. compaction、终态失败、cache error 与过期都会清除或拒绝旧 state；
5. cancellation 和已输出 stream 不能触发第二个 candidate 的 replay。

## 3. ID 映射与 SSE lifecycle 不能简化

当前 OpenAI→Responses translator tests 明确检查 `response.output_item.added`、arguments delta/done、`response.output_item.done` 和 `response.completed` 之间的 `item.id`、`call_id` 与输出内容一致性（`internal/translator/openai/openai/responses/openai_openai-responses_response_test.go:623-769`）。同一测试集还防止内部 `custom_tool_call` 泄漏到下游，即使客户端声明了同名 function（`:996-1174`）。

因此 OpenBridge 的 bridge 不能：

- 用 tool name 或 stream index 代替 `call_id`；
- 把 `output_item.done` 误作为 terminal；
- 因 transformer 引入了内部 tool item 就原样暴露给客户端；
- 在 tool identity 缺失、冲突或无法安全映射时猜测名称/ID。

适合转化为 contract fixture 的最小组合是：并行 function/custom call、fragmented argument、同名 internal/client tool、item done 后 failed/incomplete、completed 中的 usage，以及 downstream cancellation。

## 4. 作为负面案例，而非实现来源

CLIProxyAPI 同时覆盖多协议、accounts、subscription credentials、WebSocket 与多种 executor。它确实提供丰富的 translator 代码，但在 OpenBridge 矩阵中仅承担三类价值：

```text
issue / source test
→ 说明哪一种 continuation、ID 或 terminal 组合会失败
→ 转为最小 transcript 与 OpenBridge 拒绝/绑定规则
→ 在 OpenBridge 自有 fixture 中验证
```

不得从其中推导多账号 credential pool、OAuth client identity、每用户 session store、WebSocket 默认支持、全局 route failover 或业务管理 API。

## 5. 后续调研准入条件

只有当 OpenBridge 选择 Chat/Responses bridge 或 stateful continuation 作为当前焦点时，才继续阅读该项目的具体 translator。每一项必须同时记录：固定 commit、原始测试/issue、失败前提、预期 OpenBridge 行为、以及“为什么不采用其账号/会话/恢复策略”。无法在当前 OpenBridge 协议范围复现的实现只保留为线索。

## 相关资源

- [项目比较矩阵](../project-comparison.md)
- [cc-switch Chat/Responses 与 Agent Tool](../cc-switch/cc-switch-chat-responses-tool-conversion-analysis.md)
- [Codex Responses SSE 与工具生命周期](../codex/codex-sse-and-tool-lifecycle-analysis.md)
- [网关 API 与客户端兼容需求](../../functional-requirements/gateway-api-compatibility.md)
- [Provider 韧性需求](../../functional-requirements/provider-resilience.md)
