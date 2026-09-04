本文定义 Grok subscription credential 的固定 Provider、OpenBridge-owned auth 文件、显式 RFC 8628 device 登录、
expiry-driven refresh、短期 credential lease 与有界 `401` recovery。

OpenBridge 不导入任何第三方应用的登录态，也不从 OS、environment 或 terminal 推导身份。数据面只使用代码注册的
backend/model/request identity 和 OpenBridge-owned credential；普通请求不会隐式登录或选择账户。管理员 probe 只能借用
所选 Target 的 manager lease，不改变 registry 或 credential binding。

外部 OAuth authority、client registration 与订阅 proxy 协议在扩大范围前必须重新核对 Provider 官方资料；参考项目
快照、单次匿名端点探测或单次成功调用不形成长期协议承诺。

- [Grok 订阅 OAuth 登录路径与信息来源](../../references/providers/grok-oauth.md)

## 1. Provider 与 credential 边界

### 1.1 Provider 与 OpenBridge-owned 启动快照

必须满足：

1. Grok 是独立 `ProviderKind` 与 Provider instance，不复用任何其他 Provider 的 instance 或 credential pool。
2. BaseURL、operation path、credential kind 与订阅 CLI proxy 身份头来自受信 Rust 注册；业务请求和 credential 文件
   不能覆盖上游 URL、model path 或任意 header。
3. `grok-4.6` 拥有一个固定 Responses-native Grok Target，是 `grok-4.6` Public Model 在 OpenRouter source 之外的
   订阅 source；未配置或禁用 `grok-cli` pool 时该 Target 不进入候选，公开能力交集退回其余 enabled source。
4. private upstream credential TOML 可为 Grok OAuth2 binding 显式配置一个 OpenBridge-owned `auth_json_file`；不得
   默认、搜索、导入或回退到任何第三方客户端的登录缓存。
5. 启动 loader 要求 auth 文件已存在，并校验完整 id/access/refresh token bundle、账户绑定与 access-token expiry，
   再把固定 binding/locator/wiring 装入 `OAuth2CredentialManager`；缺失、空白、损坏或不完整 bundle 阻止启动，完整
   过期 bundle 进入立即 refresh。独立 login CLI 可以从 missing version 事务性创建一次完整文件。
6. Grok 只使用受信 Rust definition 固定的订阅 CLI proxy 身份头与 UA；这些常量按已记录的参考实现客户端版本生成，
   版本漂移通过提交升级，不接受 auth file、client identity 或 header override selector。
7. Grok adapter 只暴露 Responses 出口；不声明媒体输入，直到订阅 proxy 的媒体 wire 有本地证据。
8. token、账户、locator、JWT payload 和完整 auth record 不进入 report、日志、metric、Debug 或错误。

常驻服务的数据面只能取得 manager 发布的短生命周期、账户绑定 credential lease。它不能读取 auth locator 或完整 bundle，
也不能把 OAuth credential 放入通用 API-key Store、probe 或业务请求可控字段。

### 1.2 Device 登录与 token 续约

显式登录必须满足：

1. `openbridge-auth login grok` 使用 authority 官方声明的标准 RFC 8628 device authorization flow：固定
   device authorization endpoint 创建会话，管理员在浏览器人工批准，按 `interval` 轮询固定 token endpoint 的
   `device_code` grant；登录不调用任何非标准批准端点，不实现 SSO cookie、邮箱密码或自动批准旁路。
2. 轮询严格实现标准语义：`authorization_pending` 按当前 interval 继续；`slow_down` 增加 interval 后继续；
   `access_denied` 与 `expired_token` 终止且不创建 credential。
3. 登录临时状态与 device code 只在有界会话中存在，失败或取消后清除；token 响应要求完整 id/access/refresh token、
   未来 access expiry 与一致账户绑定（OIDC subject），再用 advisory lock、source-version CAS 与同目录 atomic
   replace 持久化完整 credential bundle。
4. CLI 不接受 issuer、client、endpoint、header、auth-file 或其他应用 cache override，普通服务启动和模型请求不
   隐式发起登录。

不提供运行中换账户 API。换账户时必须停止服务，手动删除 private upstream credential binding 指向的 OpenBridge-owned
`auth_json_file`，再执行显式登录并重启。

自动 refresh 必须满足：

1. 按 expiry safety window 合并 refresh，从持久化源 guarded reload，并跨进程/进程内 single-flight；
2. rotated refresh token 与 access token、expiry、identity 和 generation 原子写回，终态错误转为 `reauth_required`
   或 `ambiguous`；
3. refresh 响应省略 `tier` 声明时保留已持久化的订阅档位；subject 变化视为账户绑定变化并拒绝；
4. 429/5xx 与确认未送达错误进入有界 backoff；terminal OAuth code 进入 `reauth_required`；可能已发生 rotation 但
   无法安全落盘的结果进入 `ambiguous` 并停止自动复用旧 token。

数据面只通过 OAuth2 manager 的受控 lease 生成 Provider authentication header，并在首个预提交 `401` 后执行一次
guarded reload、必要时 refresh 和至多一次 replay；第二个 `401` fail closed 为 `reauth_required`。没有独立受信 JWT
signature source 时，校验不得表述为离线 signature、通用 issuer/audience 或 subscription policy 验证；订阅档位只以
解码声明形式作为非 secret 元数据，不参与签名验证。

## 2. Provider OAuth preflight

修改登录/refresh 协议、扩大 Grok 请求面或增加 target 前，必须用 Provider 官方资料、当前参考实现和明确授权确认：

- authorization server、issuer、device authorization endpoint 与 token endpoint；
- client registration、client 类型、允许的 grant 和 client authentication；
- scope、resource/audience 与 device flow；
- access/refresh token lifetime、rotation、revocation 与 inactivity policy；
- 订阅 CLI proxy 的身份头要求、订阅使用资格，以及自动化 gateway/proxy 场景的允许范围；
- reauthorization、用户撤销、管理员禁用和 credential 删除流程。

参考实现中的 client identity、endpoint、scope 或 header 只证明对应快照的实现行为，不自动扩大为公开协议或生产承诺。

## 3. Credential bundle

Grok auth 文件使用闭合 OAuth JSON 字段：顶层 `auth_mode: "grok"`、`OPENAI_API_KEY`、`tokens`、`last_refresh` 与
可选 `subscription_tier`，其中 `tokens` 包含 `id_token`、`access_token`、`refresh_token`（`account_id` 仅 ChatGPT
信封使用）。OpenBridge 不在该文件中加入 Provider、endpoint、pool、status 或 locator 字段；这些非 secret 绑定来自
编译期注册表和私有 upstream credential TOML。

access token、rotated refresh token、expiry 和 identity 必须原子写回。authorization server 返回新 refresh token 时
必须替换旧值；未返回新值时是否保留旧值以 Provider contract 为准。

日志、metric、lock key 和错误只使用非 secret `credential_id` 或脱敏 fingerprint；不得记录 token、authorization
code、device code、user code、账户 ID 或完整 auth record。

## 4. 功能验收要求

| ID          | 行为                                                                                                                              |
|-------------|-----------------------------------------------------------------------------------------------------------------------------------|
| GROK-01     | Grok 使用独立 ProviderKind/Provider instance、OAuth bearer credential kind、固定受信 CLI proxy BaseURL 与 Responses-only adapter。 |
| GROK-02     | `grok/grok-4-6` Target 是 `grok-4.6` Public Model 的订阅 source；`grok-cli` pool 未配置或禁用时不进入候选。                        |
| GROK-03     | Grok OAuth 文件只由 private upstream credential TOML 显式定位并由 OpenBridge 拥有；不得搜索、导入或回退到第三方登录缓存。           |
| GROK-04     | 登录使用标准 RFC 8628 device flow 与人工批准；不实现非标准批准端点、SSO cookie、邮箱密码或自动批准旁路。                          |
| GROK-05     | login CLI 可以从 missing version 事务性创建完整 `auth_json_file`；主服务启动要求文件已存在且完整校验，再构建 `OAuth2CredentialManager`。 |
| GROK-06     | 订阅身份头与 UA 是编译期常量；业务请求与配置不能覆盖。                                                                            |
| GROK-07     | refresh 省略 `tier` 声明时保留已持久化订阅档位；subject 变化拒绝写回。                                                            |
| GROK-08     | 数据面只借用受控 snapshot；401 recovery 先 reload、再至多 refresh/重放一次，并服从 response commit 边界。                          |
| GROK-09     | 不提供运行中换账户；必须停止服务、删除该 binding 的 OpenBridge-owned auth 文件、显式重新登录并重启。                               |
