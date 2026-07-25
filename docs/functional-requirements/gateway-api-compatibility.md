# 网关 API 与客户端兼容需求

## 状态

**当前目标。** 本文定义 OpenBridge 对下游客户端可见的 API、认证、原生 HTTP/SSE 语义和兼容边界；不规定内部模块、converter 形态或实现顺序。当前已经由代码和测试证明的范围以[当前实现说明](../implementation-status/current-implementation.md)为准。

## 1. 用户结果

单个受信用户应能把本地 Agent 或 OpenAI-compatible SDK 指向一个稳定的 OpenAI-compatible base URL，仅使用 public model alias 与可选的单一静态 Bearer token 调用服务，而不需要知道上游 Provider、真实模型、URL、凭证或候选切换情况。

初期的兼容目标按优先级为：

1. OpenAI SDK 的 Chat Completions 与 Responses HTTP JSON/SSE；
2. Codex custom Provider 的 Responses HTTP/SSE profile；
3. 只有在明确声明时，才验证 Hermes 的具体协议、transport 与 tool-loop 行为。

“某个请求能被转发”不等于“某个 Agent 已完整兼容”。每项声明必须限定 endpoint、stream、tool、continuation、Provider 与实际验证版本。

## 2. 接口与认证

| 接口 | 功能要求 | 不包含的语义 |
|---|---|---|
| `GET /healthz` | 提供不访问上游凭证的最小本地存活信息；不得泄露 route、deployment 或 secret。 | Provider 健康探测、控制面或客户端管理。 |
| `GET /v1/models` | 只返回受信配置声明的 public model aliases，使用稳定的 OpenAI-compatible model-list 形状。 | 枚举上游模型、provider/deployment id 或动态能力发现。 |
| `POST /v1/chat/completions` | 支持已声明能力范围内的 Chat JSON/SSE 请求。 | 对全部 Chat 扩展或 hosted tool 的默认兼容承诺。 |
| `POST /v1/responses` | 支持已声明能力范围内的 Responses JSON/SSE 请求，作为 Codex HTTP/SSE profile 的首要入口。 | Responses WebSocket、资源 retrieve/cancel/store/background/conversation API。 |

业务 endpoint 必须使用静态 Bearer token 或明确配置的等价单用户认证方式。服务不建立用户、client registration、key issuance、scope、撤销列表、配额或 billing identity。认证失败与未知/不支持 endpoint 必须在进入路由或上游调用前结束，且不泄露配置细节。

## 3. 请求、别名与安全边界

### 3.1 public alias

- 下游只能提供已配置的 public model alias；每次请求由该 alias 选择受信候选 deployment。
- 服务对上游只改写为选中 deployment 的真实模型名；下游不能通过 body、query 或 header 指定上游 URL、模型、credential、provider family、header 或转换脚本。
- `GET /v1/models` 的可见集合与可路由 alias 一致；上游 `/v1/models`、probe 结果和未配置模型不得自动暴露。
- 请求开始后，alias、RoutePlan、credential binding 与配置版本保持固定；reload 只影响后续请求。

### 3.2 输入保护

- 仅接受端点契约允许的 content type、JSON body 和受配置约束的大小；无法安全解析的请求在 egress 前返回稳定错误。
- 请求分类必须识别 protocol、`stream`、function/custom/hosted tool、并行工具、结构化输出、multimodal、reasoning、`previous_response_id`、background/store 与输出上限等会影响能力或状态的特征。
- 未知 feature 不能因“目标 Provider 也许支持”而默认放行到 bridge；Native Path 可保留同协议的未知合法字段，前提是它们不改变路由、安全或 state-affinity 决策。
- 服务为每个请求生成或传播安全的 request id，用于响应和受控诊断；该 id 不是 client identity、tool identity 或聚合指标 label。

## 4. 原生协议与流式语义

当下游与上游协议一致且已获 capability 许可时，Native Path 是兼容性基线：它只做受信路由、模型和认证改写，保留 JSON、HTTP status、必要 allowlist header 与未知合法字段，不经过通用 IR 重渲染。

流式请求必须满足：

- 原样保持协议的 SSE framing、event/data 负载与输出顺序；不得注入 OpenBridge 自定义 SSE event。
- Chat 以其自身终态（包括 `[DONE]`）处理；Responses 区分 item/content lifecycle 与 `response.completed`、`response.incomplete`、`response.failed`、`response.cancelled` 或顶层 `error` 等 response terminal。
- `output_item.done`、tool input delta、metadata/header 到达或任意首字节都不等于请求成功。已写出首个业务 body byte 后，不得 retry、fallback 或将其他 deployment 的内容拼入当前 stream。
- 下游取消、连接中断、deadline 和错误终态应停止相应上游工作；合法但无 terminal 的 EOF 不得伪造成 completed。
- response headers 和 SSE bytes 的处理必须受大小、UTF-8、event 数量/长度与慢消费者资源上限保护。

## 5. tools、continuation 与扩展

### 5.1 function tools

对于已声明支持的普通 `type: "function"` tool：

- 需要保持请求 schema、并行调用顺序、`call_id` / `tool_call_id`、arguments 分片和 tool result 的关联；
- arguments 在完成前是未可信的字符串，网关不得执行或授权模型返回的工具调用；
- tool call/result、`item_id`、stream output index 与 request id 是不同身份，不能相互替代。

### 5.2 状态亲和与私有扩展

- `previous_response_id`、Provider resource、tool continuation、opaque reasoning 与 issuing call 都是可能绑定 deployment 的状态。不能安全证明等价时，拒绝、保持同一 issuing deployment，或要求完整可转换历史；不得跨候选猜测或 replay。
- Codex 所需的 `x-codex-turn-state` 及 `response.metadata` 属于受限私有扩展：只在显式启用的 Codex Native Responses profile 中透明保留，不能进入 Bridge IR、用户 transcript、普通日志或跨 deployment fallback。
- MCP、custom tool、hosted tool、reasoning、annotation、image generation 等不是普通 text 的同义词。若没有已验证的转换规则，Bridge 必须在输出前拒绝或返回明确的 loss/unsupported 结果；不得静默丢弃。

Responses 标准 event 与 Codex 私有扩展的细节见[Responses 协议参考](../references/openai/responses-protocol.md)。

## 6. 错误与客户端可见结果

| 时机 | 必需行为 |
|---|---|
| ingress、alias、能力、认证或配置拒绝 | 上游调用前返回安全、稳定的 OpenAI-compatible JSON error；不暴露 URL、credential、候选列表或内部栈。 |
| 首个业务输出前的上游失败 | 依[路由与 Provider 韧性](provider-resilience.md)判断有限 retry/fallback，最终保留安全的 status、error code、request id 与 allowlist rate-limit 信息。 |
| 已开始 JSON/SSE body 后的失败 | 只使用目标协议已有的 terminal/error 或关闭语义；不重写已发内容、不注入私有 event、不切换 candidate。 |
| 下游取消 | 停止当前请求及可取消的 retry/backoff；终态单列为 client cancellation，而非上游成功或错误。 |

所有错误类别必须稳定、低基数且可用于调用统计；原始上游错误正文只能在受保护诊断中按脱敏规则处理，不能成为对外契约。

## 7. 功能验收要求

| ID | 应被保护的用户可观察行为 |
|---|---|
| API-01 | 有效静态 token 可访问模型与业务 endpoint；认证失败、未知 alias、不支持 feature 与非 JSON 请求在 egress 前安全失败。 |
| API-02 | `GET /v1/models` 仅暴露 public alias，且不因 probe、上游模型列表或 route reload 泄露内部目标。 |
| API-03 | Native Chat/Responses JSON 与 SSE 除受信模型/认证改写外保持 wire 语义；未知合法同协议字段/event 不因网关丢失。 |
| API-04 | SSE 分片、终态、EOF、上游 error 和下游 cancel 不会产生伪成功、重复 terminal 或跨 deployment 拼接。 |
| API-05 | 普通 function tool 的 call/result identity 与 fragmented arguments 在已声明路径中保持；网关不执行工具。 |
| API-06 | Codex Native profile 能在受限 allowlist 下保留其已验证的 turn-state 扩展；bridge、route change 或 fallback 不会误复用该状态。 |
| API-07 | 对 Codex、OpenAI SDK 或 Hermes 的兼容声明均有相应 endpoint/feature 的可重复证据，并写入实施现状而非仅引用设计。 |

## 8. 非目标

- GUI、Web 控制台、客户端安装/注册/配置管理；
- Realtime、Responses WebSocket、Files、Conversations、管理 API 或“实现全部 OpenAI API”；
- 让 Chat ↔ Responses、任何 tool 或 Provider 私有扩展自动无损互转；
- 代表下游 Agent 执行任意 function tool、shell、computer 或网页操作；
- 用 API token 建立多用户权限、配额、账单或审计系统。

## 关联文档

- [产品范围](product-scope.md)
- [配置与凭证](configuration-and-credentials.md)
- [路由与 Provider 韧性](provider-resilience.md)
- [调用统计与可观测性](observability.md)
- [交付与证据要求](delivery-and-evidence.md)
- [当前实现说明](../implementation-status/current-implementation.md)
