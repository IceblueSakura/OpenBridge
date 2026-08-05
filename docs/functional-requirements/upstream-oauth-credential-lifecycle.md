# ChatGPT subscription OAuth credential lifecycle

## 状态

**本文定义已批准、但必须串行进入当前焦点的两阶段范围。** 第一阶段只完成 ChatGPT Provider 与真实上游 probe；第二阶段再实现
OAuth2 PKCE 登录和 token 续约。当前实施授权只来自[当前开发焦点](../implementation-plans/current-focus.md)，因此第二阶段在第一阶段
收口并另立焦点前不得并行实现。

外部行为以当前 Codex 源码为主要实现基线，产品文档只补充公开登录与凭证存储边界。每次进入焦点必须记录实际源码 commit、Codex CLI
版本、平台和真实验收日期，不能把历史快照或一次成功调用视为长期稳定协议。

- [Codex 设备登录与 token 刷新调研](../references/codex/codex-device-auth-token-refresh-analysis.md)
- [Codex 浏览器 OAuth 调研](../references/codex/codex-oauth-and-tool-call-analysis.md)
- [OAuth 设备登录与 token 刷新综合调研](../references/cross-project/upstream-oauth-device-code-token-refresh-analysis.md)

## 1. 两阶段范围

### 1.1 第一阶段：Provider 与只读 credential probe

第一阶段的用户可观察结果是：管理员显式运行 probe 时，OpenBridge 可以从指定的 Codex file credential store 只读加载当前 ChatGPT
access token 与账户绑定，使用与同机 Codex CLI 完全一致的 `User-Agent` 请求固定 ChatGPT Codex backend 的模型目录和一个 Responses
文本调用，并只输出脱敏结果。

第一阶段必须满足：

1. ChatGPT 是独立 `Provider`，不能复用 `OpenAI` API-key Provider 身份、endpoint profile 或 credential pool。
2. endpoint、路径和必要普通 header 来自编译期 ChatGPT/Codex profile；业务请求、credential 文件和 probe 参数都不能覆盖上游 URL、
   model path、`originator`、`version` 或任意 header。
3. 当前 Codex profile 只注册固定的 Codex backend base、`GET models` 与 `POST responses`；不声称 Chat Completions、Embeddings、
   WebSocket 或其他 ChatGPT resource API。
4. ChatGPT target 默认禁用且不加入 Route/Public Model，只允许显式管理员 probe 选择。常驻服务启动不读取 Codex credential，也不因该
   target 要求新的运行时 secret。
5. Codex auth 文件路径由管理员显式选择；loader 只读取一次，不写回、不删除、不改变时间戳、不取得或保留 refresh token，也不复制文件到
   OpenBridge 配置、fixture 或输出目录。
6. loader 只接受当前 Codex ChatGPT auth 形状，提取本次请求所需的 access token、账户绑定、FedRAMP routing claim 和可验证的
   access-token expiry；API key、personal access token、缺失账户、空 token、无效 JSON/JWT 或已经过期的 token 在网络前失败。
7. `User-Agent` 必须由同机 Codex CLI 的当前运行时结果提供，并在测试中逐字节比较；只根据版本号猜测或长期硬编码字符串不满足要求。
   `originator`、版本、account header 和按账户条件启用的 FedRAMP header 同样按该次 Codex 源码/CLI 基线验证，但其值不得出现在
   probe report。
8. 第一阶段不读取 refresh token、不执行 refresh、不在 401 后重放、不启动浏览器或设备码登录。过期、401、403 或账户不匹配只产生脱敏
   失败并保持 auth 文件不变。

真实 probe 只证明指定账户、指定模型、当次 endpoint 和当次 Codex 请求身份可用；它不证明第三方 gateway 获得通用授权、生产稳定性、
其他订阅计划、多账户兼容或未来 endpoint 不变。

### 1.2 第二阶段：PKCE 登录与 token 续约

第二阶段在单独焦点中实现 OpenBridge 自己管理的 ChatGPT OAuth credential：

1. 显式管理员登录入口实现 authorization-code + PKCE `S256`，并按届时 Codex 源码决定浏览器 callback 与设备交互的具体 adapter；
2. 登录临时状态、authorization code、PKCE verifier 和 device state 只在有界会话中存在，失败或取消后清除；
3. token exchange 后校验 issuer、audience、scope、账户/workspace 与 access-token expiry，再原子持久化完整 credential bundle；
4. 请求前按 expiry safety window 合并 refresh，401 recovery 先 reload、再至多一次 refresh/重放，并受 response commit 边界约束；
5. rotated refresh token 与 access token、expiry、identity 和 generation 原子写回，终态错误转为 `reauth_required`；
6. 第二阶段完成前，第一阶段的 Codex auth 文件仍只是一次性只读验收来源，不能演变为常驻服务的隐式 credential backend。

第二阶段不是当前焦点。其失败测试、持久化 backend、并发模型和真实登录验收必须在第一阶段完成后重新核对当前 Codex 源码再确定。

## 2. Provider OAuth preflight

进入第二阶段或把 ChatGPT target 接入常驻数据面前，必须用 Provider 官方资料、当前 Codex 源码和明确授权确认：

- authorization server、issuer、device authorization endpoint 与 token endpoint；
- client registration、client 类型、允许的 grant 和 client authentication；
- scope、resource/audience、redirect/device flow；
- access/refresh token lifetime、rotation、revocation 与 inactivity policy；
- account/workspace/organization 绑定和必要的非 secret header；
- subscription 使用资格，以及自动化 gateway/proxy 场景的允许范围；
- reauthorization、用户撤销、管理员禁用和 credential 删除流程。

Codex 内置的 client identity、私有 endpoint、redirect、scope 或 header 只能证明该 Codex 快照的实现行为。第一阶段的明确本机验收授权
不自动扩大为公开协议或生产承诺。

## 3. 登录入口与控制面边界

第二阶段的登录必须是显式运维命令或受保护的管理操作，不能在普通模型请求路径中自动开始。

1. Provider、endpoint、client registration 和 scope 只能来自受信注册；下游业务请求不能提供或覆盖。
2. login session 使用 Provider 给定的 TTL，只向发起者显示 verification URI 与一次性 user code。
3. 标准 Provider 严格实现对应标准语义；Codex 私有 device interaction 使用独立、明确命名的 adapter。
4. token exchange 后校验 issuer、audience、scope 与 account/workspace allow-list。
5. 完整 credential 写入 secret backend 后才返回登录成功。
6. cancel、denied、expired 或校验失败时清除临时 state，不持久化半成品 token。
7. 界面必须提示只有本人主动发起登录时才输入 code，降低 device-code phishing 风险。

不得在普通请求因 refresh 失败时自动退回交互式登录，也不得导入 Hermes、LiteLLM 或其他应用的 auth cache。

## 4. Credential bundle

第二阶段的可刷新 credential 至少以同一版本管理：

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
| OAUTH-01 | ChatGPT 使用独立 Provider、OAuth bearer credential kind、固定 Codex backend endpoint profile 与 Responses-only adapter；OpenAI API-key Provider 行为不变。 |
| OAUTH-02 | 第一阶段 target 默认禁用、没有 Route/Public Model，只能由显式管理员 probe 选择，常驻服务不要求或读取 Codex credential。                                   |
| OAUTH-03 | Codex auth loader 只读最小字段，过期、错型、缺失账户或损坏文件在 egress 前失败；文件内容、token、账户和路径不进入报告、日志或测试 fixture。                 |
| OAUTH-04 | 模型目录与 Responses probe 使用当前 Codex 源码定义的路径、query、认证和普通 header；`User-Agent` 与同机 Codex CLI 运行时值逐字节一致且不可由业务请求覆盖。 |
| OAUTH-05 | 真实验收记录 Codex source commit、CLI 版本、平台、endpoint、model、HTTP/SSE 终态和脱敏 header parity；未运行层不声称成功。                                 |
| OAUTH-06 | 第二阶段登录使用 PKCE `S256`、有界 state/device session、严格 callback/token 校验和失败清理，不在普通请求中隐式启动。                                      |
| OAUTH-07 | 第二阶段 refresh 具有 expiry safety window、single-flight、guarded reload、原子 rotation 写回和有界 401 recovery。                                       |
| OAUTH-08 | refresh 终态错误、账户变化或 rotation 歧义 fail closed；不会泄露 token、跨账户重放或自动切换到其他 credential。                                           |

## 9. 仍不在范围内

- subscription 多账号池、账号级负载均衡、余额/配额控制面或账号自动轮转；
- 把第一阶段 probe target 暴露为 Public Model 或常驻数据面 Route；
- 导入 Hermes、LiteLLM 或任意非 Codex credential cache；
- 下游 user OAuth、平台代理 authorization server、动态 endpoint/client registration/scope；
- 未经重新核对当前 Codex 源码和真实验收就复制私有 flow；
- keyring、远程 secret manager、跨主机 refresh 协调或多实例共享 credential；这些能力需要另行批准。
