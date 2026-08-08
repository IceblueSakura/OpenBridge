# Bootstrap、代码注册表、凭证与受信边界

## 状态

**当前约束。** OpenBridge 是单配置所有者管理的 headless 网关。Provider contract、Model、 Upstream Target、Upstream
API、Route、Public Model、endpoint、能力和字段转换由 Rust 代码显式注册；运行时配置不提供 Provider DSL，也不支持 route 热重载。

[Model 目录与 Provider 接入配置](model-catalog-configuration.md)目前是待定方案，不属于本页当前约束，也不进入
实施。除非再次明确批准，启动过程、注册表所有权和验收要求继续以代码注册方式为准。

## 1. 所有权划分

| 来源                                        | 内容                                                                                                                            | 能否包含 secret                    |
|---------------------------------------------|---------------------------------------------------------------------------------------------------------------------------------|------------------------------------|
| `config/bootstrap.toml`                     | loopback listener、两份私有 credential 文件位置、request/JSON response/replay/SSE 上限、共享 HTTP client 参数、默认禁用的 OTLP/HTTP 导出策略 | 否                                 |
| 被忽略的 `config/users.toml`                | 下游用户、API Key 与启停状态                                                                                                    | 是                                 |
| 被忽略的 `config/upstream-credentials.toml` | 编译期 credential binding id 与互斥的有序 API key 或单一 OAuth2 `auth_json_file` locator；来源是否存在决定已注册 pool 的启动激活状态 | API key 是；locator 本身不是 secret |
| `src/models/*`                              | Model 事实、token 限制、参数和 reasoning                                                                                        | 否                                 |
| `src/providers/*`                           | Provider Family 定义、Provider 实例、共享协议机制、request-header hook、target/upstream API、credential pool/binding、route 与 Public Model | 否                                 |
| 下游业务请求                                | Public Model 和模型调用参数                                                                                                     | 否；也不能选择 endpoint/credential |

每个运行配置都有同名 `.example` 模板：`config/bootstrap.example.toml`、`config/users.example.toml` 和
`config/upstream-credentials.example.toml`。模板不得包含真实凭证；Bootstrap 模板由测试约束为与默认配置一致。 Bootstrap
schema v2 要求 `max_request_body_bytes`、`max_json_response_body_bytes`、
`max_replay_body_bytes` 与 `max_sse_event_bytes` 均为非零值，并要求 replay limit 不大于 request limit；这些
字段职责独立，不互相提供缺省或回退。

当前只允许 `OPENBRIDGE_CONFIG` 改变 bootstrap 文件位置；两份私有 credential 文件位置由 bootstrap 固定。不存在
`OPENBRIDGE_ROUTES_CONFIG`，CLI 也不能注入 Provider、URL、header、model id 或转换规则。

OTLP exporter 属于启动时进程资源策略：`[telemetry.traces]` 与 `[telemetry.metrics]` 分别默认禁用，collector 地址只能来自
bootstrap，并允许配置所有者选择 loopback、非 loopback IP 或 DNS host；不接受 URL credential、自定义认证 header、环境注入 header
或业务请求覆盖。无效 scheme、缺失 host、自定义 path/query/fragment 或不支持字段必须在 listener 与 exporter egress 前阻止启动；
signal path 固定为 `/v1/traces` 或 `/v1/metrics`，exporter 不得成为 Provider、Route、credential 或动态控制配置入口。

## 2. 代码注册表要求

本节约束逻辑所有权和受信装配结果，不把当前 Rust 文件名、目录层级或 facade 形式固化为产品契约。当前物理模块边界见
[当前代码架构](../implementation-status/current-architecture.md)，维护规则见仓库 `AGENTS.md`。

- 每个具体 Provider family 必须有唯一、闭合的静态 definition owner；同一 wire family 的协议机制可以由受信编译期代码共享；
- 静态 Provider definition 不自动构成运行链路；只有显式加入 compiled target、Route 与 Public Model 后才可被请求选择；
- 每个 canonical Model 必须由一个显式定义完整拥有自身事实；目录聚合不能提供会隐式扩大单个 Model 能力的运行时默认值；
- 编译目录必须只有一个显式 composition root；Provider、Model、Target、API、Route 与 Public Model 不得通过链接器、反射或文件扫描自动注册；
- 不使用运行时插件、链接器自动注册、JSON/TOML 转换模板或脚本；
- Provider contract 定义 adapter 能力上界和 credential kind；
- Provider 实例绑定一个 `ProviderKind` 与一个受信 BaseURL；同一 Family 的不同区域或其他多 URL 部署必须注册为不同实例，不能由
  Upstream API、Route 或业务请求选择 URL；
- Model 定义模型事实、token 限制、支持参数、reasoning 状态与 reasoning level；
- Credential Pool 绑定 Provider、credential kind 和一个有序 API-key member 集合；多个同 Provider Target 可引用同一个
  pool，但不能跨 Provider 或 credential kind 复用；
- Upstream Target 引用一个 Provider 实例，并绑定 Model、credential pool、timeout 和共享故障边界；
- 每个 Upstream Target 对每个 `OperationKind` 最多注册一个 Upstream API；API 的 capabilities variant 是 operation 的唯一事实源，
  transport 由 operation 固定，API 不再拥有字符串 ID 或 endpoint profile；
- Upstream API 独立声明一个 operation 的 upstream model、served limit、能力、state affinity，以及可选的 canonical reasoning
  level 到安全上游 wire 值的显式映射；Route 以 Target + typed upstream operation 引用它；
- 同一 Public Model 可以显式列出多个 Provider route source；相同 canonical Model ID 本身不得触发自动发现、 隐式 Route 注册或
  Provider 聚合；
- Public Model 保存由这些 source 生成的有序完整 Route；对每个下游协议，代码目录先按 source 声明顺序排列 Native
  Route，再按相同顺序排列 Bridge Route；
- 启动监听前必须完成唯一性、引用、能力、reasoning、credential pool 和 URL 校验。

修改 Provider、Model 或路由必须重新编译并重启。项目不要求热重载。

## 3. 凭证

- 下游用户表只在启动时读取；用户增删、启停和 API Key 轮换都需要重启；
- 用户 ID 和 API Key 必须唯一，至少有一个启用用户，API Key 不得少于 32 bytes；
- 认证成功后只把不含 Key 的 `Arc<User>` 放入请求上下文；
- 代码注册表只保存非敏感 pool/member id、Provider 和 credential kind，不保存 secret 或 secret locator；
- 服务与常规 API-key probe 只从 bootstrap 指定的私有 upstream credential TOML 读取上游 API key，不读取 `*_API_KEYS`、旧单值
  环境变量或 `.env`；任何 probe 都不得发现或导入本机 Codex credential、环境或 terminal 状态；
- TOML 只允许声明 `schema_version` 与 `credential_pools`；每项包含编译期 binding id，并且可以选择有序 `api_keys` 数组、单一
  `auth_json_file` locator 或不提供 source（未激活），不能配置 Provider、credential kind、endpoint、route 或 member id；
- 未由代码注册的 pool、重复 pool、空白成员或 pool 内重复 secret 必须在 listener 绑定或网络 probe 前失败；缺少已注册 pool、无 source
  的已知 pool 或空 API-key 数组表示该 pool 本次启动未激活，不构成动态 Provider 注册；
- 服务在监听前把已启用用户 Key 与所有已激活 API-key Target 引用的 pool 一次性装入不可变 `CredentialStore`，并把所有显式配置的
  OAuth2 auth 文件装入内部可变、对外 snapshot 化的 `OAuth2CredentialManager`；完整过期 bundle 作为立即 refresh 输入而不是损坏文档；
- `CredentialId` 必须区分 `DownstreamUser` 与带 `ProviderKind` 的 `UpstreamPoolMember`，上下游同名 ID 不得造成命名冲突；
- 每个 credential 条目必须冻结受控的 type、source、从 1 开始的 generation 与可选过期时间；source 只保存
  `UserConfiguration`、`UpstreamConfiguration`、`OAuth2AuthJsonFile` 或 `Programmatic` 类别，不能把文件路径、
  issuer URL 或任意业务字符串作为诊断元数据；
- `RuntimeRegistry` 与 `UserRegistry` 不保存 secret；`CredentialStore`、两类注册表、日志、错误响应和 probe report 的
  Debug/输出都不得包含 secret；
- 下游认证只能经 Store 的 constant-time 匹配返回用户 ID；上游只能按完整
  `pool_id + member_id + ProviderKind + CredentialKind` 借用短时 credential 视图，不提供通用明文查询；
- 缺失、空值、零 generation、重复下游 Key 或 binding/Provider/credential kind 不匹配时 fail closed；已注册但未激活的 pool 只会让其
  引用的 Target 在本次启动中不可执行；显式配置但不存在的 OAuth2 `auth_json_file` 在启动时创建为空文件并保持待登录，不构造
  credential snapshot；
- 运行时不得重新读取 `users.toml` 或 `upstream-credentials.toml`；改变用户、API Key 或 locator 必须重启。OAuth2 manager 只可在
  expiry-driven refresh 或首个预提交 `401` recovery transaction 中通过同主机 advisory lock guarded reload 自有 auth 文件，并将完整
  rotation 原子写回后发布新 generation；普通成功路径不读文件，任何请求都不得触发交互式登录。当前不支持通用热更新；
- 业务请求不能提供或覆盖 Authorization、cookie、Host、proxy header 或上游 credential；Provider 的受信代码可声明固定的非敏感
  `User-Agent` 与普通 header，也可通过 hook 按编译期规则增添、替换、转换或删除普通 header。固定 header 在 hook 后应用，业务请求
  不能覆盖；authentication header 最后从 purpose-bound credential 生成。共享层不维护普通 header allowlist，具体 Provider 的 header
  值属于实现事实，不应在本需求文档中固化。

### 3.1 上游 API-key pool

- pool 与 member 都使用稳定、非敏感 ID；member secret 只来自私有 upstream credential TOML，业务请求 不能提供
  pool/member、改变顺序或扩大候选集合；member ID 只能由 pool id 与数组顺序派生，不能 由 secret 内容派生；
- 一个激活的 API-key pool 至少包含一个 member；member ID 必须唯一，所有 member 必须属于同一 Provider 和 credential kind，重复
  secret 必须拒绝；单 member pool 与现有单 key 行为等价；未激活 pool 可以没有 member；
- 同一个 pool 可由同 Provider 的多个 Target 引用，使 key cooldown 与 round-robin cursor 跨模型共享；不得 为每个模型复制同一组
  key 后形成互不知晓的健康状态；
- 每个 API-key pool 只有一个 TOML `api_keys` 数组；未知或重复 pool、空白或重复 member 必须在 listener 绑定前 fail closed；缺少
  pool、source-less pool 或空数组只表示该 pool 未激活。本阶段不提供环境变量 fallback、member 级 enabled 或热增删；
- `CredentialStore` 继续不可变地持有 secret。运行时可变状态只保存 pool cursor、member binding ID、 generation 与 cooldown
  deadline，不保存、复制或重新读取 secret；
- pool 选择只返回短时 credential 借用视图；每次 attempt 必须重新构造敏感认证 header，不能缓存或复用 上一次 member 的
  header；
- `previous_response_id` 等 `TargetBound` Upstream API 在没有 credential affinity 证据或 ledger 时不得引用 多 member
  pool，避免 continuation 在不同账号/key 间漂移；
- 更换 API key、改变 pool member 或顺序仍需重启。API-key pool 不承担 OAuth、余额查询、keyring、加密 secret 文件、远程 secret
  manager、动态 reload 或跨进程 pool 状态；ChatGPT OAuth 使用独立 credential kind 和生命周期要求。

### 3.2 ChatGPT 本地状态隔离

- 四个 ChatGPT target 使用同一个独立 `OAuth2BearerAccessToken` pool，并各自只加入一个 Responses-native Route/Public Model；
  通用 probe 只允许选择已启用 target，ChatGPT Models probe 可显式借用该 pool 的 OAuth manager lease；
- OpenBridge 不搜索 `$CODEX_HOME`、Codex auth cache 或其他本机 Agent 状态，不接受 probe 专用 Codex auth file 或 executable selector；
- OpenBridge 不读取 terminal 相关环境变量，不根据本机 OS、architecture 或 terminal 构造 Codex-compatible 请求身份，也不启动 Codex
  CLI 或 app-server；
- ChatGPT credential 只能来自下节定义的 OpenBridge-owned OAuth2 auth 文件；服务数据面和显式 ChatGPT Models probe 只可通过 manager 的短生命周期
  lease 借用当前 generation，不能通过 CLI 参数或本机 Agent 状态隐式获取 credential；
- 显式登录、可刷新 bundle、持久化、数据面借用和 guarded reload/refresh/401 recovery 以
  [ChatGPT subscription OAuth lifecycle](upstream-oauth-credential-lifecycle.md)为准。

### 3.3 OpenBridge-owned OAuth2 auth 文件

- OAuth2 auth 文件路径只来自 private upstream credential TOML 的 `auth_json_file`；相对路径以该 TOML 所在目录为基准，业务请求、
  Provider response 和 probe 参数不能覆盖；
- 配置项仍使用编译期 credential binding id，loader 必须从 `RuntimeRegistry` 解析唯一 Provider 与
  `OAuth2BearerAccessToken` kind；TOML 不获得动态 Provider 选择权；
- 每个 OAuth2 Provider 最多配置一个 auth 文件，并派生一个稳定的内部 member id；本阶段不提供 auth 文件数组、账号 pool、轮转、
  cooldown 或负载均衡；
- ChatGPT 文件使用当前兼容的 OAuth 字段形状，但由 OpenBridge 独立拥有；不得默认、搜索、导入或回退到
  `$CODEX_HOME/auth.json`；
- 文件在 listener 绑定前完成首次读取；不存在时在 advisory lock 内以排他方式创建空的 OpenBridge-owned 文件并保持待登录，非空文件仍须通过
  完整校验；之后只允许显式登录事务、expiry-driven refresh 或首个预提交 `401` recovery transaction 在 advisory lock 内 guarded reload，
  rotation 只能原子替换。错误、`Debug`、日志和 metric 不得包含 locator、token、账户或完整 auth record；
- `OAuth2CredentialManager` 对外只发布脱敏 snapshot，对内维护 guarded reload、single-flight、refresh、generation 与后台调度；
  数据面只能取得不暴露 locator/完整 bundle 的短生命周期 credential lease，并按同一账户/Provider 边界执行一次有界 `401` recovery。
- 当前不提供运行中换账户 API 或热重载。换账户必须先停止服务，手动删除该 binding 的 OpenBridge-owned `auth_json_file` 及同一登录流程明确
  创建的其他 OpenBridge-owned 授权文件（如有），再显式登录并重启；不得借此搜索、导入或删除本机 Codex auth cache。

## 4. Endpoint 与出站边界

Endpoint 只来自代码注册的 Provider 实例。每个实例只有一个 BaseURL；Provider adapter 对每个受支持 operation 只提供一条静态相对
path，因此一个实例对每个 operation 至多形成一份上游 URL。Registry builder 必须拒绝：

- 非 HTTPS endpoint；
- 缺少 host；
- userinfo、query 或 fragment；
- 双斜线、空 segment、`.`、`..`；
- 编码斜线或不受限字符构成的 path prefix。

共享 transport 只能把 Provider adapter 生成的相对 path 追加到已校验 endpoint base，且禁用 redirect。业务请求、adapter 和
credential 均不能替换 endpoint origin。

## 5. 生命周期

```text
read bootstrap.toml
→ validate BootstrapConfig
→ read users.toml
→ validate UserConfiguration and collect downstream credentials
→ read upstream-credentials.toml
→ validate UpstreamCredentialConfiguration
→ derive redacted active credential-pool set
→ compiled_config()
→ validate and build RuntimeRegistry with active Target eligibility
→ bind active API-key pools and configured OAuth2 auth files by compiled binding id
→ build immutable CredentialStore + OAuth2CredentialManager
→ render configuration-only Provider/Public Model availability without Provider egress
→ create shared HTTP client
→ Arc<RuntimeRegistry> + Arc<UserRegistry> + Arc<CredentialStore> + Arc<OAuth2CredentialManager>
→ start listener
```

完成全部私有配置和 credential binding 校验后、绑定 listener 前，主服务以默认 info 日志输出两张 ASCII 双列表格。Provider family
至少有一个经 active pool 收窄后 enabled 的 Target 时进入可用列；Public Model 至少有一个已编译执行接口时进入可用列。另一列只显示
稳定 Provider/Public Model 名称、Target 计数、接口和脱敏原因，不得显示 pool、Target、Route、endpoint、auth-file locator 或 secret。

表头必须明确标注 `configuration only` 和 `no network probe`。这里的“可用”只表示本次启动配置允许形成执行候选：OAuth2 auth-file
locator 仍按既有 active-pool 语义参与配置筛选，可能处于待登录状态；该表不证明当前 credential lease、网络、配额、远端模型或协议能力
实际可用。无效配置继续在表格输出前阻止启动，真实探测只能由管理员显式运行独立 probe。

注册表与 credential manager 启动后不可变。服务没有文件监听、user/route/auth reload、`ArcSwap` 或部分更新语义。运行中的请求和
后续请求都读取同一组 `RuntimeRegistry`、`UserRegistry`、`CredentialStore` 与 `OAuth2CredentialManager`；改变任一启动输入都必须重启。

## 6. 验收要求

| ID     | 行为                                                                                                                                                           |
|--------|----------------------------------------------------------------------------------------------------------------------------------------------------------------|
| CFG-01 | 仓库不存在 Provider/Model route 配置文件或动态 Provider schema。                                                                                               |
| CFG-02 | 代码注册表中的重复 ID、未知引用、能力扩大、无效 reasoning/level 映射和不安全 URL 在监听前失败。                                                                |
| CFG-03 | 业务请求无法覆盖 endpoint、真实 model、credential、敏感 header 或 candidate 顺序；普通 header 只能由受信 Provider 代码声明或转换，固定 UA/header 在 hook 后应用，业务请求不能选择规则或覆盖固定值。 |
| CFG-04 | secret 不进入代码注册项、`RuntimeRegistry`、日志、错误或 probe report。                                                                                        |
| CFG-05 | 每个 Provider family 由独立、闭合的 definition owner 管理，并经单一显式 composition root 注册；不存在自动注册。                                             |
| CFG-06 | bootstrap 只控制 listener、文件位置、资源上限、HTTP client 与默认禁用的 telemetry 导出等进程资源策略，不能注册或修改 Provider；collector host 可由配置所有者选择。 |
| CFG-07 | listener 只允许 loopback；非 loopback 地址必须在监听前拒绝。                                                                                                   |
| CFG-08 | 用户文件中的无效 schema、重复 ID/Key、短 Key 或无启用用户会阻止启动。                                                                                          |
| CFG-09 | 上下游 secret 只进入启动时不可变 `CredentialStore`；运行时按用途受限接口访问，不重新读取来源。                                                                 |
| CFG-10 | 私有 upstream credential TOML 出现未知或重复 pool、空白/重复 secret 或不能解析时，会在 listener 绑定前阻止服务启动；缺失或为空的已注册 pool 会让其引用 Target 在本次启动中不可执行。 |
| CFG-11 | 同 Provider 的 Target 可引用共享 API-key pool；激活 pool 必须满足 Provider/kind 与 member 约束，未激活 pool 不要求 secret。                                               |
| CFG-12 | 多 member pool 不得用于缺少 credential affinity 证明的 `TargetBound` Upstream API。                                                                            |
| CFG-13 | 四个 ChatGPT target 只进入固定 Responses-native Route/Public Model；请求和 probe 都不接受本机 Codex auth、environment、terminal 或 executable selector，OAuth credential 只从 OpenBridge-owned 配置加载并由 manager 受控借用。 |
| CFG-14 | Provider 实例唯一拥有一个受信 BaseURL；Target 必须引用已注册实例，不同 URL/区域使用不同实例，业务请求不能覆盖实例或 URL。                                            |
| CFG-15 | 每个 Target 对每个 `OperationKind` 最多注册一个 Upstream API；Route、probe、telemetry 与 continuation issuer 使用 typed upstream operation，不依赖 API 字符串 ID。 |
| CFG-16 | Upstream API 的 operation 只由 capabilities variant 决定；当前 transport 由 operation 固定，注册表不保留独立 operation、transport 或无执行语义的 endpoint profile。 |
| CFG-17 | 主服务在配置验证后、listener 前输出配置态 Provider/Public Model 可用/不可用双表；分类复用 active Target/执行接口且不触发 Provider egress，不输出 credential 或内部拓扑，也不把配置态结果声明为真实健康。 |

## 关联文档

- [Public Model 与模型能力契约](model-information-and-capability-contract.md)
- [待定 Model 目录与 Provider 接入配置](model-catalog-configuration.md)
- [ChatGPT subscription OAuth credential lifecycle](upstream-oauth-credential-lifecycle.md)
- [当前代码架构](../implementation-status/current-architecture.md)
- [当前实现总览](../implementation-status/current-implementation.md)
- [Models 与基础 API 探测](../implementation-status/capability-probing.md)
