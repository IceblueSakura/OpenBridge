# OpenBridge 单用户 Provider 聚合代理：核心需求

## 状态

**Working scope；用于设计收敛，不代表最终实现已经确定。**

本文定义 OpenBridge 核心产品边界、外部契约和验收方向。当前代码是实验性验证版本；具体调研问题、决策门与候选实施顺序见[开发与调研收敛计划](../plans/development-plan.md)。

## 1. 产品目标

OpenBridge 是一个由单个用户管理、以单个服务部署的 Agent API proxy。它在本地或用户自有云环境中：

- 集中配置多个上游 Provider、credential、deployment 和模型；
- 向 Codex、Hermes Agent 等客户端提供稳定的 Chat Completions 与 Responses 接口；
- 优先原生转发，在协议不一致且语义可表达时执行受限 bridge；
- 对 tool-call identity、stream terminal、continuation state 和 fallback 保持可预测行为；
- 让下游客户端只接触 OpenBridge 的地址和可选静态 token，不接触上游 credential。

OpenBridge 的核心价值不是企业级治理，而是：

> **以一个稳定入口聚合异构 Provider，并尽可能保留目标 Agent 所依赖的原生协议、流式事件、工具调用和状态语义。**

## 2. 用户与部署模型

### 2.1 参与者

| 参与者 | 职责 |
|---|---|
| Service owner | 唯一受信管理员；编辑本地配置、提供 credential、选择模型 alias 和部署方式。 |
| Agent client | 通过 Chat Completions 或 Responses 调用稳定 alias，并执行多轮 tool loop。首批目标是 Codex 与 Hermes Agent。 |
| Provider adapter | 将受信 deployment 配置和请求映射到上游协议；构造认证、解析响应/SSE 和错误。 |

### 2.2 部署假设

- 单用户、单配置所有者；不建立 tenant、team、principal 或成员模型。
- 单进程/单服务是默认形态；内部模块化不等于独立控制面与数据面。
- 本地部署默认只监听 loopback。
- 云端或非 loopback 部署使用 HTTPS/可信反向代理，并至少配置一个静态高熵 Bearer token；未配置时应拒绝启动或拒绝业务请求。
- 不以高可用、多实例一致性或分布式状态为核心设计前提。

## 3. 核心范围

### 3.1 下游接口

- `POST /v1/responses`：Codex 的首要契约，也是 Hermes Responses 模式的入口。
- `POST /v1/chat/completions`：Hermes 和通用 OpenAI-compatible client 的一等入口。
- `GET /v1/models`：返回用户配置的 public model aliases。
- HTTP JSON 与 SSE 流式/非流式响应。
- 首个 Codex profile 使用独立 custom Provider id，并显式配置 `wire_api = "responses"`、`supports_websockets = false`；Responses WebSocket 单独评估，不隐含在首版兼容承诺中。

### 3.2 Provider 聚合

- 多个 Provider Family、deployment 和稳定 alias。
- 首批 Provider archetype：
  1. OpenAI Responses 原生 Provider；
  2. Generic OpenAI-compatible Chat Provider；
  3. Anthropic Messages，用于验证真正异构的协议抽象。
- 一个 alias 可指向一个有序 candidate list。
- 第一版以确定性顺序和 capability filtering 选路，不要求动态权重、成本优化或分布式健康系统。

### 3.3 凭证

- 标准 API key 是核心基线。
- credential 可通过环境变量、系统 keyring/secret store 或受限文件引用提供；普通配置和日志不保存明文。
- OAuth 是可选 Provider credential adapter，不是 Provider 聚合、native forwarding 或 bridge 的前置条件。

### 3.4 协议与工具

- 下游和上游协议相同：走 Native Path，进行最小必要改写。
- 协议不同：仅在 capability 明确允许时进入 Protocol Bridge。
- 核心 bridge 首先覆盖文本、function tool schema、tool call、tool result、usage 和必要 terminal state。
- Provider-hosted tool、本地/MCP Tool Bridge 和使用量分析属于核心后的增强功能。

## 4. 当前非目标

- 多租户、principal/ACL、团队成员、虚拟 key 管理、RPM/TPM 配额、计费与合规审计；
- 同 Provider 多账号池、credential pool、账号轮转或账号级负载均衡；
- 独立控制面/数据面、数据库驱动管理 API、集群和高可用一致性；
- OpenAI 全部资源 API、Realtime、Files、Conversations 或管理 API；
- 首版 Responses WebSocket transport；若目标 Codex 版本无法稳定使用 custom Provider 的 HTTP/SSE profile，则必须重新打开该范围决策；
- 将 Chat ↔ Responses 承诺为无损；
- 执行 Agent 返回的普通 function tool；OpenBridge 核心只转发或转换 wire-level call/result；
- 允许业务请求提供任意上游 URL、credential、认证 header、代理地址或转换脚本；
- 让 Codex/Hermes 的本地 credential cache 成为 OpenBridge 的隐式上游凭证来源。

## 5. 核心不变量

1. **Native first**：同协议请求不经过 Bridge IR；只做路由所需解析、模型/URL/认证改写和必要兼容修正。
2. **Capability before call**：endpoint、transport、stream、tools、structured output、reasoning、hosted tool 和 continuation 等能力在上游调用前判断。
3. **State affinity**：`previous_response_id`、opaque reasoning、provider resource ID、tool continuation 等状态绑定 issuing deployment/issuer，不跨 route 猜测重放。
4. **Immutable RoutePlan**：请求开始后固定 alias、deployment、协议模式、credential binding、capability decision 和 fallback 边界。
5. **No silent downgrade**：不能表达的语义必须拒绝；允许近似时必须生成可机器识别的 conversion 结果。
6. **Secret isolation**：下游静态 token 与上游 credential 分离；业务请求、响应、普通日志和 fixture 不包含上游 secret。
7. **No stream stitching**：下游已收到业务 JSON/SSE 后，不把另一个 candidate 的结果拼接进当前响应。
8. **Owner-configured, client-constrained**：服务所有者可在受信配置中定义 endpoint；下游客户端不可动态改变出站目标或认证行为。

## 6. 功能需求

### 6.1 下游 API 与目标客户端

| ID | 需求 | 验收方向 |
|---|---|---|
| API-01 | 同时提供 `/v1/responses` 与 `/v1/chat/completions` 的 HTTP JSON/SSE。 | OpenAI SDK fixture 可消费，且 Codex/Hermes 固定版本完成真实 tool loop。 |
| API-02 | `/v1/models` 返回配置中的 public aliases，不暴露 credential 值。 | alias 与配置快照一致；未知 alias 在上游调用前返回明确 4xx。 |
| API-03 | Codex 按 Responses-first、HTTP/SSE-first 验证；首个 custom Provider profile 显式关闭 WebSocket。Hermes 分别验证 Chat 与 Responses transport。 | 测试矩阵和重开 WebSocket 范围的条件见[目标客户端契约](../design/target-client-contracts.md)。 |
| API-04 | 不向 SSE 流注入目标客户端未定义的自定义 event。 | OpenBridge 元数据放入安全响应 header 或本地结构化日志。 |

### 6.2 Provider、deployment 与配置

| ID | 需求 | 验收方向 |
|---|---|---|
| PRV-01 | Provider 协议行为由代码中的 `ProviderFamily`/adapter 实现；deployment 数据由受信运行时配置提供。 | 新增同族兼容 endpoint 不要求复制 transport；新增异构协议需要明确 adapter。 |
| PRV-02 | deployment 至少包含 provider family、base URL、credential reference、upstream model、native protocols、capabilities 和 timeout。 | 配置加载时校验 URL、协议、credential reference 和 capability 上界。 |
| PRV-03 | 服务所有者可配置 base URL 与少量 adapter 明确允许的非认证 header；业务请求不得覆盖。 | SSRF/redirect/header tests 证明客户端不能改变出站目标或认证。 |
| PRV-04 | alias 映射到有序 deployment candidates；candidate 先按协议和 capability 过滤。 | 相同配置快照与请求产生确定 candidate set。 |
| PRV-05 | 首批真正异构 Provider 应至少包含 Anthropic Messages 或等价 archetype。 | Provider 抽象在非 OpenAI wire protocol 下通过 conformance，或记录需要推翻的假设。 |

### 6.3 Native Path

| ID | 需求 | 验收方向 |
|---|---|---|
| NAT-01 | 同协议转发尽量保留原始 JSON 字段，只读取/改写路由所需部分。 | 未知但合法字段不会因内部 schema 不认识而被删除。 |
| NAT-02 | 原生 SSE 以完整 SSE event 为验证单位，但可保留上游 wire bytes；不得把网络 chunk 当 event。 | fragmented UTF-8、多 event 同 chunk、跨 chunk event、多行 `data:` 通过。 |
| NAT-03 | provider-specific path、认证、必要 header、错误和 terminal 判定由 adapter 表达。 | 核心 ingress/router 不积累 provider-name 分支。 |

### 6.4 Protocol Bridge

| ID | 需求 | 验收方向 |
|---|---|---|
| BRG-01 | 只有上下游协议不一致时使用 `wire → Bridge IR → wire`。 | Native Path 的 benchmark/fixture 不经过 Bridge IR。 |
| BRG-02 | 每个转换按 `exact`、`structure_preserving`、`approximate`、`unsupported` 分类。 | 不支持能力在上游调用前拒绝；近似转换有 machine-readable result。 |
| BRG-03 | `call_id`、item/response identity、output index、tool index 和 terminal ownership 不得混用。 | 并行 call、arguments 分片、文本/tool 交错和 late identity fixtures 通过。 |
| BRG-04 | bridge 第一切片只承诺文本与普通 function tool loop。 | hosted tool、resource/background、opaque continuation 和未知 item 默认拒绝。 |
| BRG-05 | bridge 具备 re-entry guard，不能递归选择另一 bridge。 | 所有 protocol pair 有无递归 fixture。 |

### 6.5 Stream、取消与 fallback

| ID | 需求 | 验收方向 |
|---|---|---|
| STR-01 | 下游 disconnect 必须取消/关闭上游请求并释放 stream state。 | Codex/Hermes/SDK cancellation fixture 与 mock upstream 观察通过。 |
| STR-02 | EOF、idle timeout、transport error、provider error、client cancel 和正常 terminal 是不同 outcome。 | 每个 case 有唯一终止记录；不伪造完成。 |
| STR-03 | fallback 仅在尚未输出业务响应、没有 provider-bound continuation，且失败分类允许时发生。 | 已输出事件、`previous_response_id` 和 tool continuation 禁止跨 candidate。 |
| STR-04 | 第一版允许只实现连接失败、明确 429/5xx 或首字节前失败的有界 fallback。 | 不要求复杂 health/weight 系统即可完成核心。 |

### 6.6 Credential 与最小网络安全

| ID | 需求 | 验收方向 |
|---|---|---|
| SEC-01 | 上游 credential 仅由服务所有者配置并在发送前短时构造认证头。 | secret 不进入响应、普通日志、fixture 或错误消息。 |
| SEC-02 | loopback 可使用一个静态下游 token；非 loopback 必须启用静态高熵 token，并由 TLS/可信反向代理保护。 | 无 token 的非 loopback 配置启动失败或业务 endpoint fail closed。 |
| SEC-03 | 禁止自动重定向到未配置 origin。 | 3xx、DNS rebinding/host mismatch 和 client-supplied URL tests 通过。 |
| SEC-04 | OAuth 仅在官方契约、client registration、redirect、scope/resource 和条款 preflight 明确后启用。 | preflight 未通过不影响 API-key Provider 核心。 |

## 7. Capability 模型

核心不需要通用策略引擎，但必须区分“上游原生能力”和“当前路由可提供的有效能力”：

```text
deployment/model native claim:
  Supported | Unsupported | Unknown

effective route decision:
  Native | Bridged | Unsupported | Unknown
```

`Bridged` 由明确存在的协议 converter、目标协议和 feature preservation rule 推导，不能仅靠配置声明。最低能力集合：

- `responses`
- `chat_completions`
- `responses_websocket`
- `streaming`
- `function_tools`
- `parallel_tools`
- `structured_output`
- `reasoning`
- `multimodal_input`
- `hosted_tools`
- `continuation`
- `usage_streaming`

能力是 deployment/model/endpoint 级属性，不应只按 Provider 名称推断。`Unknown` 不得被当作 `Native`。

## 8. 质量与兼容性要求

### 8.1 兼容性证据

优先级从高到低：

1. 目标客户端固定版本的完整 Agent tool loop；
2. 脱敏真实 Provider JSON/SSE corpus；
3. 官方 SDK fixture；
4. mock provider contract test；
5. 外部项目源码推论。

源码推论不能替代真实 wire evidence。

### 8.2 有界资源

- request body、SSE event、tool arguments、Bridge IR、reasoning/provider state 和 slow-client buffer 必须有上限；
- stream transform 使用 pull-based stream 或有界 channel；
- 不逐 token 同步写 SQLite/日志或执行复杂路由；
- 记录 proxy 开销、TTFT、取消传播和 stream duration，用于回归比较，不设企业级容量目标。

### 8.3 错误与前向兼容

- 未知上游字段在 Native Path 尽量保留；
- 未知 SSE event 的处理必须按协议路径显式定义；
- 错误尽量保留安全的上游 status、request id、retry header 和可诊断 message；
- 不把 HTTP 200 中的 provider error event 当正常 token 输出。

## 9. P0 调研与实验 backlog

| 调研项 | 最小产物 | 收敛门 |
|---|---|---|
| Codex contract corpus | 固定版本、custom Provider 配置、HTTP/SSE Responses request/tool loop/cancel/error fixtures，并记录 `supports_websockets = false` 的诊断结果 | Native Responses HTTP/SSE tool loop 可重复通过，且没有隐式 WebSocket 尝试。 |
| Hermes contract corpus | 固定版本的 Chat 与 Responses tool loop、transport 切换、strict endpoint fixtures | 两个 native mode 行为可重复。 |
| Provider archetype matrix | OpenAI Responses、Generic Chat、Anthropic Messages 的 request/response/tool/stream/state 对比 | Provider Family 与 capability 模型无需 provider-name hacks，或明确修订。 |
| Native vs Bridge experiment | 相同协议直通与完整 IR round-trip 的字段保留、分配和兼容性比较 | 接受或推翻“Native Path 绕过 IR”。 |
| Bridge negative corpus | CLIProxyAPI、LiteLLM、Hermes、cc-switch issue/fixture 中的 continuation、tool identity、terminal failure | 每类失败都有 reject/repair 规则和 regression fixture。 |
| Reference project comparison | Codex、Hermes、LiteLLM、cc-switch、Bifrost、CLIProxyAPI 的问题驱动矩阵 | 每项核心决策至少有支持证据、替代方案和反例。 |

详细矩阵见[参考项目比较矩阵](../research/project-comparison-matrix.md)。

## 10. 核心接受条件

OpenBridge 核心方向在以下条件满足后可视为基本收敛：

1. Codex custom Provider（Responses HTTP/SSE、`supports_websockets = false`）→ Responses Provider 的 native tool loop 通过；
2. Hermes → Chat Provider 的 native tool loop 通过；
3. 至少两个不同 Provider Family 可由 alias 聚合；
4. ordered candidate、capability gate、首输出前 fallback 和 state affinity 通过；
5. Responses → Chat 的最小 bridge 支持文本与 function tool loop；
6. Chat → Responses 的最小 bridge 支持文本与 function tool loop；
7. Anthropic Messages 或等价异构 Provider 验证 adapter/Bridge IR 边界；
8. 真实/脱敏 SSE corpus 覆盖 fragmentation、unknown event、error、EOF、cancel 和 terminal；
9. 非 loopback 部署不会在无静态 token 的情况下开放业务接口；
10. 文档明确记录每个原型实验证明和未证明的事项。

## 11. 核心后的增强

建议顺序：

1. 轻量使用量/成本记录（JSONL 或 SQLite）；
2. 被动健康冷却与更丰富的 fallback reason；
3. Provider-hosted tool facade；
4. 本地/MCP Tool Bridge；
5. 可选 OAuth credential adapter；
6. 简单 Web UI 与更多模型/成本展示。

这些增强不得反向要求核心引入多租户、配额、合规审计或独立控制面。

## 12. 关联文档

- [目标客户端契约](../design/target-client-contracts.md)
- [目标架构与路线](../architecture/architecture-and-roadmap.md)
- [Rust Provider adapter 与数据流](../architecture/rust-provider-adapter-dataflow.md)
- [本地配置、路由与使用量](../architecture/local-configuration-routing-and-usage.md)
- [Chat/Responses bridge](../design/chat-responses-conversion.md)
- [开发与调研收敛计划](../plans/development-plan.md)
- [参考项目比较矩阵](../research/project-comparison-matrix.md)
- [当前实现说明](../implementation/current-implementation.md)
