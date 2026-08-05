# Codex 设备登录与 token 刷新调研

## 状态与证据

本文只记录 Codex 产品文档和固定源码快照中的认证行为，不讨论任何网关的实现状态。

- 官方资料：[Codex authentication](https://learn.chatgpt.com/docs/auth)
- 源码快照：`openai/codex` commit `ee0247f95a6fe2b094ba2253d82cae2a2b4c2dff`
- 阅读范围：`codex-rs/login/src/device_code_auth.rs`、`codex-rs/login/src/auth/manager.rs`、
  `codex-rs/login/tests/suite/device_code_login.rs`
- 复核日期：2026-08-05
- 未读取或记录任何本地 credential、token、client identifier 值或认证缓存内容。

## 1. 官方产品行为

Codex 默认通过浏览器完成 ChatGPT OAuth。官方文档把 `codex login --device-auth` 标为 beta 的 headless 登录方式，并要求账户或
workspace 先启用 device code authentication。文档还说明 ChatGPT 登录会在使用期间于过期前自动刷新。

这些说明适用于 Codex CLI。官方页面没有在同一处给出第三方客户端可注册的通用 device authorization contract。

## 2. 当前设备登录 wire flow

[
`device_code_auth.rs`](https://github.com/openai/codex/blob/ee0247f95a6fe2b094ba2253d82cae2a2b4c2dff/codex-rs/login/src/device_code_auth.rs)
执行以下步骤：

1. 向私有 `deviceauth/usercode` endpoint 请求 verification URL、user code、内部 device auth ID 和 poll interval；
2. 显示 URL、一次性 code、15 分钟有效期和防钓鱼提示；
3. 在 15 分钟上限内轮询私有 `deviceauth/token` endpoint；
4. 当前实现把 HTTP 403/404 当作仍在等待，其他非成功状态终止登录；
5. 成功轮询得到 authorization code 与 PKCE verifier/challenge；
6. 再执行 authorization-code + PKCE token exchange；
7. 校验 workspace allow-list，并持久化 id/access/refresh token 与身份元数据。

因此，Codex 使用“设备交互 + authorization code/PKCE”的产品 flow，而不是原样 RFC 8628 token polling。其 pending 状态、字段和
endpoint 都是 Codex 当前实现事实。

## 3. access token refresh

[
`auth/manager.rs`](https://github.com/openai/codex/blob/ee0247f95a6fe2b094ba2253d82cae2a2b4c2dff/codex-rs/login/src/auth/manager.rs)
的主要行为是：

- 每次解析当前 ChatGPT auth 时检查 access token 的 JWT `exp`；距离过期不超过 5 分钟时尝试 refresh；
- 无法读取 `exp` 时，以 `last_refresh` 超过 8 天作为后备陈旧性判断；
- 一个单 permit semaphore 合并同进程 refresh；取得锁后重新加载 credential，若其他调用已经更新则跳过；
- refresh grant 成功后一起更新 access token、可选 rotated refresh token 和 `last_refresh`；
- API key 与 personal access token 不进入 ChatGPT refresh 路径。

这是一种使用时主动刷新，不是常驻后台 timer。

## 4. 401 recovery 与身份边界

401 recovery 使用有界状态机：先重新加载存储中的 credential，再尝试 refresh，随后结束，不形成无限重试。refresh 前后还检查
account/workspace identity，避免把另一个登录状态误用于当前请求。

设备登录测试覆盖成功写入、pending poll、workspace 不匹配和失败时不写入 auth cache。refresh 测试覆盖临近过期触发、仍有 6
分钟时跳过、guarded reload、account mismatch 和终态 refresh 错误。

## 5. 适用边界

- Codex 的私有 endpoint、client identity、callback 和 header 只能说明 Codex 自身行为。
- Codex auth cache 是本地产品 credential store，不是通用 OAuth 交换格式。
- 进程内 semaphore 不提供跨进程或多主机 refresh 协调。
- “自动刷新”在当前源码中指 credential 被使用时的主动检查，不能外推为后台周期任务。

## 一手源码

- [
  `device_code_auth.rs`](https://github.com/openai/codex/blob/ee0247f95a6fe2b094ba2253d82cae2a2b4c2dff/codex-rs/login/src/device_code_auth.rs)
- [
  `auth/manager.rs`](https://github.com/openai/codex/blob/ee0247f95a6fe2b094ba2253d82cae2a2b4c2dff/codex-rs/login/src/auth/manager.rs)
- [
  `device_code_login.rs`](https://github.com/openai/codex/blob/ee0247f95a6fe2b094ba2253d82cae2a2b4c2dff/codex-rs/login/tests/suite/device_code_login.rs)

