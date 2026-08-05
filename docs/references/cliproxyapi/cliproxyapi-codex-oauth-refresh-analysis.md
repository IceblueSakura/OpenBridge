# CLIProxyAPI 的 Codex OAuth 与后台刷新调研

## 状态与证据

- 源码快照：`router-for-me/CLIProxyAPI` commit `bc71c77f5cc42f3fbe1bf040cf14d4f166894835`
- 阅读范围：Codex device login、refresh grant、credential conductor 和 auto-refresh scheduler
- 复核日期：2026-08-05
- 本文只记录 CLIProxyAPI 行为，不读取 credential 或评价 OAuth 使用资格。

## 1. Codex 设备登录

[
`sdk/auth/codex_device.go`](https://github.com/router-for-me/CLIProxyAPI/blob/bc71c77f5cc42f3fbe1bf040cf14d4f166894835/sdk/auth/codex_device.go)
复现 Codex 的产品 flow：

1. 请求私有 device user code；
2. 显示 verification URL 与 code；
3. 最长轮询 15 分钟，默认 interval 为 5 秒，并把 HTTP 403/404 视为 pending；
4. 取得 authorization code 与 PKCE verifier/challenge；
5. 执行 authorization-code + PKCE exchange；
6. 保存 token 与 account metadata。

该流程不是原样 RFC 8628，使用的 pending 语义和私有 endpoint 与 Codex 当前实现一致。

## 2. refresh grant

`internal/auth/codex/openai_auth.go` 发送 `grant_type=refresh_token`，返回新的 access/id token、可选 refresh token、account
metadata 与 expiry。当前实现：

- 以 refresh token 为 key 使用进程内 `singleflight.Group` 合并底层请求；
- 共享 refresh 使用不继承单个调用取消的 context，避免一个 waiter 取消整次刷新；
- 对请求做有界 retry，并把 `refresh_token_reused` 识别为不可重试错误；
- credential 更新保留上游未替换的旧 refresh token。

以 secret token 作为 singleflight key 是源码内部实现；该值不应出现在日志或可观察 lock 标识中。

## 3. 后台 auto-refresh scheduler

[
`auto_refresh_loop.go`](https://github.com/router-for-me/CLIProxyAPI/blob/bc71c77f5cc42f3fbe1bf040cf14d4f166894835/sdk/cliproxy/auth/auto_refresh_loop.go)
和 [
`conductor_refresh.go`](https://github.com/router-for-me/CLIProxyAPI/blob/bc71c77f5cc42f3fbe1bf040cf14d4f166894835/sdk/cliproxy/auth/conductor_refresh.go)
是四个 OAuth 样本中唯一持续运行的后台调度器：

- 以最小堆按下一次 due time 排序；
- 默认约每 5 秒检查到期工作；
- worker pool 有上限，避免同时刷新所有 credential；
- pending、失败和“刷新未产生变化”使用不同重新调度间隔；
- API key 或已终态失效的 credential 不进入 refresh queue。

Codex provider 的 [
`sdk/auth/codex.go`](https://github.com/router-for-me/CLIProxyAPI/blob/bc71c77f5cc42f3fbe1bf040cf14d4f166894835/sdk/auth/codex.go)
返回 5 天 refresh lead。这是 CLIProxyAPI 的项目常量，不是 OAuth 规范要求。

## 4. 并发与 401 recovery

conductor 为每条 auth state 使用进程内 mutex。取得锁后重新检查 access token 是否已经变化，从而复用并发 refresh 的结果。业务请求收到
401 后，同一 credential 最多触发一次 refresh，再进入既有 fallback 路径。

这些机制只协调一个进程。源码快照没有展示多副本共享 credential 的 distributed lease 或持久化 CAS；多个进程仍可能同时使用一个会轮换的
refresh token。

## 5. 适用边界

- 最小堆、worker limit 和结果分类说明后台刷新调度的形状。
- 5 天 lead、retry status 集合和失败后的账号轮换属于该项目策略。
- account pool、订阅 credential 聚合和管理面不是 OAuth 标准的一部分。
- 复现 Codex 私有 flow 不等于获得可复用的 client registration。

## 一手源码

- [
  `codex_device.go`](https://github.com/router-for-me/CLIProxyAPI/blob/bc71c77f5cc42f3fbe1bf040cf14d4f166894835/sdk/auth/codex_device.go)
- [
  `codex.go`](https://github.com/router-for-me/CLIProxyAPI/blob/bc71c77f5cc42f3fbe1bf040cf14d4f166894835/sdk/auth/codex.go)
- [
  `auto_refresh_loop.go`](https://github.com/router-for-me/CLIProxyAPI/blob/bc71c77f5cc42f3fbe1bf040cf14d4f166894835/sdk/cliproxy/auth/auto_refresh_loop.go)
- [
  `conductor_refresh.go`](https://github.com/router-for-me/CLIProxyAPI/blob/bc71c77f5cc42f3fbe1bf040cf14d4f166894835/sdk/cliproxy/auth/conductor_refresh.go)
- [
  `codex_executor_auth.go`](https://github.com/router-for-me/CLIProxyAPI/blob/bc71c77f5cc42f3fbe1bf040cf14d4f166894835/internal/runtime/executor/codex_executor_auth.go)

