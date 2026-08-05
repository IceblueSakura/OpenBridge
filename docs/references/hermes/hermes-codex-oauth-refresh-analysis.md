# Hermes Agent 的 Codex OAuth credential lifecycle 调研

## 状态与证据

- 原始逐行证据快照：`NousResearch/hermes-agent` commit `e598cef87465981fcea1c0339edfcf5d9716c917`
- 当前模块级复核：commit `470cf66b039c73bdd2c21d43094ce41a4db74eae`，2026-08-05
- 阅读范围：`hermes_cli/auth.py`、`agent/credential_pool.py` 及相关 Codex tests
- 未读取、输出或复制本地 credential、token、client identifier 或 auth file 内容。

## 1. 登录与持久化

Hermes 的 `openai-codex` 登录顺序为：

```text
existing Hermes credential
  -> optional user-approved import from Codex CLI cache
  -> otherwise device user-code flow
  -> poll for authorization code + PKCE verifier
  -> token exchange
  -> write Hermes auth store
```

设备登录包含 15 分钟 poll timeout 和登录端 429/`Retry-After` 处理。其 wire flow 与 Codex 私有 device-auth 实现一致，而不是原样
RFC 8628。

## 2. 按需 refresh 与进程协调

`resolve_codex_runtime_credentials()` 在 access token 的 JWT `exp` 前 120 秒触发 refresh。它在跨进程 auth-store 文件锁内重新读取
credential，再次判断是否仍需刷新，然后执行 refresh 和写回。

`refresh_codex_oauth_pure()` 使用标准 refresh grant，接受上游返回的 rotated refresh token；未返回新值时保留旧值。锁内
reload/re-check 减少同一主机多个 Hermes 进程同时消费旧 refresh token。

文件锁只协调共享同一文件系统的进程，不是多主机 lease 或版本化 secret store。

## 3. credential pool 与终态失败

Hermes `CredentialPool` 可以保存多个同 Provider credential，记录 priority、request count、rate-limit reset、最后错误和状态。
`invalid_grant`、token revoked、token invalidated 与 `refresh_token_reused` 等已知终态认证错误会把 entry 标为 `dead`，不在普通
cooldown 后重新投入 rotation。

singleton-seeded device credential 在选择前会从 auth store 同步已轮换的 token pair。手工添加的独立 credential 不会被
singleton 登录覆盖。

## 4. account context

特定 Codex auxiliary path 从 token claim 提取 account ID，并随 bearer、Codex-shaped originator 和 User-Agent 一起构造请求。该行为说明
Hermes 把 token pair 与 account/header context 共同视为一次认证状态。

## 5. 适用边界

- 从 Codex CLI cache 自愈是 Hermes 本地 Agent 的 credential ownership 选择。
- auth-store file lock 不能替代跨节点协调。
- credential pool 和账号轮换属于 Hermes 产品行为，不是 Codex OAuth wire contract。
- 私有 endpoint、header 和 client identity 的出现不构成第三方复用授权。

## 一手源码

- [
  `hermes_cli/auth.py`](https://github.com/NousResearch/hermes-agent/blob/470cf66b039c73bdd2c21d43094ce41a4db74eae/hermes_cli/auth.py)
- [
  `agent/credential_pool.py`](https://github.com/NousResearch/hermes-agent/blob/470cf66b039c73bdd2c21d43094ce41a4db74eae/agent/credential_pool.py)

