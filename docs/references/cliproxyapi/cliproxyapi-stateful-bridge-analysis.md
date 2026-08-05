# CLIProxyAPI stateful routing 与 Protocol Bridge 负面案例调研

## 状态与证据

| 项目           | 值                                                                                                     |
|----------------|--------------------------------------------------------------------------------------------------------|
| 调研仓库       | `router-for-me/CLIProxyAPI`                                                                            |
| 原始逐行快照   | `a14dfc779f43aed588e68b31fb34ab5ced700851`，2026-07-25                                                 |
| 当前模块级复核 | `bc71c77f5cc42f3fbe1bf040cf14d4f166894835`，2026-08-01                                                 |
| 阅读范围       | Responses translator/tests、Codex/xAI executor 的 continuation、reasoning replay 与 WebSocket ID state |
| 排除           | account/OAuth、管理 API、账号轮转与负载均衡                                                            |

executor 在当前提交已经拆分，因此原始行号只用于固定证据快照。

## 1. `previous_response_id` 的绑定状态

`codex_executor_retry_test.go` 把上游 `previous_response_not_found` 保留为 400，而不是普通可恢复失败。xAI WebSocket
executor 则维护 downstream response id 到 upstream response id 的会话内映射；找不到映射时会删除 `previous_response_id`
，并用本地 transcript 补入 input。

这两种路径共同说明 opaque response id 依赖原 issuer、deployment 和会话上下文。transcript 补偿是 CLIProxyAPI 的产品恢复策略，不是
Responses 字段本身的可移植语义。

## 2. Reasoning replay scope

`xai_reasoning_replay.go` 以 model name 和 session key 形成 replay scope；客户端可控 session key 还会加入下游 API key
hash 前缀，降低不同调用者共享 state 的风险。

它在以下情况跳过注入：

- reasoning 重复；
- 找不到匹配 tool output；
- assistant history 歧义；
- replay state 与当前 transcript 不一致。

completed output 中的 reasoning、message、function 和 custom-tool items 可以进入缓存。该 scope 没有形成跨
issuer、credential、route、TTL 和容量的通用状态契约。

## 3. ID 映射与 SSE lifecycle

OpenAI → Responses translator tests 检查：

- `response.output_item.added`；
- function arguments delta/done；
- `response.output_item.done`；
- `response.completed`；
- `item.id`、`call_id` 与输出内容的一致性；
- 内部 custom tool item 不泄漏为客户端同名 function。

这些 tests 证明 CLIProxyAPI 的 translator 区分 item lifecycle 与 response terminal，并维护多种 identity。它们也暴露典型失败类别：用
tool name/index 代替 `call_id`、内部 tool 泄漏、缺失 identity 时猜测、或把 item done 当 terminal。

## 4. 项目策略与证据边界

CLIProxyAPI 同时支持多协议、account、subscription credential、WebSocket 和多种 executor。其 state mapper、transcript recovery
与 reasoning cache 都服务于这一产品形状。

可从源码确认的是具体 failure transcript、ID/lifecycle 断言和本地恢复方式；不能从中推导所有 opaque state 都可重写、跨 route
replay，或账号/credential rotation 对 stateful request 安全。

## 一手入口

- [CLIProxyAPI repository](https://github.com/router-for-me/CLIProxyAPI/tree/bc71c77f5cc42f3fbe1bf040cf14d4f166894835)
- [cc-switch Chat/Responses 与 Agent Tool](../cc-switch/cc-switch-chat-responses-tool-conversion-analysis.md)
- [Codex Responses SSE 与工具生命周期](../codex/codex-sse-and-tool-lifecycle-analysis.md)
