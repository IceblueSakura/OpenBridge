# OpenBridge 架构

本文描述当前 checkout 的模块职责、启动装配和请求/响应数据流，并明确与下一步设计的差距。设计理由由 [ADR](decisions/README.md)维护，下一步范围见[多任务 IR 与 codec 验收目标](implementation-plans/next-goal.md)。

## 架构摘要

| 方面 | 当前结构 | 下一步 |
|---|---|---|
| 控制与执行 | 启动编译 immutable registry；请求使用固定 Public Model 接口和候选 | 保持静态路由、凭据和受信出口边界 |
| Generation 语义 | 已有 Static/Event IR；Native 静态内容部分由 IR 编码，Event 仍主要按来源事件编码 | 让所有 Generation 请求/响应的编码以 IR 为语义权威 |
| 其他推理任务 | Embedding、Images 有独立 pipeline，但没有完整语义 IR；专用音频任务仍复用 Chat wire | 区分任务、协议和模态，建立共享基础值的任务 IR 类型族 |
| Provider 差异 | adapter 绑定目标并执行部分 JSON 字段变换 | 明确协议编码与 Provider 语义映射的唯一所有权 |
| 生命周期 | ingress/execution/transport 管理有界 body、attempt、取消与 commit | 不因统一 IR 而引入全流缓存或提交后重试 |
| 扩展 | 有局部 ToolPlan 等基础，尚无生产通用 hook 管线 | 先建立语义处理位置，工具注入、拦截与分析策略另行实现 |

**先确认任务语义能被 IR 区分，再验证修改 IR 是否真正改变最终 wire；调用过 decode 不等于完整覆盖。** 原则见 [ADR-0001](decisions/0001-generation-ir-authority.md)，任务边界与设计准入见 [ADR-0002](decisions/0002-task-ir-and-semantic-ownership.md)。

OpenBridge 是运行在所有者控制环境中的 headless、OpenAI-compatible、多 Provider 网关。受信 Rust catalog 将 Model、Provider、Upstream Target、Upstream API、Route 和 Public Model 编译为固定的下游接口；Bootstrap 与私有文件只提供进程策略、用户和 credential material。下游请求只能选择 Public Model，不能提交上游 URL、Provider、Target、Route、credential、认证 header 或转换规则。

## 1. 系统上下文

```text
Bootstrap + private users + private upstream credentials
                         │
                         ▼
                 composition root (main)
                         │
        ┌────────────────┼────────────────┐
        ▼                ▼                ▼
 immutable RuntimeRegistry  User/Credential  Telemetry runtime
        │                │                │
        └────────────────┴───────┬────────┘
                                  ▼
                     authenticated downstream HTTP
                                  │
             ┌────────────────────┼────────────────────┐
             ▼                    ▼                    ▼
       Models projection   operation pipeline       MCP service
                                  │
                                  ▼
                      trusted Provider/Target egress
                                  │
                                  ▼
                   bounded JSON or SSE response lifecycle
                                  │
                                  ▼
                     downstream response + observations
```

- 下游客户端面向固定的 OpenAI-compatible HTTP surface；MCP 在独立的 transport/discovery/tool dispatch 中运行，不进入 Provider generation 链路。
- Provider、Target 和 credential binding 属于受信配置/代码边界；业务请求只携带协议事实和 Public Model 名称。
- 健康检查只反映本地进程和编译注册表，不在普通请求外隐式探测真实 Provider。

## 2. 依赖方向与跨模块不变量

依赖从启动装配流向不可变运行时快照，再流向请求处理和受信上游出口：

```text
bootstrap/private files
  → composition root
  → immutable registries and credential stores
  → ingress admission
  → operation facts and fixed plan
  → Provider adapter
  → shared transport
  → response/observation lifecycle
```

保持以下边界：

- 配置解析、Model/Provider 注册和 registry compilation 只验证并装配数据，不执行网络 I/O；启动失败不会发布半成品快照。
- request analyzer 只提取 wire facts；它不解析 registry entity、不解析 credential，也不选择 Route。
- planner 只消费已编译的 Public Model interface 和固定候选；请求期不根据 Provider 名称、动态探测结果或请求提交的 endpoint 重新路由。
- Provider adapter 只处理已选中的 operation、Target 和 credential binding；它不选择 Public Model/Route，也不接受请求控制的 URL、认证 header、proxy header 或转换脚本。
- transport 只发送 adapter 生成的受信相对 URI 和安全 header；协议语义、模型能力、错误分类和 retry policy 由上层拥有。
- Public Models projection 只暴露下游安全的 identity、interface 和 capability，不序列化 Provider、Target、Route、upstream model、endpoint、credential 或健康状态。
- pipeline、IR、adapter 和 transport 的职责分离，理由是让纯验证/规划可独立测试，并让敏感数据、网络 I/O 与响应终态集中在明确的生命周期边界。

## 3. 模块职责

模块按责任或独立协议域划分，而不是按文件大小划分。根模块保留 facade 和既有 crate path，具体实现位于子模块。

| 责任域 | Owner | 主要边界 |
| --- | --- | --- |
| Bootstrap 与进程策略 | `src/config/`、`src/main.rs` | 严格解析 listener、私有文件位置、资源限制、HTTP client、日志和 telemetry；负责启动/关闭，不拥有 Model/Provider 事实 |
| 下游身份与 credential | `src/identity.rs`、`src/credential/`、`src/upstream_credentials/`、`src/oauth2_credentials/` | 启动加载用户和上游 binding，构造 purpose-bound、不可变 runtime store；OAuth lifecycle 可在固定 binding 内 refresh，不把 secret 交给 registry 或请求 |
| Canonical Model | `src/models/` | 编译期 Model identity、task 及其模型事实；不负责上游网络发现 |
| Provider contract 与实现 | `src/provider/`、`src/providers/` | 闭合 Provider kind、operation adapter、credential/header/error/SSE contract，以及各 Provider 的 trusted origin、operation path 和 registration |
| Registry 与 Public Model | `src/registry/` | 校验引用和 capability ceiling，编译 immutable runtime entities、固定 Route candidates、私有 execution interface，并投影 downstream-safe Models DTO |
| 请求分析与规划 | `src/pipeline/` | Generation、Embeddings、Images 各自拥有 analyzer、preflight、planner 和 pure response policy；不执行 body I/O、credential、transport、observation 或 commit |
| Generation semantic IR | `src/ir/generation/` | Provider-neutral Static/Event values、验证、reducer 和 materializer；不拥有 registry、routing、credential、网络 I/O 或 downstream commit |
| Generation codec / Protocol Bridge | `src/bridge.rs`、`src/bridge/static_codec/`、`src/bridge/event_codec/` | 在固定 plan 上执行部分 IR 权威的 Native 静态编码与受限跨协议 lowering；Native Event 共享 reducer 校验但仍主要编码源 payload；不拥有路由或 commit |
| HTTP ingress | `src/ingress/` | Router、Bearer admission、body lifecycle、operation handlers、attempt/fallback、streaming response、错误映射和 downstream commit |
| Attempt coordination | `src/execution.rs`、`src/execution/` | 只管理请求级 attempt/candidate state、硬预算和 backoff；不拥有 operation pipeline、Provider 分类、credential 选择或 commit policy |
| Upstream transport | `src/transport/` | 共享 HTTP client、validated target、相对 URI、timeout、safe headers 和 SSE framing；不解释业务路由 |
| Observability | `src/observability.rs`、`src/observability/` | request terminal、Provider attempt、usage、metrics、OTLP 和本地脱敏 HTTP snapshot；内容 snapshot 不进入 reviewed OTLP trace layer |
| MCP | `src/mcp/` | 独立的 HTTP transport、discovery、session 和工具执行；不读取 registry、credential 或 Provider transport |
| 管理员 probe | `src/probe.rs`、`src/bin/openbridge-probe.rs` | 在已注册边界内进行显式验证；不修改 registry，也不属于普通下游请求链路 |

`core/capability.rs` 只汇总 `ApiCapabilities`；各 operation 的 capability 规则位于对应子模块。`registry/public_model.rs`、`observability.rs` 和 `pipeline/mod.rs` 是 facade，具体 ownership 保持在各自子模块。

## 4. 启动装配

`src/main.rs` 在绑定 listener 前完成所有必需解析、校验和 runtime 构造：

```text
BootstrapConfigPath::load
  → TelemetryRuntime + tracing
  → UserConfigPath::load
  → UpstreamCredentialConfigPath::load
  → derive active credential-pool identities
  → compile RuntimeRegistry with active pools
  → materialize CredentialStore + OAuth2CredentialManager
  → validate registry/credential ownership
  → create UpstreamClient + GatewayState
  → build ingress Router
  → bind loopback listener and serve Axum
  → stop service, OAuth worker, JSONL writer, and telemetry
```

Bootstrap 拥有 listener、私有配置 locator、body/replay/SSE limits、共享 HTTP client、默认 instructions、本地下游内容日志和 OTLP exporter policy。Provider、Model、Target、API、Route 和 Public Model 由受信 catalog 编译，普通 TOML 不能新增这些实体。

私有用户和上游 credential 文件只在启动时读取并合并为不可变的 runtime snapshot；用户变更需要重启。active pool 集合只能使静态 Target 不可用，不能新增 Provider、endpoint、Route 或 capability。OAuth manager 只在固定 credential binding 内执行受控的 refresh/reload。任何解析、引用或 credential ownership 错误都在 listener 绑定前失败。

## 5. Registry、Route 与 Public Model

```text
RegistryConfig
  ├─ canonical Models
  ├─ trusted Provider instances
  ├─ credential-pool bindings
  ├─ Upstream Targets
  │    └─ typed Upstream APIs
  └─ Public Models
       └─ ordered Route candidates
           ↓ validate + compile
      immutable RuntimeRegistry
```

编译按依赖顺序解析 Model、Provider instance、credential pool、Target/API 和 Public Model。它检查引用唯一性、Provider capability ceiling、operation/task identity、endpoint 安全性、credential 与 Provider 的匹配以及 Route 的固定顺序；任一阶段失败都不产生可供请求路径使用的快照。

每个 Upstream API 保留 typed operation/task identity、effective model facts、capabilities、streaming policy 和必要的 wire narrowing。Public Model 从 private execution interface 投影出固定下游 contract；请求期只消费该 interface 和其固定 candidates，不通过反向解析 Models JSON 或临时能力筛选来改变 Route。

Generation 的同协议路径称为 Native；跨 Chat/Responses 使用受限 Bridge。两者描述协议关系，不表示所有语义都能在任意 Provider 间转换。Embeddings 和 Images 使用各自的 Native-only operation plan，不复用 Generation target 或 Bridge。

## 6. 请求与响应主数据流

### 6.1 下游 admission

Router 先设置 request ID、敏感 header 标记和 configured body limit，再执行 Bearer authentication。只有认证成功的请求进入 handler；随后建立 request observation，并在显式启用时对最终下游边界做有界、本地、脱敏的 content snapshot。响应 body wrapper 负责真实 EOF、body error 或取消时的 terminal，不按 SSE chunk 记录本地日志。

### 6.2 Generation

以下是**当前调用顺序**，不是 ADR 的目标管线：

```text
JSON admission
  → analyze Chat/Responses request facts
  → resolve Public Model and fixed operation interface
  → capability/state/limit preflight
  → normalize shared request policy on JSON
  → for each fixed candidate independently:
       candidate-specific JSON omissions
       → decode canonical Static IR
       → Native owned controls/content + source hints, or cross-protocol lowering
  → ProviderAdapter binds model and applies trusted JSON/profile mappings
  → bounded attempt loop + UpstreamTransport
  → decode/validate canonical JSON or Event IR response
  → static owned content/source hints, Native source event encoding, or cross-protocol projection
  → downstream response commit and observations
```

`analyze` 和 `plan` 可以拒绝请求，但不读取 upstream body、不取 credential、不执行网络 I/O，也不提交下游 response。Bridge 在 canonical IR 上做协议转换；Native 请求与静态响应的内容 encoder 按稳定 item/group/part 身份将已拥有的文本及 function-tool 语义写回 wire，不借源记录恢复被删内容。采样控制继续由 IR 编码，未迁移语义仍需要源保留；缺少安全绑定或存在未知依赖时，内容变换失败关闭，而不是调用较窄 Bridge encoder。具体支持与拒绝边界归[实施状态](implementation-status/current-boundaries.md#generation-与-bridge与-ir-权威目标的差距)；“已经过 IR 校验”仍不等于“整个任务最终编码完全由 IR 决定”。

当前 `plan_request` 独立地从同一个规范化 JSON body 构造候选，并非前一候选修改后一候选；但完整语义 decode 仍在候选展开后，`src/ir/generation/requirements.rs` 的纯投影尚未成为全部生产预检的权威。Native request/response encoder 还会为旧语义基线再次 decode 来源对象。

流式路径的 `StaticEventBridge` 先 decode/reduce；跨协议 encoder 消费 canonical event，而 Native 分支调用 `native::encode` 读取原 `SseEvent` payload，并在特定稀疏 terminal 中利用已验证状态补齐。这是共享生命周期校验，不是 Event IR 已全面决定输出；目标边界见 [ADR-0005](decisions/0005-event-ir-and-delivery-lifecycle.md)。

目标是先解析固定任务再 decode，由最终 IR 导出 requirements、固定预检和候选 lowering，最后 encode；阶段输入输出由 [ADR-0003](decisions/0003-ir-pipeline-and-target-compilation.md#1-目标数据流)维护。上述现行顺序不会因目标文档修订而自动改变。

### 6.3 Embeddings、Images 与 Models

Embeddings 和 Images 分别经过自己的 analysis、fixed-interface preflight、planning、bounded upstream response validation 和 response projection。它们的 response policy 不读取 body；ingress 在实际 body 生命周期中执行读取、usage 观察、错误终态和 downstream commit。Embeddings 不做 Bridge、跨模型 fallback、归一化、降维或缓存；仅在已注册 target/API policy 下允许 float32/Base64 wire 表示转换，精度边界见[Embedding 合同](functional-requirements/extended-capabilities/embeddings.md#1-用户结果)；Images 按固定候选执行单一请求，不把图片 URL/bytes 放入普通 OTLP attributes。

Models list/retrieve 与 extended Models API 都从同一 immutable Public Model snapshot 读取：标准视图只返回下游 identity，扩展视图返回下游安全的 task/interface/limit/capability。

当前 `EmbeddingRequest`、`ImagesRequest` 仍包裹 wire body，analysis 的形状/数量 facts 不是完整任务语义 IR。目标是为各任务建立独立 request/response 语义，复用有界资源等基础值；专用 Speech 与多模态 Generation 按任务契约区分，不按 Chat endpoint 猜测。不把 Models/MCP 纳入推理 IR，设计见 [ADR-0002](decisions/0002-task-ir-and-semantic-ownership.md)。

## 7. Credential、IR、retry 与 commit 边界

### Credential

Registry 只保存 credential pool 的非敏感 identity、Provider kind 和 credential kind；secret material 属于 `CredentialStore` 或 OAuth manager。请求只能引用已经编译的 Target/API，ingress 在准备固定 candidate 时取得与 Provider、operation 和 state affinity 匹配的 credential。业务 payload 不能覆盖 upstream URL、Provider、Target、credential、认证 header 或 proxy header。

### IR 与 Bridge

Generation IR 表达支持范围内的语义并集，包括文本、refusal、工具、非完成结果及有界命名空间扩展，不以任一 Provider 或协议的能力交集裁剪核心类型。固定 Public Model 能力交集仍属于 registry/preflight。IR 不携带 registry entity、Route、credential 或上游 endpoint 定位信息，也不直接访问 body、clock、task 或 observation。

当前 `prepare_routed_request` 在 Bridge 编码后解析 JSON，执行 model binding、request body hook、忽略参数删除与 reasoning level 映射。目标不是删除所有 Provider 差异，而是按 [ADR-0003](decisions/0003-ir-pipeline-and-target-compilation.md#5-provider-映射与执行上下文)将语义 policy、目标表示与执行绑定分开；来源信息按 [ADR-0004](decisions/0004-source-records-and-fidelity.md)合并，不形成第二个业务语义 owner。尚未实现通用的注入、分析或拦截 hook。

### Retry、fallback 与 cooldown

`AttemptCoordinator` 对每个请求维护有界的总 attempt 和 candidate-local state，并提供 capped backoff。Generation 与 Embeddings 共享这一请求级机制；Images 在其 handler 中只执行 finish-only 的单一 attempt policy。候选顺序来自启动时编译的 Route plan，retry/fallback 不能扩大候选集、重新选择 Provider 或重新读取配置。

重试只可发生在下游提交前，并服从 operation 的固定策略；Generation 可切换到后续固定 candidate，Embeddings 当前只有单 Route，不能跨 Route fallback。产品拒绝上游有状态请求，内部 state-affinity 防线不表示公开 continuation 支持。cooldown 只影响可选 candidate 的可用性，不改变 registry，也不执行动态权重或跨进程健康状态同步。

### Commit 与终态

下游 commit 是不可逆边界：在首个可见业务输出前，响应仍可因受信的 retryable failure 进行有界 retry/fallback；commit 后不能拼接另一个 upstream 响应。Native 与 Bridge SSE 都经 Event IR 校验后编码，当前 Native 正文仍主要来自源事件 payload；允许为确定的协议规范化做有界缓冲，一旦有合法输出即发送，不固定等待或默认缓存整条流。terminal 前 EOF、body error、非法 framing 或超限都以失败关闭，不伪造 terminal。下游取消会取消对应 upstream body。

### Observation

request observation 记录 downstream lifecycle，Provider observation 记录每个编译 candidate 的 attempt，usage 只取自经过验证的 Provider response。OTLP 和 metrics 使用固定 allowlist；credential、request body、未验证 model、endpoint URL 和高基数内部 topology 不进入 reviewed trace。内容日志是独立的 bounded JSONL sink，sink 故障不改变业务响应。
