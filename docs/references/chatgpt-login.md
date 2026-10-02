# ChatGPT 登录参考：Codex 与 pi coding agent

本页维护 ChatGPT 登录的来源、协议分支、账户和 credential 生命周期，不是 OpenBridge 已支持登录、订阅推理或第三方部署资格的声明。共用标准与授权边界归 [OAuth 来源入口](oauth-login.md)，实施方向归 [next-goal](../implementation-plans/next-goal.md)。

**必须分开 Codex 产品登录、公开 Sign in with ChatGPT（SIWC）plan-usage 登录和 Platform API key。** 同一 ChatGPT 账户、issuer、SDK `api_key` 参数或 OAuth grant 名称不使这些 credential/backend 互换。ChatGPT web 的浏览器 session、connector/MCP OAuth 也不是这里的推理凭据。

## 来源与版本

| 来源 | 固定版本 / 入口 | 用途 |
|---|---|---|
| Codex 官方产品文档 | [Authentication][codex-auth-doc]、[Access tokens][codex-access-doc] | 浏览器/设备入口、workspace 管理、存储和独立 automation 认证 |
| Codex 官方源码 | `openai/codex@d25c114d494ddb693290b76bf5e5f64ecbdb38fc`，[Apache-2.0][codex-license] | [浏览器 server][codex-server]、[私有设备交互][codex-device]、[OAuth 协议][codex-oauth-client]、[callback 绑定][codex-authorization]、[auth manager][codex-manager]、[token claims][codex-token-data]、[存储][codex-storage]、[撤销][codex-revoke] |
| OpenBridge Codex 设备凭据池 wire 来源 | 稳定 CLI [`0.160.0`][codex-pool-release]，`openai/codex@a956835d020762cb2b570053af06f643a11c0ecc`，[Apache-2.0][codex-pool-license] | [私有设备交互][codex-pool-device]、[code/refresh 编码][codex-pool-oauth]、[refresh owner][codex-pool-manager]、[JSON revocation][codex-pool-revoke]、[默认与 raw auth client][codex-pool-client]；不是 SIWC 或旧语义基线升级 |
| pi coding agent / pi-ai | `0.99.2`，`earendil-works/pi@005af57d88ee23b33778f343a9595b32e67ff788`，[MIT][pi-license] | [公开 SIWC 登录][pi-siwc]、[Codex 登录][pi-codex]、[OpenAI Provider][pi-openai-provider]、[Codex Provider][pi-codex-provider]、[刷新协调][pi-resolve] |
| 公开 SIWC 官方合同 | [Overview][siwc-overview]、[Registration and sign-in][siwc-sign-in]、[Accounts and sessions][siwc-sessions]、[Token reference][siwc-tokens]、[Models and inference][siwc-inference] | 第三方本地/开源应用的动态 registration、identity、consent、refresh 和 Responses 路径 |
| 公开 authority 元数据 | [OIDC discovery][openai-discovery]、[JWKS][openai-jwks] | SIWC issuer、端点与签名验证来源；不能覆盖 Codex 固定产品端点 |

上述 Codex 提交固定认证及 [session/cache 上下文来源](extensions-and-context.md#2-codex-session_id-的实际含义)，不替换 [upstream-sync](upstream-sync.md) 的标准/codec 基线。官方网页没有永久版本，实施时重核所选 flow、用途资格与 endpoint 合同，不记录账号状态、模型库存或历史运行结果。参考源码不构成本项目依赖，采用代码须保留许可证与 attribution。

## 认证对象与选择

- **Codex ChatGPT 登录**：`codex login`；设备入口为 `codex login --device-auth`。它受所选 ChatGPT workspace 的资格、RBAC 和管理策略约束，不只是个人订阅标签。
- **公开 SIWC plan usage**：用户授权自己的 registration，用 OAuth access token 完成获准的公共 Responses API 请求；identity 与 plan usage 是两项权限。
- **Platform API key**：独立 usage-based API credential，不经下面的 refresh flow，也不能冒充 ChatGPT workspace session。
- **Codex access token / workload identity / 外部 auth owner**：是官方其他 automation 合同的来源导航，不是浏览器 OAuth 的同义词。其创建、权限、rotation、revocation 与使用面分别查官方文档，不从名称推定可用于任意 API。

官方 Codex 配置可限制登录方式和 workspace，凭据不匹配时拒绝。Cloud、desktop、CLI、IDE 的准入边界也不相同；这里不复制套餐支持表。官方与 pi 命令不是 OpenBridge 命令，真实登录、credential 写入/撤销与推理均需另外授权。

## Codex 浏览器登录

固定 [server][codex-server] 使用 `https://auth.openai.com`，其产品路径为 `/oauth/authorize`、`/oauth/token`，默认 client ID 为 `app_EMoamEEZ73f0CkXaXp7hrann`。这是公开产品 client ID，不是 client secret，也不自动授予第三方复用资格。

1. 生成随机 state 与 PKCE S256 verifier/challenge，启动 loopback callback。默认端口为 1455，浏览器 redirect URI 使用 `http://127.0.0.1:<port>/auth/callback`，端口占用时源码有 fallback；准确 URI 仍须逐次绑定，不从端口相同推定与另一 flow 等价。
2. authorization 请求带 client ID、scope、准确 redirect URI、challenge 和 state；产品参数另有 `id_token_add_organizations=true`、`codex_cli_simplified_flow=true`、originator 及可选 workspace 约束。
3. callback 的 state 与本次事务绑定，在使用 code 或 OAuth error 前校验；预选 workspace 不代替最终 workspace 检查。
4. 使用 form-encoded authorization-code grant，把 code、client ID、相同 redirect URI 和 verifier 发送到产品 token endpoint；没有内置 client secret。
5. 按官方产品 token/workspace 合同处理 `id_token`、access token、refresh token，验证受管理 workspace 限制后保存；结束 listener、登录取消句柄和临时授权状态。

固定官方 scope 包含 `openid profile email offline_access api.connectors.read api.connectors.invoke`；pi Codex 参考实现只请求前四项。它们是不同固定客户端的选择，不应自动扩大第三方权限或将 connectors 权限视为推理必需。

[Codex token_data][codex-token-data]解析 JWT 中的产品 claims；payload decode 不是签名验证或通用 identity proof。第三方网关不能把这种便利读取直接用于建立本地可信账户；公开 SIWC 的 ID-token 验证要求见后文。

## Codex 设备交互不是标准 device grant

设备凭据池采用固定 CLI 0.160.0 的 [device_code_auth][codex-pool-device] 与 [OAuth client][codex-pool-oauth]；既有 [pi Codex OAuth][pi-codex]只作补充参考。产品私有步骤为：

```text
POST /api/accounts/deviceauth/usercode
  → device_auth_id、user_code、interval
人访问 /codex/device 完成授权
POST /api/accounts/deviceauth/token，轮询 device_auth_id + user_code
  → authorization_code + PKCE 材料
POST /oauth/token，authorization_code grant
  → 账户 token set
```

- 申请与 polling 使用 JSON，不是 RFC 8628 的 form-encoded device grant。
- 最终交换使用 `redirect_uri=https://auth.openai.com/deviceauth/callback` 和返回的 PKCE verifier；官方返回还包括 code challenge。
- 成功 polling 返回的是 code/PKCE，不是直接返回可推理 token。不能替换为 `grant_type=urn:ietf:params:oauth:grant-type:device_code`；也不能按 [Grok](grok-login.md#标准设备授权)的错误码解释所有返回。
- 私有 polling 将 403/404 视为 pending；最初申请的 404 则表示所选部署未启用设备入口。只在此合同中使用该区别，不能泛化为所有 OAuth 403/404 可重试。
- 固定客户端限定约 15 分钟的人机交互；interval、取消、每次网络 deadline 和响应预算需独立约束。pi 还识别相应 pending/slow-down 错误，依然不是标准设备 grant。
- 设备方式可能需用户或 workspace 管理员启用。人应只批准自己发起的登录，不按他人提供的 user code 授权。

公开 SIWC discovery 不因 Codex 私有端点存在而声明标准 device grant。SIWC 的 remote host 方案查 [Self-hosted VMs][siwc-vm]；不得用 Codex 产品 polling 为任意 issued SIWC client 添加设备能力。

## Codex refresh、账户保护与退出

固定 [auth manager][codex-manager]向产品 `/oauth/token`发送 `refresh_token` grant，使用对应产品 client ID；官方客户端的 refresh 为 JSON 编码，pi Codex 分支使用 form 编码。这是需要按实际 endpoint/profile 选定的 wire 差异，不是重写 token 语义的理由。

- 先获取 refresh 协调锁并 guarded reload；只在预期 account/workspace 匹配时采用存储中的新 credential。源头已刷新时不再消费旧 refresh token；账号改变时不能将原请求静默转到新账号。
- 区分过期、refresh token reused、revoked/invalidated 和瞬态失败，失败预算与 credential generation 绑定，不永久污染重新登录后的 credential。
- 官方产品可以更新有报告的 token 字段并保留未报告字段；pi Codex token 解析要求 access、refresh 和 expires_in 完整。不能把这些消费者要求当作所有 authority 的统一返回 schema。
- 官方文档提供 file/keyring/auto/ephemeral 等存储策略；其 auth 文件与 OS store 是敏感 credential，不是 OpenBridge 调查输入或运行依赖。
- 固定 Codex 有 `logout_with_revoke`：对 managed ChatGPT OAuth 尝试远端 revoke 后仍清理本地存储，remote 失败不意味着撤销已完成。普通本地 logout 与该路径须区分；pi 的共用本地退出边界见 [credential 生命周期](oauth-login.md#pi-公用-credential-生命周期)。

pi 的共用刷新协调由 [OAuth 来源入口][pi-resolve]维护；Codex 本节只拥有产品 refresh wire 与身份边界，不复制通用 store 或恢复策略。

## 公开 Sign in with ChatGPT：动态 registration

[公开 SIWC][siwc-overview]为符合其用途资格的开源/本地托管应用提供不需要 client secret 或 partner API key 的 plan-usage flow。付费/远程托管应用另有资格入口；开源许可证本身不能证明多用户网关、转售/托管服务已获准。OpenBridge 需先选定个人本地/self-hosted 用途，或重新确认其他部署合同。

### Client、账户与 host 不同

- 首次注册使用 `client_id=dynamic_agent_client`，它只是注册入口，不是最终保存或用于 token exchange 的 client ID。
- callback 返回 issued client ID，通常形如 `oaiapp_...`。registration 绑定经过认证的用户和所选 workspace；相同 email 不意味着同一 registration。
- 为每个 host 预先生成并持久化 opaque `ext_agent_host_id`。重启/重新登录不创建新 host。官方接受一次生成并持久化的 `urn:uuid:<UUID>`，并推荐公钥派生的 JWK thumbprint URI；host ID 只是标识，不是 authentication 或私钥持有证明。
- 返回登录复用所选账户已保存的 issued client ID，保持该 host ID。新 host 有自己的 host ID，但可按同用户/workspace 合同复用 registration。
- `agent_name_hint` 是首次注册时如实填写的应用展示名称，不是账户身份。后续可用已验证账户的 `id_token_hint` / `login_hint`协助选择；hint 不免除新 identity 验证，含 ID-token hint 的 authorization URL 必须脱敏。

### Authorization 与 callback

公开路径由 [sign-in][siwc-sign-in]和 [discovery][openai-discovery]定义，不使用 Codex 旧产品 `/oauth/*`端点：

- authorization：`https://auth.openai.com/api/accounts/authorize`；
- token：`https://auth.openai.com/api/accounts/oauth/token`；
- resource：`https://api.openai.com/v1`；
- identity scopes：`openid profile email`；plan scopes：`offline_access resource.invoke chatgpt.tokens.use.direct`。

每次事务重新生成 PKCE S256、state 和 nonce。初始 callback 使用 `http://127.0.0.1:<port>/auth/callback`，不能替换为 `localhost`；后续允许改变端口但保持 scheme/host/path，每次 authorize 与 exchange 使用完全相同 URI。先启动 listener，再打开系统浏览器。

先验证 callback state 与 error。首次注册必须收到 issued client ID；返回登录可不报告 client ID，此时使用本次事务绑定的已有 ID。若 callback 报告不同 ID 则拒绝，不能覆盖已有 registration。callback 中的 scope 不能替代 token 响应实际授予的 scope。

用 form-encoded authorization-code grant 交换：issued client ID、code、verifier、准确 redirect URI 和相同 resource。没有 secret；不能把 `dynamic_agent_client`、Codex 产品 client 或另一账户的 client 代入。code 已消费或结果不确定时不盲目重发；`invalid_grant`需要丢弃该 code 并重新授权。

### Identity 与 plan permission

官方要求验证 ID-token 签名/JWKS、issuer、issued-client audience、expiration 和本次 nonce；账户身份使用验证后的 `sub`。不要把 email、access-token opaque auth metadata 或 workspace 标签当作等价身份。返回登录须确认 identity 与所选 registration 匹配，再发布新 credential。

token 响应中实际授予的 scope 决定是否可以使用 plan；缺少 `chatgpt.tokens.use.direct`不得进入该订阅 inference 路径，单有合法 ID token 不够。Identity 不授予 ChatGPT 会话内容、普通 API 组织资源或用户 API key 的访问权。签名算法由该 authority discovery 核对，不从 [xAI](grok-login.md)套用 ES256 策略。

### Refresh、持久化与 revocation

[Accounts and sessions][siwc-sessions]要求保存 issued client、验证后的 identity、tokens、granted scopes 和实际 expiry；registration 映射与 host ID 的生命周期独立于一次 session。

- 以同一 issued client ID、refresh token 和 resource 发送 form-encoded refresh grant；省略 scope 保留 grant。返回 replacement refresh token 后，将新 tokens、expiry 和 scopes 作为同一 generation 原子持久化。
- 同 session 的 refresh 串行化，包括多进程；并发 rotation 或持久化结果不确定不能用旧 token 无限重试。新登录取消、失败或不匹配不覆盖当前 credential。
- [Token reference][siwc-tokens]定义此 flow 的 token lifetime、rotation 及 `earliest_refresh_at`；它们不适用于 Codex 旧产品或 xAI。依据实际响应与官方 renewal 要求调度，不用硬编码 TTL 或跨 authority 统一缺省规则。
- credential 写入自有受保护 store，Unix owner-only 权限及原子发布；实际 granted scopes、expiry、tokens 与 validated identity 绑定，不作为另一账户或 client registration 的可复用模板。
- sign-out 先停止该 session 请求，并按 discovery 的 revocation endpoint 尝试 revoke refresh token，绑定 issued client ID。空 HTTP 200 表示协议层成功，包含已无效 token；网络失败时区分“本地已退出”和“远端撤销未确认”。清理本地 tokens 不删除 registration 或 host ID，重新登录可复用。
- 账户切换由显式可信选择决定，不把一个 registration 的 client ID、refresh、identity 或 replay scope 与另一个账户拼接。

### 推理与 remote host

[Models and inference][siwc-inference]指定公共 `https://api.openai.com/v1/responses`，使用该 session access token 作 Bearer，选定账户允许的模型，并按此 flow 要求 `store=false`、`stream=true`。不能改发 ChatGPT `backend-api`，不能认为所有公共 API operation 或同名模型均获授权；[Preview limitations][siwc-limitations]拥有其他操作与 app-server 限制。

完整消费 SSE 并确认实际 terminal，不以 HTTP 200 证明推理完成。如何让现有 JSON 下游消费上游强制 streaming，应另行选定有界交付 slice；登录参考不替代 delivery 合同。

官方[self-hosted VM 方案][siwc-vm]允许同用户/workspace 在受保护渠道转移所选 registration credential 并由 VM 管理 refresh，VM 保留自己的 host ID；这不是第三方 auth-cache 搜索/导入许可。OpenBridge 默认拥有自己的登录与 secret store，任何凭据迁移需要独立授权和单 writer/rotation 边界。

## pi 的两条 ChatGPT 路径

### 内置 openai：公开 SIWC

固定 [openai Provider][pi-openai-provider]通过 `/login openai`提供 “Sign in with ChatGPT”。[OAuth 模块][pi-siwc]使用 dynamic registration、resource 和 direct-plan scopes，从 callback 取得 issued client ID；授权和 refresh 均为 form，credential 保存 client ID、scopes 与 adjusted expiry，access token 用于公共 Responses。

它是可定位的参考，不是官方合同的完整替代：

- 固定模块每次登录走新 dynamic registration；官方返回登录要求复用已保存 issued client ID。OpenBridge 需明确 registration 保存与重新授权，而不照搬重新注册策略。
- 模块发送 nonce、要求 ID token 存在，但不验证其签名/identity 或保留其 profile；官方要求验证 ID token 并绑定账户。不能将 presence 检查升级为已验证身份。
- 固定模块使用固定 1455 端口及完整 redirect URL 手动回传，要求 state 和 issued client ID；官方返回 callback 可省略 client ID 的合同需另行支持。
- pi caller 提供稳定 installation UUID；接入层须确保它真正持久化，不能把示例中的临时随机 UUID 当作 host 生命周期。
- refresh 要求 replacement refresh 和 granted scopes，使用 stored issued client ID 及 resource；更新后的 scope 必须重新准入。固定模块的提前 expiry 余量不是 authority lifetime。

### 内置 openai-codex：产品参考路径

固定 [Codex Provider][pi-codex-provider]将该路径命名为 legacy，支持浏览器及 Codex 私有设备交互；默认 inference 根为 `https://chatgpt.com/backend-api`，其[API 适配][pi-codex-api]选择 `/codex/responses`及受信账户 headers。它不等于公开 SIWC 的公共 Responses 合同。

pi Codex 固定 redirect URI 使用 `http://localhost:1455/auth/callback`，与上述官方快照及公开 SIWC 的 `127.0.0.1` 规则分别核对。它的浏览器 listener 验证 state，但手动输入可接受裸 code，只有输入携带 state 时才校验；从 JWT 提取 account ID 也不构成签名验证。OpenBridge 不应照搬这些宽松便利路径，更不能将成功拿到产品 token 解释为公开 SIWC consent。

## 登录后的缓存与上下文

本页的 renewable session 是认证生命周期，不是模型 logical session。issued client ID 绑定 registration，`ext_agent_host_id` 标识 host；它们不自动成为 prompt cache key、thread、turn 或 response reference。token rotation 不自动创建新的 logical session，opaque state 的失效仍需由实际 ownership/profile 合同判断。

缓存、存储、连接状态及固定 Codex/pi 的具体投影只由 [扩展与上下文](extensions-and-context.md#2-codex-session_id-的实际含义)维护。公开 SIWC 采用自己的公共 Responses 准入，不继承 Codex backend 的私有 session headers；`store:false` 不等于关闭 prompt cache，也不授权 OpenBridge 复用连接级 continuation。

## OpenBridge 采用边界

通用 secret ownership、单 writer/原子发布、取消、重试/commit 与真实操作授权归 [OAuth 采用边界](oauth-login.md#共用采用边界)。本页另需定稿：

1. OpenBridge 用户选定 Codex 产品登录与订阅用途，共用管理合同归 [ADR 0012](../architecture-v2/decisions/0012-grok-personal-credential-pool.md)，操作归 [凭据管理指南](../credentials.md)。所选 CLI client/metadata 是固定兼容参考，不证明第三方 backend/client 用途已获准；公开 SIWC 是另一合同，不按账户或 model label 混用。
2. Codex 的 verified subject/workspace、product client 和 credential generation 由 [credential owner](../../src/credential/mod.rs)绑定；signature/issuer/audience 校验归 [JWT owner](../../src/credential/jwt.rs)。SIWC 独立需要 issued registration/host/grant 合同，不能自动套在 Codex 上。opaque replay scope 不从 canonical model 相同推定可迁移。
3. 单独选择 streaming delivery 与需要的 cache/continuation 子集；有 credential 或低层 carrier 不表示 Gateway 已拥有 Agent turn 或 WebSocket runtime。

上述资料不选择或授权具体实现；待选边界由 [next-goal](../implementation-plans/next-goal.md)维护，获准行为 slice 才进入 [current-focus](../implementation-plans/current-focus.md)。

[codex-pool-release]: https://github.com/openai/codex/releases/tag/rust-v0.160.0
[codex-pool-license]: https://github.com/openai/codex/blob/a956835d020762cb2b570053af06f643a11c0ecc/LICENSE
[codex-pool-device]: https://github.com/openai/codex/blob/a956835d020762cb2b570053af06f643a11c0ecc/codex-rs/login/src/device_code_auth.rs
[codex-pool-oauth]: https://github.com/openai/codex/blob/a956835d020762cb2b570053af06f643a11c0ecc/codex-rs/login/src/oauth/client.rs
[codex-pool-manager]: https://github.com/openai/codex/blob/a956835d020762cb2b570053af06f643a11c0ecc/codex-rs/login/src/auth/manager.rs
[codex-pool-revoke]: https://github.com/openai/codex/blob/a956835d020762cb2b570053af06f643a11c0ecc/codex-rs/login/src/auth/revoke.rs
[codex-pool-client]: https://github.com/openai/codex/blob/a956835d020762cb2b570053af06f643a11c0ecc/codex-rs/login/src/auth/default_client.rs
[codex-auth-doc]: https://learn.chatgpt.com/docs/auth
[codex-access-doc]: https://learn.chatgpt.com/codex/enterprise/access-tokens
[codex-license]: https://github.com/openai/codex/blob/d25c114d494ddb693290b76bf5e5f64ecbdb38fc/LICENSE
[codex-server]: https://github.com/openai/codex/blob/d25c114d494ddb693290b76bf5e5f64ecbdb38fc/codex-rs/login/src/server.rs
[codex-device]: https://github.com/openai/codex/blob/d25c114d494ddb693290b76bf5e5f64ecbdb38fc/codex-rs/login/src/device_code_auth.rs
[codex-oauth-client]: https://github.com/openai/codex/blob/d25c114d494ddb693290b76bf5e5f64ecbdb38fc/codex-rs/login/src/oauth/client.rs
[codex-authorization]: https://github.com/openai/codex/blob/d25c114d494ddb693290b76bf5e5f64ecbdb38fc/codex-rs/login/src/oauth/authorization.rs
[codex-manager]: https://github.com/openai/codex/blob/d25c114d494ddb693290b76bf5e5f64ecbdb38fc/codex-rs/login/src/auth/manager.rs
[codex-token-data]: https://github.com/openai/codex/blob/d25c114d494ddb693290b76bf5e5f64ecbdb38fc/codex-rs/login/src/token_data.rs
[codex-storage]: https://github.com/openai/codex/blob/d25c114d494ddb693290b76bf5e5f64ecbdb38fc/codex-rs/login/src/auth/storage.rs
[codex-revoke]: https://github.com/openai/codex/blob/d25c114d494ddb693290b76bf5e5f64ecbdb38fc/codex-rs/login/src/auth/revoke.rs
[pi-license]: https://github.com/earendil-works/pi/blob/005af57d88ee23b33778f343a9595b32e67ff788/LICENSE
[pi-siwc]: https://github.com/earendil-works/pi/blob/005af57d88ee23b33778f343a9595b32e67ff788/packages/ai/src/auth/oauth/openai-chatgpt.ts
[pi-codex]: https://github.com/earendil-works/pi/blob/005af57d88ee23b33778f343a9595b32e67ff788/packages/ai/src/auth/oauth/openai-codex.ts
[pi-openai-provider]: https://github.com/earendil-works/pi/blob/005af57d88ee23b33778f343a9595b32e67ff788/packages/ai/src/providers/openai.ts
[pi-codex-provider]: https://github.com/earendil-works/pi/blob/005af57d88ee23b33778f343a9595b32e67ff788/packages/ai/src/providers/openai-codex.ts
[pi-codex-api]: https://github.com/earendil-works/pi/blob/005af57d88ee23b33778f343a9595b32e67ff788/packages/ai/src/api/openai-codex-responses.ts
[pi-resolve]: oauth-login.md#pi-公用-credential-生命周期
[siwc-overview]: https://developers.openai.com/siwc/token-sharing-open-source
[siwc-sign-in]: https://developers.openai.com/siwc/token-sharing-open-source/sign-in
[siwc-sessions]: https://developers.openai.com/siwc/token-sharing-open-source/profiles-and-sessions
[siwc-tokens]: https://developers.openai.com/siwc/token-sharing-open-source/token-reference
[siwc-inference]: https://developers.openai.com/siwc/token-sharing-open-source/models-and-inference
[siwc-limitations]: https://developers.openai.com/siwc/token-sharing-open-source/preview-limitations
[siwc-vm]: https://developers.openai.com/siwc/token-sharing-open-source/self-hosted-vms
[openai-discovery]: https://auth.openai.com/.well-known/openid-configuration
[openai-jwks]: https://auth.openai.com/.well-known/jwks.json
