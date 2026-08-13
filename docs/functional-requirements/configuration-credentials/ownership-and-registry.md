# 所有权划分与代码注册表

## 状态

本文是[配置与凭证域](README.md)的所有权与注册表模块：定义配置来源的所有权划分和代码注册表要求。
其他模块见[配置与凭证域](README.md)导航。

## 1. 所有权划分

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
schema v2 要求 `max_request_body_bytes`、`max_json_response_body_bytes`、
`max_replay_body_bytes` 与 `max_sse_event_bytes` 均为非零值，并要求 replay limit 不大于 request limit；这些
字段职责独立，不互相提供缺省或回退。

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

## 2. 代码注册表要求

本节约束逻辑所有权和受信装配结果，不把当前 Rust 文件名、目录层级或 facade 形式固化为产品契约。当前物理模块边界见
[当前代码架构](../../implementation-status/current-architecture.md)，维护规则见仓库 `AGENTS.md`。

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
  唯一排序与自动 Bridge 规则见[路由与 Provider 韧性](../routing-resilience/README.md)，本页不重复定义；
- 启动监听前必须完成唯一性、引用、能力、reasoning、credential pool 和 URL 校验。

修改 Provider、Model、Route、用户、API-key pool 或 OAuth binding/locator 必须重新编译或重启。OAuth manager 只可按
专有 lifecycle 更新同一 binding 内的 token snapshot/generation；这不构成 registry 或配置热重载。

## 关联文档

- [配置与凭证域导航](README.md)
- [凭证](credentials.md)
- [Endpoint 与出站边界](endpoint-and-egress.md)
- [生命周期](lifecycle.md)
- [当前代码架构](../../implementation-status/current-architecture.md)
- [路由与 Provider 韧性](../routing-resilience/README.md)
- [实施现状](../../implementation-status/README.md)
