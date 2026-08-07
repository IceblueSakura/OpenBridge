# ChatGPT subscription OAuth credential lifecycle

## 状态

本文定义 ChatGPT subscription credential 的当前边界与生命周期方向。当前已实现独立的 ChatGPT Provider/Provider instance、
四个固定 Responses-native target/Public Model、从 private upstream credential TOML 显式配置的 OpenBridge-owned OAuth2 auth
文件、显式 ChatGPT private device interaction + PKCE 登录命令、到期驱动的自动 token refresh，以及请求级 credential 借用和有界
`401` recovery。

本机 Codex auth loader、OS/environment/terminal identity 探测和 Codex executable probe 已移除。管理员显式运行的
`openbridge-probe` 仍支持四个已激活 ChatGPT target 的固定 Models probe；该 probe 只通过选定的 OAuth2 manager 借用短期 lease，
不读取本机状态、不改变注册表，也不构成 ChatGPT 数据面或长期 Provider 验收。ChatGPT adapter 可声明固定、按已记录 Codex CLI
release 源码格式生成的 headless Linux x86_64 兼容 UA 与普通 header，但不从本机状态推导 identity，也不接受运行时覆盖。
OpenBridge 不搜索或导入 Codex 用户目录，也不调用 Codex executable 或 app-server。数据面只使用代码注册的固定 backend、model、
请求身份和 OpenBridge-owned credential；普通请求不会隐式登录或选择其他账户。

外部 OAuth 行为仍应在实现前重新核对 Provider 官方资料和届时的参考实现，不能把历史快照或一次成功调用视为长期稳定协议。

- [Codex 设备登录与 token 刷新调研](../references/codex/codex-device-auth-token-refresh-analysis.md)
- [Codex 浏览器 OAuth 调研](../references/codex/codex-oauth-and-tool-call-analysis.md)
- [OAuth 设备登录与 token 刷新综合调研](../references/cross-project/upstream-oauth-device-code-token-refresh-analysis.md)

## 1. 当前边界与后续范围

### 1.1 Provider 与 OpenBridge-owned 启动快照

当前实现必须满足：

1. ChatGPT 是独立 `ProviderKind` 与 Provider instance，不能复用 `OpenAI` API-key Provider instance 或 credential pool。
2. BaseURL、operation path 与 credential kind 来自受信 Rust 注册；业务请求和 credential 文件不能覆盖上游 URL、model path 或
   任意 header。
3. `gpt-5.3-codex-spark`、`gpt-5.6-luna`、`gpt-5.6-terra` 与 `gpt-5.6-sol` 各自拥有一个启用的固定 target，并且只加入一个
   Responses-native Route/Public Model；通用 API-key probe 不借用 OAuth manager credential。
4. private upstream credential TOML 可为 ChatGPT OAuth2 binding 显式配置一个 OpenBridge-owned `auth_json_file`；不得默认、
   搜索、导入或回退到 `$CODEX_HOME/auth.json`。
5. 启动 loader 对存在且非空的 auth 文件校验完整 id/access/refresh token bundle、账户绑定与 access-token expiry，并把它装入内部可变、对外
   snapshot 化的 `OAuth2CredentialManager`；缺失文件在锁内创建为空并保持待登录，不发布 snapshot；完整过期 bundle 进入立即 refresh，非空损坏或
   不完整 bundle 仍阻止启动。
6. OpenBridge 不读取 terminal 相关环境变量，不根据部署主机 OS、architecture 或 Codex state 构造 Agent identity；ChatGPT 只使用受信 Rust
   definition 固定、按已记录 Codex CLI release 源码格式生成的 headless Linux x86_64 兼容 UA/header，并且不接受 auth file、executable、
   client identity 或 header override selector。
7. ChatGPT adapter 只接受 `stream: true` 的 Responses 请求，将标准字符串 `input` 收窄为等价 user message 数组，强制
   `store: false`，并在 egress 前拒绝当前 backend 不接受的输出 token limit 字段；这些字段不得出现在 Public Model 的有效参数集合中。
8. token、账户、locator、JWT payload 和完整 auth record 不进入 report、日志、metric、Debug、错误或测试 fixture。

常驻服务的数据面只能取得 manager 发布的短生命周期、账户绑定 credential lease。它不能读取 auth locator 或完整 bundle，也不能把
OAuth credential 放入通用 API-key Store、probe 或业务请求可控字段。

### 1.2 已实现 PKCE 登录与后续 token 续约

当前显式登录已经满足：

1. `openbridge-auth login chatgpt` 使用固定 ChatGPT private device interaction 与 authorization-code + PKCE `S256`；
2. 登录临时状态、authorization code、PKCE verifier 和 device state 只在有界会话中存在，失败或取消后清除；
3. exchange 只访问编译期固定 HTTPS token endpoint，要求完整 token、未来 access expiry 和一致 account binding，再用 advisory lock、
   source-version CAS 与同目录 atomic replace 持久化完整 credential bundle；
4. CLI 不接受 issuer、client、endpoint、header、auth-file 或其他应用 cache override，普通服务启动和模型请求不隐式发起登录。

当前不提供运行中换账户 API。换账户时必须停止服务，手动删除 private upstream credential binding 指向的 OpenBridge-owned
`auth_json_file` 及同一登录流程明确创建的其他 OpenBridge-owned 授权文件（如有），再执行显式登录并重启；不得操作本机 Codex auth cache。

当前自动 refresh 已经满足：

1. 按 expiry safety window 合并 refresh，从持久化源 guarded reload，并跨进程/进程内 single-flight；
2. rotated refresh token 与 access token、expiry、identity 和 generation 原子写回，终态错误转为 `reauth_required` 或 `ambiguous`；
3. 启动重建 expiry-driven schedule，并在 refresh 成功后发布新的 manager snapshot；
4. 429/5xx 与确认未送达错误进入有界 backoff；terminal OAuth code 进入 `reauth_required`；可能已发生 rotation 但无法安全落盘的结果进入
   `ambiguous` 并停止自动复用旧 token。

数据面已经通过 OAuth2 manager 的受控 lease 生成 Provider authentication header，并在首个预提交 `401` 后执行一次 guarded
reload、必要时 refresh 和一次重放；第二个 `401` fail closed 为 `reauth_required`。当前真实验收覆盖使用已有 bundle 的四个最小文本
Responses 请求，不覆盖真实登录/refresh authority。参考客户端也没有为 OpenBridge 提供可独立建立的 JWT signature trust store；校验不能
表述为离线 signature、通用 issuer/audience 或 subscription policy 验证。

## 2. Provider OAuth preflight

修改登录/refresh 协议、扩大 ChatGPT 请求面或增加 target 前，必须用 Provider 官方资料、当前参考实现和明确授权确认：

- authorization server、issuer、device authorization endpoint 与 token endpoint；
- client registration、client 类型、允许的 grant 和 client authentication；
- scope、resource/audience、redirect/device flow；
- access/refresh token lifetime、rotation、revocation 与 inactivity policy；
- account/workspace/organization 绑定和必要的非 secret header；
- subscription 使用资格，以及自动化 gateway/proxy 场景的允许范围；
- reauthorization、用户撤销、管理员禁用和 credential 删除流程。

参考实现中的 client identity、私有 endpoint、redirect、scope 或 header 只证明对应快照的实现行为，不自动扩大为公开协议或生产承诺，
也不构成重新引入本机 Codex state 探测的理由。

## 3. 登录入口与控制面边界

登录必须是显式运维命令或受保护的管理操作，不能在普通模型请求路径中自动开始。

1. Provider、endpoint、client registration 和 scope 只能来自受信注册；下游业务请求不能提供或覆盖。
2. login session 使用 Provider 给定的 TTL，只向发起者显示 verification URI 与一次性 user code。
3. 标准 Provider 严格实现对应标准语义；Codex 私有 device interaction 使用独立、明确命名的 adapter。
4. token exchange 只接受固定 HTTPS authority 的成功响应，并校验完整 token、access expiry 与 account/workspace binding；若后续引入
   issuer、audience、scope 或 signature trust policy，必须以 Provider 可验证 contract 为依据。
5. 完整 credential 写入 secret backend 后才返回登录成功。
6. cancel、denied、expired 或校验失败时清除临时 state，不持久化半成品 token。
7. 界面必须提示只有本人主动发起登录时才输入 code，降低 device-code phishing 风险。

不得在普通请求因 refresh 失败时自动退回交互式登录，也不得导入 Hermes、LiteLLM 或其他应用的 auth cache。

## 4. Credential bundle

后续可刷新 credential 至少以同一版本管理：

当前 ChatGPT 文件使用兼容的 OAuth JSON 字段：顶层 `auth_mode`、`OPENAI_API_KEY`、`tokens` 与 `last_refresh`，其中
`tokens` 包含 `id_token`、`access_token`、`refresh_token` 和 `account_id`。OpenBridge 不在该文件中加入 Provider、endpoint、
pool、status 或 locator 字段；这些非 secret 绑定来自编译期注册表和私有 upstream credential TOML。

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

access token、rotated refresh token、expiry、scope 和 identity 必须原子写回。authorization server 返回新 refresh token 时必须替换
旧值；未返回新值时是否保留旧值以 Provider contract 为准。

日志、metric、lock key 和错误只使用非 secret `credential_id` 或脱敏 fingerprint；不得记录 token、authorization code、PKCE
verifier、device auth ID、账户 ID 或完整 auth record。

## 5. 到期驱动 refresh

refresh 按 token expiry 调度，不固定周期刷新全部账户：

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

## 6. 请求路径与 401 recovery

1. token 已进入 safety window 时，等待同一 refresh single-flight，而不是每个请求单独刷新。
2. token 仍在安全窗口外时，不为满足固定 timer 强制 refresh。
3. 401 后先 reload；若 credential version 已变化，用新 token 至多重试一次。
4. version 未变且 Provider contract 允许时，可执行一次 refresh，再至多重试一次。
5. 一旦下游业务 response 已开始，不得 refresh 后重放形成第二个上游响应。
6. 第二次 401 或终态 OAuth error 将 credential 转为 `reauth_required`，不能进入无限 refresh、账号轮转或普通 Provider
   fallback。

401 还可能来自 audience、account/workspace header 或授权政策，不等于 access token 一定过期。refresh 前后身份绑定必须一致。

## 7. 失败分类

| 失败                                                | 状态与行为                                             |
|-----------------------------------------------------|--------------------------------------------------------|
| device `authorization_pending`                      | 按当前 interval 继续                                   |
| device `slow_down`                                  | 增加 interval 后继续                                   |
| device denied/expired                               | 终止，不创建 credential                                |
| refresh 429/明确暂态 5xx                            | `refresh_backoff`，受 Retry-After、expiry 和硬预算约束 |
| 确认请求未送达的网络错误                            | Provider policy 允许时有界重试                         |
| rotation 结果不确定                                 | `ambiguous`；不得假定旧 refresh token 有效             |
| `invalid_grant` / reused / revoked                  | `reauth_required` 或 `revoked`，停止自动 refresh       |
| CAS conflict                                        | reload 胜出版本，不能覆盖较新 token                    |
| secret-store write failure after possible rotation | `ambiguous`，不发布仅存在于内存的新 bundle             |

不能只按 HTTP status 决定 refresh retry；OAuth error、是否收到响应、rotation policy、access token 剩余时间和 response commit
状态共同决定结果。

## 8. 功能验收要求

| ID       | 行为                                                                                                                                                      |
|----------|-----------------------------------------------------------------------------------------------------------------------------------------------------------|
| OAUTH-01 | ChatGPT 使用独立 ProviderKind/Provider instance、OAuth bearer credential kind、固定受信 BaseURL 与 Responses-only adapter；OpenAI API-key Provider 行为不变。 |
| OAUTH-02 | 四个固定 ChatGPT target 各自只进入一个 Responses-native Route/Public Model；通用 API-key probe 不借用 OAuth manager credential。 |
| OAUTH-03 | ChatGPT OAuth 文件只由 private upstream credential TOML 显式定位并由 OpenBridge 拥有；不得搜索、导入或回退到本机 Codex state。 |
| OAUTH-04 | 生产代码不从 terminal、部署主机 OS、architecture、environment 或 Codex state 推导 client identity；ChatGPT 只发送编译期固定、按已记录 Codex CLI release 源码格式生成的 headless Linux x86_64 兼容 UA/header，不提供运行时 override 或 Codex auth/executable probe selector。 |
| OAUTH-05 | 启动 loader 为缺失的 `auth_json_file` 创建空的待登录文件；对存在且非空的文件完整校验 OAuth2 bundle，并构建内部 guarded、对外 snapshot 化且脱敏的 `OAuth2CredentialManager`；过期完整 bundle 可立即 refresh。 |
| OAUTH-06 | upstream credential TOML 以互斥的 `api_keys`/`auth_json_file` 绑定编译期 credential kind；每个 OAuth2 Provider 只加载一个 OpenBridge-owned auth 文件。 |
| OAUTH-07 | 数据面只通过短生命周期、账户绑定的受控 credential lease 生成 Provider authentication header，不得把 locator 或完整 token bundle 暴露给普通请求路径。 |
| OAUTH-08 | 登录使用 PKCE `S256`、有界 private device session、固定 HTTPS exchange、完整 token/account 校验、事务持久化和失败清理，不在普通请求中隐式启动。 |
| OAUTH-09 | 自动 refresh 具有 expiry safety window、single-flight、guarded reload 与原子 rotation 写回；终态错误、结果不确定或身份变化 fail closed。 |
| OAUTH-10 | 数据面只借用受控 snapshot；401 recovery 先 reload、再按 Provider contract 至多 refresh/重放一次，并服从 response commit 边界。 |
| OAUTH-11 | ChatGPT Responses adapter 要求 `stream: true`、将字符串 `input` 转为 user message 数组、强制 `store: false`，并在 egress 前拒绝且不公开输出 token limit 参数。 |
| OAUTH-12 | 不提供运行中换账户；用户必须停止服务、手动删除该 binding 的 OpenBridge-owned auth 授权文件、显式重新登录并重启，且不得操作本机 Codex cache。 |

## 9. 仍不在范围内

- subscription 多账号池、账号级负载均衡、余额/配额控制面或账号自动轮转；
- 其他 ChatGPT model、Chat Completions/WebSocket/Batch/Embeddings API、function/hosted tool、MCP、多模态或完整 Agent loop；
- 导入 Codex、Hermes、LiteLLM 或任意其他应用的 credential cache；
- 下游 user OAuth、平台代理 authorization server、动态 endpoint/client registration/scope；
- 未经重新核对当前 Codex 源码和真实验收就复制私有 flow；
- keyring、远程 secret manager、跨主机 refresh 协调或多实例共享 credential；这些能力需要另行批准。
