# OpenBridge 基础目标

本文定义当前产品范围、信任边界和明确非目标。代码与验证已经覆盖到哪里只由
[实施现状](../../implementation-status/README.md)记录。

OpenBridge 尚未发布受支持的外部兼容基线。首个一致契约形成前，获准的行为变更可以直接替换原型 API、
bootstrap 字段、DTO 与内部接口；不得为未发布形态保留无意义的 alias、双读写或版本垫片。任何变更仍须同步
所有受影响的线格式、OpenAPI、示例和文档，并保持私有数据与信任边界。

## 1. 产品目标

OpenBridge 是由单个配置所有者管理、以单个进程部署的 headless Provider 网关。登记在私有用户表中的本地
Agent 或 OpenAI-compatible SDK 通过稳定的 loopback HTTP 地址和 Public Model 调用代码注册的上游服务，
无需知道 Provider、真实模型、endpoint、credential 或 Route。

当前产品契约包括：

- 受 Bearer 认证保护的 Chat Completions、Responses、Embeddings、Models 与扩展 Models 接口；
- 受同一认证保护的 MCP dual-era 本地入口：`2026-07-28` 无状态 discovery 与 legacy `initialize` 会话都只
  提供静态、无副作用的 `hello(name: string)` 工具，不访问 Provider；
- Chat/Responses 同协议 Native Path，以及只转换已证明可完整表达的 text/function/reasoning 共同语义的
  Protocol Bridge；
- 由 Public Model 固定接口执行的一次能力预检，以及预检后不可因请求能力改变的 Route 顺序；
- 首个下游业务输出前的有限 retry/fallback、credential rotation 与单进程短时 cooldown；
- 独立的 Embeddings 与按任务分离的 Native 图片、文件和音频契约；
- 默认禁用、只由启动配置启用的 OTLP/HTTP traces、metrics 与安全 logs 出站；
- 管理员显式运行的固定 Models 与基础 API probe。probe 不修改 registry，也不把一次探测提升为模型语义、
  客户端、负载或长期兼容结论。

## 2. Generation 状态契约

无状态请求是默认调用方式。客户端在每次请求中携带完整历史；`store` 省略或为 `false`，
`previous_response_id` 省略或为 `null`，`background` 省略或为 `false`。

- `store:true` 以及非布尔的显式 `store` 在任何 Provider egress 前拒绝；每个 Native Responses candidate 显式
  编码 `store:false`，Responses-to-Chat Bridge 消费该事实而不伪造 Chat 字段。
- 当前 Public Model 固定接口不公开 background execution；`background:true` 在 egress 前拒绝，OpenBridge 也不
  提供 response retrieve/cancel 或后台 job API。
- 当前 Public Model 固定接口不公开 response-ID continuation；非 `null` 的 `previous_response_id` 在 egress 前
  拒绝。OpenBridge 不保存、查询、删除、迁移或恢复上游 response 状态，也不维护 continuation ledger。
- opaque continuation、Provider resource 与私有 turn state 不能进入 Bridge 或跨 Target fallback。只有未来的
  功能需求明确 issuer、credential affinity 与完整状态生命周期后，才可扩大上述契约。

详细 envelope 与状态规则由 [Generation envelope 与状态](../gateway-api/generation-state.md)唯一拥有。

## 3. 路由与静态装配

- Provider、Canonical Model、Upstream Target、Upstream API、Route 与 Public Model 由受信 Rust 代码显式注册；
  不从业务请求、Provider `/models`、文件扫描或插件动态生成。
- 每个 generation Public Model 必须显式选择 `NativeFirst` 或 `SourceFirst`。具体排序和自动 Bridge 规则只由
  [路由与 Provider 韧性](../routing-resilience/README.md)拥有；产品范围不再笼统承诺“始终 Native-first”。
- 所选 Public Model 先按 operation 的唯一固定契约完成能力预检；通过后 Route 保持启动时冻结的顺序，不按
  请求能力、价格、健康或 Provider 名称筛选和重排。
- Registry、用户表、Route topology、API-key credential store 以及 OAuth manager 的 binding/locator/wiring 在
  启动后保持不变。改变这些输入需要重启。
- OAuth manager 内部的 token snapshot 与 generation 是受控可变状态；它们只能由显式登录、到期 refresh 或
  首个预提交 `401` recovery 按固定账户边界 guarded reload/rotation，不能修改 registry、Route、账户 binding
  或 auth-file locator。

ChatGPT subscription 集成固定注册五个 Responses-native Target。`gpt-5.3-codex-spark`、`gpt-5.5`、
`gpt-5.6-luna` 与 `gpt-5.6-terra` 是四个 ChatGPT-only Public Model；第五个 ChatGPT Target 是
`gpt-5.6-sol` 多 source Public Model 的 ChatGPT source，该 Public Model 还包含 OpenAI 后备 source。五者不能被
简写成“四个 target”或“五个 ChatGPT-only Public Model”。

## 4. 部署、凭证与观测边界

- listener 只允许 loopback；下游业务请求不能覆盖上游 URL、模型、credential、认证 header、Route 或 header
  转换规则。
- 下游静态 Bearer token 和上游 API key 只来自私有配置；ChatGPT OAuth bundle 只来自显式配置的
  OpenBridge-owned auth 文件。任何路径都不得搜索或导入本机 Codex、Hermes、LiteLLM 或其他应用的 auth cache。
- `RuntimeRegistry` 与 `UserRegistry` 不保存 secret。日志、错误、probe report、trace 和 metric 不得暴露
  credential、完整私人正文或真实 endpoint URL。
- 本地下游 HTTP 内容日志只覆盖认证后的最终客户端边界，强制脱敏 header 并有界捕获正文；它不是原始上游
  Provider wire dump，也不进入 reviewed OTLP trace layer。
- OpenBridge 不提供进程内 metrics snapshot 查询 API。metrics 只通过启动时配置的 OTLP/HTTP 出站；持久化、
  时间窗口、比例、排名、告警与可视化由外部 collector/backend 负责。

## 5. 客户端接口

| 接口 | 契约 |
|---|---|
| `GET /healthz` | 最小本地存活信息，不访问上游 credential，不承担 Provider 健康探测。 |
| `GET /v1/models`、`GET /v1/models/{model}` | 同一 Public Model 目录的 OpenAI 标准四字段 list/retrieve。 |
| `GET /openbridge/v1/models`、`GET /openbridge/v1/models/{model}` | 同一目录的模型事实与每 operation 固定能力，不暴露执行拓扑。 |
| `POST /v1/chat/completions` | 在所选 Public Model 的固定 Chat 契约内提供 JSON/SSE。 |
| `POST /v1/responses` | 在所选 Public Model 的固定 Responses 契约内提供 JSON/SSE。 |
| `POST /v1/embeddings` | 在独立 Embedding Public Model 契约内提供有界 JSON 向量结果。 |
| `/mcp` | 按 [MCP 本地服务](../gateway-api/mcp.md)提供 dual-era Streamable HTTP 生命周期。 |

## 6. 暂不纳入产品承诺

- image、file、audio、opaque reasoning、Provider 私有扩展或 continuation 的跨协议转换；
- response 状态存储、查询、删除、后台 job、跨 Provider/Target 迁移和 continuation ledger；
- Responses WebSocket、Realtime、Files、Images、Videos、Conversations 等专用媒体或资源 API；
- ChatGPT 范围之外的 OAuth Provider、subscription 多账号池、账号级负载均衡、动态 credential 控制面、
  keyring 或远程 secret manager；
- 动态权重、持久化或分布式健康、后台探测和多进程协调；
- 内置 Prometheus exporter、metrics 查询/重置 API、历史数据库、dashboard 或分布式指标聚合；
- hosted tool、MCP Tool Bridge、`hello` 之外的本地 MCP tool 或由 generation gateway 执行普通 function tool；
- 多租户控制面、在线用户管理、配额、计费、审计或 GUI。

## 7. 术语

- **Provider**：一类受信协议、认证和错误处理行为。
- **Provider Instance**：一个 Provider family 的受信部署实例，唯一拥有一个 BaseURL。
- **Model**：与具体调用 endpoint 分离的 canonical 模型事实。
- **Credential Pool**：绑定同一 Provider/credential kind 的 credential source。
- **Upstream Target**：引用 Provider Instance，并绑定 Model、credential pool、timeout 和故障边界的调用边界。
- **Upstream API**：Target 下由 `OperationKind` 唯一标识的一条原生供应及其能力。
- **Route**：固定下游 operation、Target、typed upstream operation 和 `Native`/`Bridged` 模式的路径。
- **Public Model**：下游稳定模型身份、每协议固定能力契约及私有有序 Routes。
- **Native Path**：上下游协议一致时的最小改写转发路径。
