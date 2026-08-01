# Codex OAuth：可选 credential adapter 的边界与实施前验证

## 状态

**Deferred / Blocked；不属于 M0–M7。** OAuth 不是 OpenBridge 单用户 Provider 聚合核心或[架构迁移总计划](registry-architecture-migration.md)的前置条件。核心先使用标准 Provider API key；只有公开、适用的 OAuth client registration、redirect、scope/resource、token/refresh contract 与条款均确认后，才实施真实 Codex/ChatGPT OAuth。

## 1. 结论

OpenBridge 可以研究由 proxy 自主管理的 refreshable OAuth credential，但必须保持以下边界：

- 不通过 Codex CLI 中转登录、refresh 或 token storage；
- 不导入、复制或监视 `~/.codex/auth.json`；
- 不复用未经确认可供第三方 proxy 使用的 Codex CLI client identity、私有 endpoint 或 scope；
- 不把上游 OAuth token 当作下游 OpenBridge Bearer token；
- OAuth 失败或长期阻塞不影响 API-key Provider、native forwarding、routing 或 Protocol Bridge。

Codex 源码可用于学习 PKCE、secret storage、rotation 和 refresh state machine，但不能替代正式授权契约。

## 2. 适用范围

若未来启用，一个 Upstream Target 绑定一个明确 credential reference：

```text
UpstreamTarget
  └─ CredentialBinding
       ├─ ApiKey
       └─ RefreshableOAuth   # optional
```

首个 OAuth adapter 只需支持单用户、单 active credential；不提前设计：

- 多账号池；
- workspace/tenant routing；
- credential priority/weight；
- 多账号 failover；
- subscription account aggregation。

## 3. Credential 与 secret 边界

```text
OAuthCredentialMetadata
  provider_family
  upstream_target_id
  issuer
  account_fingerprint       # non-secret
  scopes/resource           # confirmed contract only
  state
  expires_at
  secret_ref
  secret_version
  refreshed_at
  last_error_code
```

secret store 中保存完整 rotated credential bundle；普通配置/数据库只保存 reference 和非 secret metadata。

不得进入普通日志、HTTP response、fixture 或错误：

- authorization code；
- PKCE verifier；
- access token；
- refresh token；
- cookie；
- 完整 callback query；
- 可重放 account/session material。

Provider、issuer、resource/audience、account fingerprint 和 Upstream Target 必须绑定。token 不能发送到业务请求指定的 URL。

## 4. 登录方式

### 4.1 Browser authorization-code + PKCE

仅在官方契约确认支持时：

```text
start login
→ short-lived LoginSession
→ authorization URL
→ callback validates provider/state/redirect/issuer
→ one-time code exchange
→ atomically persist secret bundle
→ credential becomes Active
```

要求：

- PKCE S256；
- state 高熵、短 TTL、单次消费；
- callback 绑定发起会话和 Provider；
- redirect URI 必须在注册范围内；
- callback response 和日志不包含 token/code/verifier；
- 成功后校验 issuer、audience/resource 和可获得的 account binding。

### 4.2 Device authorization

只有在公开契约和条款确认允许第三方 OpenBridge client 后才考虑。不得直接仿造 Codex CLI 的私有 device endpoints。

## 5. Refresh 状态机

```text
NeedsLogin
→ Active
→ Refreshing
→ Active | NeedsReauth
→ Revoked
```

规则：

1. 同一 credential 使用 single-flight refresh；
2. refresh 以 secret version CAS 提交，旧结果不能覆盖新登录/新 refresh；
3. near-expiry 可预刷新；
4. 上游 401 最多触发一次受控 refresh/retry，且只允许在下游尚未收到业务输出时；
5. `invalid_grant`、rotation/reuse 错误、issuer/account mismatch 进入 `NeedsReauth`；
6. revoke 使 secret reference 不可再解析，并清理 adapter cache。

## 6. Headless 运维边界

若实施，只提供 loopback callback 配合 CLI command：

```text
credential status
start login
callback
revoke
```

它不提供企业级 admin control plane、GUI、Web 控制台或客户端管理。`credential status` 只在 CLI 输出 Provider、状态、account fingerprint、expiry、last refresh 和 error code。

非 loopback 暴露登录 callback 前，必须另行设计静态管理员认证与 TLS；初期范围不提供远程管理接口。

## 7. Mock issuer 实验

在真实契约确认前，可以使用 mock issuer 验证通用 credential lifecycle：

| 场景 | 必须断言 |
|---|---|
| authorization-code success | state、PKCE、issuer、redirect 被验证；secret 只进入 store |
| state replay/expiry | 拒绝且不覆盖 credential |
| issuer/redirect substitution | 拒绝 |
| refresh rotation | 新 secret version 原子生效 |
| concurrent refresh | 只发生一次 token endpoint call |
| `invalid_grant`/revoked | 进入 `NeedsReauth`，不无限 retry |
| revoke | adapter 不再获得 secret |
| secret scan | log、error、fixture 无 OAuth material |

该实验只证明 state machine 和 secret boundary，不证明真实 Codex OAuth 可合法接入。

## 8. 真实 OAuth preflight

实施前必须获得适用于 OpenBridge 的明确答案：

- OAuth client 如何注册，client identity 是否允许自建 proxy；
- redirect URI、issuer、authorization/token/refresh/revocation endpoint；
- scope/resource/audience；
- refresh rotation、expiry、revoke 与 account/workspace 行为；
- token 可调用的正式 Provider API；
- 服务条款是否允许该单用户 credential-owner 模式。

结果只能是：

```text
Implementable
Blocked
Rejected
Insufficient evidence
```

没有明确结论时保持 API-key baseline，不使用源码逆向细节绕过。

## 9. Codex 源码参考与限制

已核对的既有调研快照：`openai/codex@0fb559f0f6e231a88ac02ea002d3ecd248e2b515`。

- [PKCE](https://github.com/openai/codex/blob/0fb559f0f6e231a88ac02ea002d3ecd248e2b515/codex-rs/login/src/pkce.rs#L12-L26)
- [browser callback/state](https://github.com/openai/codex/blob/0fb559f0f6e231a88ac02ea002d3ecd248e2b515/codex-rs/login/src/server.rs#L329-L391)
- [token exchange](https://github.com/openai/codex/blob/0fb559f0f6e231a88ac02ea002d3ecd248e2b515/codex-rs/login/src/server.rs#L784-L857)
- [device code](https://github.com/openai/codex/blob/0fb559f0f6e231a88ac02ea002d3ecd248e2b515/codex-rs/login/src/device_code_auth.rs#L62-L146)
- [storage](https://github.com/openai/codex/blob/0fb559f0f6e231a88ac02ea002d3ecd248e2b515/codex-rs/login/src/auth/storage.rs#L38-L61)
- [refresh lock](https://github.com/openai/codex/blob/0fb559f0f6e231a88ac02ea002d3ecd248e2b515/codex-rs/login/src/auth/manager.rs#L2362-L2455)

这些源码说明 PKCE、state、storage、single-flight refresh 和 rotation protection 的工程需求；不证明其中的 client ID、endpoint、scope、claims 或 backend header 是第三方稳定接口。

## 10. 一手参考

- Codex authentication：https://developers.openai.com/codex/auth
- OAuth 2.0 Security Best Current Practice：https://datatracker.ietf.org/doc/html/rfc9700
- PKCE：https://datatracker.ietf.org/doc/html/rfc7636
- Codex source：https://github.com/openai/codex
- [本仓库 Codex OAuth 源码调研](../references/codex/codex-oauth-and-tool-call-analysis.md)
- [Hermes/LiteLLM subscription OAuth 调研](../references/cross-project/hermes-litellm-oauth-analysis.md)
