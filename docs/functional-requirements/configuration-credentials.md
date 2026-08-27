# 配置与凭证合同

本文集中定义 Bootstrap、私有用户、上游 credential、静态注册、endpoint/egress 和 ChatGPT OAuth 生命周期。

Provider contract、Model、Target、Upstream API、Route、Public Model、endpoint、能力与 wire mapping 由受信 Rust
代码显式注册；运行时配置不提供 Provider DSL 或 Route hot reload。Registry、用户、API-key store 与 OAuth manager
wiring/locator 在启动时校验并冻结；OAuth manager 内部 token snapshot/generation 可以按专有 lifecycle guarded
refresh/rotation，但这不改变 registry、Route、账户 binding 或配置拓扑。业务请求不能选择 endpoint、credential、header
policy 或 routing topology。

## 所有权与代码注册表

### 1. 所有权划分

| 来源                                        | 内容                                                                                                                            | 能否包含 secret                    |
|---------------------------------------------|---------------------------------------------------------------------------------------------------------------------------------|------------------------------------|
| `config/bootstrap.toml`                     | loopback listener、两份私有 credential 文件位置、Generation `default_instructions`、request/JSON response/replay/SSE 上限、共享 HTTP client 参数、本地下游 HTTP 内容日志与 OTLP/HTTP 导出策略 | 否                                 |
| 被忽略的 `config/users.toml`                | 下游用户、API Key 与启停状态                                                                                                    | 是                                 |
| 被忽略的 `config/upstream-credentials.toml` | 编译期 credential binding id 与互斥的有序 API key 或单一 OAuth2 `auth_json_file` locator；非空 source 字段决定已注册 pool 是否进入启动激活集合，后续仍须加载校验 | API key 是；locator 本身不是 secret |
| `src/models/*`                              | Model 事实、token 限制、参数和 reasoning                                                                                        | 否                                 |
| `src/providers/*`                           | Provider Family 定义、Provider 实例、共享协议机制、request-header hook、target/upstream API、credential pool/binding、route 与 Public Model | 否                                 |
| 下游业务请求                                | Public Model 和模型调用参数                                                                                                     | 否；也不能选择 endpoint/credential |

每个运行配置都有同名 `.example` 模板：`config/bootstrap.example.toml`、`config/users.example.toml` 和
`config/upstream-credentials.example.toml`。模板不得包含真实凭证；两个 Bootstrap profile 必须解析为相同配置。Bootstrap
schema v3 要求 `max_request_body`、`max_json_response_body`、`max_replay_body` 与
`max_sse_event` 使用“正整数紧邻显式、大小写敏感 SI/IEC byte 单位”的字符串并解析为非零 byte 上限；`upstream_connect_timeout` 与
`upstream_pool_idle_timeout` 使用带单位的 duration 字符串并解析为非零 `Duration`。配置应优先使用明确的 IEC 单位
`KiB`、`MiB`、`GiB` 以及时间单位 `ms`、`s`、`m`、`h`，不得接受旧 `_bytes`/`_ms` 字段或 unitless 数字。
Replay limit 不得大于 request limit；各字段职责独立，不互相提供缺省或回退。

`default_instructions` 是项目级 Bootstrap 字符串；只要启动编译结果保留至少一个可执行的通用 Generation Chat/Responses
interface，它就必须存在且不能是空字符串或纯空白。该值只在客户端没有有效指令来源时回落，并在候选展开前统一写入 canonical
request；它不是 Provider-owned hook，也不要求 canonical Model 重复声明。只有 Embeddings 或专用 ASR/TTS/voice task 可执行时不制造
该要求。旧 `chatgpt_instructions` 因严格 schema 直接拒绝，不提供 alias 或双写。

当前只允许 `OPENBRIDGE_CONFIG` 改变 bootstrap 文件位置；两份私有 credential 文件位置由 bootstrap 固定。不存在
`OPENBRIDGE_ROUTES_CONFIG`，CLI 也不能注入 Provider、URL、header、model id 或转换规则。

`[logging]` 拥有条件必填的绝对 `http_jsonl_directory`，以及 `request_headers`、`request_body`、`response_headers` 与 `response_body` 四个彼此独立的布尔值。仓库随附的
活动开发配置和示例配置显式将四项全部设为 `true`；自定义文档省略整个表或任一字段时，对应解析回退为 `false`。
它们只控制认证成功后的下游客户端 HTTP 边界：header snapshot 在进入 tracing 字段前必须强制脱敏认证、Cookie 与
token/key/secret/password-like header；request body 只能在现有 request limit 内保留，response body 只能在现有 JSON response
budget 内保留前缀，并以 `complete`、`truncated`、captured/observed byte count 区分完整、截断、错误或取消。一个 body 生命周期最多
产生一个本地内容事件，不得逐 SSE chunk/delta 打日志。这些事件受本地 `RUST_LOG` 过滤，不进入只接受 allowlist span 的 OTLP trace
layer，也不构成 OTLP logs。匿名认证失败、原始上游 Provider wire、credential 和 secret 不属于该功能。

OTLP exporter 属于启动时进程资源策略：省略 `[telemetry.traces]` 或 `[telemetry.metrics]` 时对应 signal 禁用；随附的两个开发
profile 显式启用二者并指向 `http://127.0.0.1:4318`。collector 地址只能来自 bootstrap，并允许配置所有者选择 loopback、非 loopback IP 或 DNS host；不接受 URL credential、自定义认证 header、环境注入 header
或业务请求覆盖。无效 scheme、缺失 host、自定义 path/query/fragment 或不支持字段必须在 listener 与 exporter egress 前阻止启动；
signal path 固定为 `/v1/traces` 或 `/v1/metrics`，exporter 不得成为 Provider、Route、credential 或动态控制配置入口。

### 2. 代码注册表要求

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
- Upstream API 独立声明一个 operation 的 upstream model、served limit、能力，以及可选的 canonical reasoning level 到安全上游
  wire 值的显式映射；Responses executable profile 以 `Unbound | TargetBound | TargetBoundContinuation` 判别联合拥有状态归属，
  Route 以 Target + typed upstream operation 引用它；
- 同一 Public Model 可以显式列出多个 Provider route source；相同 canonical Model ID 本身不得触发自动发现、 隐式 Route 注册或
  Provider 聚合；
- Public Model 必须显式选择 `NativeFirst` 或 `SourceFirst`，并保存由 route source 生成的有序完整 Route；策略的
  唯一排序与自动 Bridge 规则见[路由与 Provider 韧性](routing-resilience.md)，本页不重复定义；
- 启动监听前必须完成唯一性、引用、能力、reasoning、credential pool 和 URL 校验。

修改 Provider、Model、Route、用户、API-key pool 或 OAuth binding/locator 必须重新编译或重启。OAuth manager 只可按
专有 lifecycle 更新同一 binding 内的 token snapshot/generation；这不构成 registry 或配置热重载。

## 凭证

### 1. 凭证总则

- 下游用户表只在启动时读取；用户增删、启停和 API Key 轮换都需要重启；
- 用户 ID 和 API Key 必须唯一，至少有一个启用用户，API Key 不得少于 32 bytes；
- 认证成功后只把不含 Key 的 `Arc<User>` 放入请求上下文；
- 代码注册表只保存非敏感 pool/member id、Provider 和 credential kind，不保存 secret 或 secret locator；
- 服务与常规 API-key probe 只从 bootstrap 指定的私有 upstream credential TOML 读取上游 API key，不读取 `*_API_KEYS`、旧单值
  环境变量或 `.env`；任何 probe 都不得发现或导入本机 Codex credential、环境或 terminal 状态；
- 管理员 probe 只能从显式选择的已启用 Target 继承 trusted origin、Provider operation path、timeout 与 credential binding；candidate
  model Generation probe 还要求该 Target 已注册 Generation task，不能借 Embeddings/Images/Audio Target 扩大 operation；model ID
  只覆盖固定合成请求的 `model` 字段，不能覆盖 endpoint、path、credential、header、prompt 或任意 JSON；
- 固定 Generation probe 默认携带 16-token upstream output limit；只有显式 `--allow-unbounded-streaming-output` 才能为拒绝该字段的
  streaming backend 省略限制，报告和使用说明必须暴露该计费/长 reasoning 风险；
- Models probe 必须在完整有界 response 内计算总 ID 数和 candidate 可见性，但报告中的 ID sample 最多保留 1024 项并显式标记截断；
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
  Debug/输出都不得包含 secret；probe report 也不得包含认证 header、完整合成请求正文或完整 upstream response body；
- 下游认证只能经 Store 的 constant-time 匹配返回用户 ID；上游只能按完整
  `pool_id + member_id + ProviderKind + CredentialKind` 借用短时 credential 视图，不提供通用明文查询；
- 缺失、空值、零 generation、重复下游 Key 或 binding/Provider/credential kind 不匹配时 fail closed；已注册但未激活的 API-key
  pool 只会让其引用的 Target 在本次启动中不可执行。显式配置 OAuth2 `auth_json_file` 会激活对应 binding；主服务要求文件已存在且
  包含完整 bundle，缺失、空白或损坏文件都在 listener 前失败；
- 运行时不得重新读取 `users.toml` 或 `upstream-credentials.toml`；改变用户、API Key 或 locator 必须重启。OAuth2 manager 只可在
  expiry-driven refresh 或首个预提交 `401` recovery transaction 中通过同主机 advisory lock guarded reload 自有 auth 文件，并将完整
  rotation 原子写回后发布新 generation；普通成功路径不读文件，任何请求都不得触发交互式登录。这是同一 binding 内的
  credential lifecycle，不是通用配置热更新；
- 业务请求不能提供或覆盖 Authorization、cookie、Host、proxy header 或上游 credential；Provider 的受信代码可声明固定的非敏感
  `User-Agent` 与普通 header，也可通过 hook 按编译期规则增添、替换、转换或删除普通 header。固定 header 在 hook 后应用，业务请求
  不能覆盖；authentication header 最后从 purpose-bound credential 生成。共享层不维护普通 header allowlist，具体 Provider 的 header
  值属于实现事实，不应在本需求文档中固化。

### 2. 上游 API-key pool

- pool 与 member 都使用稳定、非敏感 ID；member secret 只来自私有 upstream credential TOML，业务请求 不能提供
  pool/member、改变顺序或扩大候选集合；member ID 只能由 pool id 与数组顺序派生，不能 由 secret 内容派生；
- 一个激活的 API-key pool 至少包含一个 member；member ID 必须唯一，所有 member 必须属于同一 Provider 和 credential kind，重复
  secret 必须拒绝；单 member pool 与现有单 key 行为等价；未激活 pool 可以没有 member；
- 同一个 pool 可由同 Provider 的多个 Target 引用，使 key cooldown 与 round-robin cursor 跨模型共享；不得 为每个模型复制同一组
  key 后形成互不知晓的健康状态；
- 每个 API-key pool 只有一个 TOML `api_keys` 数组；未知或重复 pool、空白或重复 member 必须在 listener 绑定前 fail closed；缺少
  pool、source-less pool 或空数组只表示该 pool 未激活。不提供环境变量 fallback、member 级 enabled 或热增删；
- `CredentialStore` 继续不可变地持有 secret。运行时可变状态只保存 pool cursor、member binding ID、 generation 与 cooldown
  deadline，不保存、复制或重新读取 secret；
- pool 选择只返回短时 credential 借用视图；每次 attempt 必须重新构造敏感认证 header，不能缓存或复用 上一次 member 的
  header；
- 只有 `TargetBoundContinuation` Responses executable profile 可以接受 `previous_response_id`；在没有 credential affinity ledger
  时，其启用 Target 不得引用多 member pool，避免 continuation 在不同账号/key 间漂移；普通 `TargetBound` 不虚构该限制；
- 更换 API key、改变 pool member 或顺序仍需重启。API-key pool 不承担 OAuth、余额查询、keyring、加密 secret 文件、远程 secret
  manager、动态 reload 或跨进程 pool 状态；ChatGPT OAuth 使用独立 credential kind 和生命周期要求。

### 3. ChatGPT 本地状态隔离

- 五个 ChatGPT Responses-native Target 使用同一个独立 `OAuth2BearerAccessToken` pool。Spark、GPT-5.5、Luna 与
  Terra 分别只为一个 ChatGPT-only Public Model 提供 source；Sol Target 则是还包含 OpenAI 后备 source 的
  `gpt-5.6-sol` Public Model 的 ChatGPT source。通用 API-key probe 不借用 OAuth manager credential，ChatGPT
  probe 只能显式借用所选 Target 的 manager lease；
- OpenBridge 不搜索 `$CODEX_HOME`、Codex auth cache 或其他本机 Agent 状态，不接受 probe 专用 Codex auth file 或 executable selector；
- OpenBridge 不读取 terminal 相关环境变量，不根据本机 OS、architecture 或 terminal 构造 Codex-compatible 请求身份，也不启动 Codex
  CLI 或 app-server；
- ChatGPT credential 只能来自下节定义的 OpenBridge-owned OAuth2 auth 文件；服务数据面和显式 ChatGPT Models probe 只可通过 manager 的短生命周期
  lease 借用当前 generation，不能通过 CLI 参数或本机 Agent 状态隐式获取 credential；
- 显式登录、可刷新 bundle、持久化、数据面借用和 guarded reload/refresh/401 recovery 以
  [ChatGPT subscription OAuth lifecycle](configuration-credentials.md#chatgpt-oauth-credential-生命周期)为准。

### 4. OpenBridge-owned OAuth2 auth 文件

- OAuth2 auth 文件路径只来自 private upstream credential TOML 的 `auth_json_file`；相对路径以该 TOML 所在目录为基准，业务请求、
  Provider response 和 probe 参数不能覆盖；
- 配置项仍使用编译期 credential binding id，loader 必须从 `RuntimeRegistry` 解析唯一 Provider 与
  `OAuth2BearerAccessToken` kind；TOML 不获得动态 Provider 选择权；
- 每个 OAuth2 Provider 最多配置一个 auth 文件，并派生一个稳定的内部 member id；不提供 auth 文件数组、账号 pool、轮转、
  cooldown 或负载均衡；
- ChatGPT 文件使用当前兼容的 OAuth 字段形状，但由 OpenBridge 独立拥有；不得默认、搜索、导入或回退到
  `$CODEX_HOME/auth.json`；
- 主服务在 listener 绑定前完成首次读取并要求完整校验；缺失、空白或损坏文件均阻止启动。显式 login CLI 可以在成功取得并校验
  bundle 后，从 missing version 事务性创建完整文件；之后只有 expiry-driven refresh 或首个预提交 `401` recovery transaction 在
  advisory lock 内 guarded reload，rotation 只能原子替换。错误、`Debug`、日志和 metric 不得包含 locator、token、账户或完整 auth record；
- `OAuth2CredentialManager` 对外只发布脱敏 snapshot，对内维护 guarded reload、single-flight、refresh、generation 与后台调度；
  数据面只能取得不暴露 locator/完整 bundle 的短生命周期 credential lease，并按同一账户/Provider 边界执行一次有界 `401` recovery。
- 不提供运行中换账户 API 或配置热重载。换账户必须先停止服务，手动删除该 binding 的 OpenBridge-owned `auth_json_file` 及同一登录流程明确
  创建的其他 OpenBridge-owned 授权文件（如有），再显式登录并重启；不得借此搜索、导入或删除本机 Codex auth cache。

## Endpoint 与出站

### 1. Endpoint 与出站边界

Endpoint 只来自代码注册的 Provider 实例。每个实例只有一个 BaseURL；Provider adapter 对每个受支持 operation 只提供一条静态相对
path，因此一个实例对每个 operation 至多形成一份上游 URL。Registry builder 必须拒绝：

- 非 HTTPS endpoint；
- 缺少 host；
- userinfo、query 或 fragment；
- 双斜线、空 segment、`.`、`..`；
- 编码斜线或不受限字符构成的 path prefix。

共享 transport 只能把 Provider adapter 生成的相对 path 追加到已校验 endpoint base，且禁用 redirect。业务请求、adapter 和
credential 均不能替换 endpoint origin。

## 启动与运行生命周期

### 1. 启动装配顺序

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

表头必须明确标注 `configuration only` 和 `no network probe`。这里的"可用"只表示本次启动配置允许形成执行候选；OAuth2
auth-file locator 会先参与 active-pool 筛选，但主服务必须在输出表格前读取并校验完整 bundle，缺失、空白或损坏文件会阻止启动。
该表不证明当前 credential lease、网络、配额、远端模型或协议能力实际可用；真实探测只能由管理员显式运行独立 probe。

### 2. 冻结 wiring 与可变 OAuth generation

`RuntimeRegistry`、`UserRegistry`、API-key `CredentialStore`、OAuth manager 实例及其 binding/locator/wiring 在启动后
保持不变。服务没有用户、Route、Provider、API-key pool 或 auth-file locator 的文件监听、`ArcSwap` 或部分更新语义；
改变这些启动输入必须重启。

`OAuth2CredentialManager` 内部 token snapshot 与 generation 不是不可变 registry 事实。独立显式登录把完整文件写好后需要
重启服务，由新进程建立初始 snapshot；运行中的 manager 只允许 expiry-driven refresh 或首个预提交 `401` recovery 按同一
Provider/account binding 执行 guarded reload、single-flight 和原子 rotation。该内部更新不得替换 manager、改变 locator、切换账户、
修改 Route 或发布另一套 RuntimeRegistry。

## ChatGPT OAuth credential 生命周期

本文定义 ChatGPT subscription credential 的固定 Provider、OpenBridge-owned auth 文件、显式 PKCE 登录、
expiry-driven refresh、短期 credential lease 与有界 `401` recovery。

OpenBridge 不搜索或导入 Codex 用户目录，不调用 Codex executable/app-server，也不从 OS、environment 或 terminal
推导身份。数据面只使用代码注册的 backend/model/request identity 和 OpenBridge-owned credential；普通请求不会
隐式登录或选择账户。管理员 probe 只能借用所选 Target 的 manager lease，不改变 registry 或 credential binding。

外部 OAuth authority、client registration 与私有 protocol 在扩大范围前必须重新核对 Provider 官方资料；参考项目
快照或单次成功调用不形成长期协议承诺。

- [Codex 设备登录与 token 刷新调研](../references/codex/codex-device-auth-token-refresh-analysis.md)
- [Codex 浏览器 OAuth 调研](../references/codex/codex-oauth-and-tool-call-analysis.md)
- [OAuth 设备登录与 token 刷新综合调研](../references/cross-project/upstream-oauth-device-code-token-refresh-analysis.md)

### 1. Provider 与 credential 边界

#### 1.1 Provider 与 OpenBridge-owned 启动快照

必须满足：

1. ChatGPT 是独立 `ProviderKind` 与 Provider instance，不能复用 `OpenAI` API-key Provider instance 或 credential pool。
2. BaseURL、operation path 与 credential kind 来自受信 Rust 注册；业务请求和 credential 文件不能覆盖上游 URL、model path 或
   任意 header。
3. `gpt-5.3-codex-spark`、`gpt-5.5`、`gpt-5.6-luna`、`gpt-5.6-terra` 与 `gpt-5.6-sol` 各自拥有一个
   固定 Responses-native ChatGPT Target。前四个 Target 分别是四个 ChatGPT-only Public Model 的唯一 source；Sol
   Target 是还包含 OpenAI 后备 source 的 `gpt-5.6-sol` Public Model 的 ChatGPT source。通用 API-key probe 不借用
   OAuth manager credential。
4. private upstream credential TOML 可为 ChatGPT OAuth2 binding 显式配置一个 OpenBridge-owned `auth_json_file`；不得默认、
   搜索、导入或回退到 `$CODEX_HOME/auth.json`。
5. 启动 loader 要求 auth 文件已存在，并校验完整 id/access/refresh token bundle、账户绑定与 access-token expiry，
   再把固定 binding/locator/wiring 装入 `OAuth2CredentialManager`；缺失、空白、损坏或不完整 bundle 阻止启动，完整过期 bundle
   进入立即 refresh。独立 login CLI 可以从 missing version 事务性创建一次完整文件；manager wiring 保持不变，内部
   token snapshot/generation 可以按本页规则 guarded reload/rotation。
6. OpenBridge 不读取 terminal 相关环境变量，不根据部署主机 OS、architecture 或 Codex state 构造 Agent identity；ChatGPT 只使用受信 Rust
   definition 固定、按已记录 Codex CLI release 源码格式生成的 headless Linux x86_64 兼容 UA/header，并且不接受 auth file、executable、
   client identity 或 header override selector。
7. ChatGPT adapter 只接受 `stream:true` 的 Responses 请求，将标准字符串 `input` 收窄为等价 user message 数组，保持
   `store:false`，并在 egress 前拒绝当前 backend 不接受的输出 token limit 字段。通用 planning 已在进入 adapter 前按客户端优先、
   项目默认回落的统一规则写入 `instructions`；ChatGPT 不再拥有专属配置、context 或覆盖 hook。
8. token、账户、locator、JWT payload 和完整 auth record 不进入 report、日志、metric、Debug 或错误。

常驻服务的数据面只能取得 manager 发布的短生命周期、账户绑定 credential lease。它不能读取 auth locator 或完整 bundle，也不能把
OAuth credential 放入通用 API-key Store、probe 或业务请求可控字段。

#### 1.2 PKCE 登录与 token 续约

显式登录必须满足：

1. `openbridge-auth login chatgpt` 使用固定 ChatGPT private device interaction 与 authorization-code + PKCE `S256`；
2. 登录临时状态、authorization code、PKCE verifier 和 device state 只在有界会话中存在，失败或取消后清除；
3. exchange 只访问编译期固定 HTTPS token endpoint，要求完整 token、未来 access expiry 和一致 account binding，再用 advisory lock、
   source-version CAS 与同目录 atomic replace 持久化完整 credential bundle；
4. CLI 不接受 issuer、client、endpoint、header、auth-file 或其他应用 cache override，普通服务启动和模型请求不隐式发起登录。

不提供运行中换账户 API。换账户时必须停止服务，手动删除 private upstream credential binding 指向的 OpenBridge-owned
`auth_json_file` 及同一登录流程明确创建的其他 OpenBridge-owned 授权文件（如有），再执行显式登录并重启；不得操作本机 Codex auth cache。

自动 refresh 必须满足：

1. 按 expiry safety window 合并 refresh，从持久化源 guarded reload，并跨进程/进程内 single-flight；
2. rotated refresh token 与 access token、expiry、identity 和 generation 原子写回，终态错误转为 `reauth_required` 或 `ambiguous`；
3. 启动重建 expiry-driven schedule，并在 refresh 成功后发布新的 manager snapshot；
4. 429/5xx 与确认未送达错误进入有界 backoff；terminal OAuth code 进入 `reauth_required`；可能已发生 rotation 但无法安全落盘的结果进入
   `ambiguous` 并停止自动复用旧 token。

数据面只通过 OAuth2 manager 的受控 lease 生成 Provider authentication header，并在首个预提交 `401` 后执行一次
guarded reload、必要时 refresh 和至多一次 replay；第二个 `401` fail closed 为 `reauth_required`。没有独立受信 JWT
signature source 时，校验不得表述为离线 signature、通用 issuer/audience 或 subscription policy 验证。

### 2. Provider OAuth preflight

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

### 3. 登录入口与控制面边界

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

### 4. Credential bundle

可刷新 credential 至少以同一版本管理：

ChatGPT auth 文件使用闭合 OAuth JSON 字段：顶层 `auth_mode`、`OPENAI_API_KEY`、`tokens` 与 `last_refresh`，其中
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

### 5. 到期驱动 refresh

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
- 是否为了 refresh-token inactivity 主动 refresh 只能来自 Provider 正式政策；
- single-use rotation 下，refresh 请求可能成功但响应丢失时进入 `ambiguous`，不得盲目重用旧 token。

### 6. 请求路径与 401 recovery

1. token 已进入 safety window 时，等待同一 refresh single-flight，而不是每个请求单独刷新。
2. token 仍在安全窗口外时，不为满足固定 timer 强制 refresh。
3. 401 后先 reload；若 credential version 已变化，用新 token 至多重试一次。
4. version 未变且 Provider contract 允许时，可执行一次 refresh，再至多重试一次。
5. 一旦下游业务 response 已开始，不得 refresh 后重放形成第二个上游响应。
6. 第二次 401 或终态 OAuth error 将 credential 转为 `reauth_required`，不能进入无限 refresh、账号轮转或普通 Provider
   fallback。

401 还可能来自 audience、account/workspace header 或授权政策，不等于 access token 一定过期。refresh 前后身份绑定必须一致。

### 7. 失败分类

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

### 8. 功能验收要求

| ID       | 行为                                                                                                                                                      |
|----------|-----------------------------------------------------------------------------------------------------------------------------------------------------------|
| OAUTH-01 | ChatGPT 使用独立 ProviderKind/Provider instance、OAuth bearer credential kind、固定受信 BaseURL 与 Responses-only adapter；OpenAI API-key Provider 行为不变。 |
| OAUTH-02 | 五个固定 ChatGPT Target 都是 Responses-native；Spark/GPT-5.5/Luna/Terra 分别属于四个 ChatGPT-only Public Model，Sol Target 属于还包含 OpenAI source 的 `gpt-5.6-sol`；Provider-qualified identity 只保留在内部，通用 API-key probe 不借用 OAuth manager credential。 |
| OAUTH-03 | ChatGPT OAuth 文件只由 private upstream credential TOML 显式定位并由 OpenBridge 拥有；不得搜索、导入或回退到本机 Codex state。 |
| OAUTH-04 | 生产代码不从 terminal、部署主机 OS、architecture、environment 或 Codex state 推导 client identity；ChatGPT 只发送编译期固定、按已记录 Codex CLI release 源码格式生成的 headless Linux x86_64 兼容 UA/header，不提供运行时 override 或 Codex auth/executable probe selector。 |
| OAUTH-05 | login CLI 可以从 missing version 事务性创建完整 `auth_json_file`；主服务启动要求文件已存在且完整校验 OAuth2 bundle，再构建内部 guarded、对外 snapshot 化且脱敏的 `OAuth2CredentialManager`；缺失、空白或损坏文件阻止启动，过期完整 bundle 可立即 refresh。 |
| OAUTH-06 | upstream credential TOML 以互斥的 `api_keys`/`auth_json_file` 绑定编译期 credential kind；每个 OAuth2 Provider 只加载一个 OpenBridge-owned auth 文件。 |
| OAUTH-07 | 数据面只通过短生命周期、账户绑定的受控 credential lease 生成 Provider authentication header，不得把 locator 或完整 token bundle 暴露给普通请求路径。 |
| OAUTH-08 | 登录使用 PKCE `S256`、有界 private device session、固定 HTTPS exchange、完整 token/account 校验、事务持久化和失败清理，不在普通请求中隐式启动。 |
| OAUTH-09 | 自动 refresh 具有 expiry safety window、single-flight、guarded reload 与原子 rotation 写回；终态错误、结果不确定或身份变化 fail closed。 |
| OAUTH-10 | 数据面只借用受控 snapshot；401 recovery 先 reload、再按 Provider contract 至多 refresh/重放一次，并服从 response commit 边界。 |
| OAUTH-11 | ChatGPT Responses adapter 要求 `stream:true`、将字符串 `input` 转为 user message 数组并保持 `store:false`；通用 instructions 必须在 candidate 展开前解析，adapter 不持有 Provider 专属 instruction context/hook，并在 egress 前拒绝且不公开 output-token-limit 参数。 |
| OAUTH-12 | 不提供运行中换账户；用户必须停止服务、手动删除该 binding 的 OpenBridge-owned auth 授权文件、显式重新登录并重启，且不得操作本机 Codex cache。 |

### 9. 仍不在范围内

- subscription 多账号池、账号级负载均衡、余额/配额控制面或账号自动轮转；
- 其他 ChatGPT model、Chat Completions/WebSocket/Batch/Embeddings API、function/hosted tool、MCP、多模态或完整 Agent loop；
- 导入 Codex、Hermes、LiteLLM 或任意其他应用的 credential cache；
- 下游 user OAuth、平台代理 authorization server、动态 endpoint/client registration/scope；
- 未经重新核对当前 Codex 源码和真实验收就复制私有 flow；
- keyring、远程 secret manager、跨主机 refresh 协调或多实例共享 credential；这些能力需要另行批准。

## 验收

### 1. 功能验收要求

| ID     | 行为                                                                                                                                                           |
|--------|----------------------------------------------------------------------------------------------------------------------------------------------------------------|
| CFG-01 | 仓库不存在 Provider/Model route 配置文件或动态 Provider schema。                                                                                               |
| CFG-02 | 代码注册表中的重复 ID、未知引用、能力扩大、无效 reasoning/level 映射和不安全 URL 在监听前失败。                                                                |
| CFG-03 | 业务请求无法覆盖 endpoint、真实 model、credential、敏感 header 或 candidate 顺序；普通 header 只能由受信 Provider 代码声明或转换，固定 UA/header 在 hook 后应用，业务请求不能选择规则或覆盖固定值。 |
| CFG-04 | secret 不进入代码注册项、`RuntimeRegistry`、日志、错误或 probe report。                                                                                        |
| CFG-05 | 每个 Provider family 由独立、闭合的 definition owner 管理，并经单一显式 composition root 注册；不存在自动注册。                                             |
| CFG-06 | bootstrap 只控制 listener、文件位置、资源上限、HTTP client、本地 HTTP 内容日志与 telemetry 导出等进程资源策略，不能注册或修改 Provider；collector host 可由配置所有者选择。 |
| CFG-07 | listener 只允许 loopback；非 loopback 地址必须在监听前拒绝。                                                                                                   |
| CFG-08 | 用户文件中的无效 schema、重复 ID/Key、短 Key 或无启用用户会阻止启动。                                                                                          |
| CFG-09 | 下游与上游 API-key secret 只进入启动时不可变 `CredentialStore`；OAuth bundle 只进入固定 wiring 的 manager，并仅按专有 lifecycle 发布受控 token snapshot/generation。 |
| CFG-10 | 私有 upstream credential TOML 出现未知或重复 pool、空白/重复 secret 或不能解析时，会在 listener 绑定前阻止服务启动；缺失或为空的已注册 pool 会让其引用 Target 在本次启动中不可执行。 |
| CFG-11 | 同 Provider 的 Target 可引用共享 API-key pool；激活 pool 必须满足 Provider/kind 与 member 约束，未激活 pool 不要求 secret。                                               |
| CFG-12 | 多 member pool 不得用于启用 `TargetBoundContinuation` 的 Responses API；普通 Target-bound、无 continuation 的 API 不因此失去 credential rotation。                         |
| CFG-13 | 五个 ChatGPT Responses-native Target 共用独立 OAuth pool；四个分别属于 ChatGPT-only Public Model，Sol Target 属于还含 OpenAI source 的 `gpt-5.6-sol`；请求和 probe 不接受本机 Codex auth/environment/terminal/executable selector。 |
| CFG-14 | Provider 实例唯一拥有一个受信 BaseURL；Target 必须引用已注册实例，不同 URL/区域使用不同实例，业务请求不能覆盖实例或 URL。                                            |
| CFG-15 | 每个 Target 对每个 `OperationKind` 最多注册一个 Upstream API；Route、probe、telemetry 与 continuation issuer 使用 typed upstream operation，不依赖 API 字符串 ID。 |
| CFG-16 | Upstream API 的 operation 只由 capabilities variant 决定；当前 transport 由 operation 固定，注册表不保留独立 operation、transport 或无执行语义的 endpoint profile。 |
| CFG-17 | 主服务在配置验证后、listener 前输出配置态 Provider/Public Model 可用/不可用双表；分类复用 active Target/执行接口且不触发 Provider egress，不输出 credential 或内部拓扑，也不把配置态结果声明为真实健康。 |
| CFG-18 | 随附开发配置的四个本地下游 HTTP 内容日志开关显式全开，自定义配置缺表/缺字段时回退关闭且可独立覆盖；未知 logging 字段阻止启动，敏感 header 始终脱敏，body capture 有界且不进入 OTLP，开关不改变请求/响应字节、路由或终态。 |
| CFG-19 | 任一通用 Generation interface 可执行时要求非空 `default_instructions`；仅有 Embeddings/专用音频 task 时不要求。客户端有效值优先，默认值在 candidate 展开前统一解析，Provider、probe 或请求不能另行覆盖。 |
