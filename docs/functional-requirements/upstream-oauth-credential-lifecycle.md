# 上游 OAuth credential lifecycle 条件性安全边界

## 状态

**本文不是当前产品承诺，也不授权实施。** OAuth、subscription credential、keyring、远程 secret manager 与动态 credential 控制面仍由[产品范围](product-scope.md)明确排除。

本文只保存一项条件性边界：如果未来某个上游 Provider 明确允许独立 gateway client，并且用户重新批准该行为，那么登录、持久化、refresh 和请求恢复至少必须满足本文要求。外部研究证据见[OAuth 设备登录与 token 刷新综合调研](../references/cross-project/upstream-oauth-device-code-token-refresh-analysis.md)。

## 1. Provider OAuth preflight

进入实现前，必须用 Provider 官方资料和明确授权确认：

- authorization server、issuer、device authorization endpoint 与 token endpoint；
- 独立 client registration、client 类型、允许的 grant 和 client authentication；
- scope、resource/audience、redirect/device flow；
- access/refresh token lifetime、rotation、revocation 与 inactivity policy；
- account/workspace/organization 绑定和必要的非 secret header；
- subscription/API 使用资格，以及自动化 gateway/proxy 场景是否允许；
- reauthorization、用户撤销、管理员禁用和 credential 删除流程。

参考客户端内置的 client identity、私有 endpoint、redirect、scope 或 header 不能替代这项 preflight。

## 2. 登录入口与控制面边界

设备登录必须是显式运维命令或受保护的管理操作，不能在普通模型请求路径中自动开始。

1. Provider、endpoint、client registration 和 scope 只能来自受信注册；下游业务请求不能提供或覆盖。
2. login session 使用 Provider 给定的 TTL，只向发起者显示 verification URI 与一次性 user code。
3. 标准 Provider 严格实现 RFC 8628 poll/error 语义；私有 flow 使用独立、明确命名的 adapter。
4. token exchange 后校验 issuer、audience、scope 与 account/workspace allow-list。
5. 完整 credential 写入 secret backend 后才返回登录成功。
6. cancel、denied、expired 或校验失败时清除临时 device state，不持久化半成品 token。
7. 界面必须提示只有本人主动发起登录时才输入 code，降低 device-code phishing 风险。

不得导入其他应用的 auth cache，也不得在普通请求因 refresh 失败时自动退回交互式登录。

## 3. Credential bundle

一个可刷新 credential 至少以同一版本管理：

```text
credential_id          非 secret 稳定标识
provider / issuer      受信注册事实
client_registration    获授权 registration 的引用
subject / account      token 与 route 的身份绑定
workspace / org        Provider 要求时的 allow-list/header context
access_token           secret
refresh_token          secret，可选且可能轮换
expires_at             绝对过期时间
scope / audience       响应与请求前校验
version                reload/CAS 边界
status                 active / refresh_backoff / reauth_required / revoked / ambiguous
refreshed_at           lifecycle metadata
```

access token、rotated refresh token、expiry、scope 和 identity 必须原子写回。authorization server 返回新 refresh token 时必须替换旧值；未返回新值时是否保留旧值以 Provider contract 为准。

日志、metric、lock key 和错误只使用非 secret `credential_id` 或脱敏 fingerprint；不得记录 token、authorization code、PKCE verifier、device auth ID 或完整 auth record。

## 4. 到期驱动 refresh

refresh 应按 token expiry 调度，不是固定周期刷新全部账户：

```text
due_at = expires_at - provider_safety_window - bounded_jitter
```

到达 due time 后：

1. 取得以 `credential_id` 为键的 refresh lease/single-flight；
2. 从 secret store 重新加载 bundle 与 version；
3. 若其他 worker 已刷新且新 token 在 safety window 外，跳过重复 refresh；
4. 按 Provider contract 执行 refresh grant；
5. 校验 token type、issuer、audience、scope、expiry 与 identity；
6. 用 version CAS 原子写入完整 bundle；
7. 发布新 snapshot 并依据新 expiry 安排下一次 due time；
8. 唤醒等待同一 credential 的请求。

调度还必须满足：

- 启动时从持久化 expiry 重建 due queue；
- 全局 worker limit 与每 credential 单飞；
- bounded jitter 不得把 refresh 推迟到 access token 过期以后；
- 多实例共享 credential 时使用共享 lease 与 CAS，进程内 mutex 不足够；
- 是否为了 refresh-token inactivity 主动 refresh 只能来自 Provider 正式政策；
- single-use rotation 下，refresh 请求可能成功但响应丢失时进入 `ambiguous`，不得盲目重用旧 token。

## 5. 请求路径与 401 recovery

1. token 已进入 safety window 时，等待同一 refresh single-flight，而不是每个请求单独刷新。
2. token 仍在安全窗口外时，不为满足固定 timer 强制 refresh。
3. 401 后先 reload；若 credential version 已变化，用新 token 至多重试一次。
4. version 未变且 Provider contract 允许时，可执行一次 refresh，再至多重试一次。
5. 一旦下游业务 response 已开始，不得 refresh 后重放形成第二个上游响应。
6. 第二次 401 或终态 OAuth error 将 credential 转为 `reauth_required`，不能进入无限 refresh、账号轮转或普通 Provider fallback。

401 还可能来自 audience、account/workspace header 或授权政策，不等于 access token 一定过期。refresh 前后身份绑定必须一致。

## 6. 失败分类

| 失败 | 状态与行为 |
| --- | --- |
| device `authorization_pending` | 按当前 interval 继续 |
| device `slow_down` | 增加 interval 后继续 |
| device denied/expired | 终止，不创建 credential |
| refresh 429/明确暂态 5xx | `refresh_backoff`，受 Retry-After、expiry 和硬预算约束 |
| 确认请求未送达的网络错误 | Provider policy 允许时有界重试 |
| rotation 结果不确定 | `ambiguous`；不得假定旧 refresh token 有效 |
| `invalid_grant` / reused / revoked | `reauth_required` 或 `revoked`，停止自动 refresh |
| CAS conflict | reload 胜出版本，不能覆盖较新 token |
| secret-store write failure after possible rotation | `ambiguous`，不发布仅存在于内存的新 bundle |

不能只按 HTTP status 决定 refresh retry；OAuth error、是否收到响应、rotation policy、access token 剩余时间和 response commit 状态共同决定结果。

## 7. 当前非目标保持不变

本文不改变以下非目标：

- Codex/ChatGPT subscription OAuth 接入；
- 导入 Codex、Hermes、LiteLLM 或其他应用的 auth file；
- 多账号 subscription pool、账号级负载均衡、余额或配额控制面；
- 浏览器用户登录、下游 user OAuth 或平台代理 authorization server；
- 动态 endpoint/client registration/scope；
- 未经 Provider 正式授权的私有 device flow 模拟。
