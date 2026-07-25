# 本地配置、模型目录同步、路由与调用统计

## 状态

**Working hypothesis。** 本文定义单用户 OpenBridge 的配置文件优先、路由和调用统计接入边界。它取代独立控制面、proxy-issued key、principal ACL、配额和合规审计设计；当前运行时仍是环境变量 credential 基线，目标与已实现行为的差异见[当前实现说明](../implementation-status/current-implementation.md)。

## 1. 结论

OpenBridge 的服务所有者就是唯一管理员。核心不需要独立数据库控制面；由受信配置文件承载路由、认证和 telemetry 设置，环境变量不作为常规配置覆盖机制：

- 外部模型目录快照、model metadata、deployment；
- 上游 credential 与静态下游 Bearer token；
- public model alias；
- capability；
- timeout、candidate 顺序和 enable state；
- usage/TTFT/错误率的 headless 输出设置。

每个请求仍应生成不可变 `RoutePlan`，但它用于实际调用、诊断和调用统计，不承载 principal 授权或配额。

## 2. 配置模型

建议保持四个核心配置对象：

```text
ExternalCatalogSnapshot
  source: openrouter
  fetched_at / content_hash / schema_version
  normalized external model records

Model
  id
  catalog source + catalog id
  name
  description
  context_length: total, input?, output?
  supported_parameters
  reasoning: supported | unsupported | unknown
  optional local metadata correction provenance

Deployment
  id
  provider_family
  base_url
  credential_ref
  allowed_headers
  model
  upstream_model
  native_protocols
  native_transports
  native_capabilities
  model_constraints: limits and capability reductions only
  timeout
  enabled

PublicModelAlias
  name
  candidates: ordered deployment ids
```

外部目录快照是 OpenRouter 等来源的离线输入，不是运行时网络依赖。Model 是与 Provider 无关、由
服务所有者维护的稳定元信息目录，并可引用外部记录；Deployment 是一个可实际调用的上游模型目标，
将目录项映射到该 Provider 的原生 `upstream_model`，且能收窄实际通道可用能力；Alias 是下游稳定名称。
单用户核心优先保持配置直接，未来只有在重复配置成为明确问题时再增加可复用 Provider profile。

`context_length.total` 表示总上下文窗口；`input` 只在上游确实提供独立输入上限时填写；`output`
是单次生成上限。不得把外部目录的总 context window 自动解释为纯输入上限。

示例：

```toml
[[models]]
id = "openai/example-responses-model"
name = "Example responses model"
description = "Owner-maintained metadata for routing and model selection."
supported_parameters = ["max_tokens", "tools", "reasoning"]
reasoning = "supported"

[models.context_length]
total = 128000
output = 16384

[[deployments]]
id = "openai-coder"
provider_family = "openai"
base_url = "https://api.openai.com/v1"
credential = "config://openai_primary"
model = "openai/example-responses-model"
upstream_model = "example-responses-model"
native_protocols = ["responses"]
native_transports = ["http_json", "sse"]

[deployments.native_capabilities]
streaming = "supported"
function_calling = "supported"
parallel_tool_calls = "supported"
continuation = "supported"

[[models]]
id = "local/example-chat-model"
name = "Example local chat model"
supported_parameters = ["max_tokens", "tools"]
reasoning = "unknown"

[[deployments]]
id = "local-coder"
provider_family = "openai-compatible"
base_url = "http://127.0.0.1:8000/v1"
credential = "config://local_provider"
model = "local/example-chat-model"
upstream_model = "example-chat-model"
native_protocols = ["chat_completions"]
native_transports = ["http_json", "sse"]

[deployments.native_capabilities]
streaming = "supported"
function_calling = "supported"
parallel_tool_calls = "unknown"
continuation = "unsupported"

[[aliases]]
name = "code-primary"
candidates = ["openai-coder", "local-coder"]
```

### 2.1 目录、修正与 deployment constraint

下列是**目标配置形状**，不表示当前已实现 schema 已接受这些字段。同步结果写入独立快照，
不得由同步命令直接重写服务所有者手写的 `routes.toml`：

```toml
[[models]]
id = "openai/gpt-5"

[models.catalog]
source = "openrouter"
id = "openai/gpt-5"

# 仅用于有证据的全局模型事实修正，不放置某个订阅/账号的实际限制。
[models.local_correction]
description = "Verified local display description."
source = "operator-evidence"
reason = "Corrected vendor description"
verified_at = "2026-07-25T00:00:00Z"

[[deployments]]
id = "codex-subscription"
provider = "codex"
model = "openai/gpt-5"
upstream_model = "gpt-5"

# Codex subscription is a narrower route than the API model record.
[deployments.model_constraints.context_length]
total = 32768
output = 8192

[deployments.model_constraints]
reasoning = "unsupported"
disabled_parameters = ["reasoning"]
```

`model_constraints` 是 deployment 对基础模型元信息的 **overlay**，但其配置语义是约束：
它只能收窄，不得凭空增加能力。具体规则：

| 字段类别 | 合并规则 |
|---|---|
| 名称、描述、外部目录标识 | `local_correction` 覆盖外部快照；必须记录来源、原因和验证时间。 |
| `total`、`input`、`output` token 限制 | 取所有已知有效限制中的最小值。 |
| reasoning、协议/工具能力 | adapter 上界、模型声明与 deployment constraint 求交；`unknown` 不视为支持。 |
| `supported_parameters` | 以模型集合为基线，deployment 仅可用 `disabled_parameters` 删除。 |

因此，Codex 订阅窗口较小的事实只写入对应 deployment，不污染同一 Model 的 OpenRouter/API
deployment。若确有外部目录事实错误，才使用带 provenance 的 `local_correction`。

受版本控制的 `routes.toml` 只保存 `config://` 名称，不保存 secret。配套的、被忽略且权限受限的 `config/local.toml` 是实际密钥的首要来源，例如：

```toml
# config/local.toml — 仅当前服务所有者可读；不得提交
[secrets]
openai_primary = "replace-with-upstream-api-key"
local_provider = "replace-with-local-provider-key"
downstream_bearer = "replace-with-local-client-token"

[downstream_auth]
bearer_token = "config://downstream_bearer"
```

具体字段名仍可调整；关键边界是 Provider 行为由代码实现，deployment 数据与下游认证由服务所有者的配置文件定义。

当前原型中，`base_url` 必须是 HTTPS URL，且其 origin 必须命中 bootstrap 的 `allowed_origins`。除根路径外，它可以携带安全、固定的路径前缀，例如 `https://provider.example/openai`；transport 会把 adapter 的 `/v1/...` 目标追加为 `/openai/v1/...`。前缀仅允许未编码的 URL-safe segment，禁止 userinfo、query、fragment、空 segment、`.`、`..` 和双斜线；业务请求无权指定或改写它。

## 3. 配置来源与 secret

### 3.1 配置文件优先

首批配置格式收敛为 TOML。按从低到高的合并顺序是：

```text
内建无密钥默认值
→ 受版本控制的 bootstrap.toml / routes.toml
→ 私有的 local.toml
→ 启动参数显式指定的同类配置文件
```

模型目录不按上述 secret/config 文件规则做隐式字段覆盖。它有单独且固定的解析顺序：

```text
已接受的 ExternalCatalogSnapshot
→ Model local_correction（仅有 provenance 的字段修正）
→ Deployment model_constraints（仅收窄）
→ Adapter 与请求实际需求的运行时交集
```

目录快照由同步工具写入专用 cache/artifact；`routes.toml` 只声明逻辑模型、修正和 deployment
constraint，不复制整份外部目录，也不因为同步而被改写。

私有文件必须被 `.gitignore` 排除，并限制为运行服务的当前用户可读；它可以保存上游 API key 和下游静态 Bearer token，或保存供 keyring/file-secret 使用的引用。基础配置、示例、日志、诊断输出和测试 fixture 不得包含这些实际值。

环境变量只保留两类用途：部署系统选择配置文件位置，以及配置中明确写出的 `env://NAME` 兼容/迁移引用。它不参与按字段的隐式覆盖，尤其不能在存在 `config://` secret 时覆盖下游 token 或上游 API key。启动参数可以指定配置文件路径，但不接受单个 credential 或路由字段的命令行覆盖。

配置 reload 仍必须原子地重建完整 snapshot；私有 secret 文件变更与路由文件变更具有同等校验、失败保留旧 snapshot 的语义。同步工具可以写入专用目录快照 artifact，但第一版不需要管理 API，也绝不写回手写配置文件。

### 3.2 Credential 与下游 token

首批目标来源为：

```text
config://name                    # 私有 local.toml 中的 secret，首选
env://NAME                       # 明确配置时的兼容/迁移来源
keyring://service/account        # 后续
file-secret://absolute/path      # 可选，权限检查后
```

`config://name` 可以被 deployment credential 或 `downstream_auth.bearer_token` 使用。普通基础配置只保存 reference；私有配置可以保存实际值，但不得被版本控制、打印、回写或包含进 support bundle。解析后的 secret：

- 不写回 snapshot 序列化；
- 不进入 Debug 输出；
- 不进入响应、错误和普通日志；
- 仅在 Provider request 构造阶段短时使用。

### 3.3 受限 header

部分兼容 Provider 需要 version、account 或 routing header。允许值必须由对应 Provider Family 的配置 schema 明确列出；不提供通用 `headers = { ... }` 透传客户端 header 的能力。

### 3.4 外部目录同步

OpenRouter 是模型元信息的默认候选来源，不是生产请求的依赖。同步必须是管理员或 CI 显式触发的
离线动作，运行服务既不在启动时也不在请求路径查询外部目录。

```text
catalog sync openrouter
→ 下载并保留来源响应
→ 规范化为内部 ExternalCatalogSnapshot
→ 校验外部 id、字段类型与内容 hash
→ 与当前已接受快照生成 diff
→ 可选 probe / 人工审阅
→ catalog promote
→ 以新目录快照重建并原子发布 RegistrySnapshot
```

同步工具最低要求：

- 支持 `--dry-run`、机器可读 diff 与不含 secret 的退出状态；
- 每份快照保存 `source`、URL、`fetched_at`、内容 hash、规范化器版本和原始记录的可追溯引用；
- 不写入或格式化 `routes.toml`、`local.toml`，更不写入 credential；
- 外部删除、重命名、context 变化、参数变化、reasoning 变化均列为显式 diff；
- 只有 `promote` 成功后才影响新请求，失败时继续使用最近一次已接受的目录快照。

默认不自动接受会放宽可用能力的变更。外部目录将 context 或参数集合调大时，不得自动扩大
deployment 的有效能力；外部目录调小时必须产生高优先级审阅项。仍被 alias/deployment 引用的
外部模型若在新目录消失，标记为 `stale` 或 `deprecated`，不自动删除路由。

### 3.5 证据状态与过期

每项模型事实应附带状态，而非把同步成功等同于真实 deployment 验收：

| 状态 | 含义 | 路由处理 |
|---|---|---|
| `catalog` | 外部目录声明，尚未在本地通道验证。 | 可用于展示；能力是否可路由仍受 deployment/adapter 约束。 |
| `verified` | 有受控 probe 或实际契约证据。 | 可参与对应 deployment 的能力选择。 |
| `local_correction` | 服务所有者修正，含 provenance。 | 仅按声明字段覆盖目录。 |
| `stale` | 目录快照过期、模型下架或来源不可达。 | 不能自动放宽；现有 route 是否继续使用取决于显式策略。 |
| `unknown` | 没有可靠事实。 | 对需要该能力的请求 fail closed。 |

探测报告只提供证据，绝不自动把 `catalog` 或 `unknown` 改为 `verified`，也不自动修改 constraints。

## 4. 配置验证与 reload

构建新 snapshot 时验证：

1. id 唯一；
2. Provider Family 已编译；
3. base URL scheme/host/path 合法；
4. 禁止 URL userinfo；
5. credential reference scheme 已支持；
6. native protocol/transport 不超过 Provider Family 上界；
7. native capability 不无证据扩大 adapter 上界；
8. `responses_websocket` 只有在 adapter 和 transport 均实现时才允许声明 supported；
9. alias candidate 引用存在的 deployment；
10. 非 loopback listener 已通过受信配置解析到下游静态 token；
11. request/SSE/arguments limits 非零且有上限。
12. Model id 唯一，catalog source/id 可解析，且 deployment 引用存在的 Model；
13. `context_length.total/input/output` 为正值或未知，不把 total 伪装为 input；
14. `supported_parameters` 规范、去重，并与 reasoning 声明一致；
15. `model_constraints` 只收窄基础模型、adapter 与 Provider 的交集；
16. `local_correction` 含来源、原因、验证时间，且不携带 secret；
17. 目录快照完整性、内容 hash 与允许的来源策略有效；
18. 任何 `stale`/下架模型仍被 alias 引用时按显式发布策略处理，而非静默删除。

reload 流程：

```text
read files
→ load accepted catalog snapshot
→ parse
→ resolve references
→ merge model correction and deployment constraints
→ validate complete snapshot
→ compute config version
→ atomic swap
```

任何错误都保留旧 snapshot；不允许部分应用。外部同步本身与 reload 分离：sync 失败只表示没有
新目录候选，不能中断正在运行的服务；promote/reload 失败则保留上一个已接受目录和 route snapshot。

## 5. Alias 与路由

`GET /v1/models` 返回 alias，而不是内部 Provider model catalog：

```json
{
  "object": "list",
  "data": [
    {"id": "code-primary", "object": "model", "owned_by": "openbridge"}
  ]
}
```

路由顺序：

```text
client model
→ alias
→ ordered candidate deployments
→ resolve base model + local correction + deployment constraints
→ adapter/protocol/transport/capability/state filter
→ first eligible deployment
→ immutable RoutePlan
```

RoutePlan 至少固定：public alias、logical model id、deployment id、`upstream_model`、目录快照版本、
命中的 local correction/constraint revision、credential binding、协议模式、candidate 顺序与 fallback
边界。后续 reload 或目录 promote 不得改变已经开始的请求或 stream。

对每个 candidate 的有效能力计算顺序为：

```text
Provider adapter compile-time upper bound
∩ deployment native capability declaration
∩ model metadata fact
∩ deployment model_constraints
∩ request feature requirement
```

任何一层为 `unsupported` 或无法证明的 `unknown`，该 candidate 对相应请求均不可用。展示元信息可以
保留未知字段；请求路由不能把未知当作兼容。

第一版不做：

- principal-specific alias filtering；
- weight/percentage split；
- cost optimizer；
- distributed health；
- per-user budget。

最小被动 cooldown 是 Provider 聚合需要时的基础行为：某 deployment 在明确 429 或 adapter 认可的临时错误后短期跳过，优先遵循 `Retry-After`/rate-limit reset，并受本地最大冷却时间约束。它不能覆盖 continuation affinity；主动探测、跨进程共享和自适应权重仍属于后续方向。

可选 deployment capacity hint 可描述 owner 已知的 RPM/TPM/concurrency 上限，用于保守 admission/pacing；由于 Provider 可能按账号、模型、区域或外部流量共享配额，本地 hint 不是权威配额计数。具体要求见[Provider 韧性需求](../functional-requirements/provider-resilience.md)。

## 6. 最小入站认证

### 本地 loopback

可以在私有配置中配置 `downstream_auth.bearer_token = "config://..."`，也可以在明确本机信任模型下允许关闭；默认示例继续启用 token，减少客户端误连和未来迁移风险。`env://` 只是在配置显式选择时的兼容来源。

### 非 loopback

必须：

- 使用由私有配置（或其明确引用）提供的静态高熵 Bearer token；
- 通过 TLS 或可信反向代理传输；
- 不在 URL/query 中接受 token；
- constant-time compare；
- 日志只记录认证结果，不记录 token。

核心不需要：

- token 签发 API；
- revoke list；
- 多 key；
- principal/scopes；
- 面向下游用户/key 的 RPM/TPM 配额。

需要轮换时，服务所有者更新私有配置或其 secret reference 并 reload/restart。

## 7. 调用记录与统计接入

调用统计是 headless 运维能力，不是合规审计，也不参与路由、重试、fallback 或下游配额。详细的计时、终态、错误率与隐私口径见[调用统计与可观测性需求](../functional-requirements/observability.md)。

建议请求结束后生成：

```text
CallRecord
  request_id
  started_at
  model_alias
  provider_family
  deployment
  upstream_model
  downstream_protocol
  downstream_transport
  upstream_protocol
  upstream_transport
  route_mode: native | bridge
  attempt_count
  terminal_outcome
  error_class
  gateway_latency_ms
  gateway_ttft_ms | gateway_ttfb_ms
  input_tokens
  output_tokens
  reasoning_tokens
  cached_tokens
  estimated_cost
```

默认不记录：

- prompt/completion 正文；
- tool arguments/result 正文；
- credential/token/cookie；
- 完整 Provider payload。

首批输出组合：

```text
in-process bounded aggregates
→ Prometheus-compatible endpoint（可选，仅受保护/loopback）
→ rotated local JSONL（可选）
```

使用有界 channel 或请求结束后的非阻塞提交。sink 故障默认不阻塞模型响应，但应增加 dropped-record 计数/警告；SQLite、远程上传和用户级分析不属于首批目标。

## 8. 成本与模型 metadata

成本估算可以使用用户维护的静态 model metadata：

```text
input_price_per_million
output_price_per_million
cached_input_price_per_million
currency
source
updated_at
```

该数据只用于本地分析，不参与核心路由。Provider 价格可能变化，因此必须显示更新时间和未知状态，不能把过期值伪装为精确账单。

## 9. 安全边界

- 业务请求不能覆盖 base URL、credential reference 或 Provider auth/header；
- 配置文件权限应限制为当前用户；
- 禁用跨 origin redirect；
- 使用量 sink 不记录 secret 或正文；
- debug 日志不能打印完整请求 header/config；
- 非 loopback 无静态 token 时 fail closed；
- Hosted Tool/Tool Bridge 使用相同受信配置边界，不另开任意 URL 接口。

## 10. 与其他模块的接口

### Catalog store / sync CLI

sync CLI 只负责下载、规范化、diff、写入候选/已接受目录快照；Catalog store 向配置编译器提供
指定版本的只读快照。两者都不能读取 credential 明文、修改 route 文档或在请求热路径联网。

### Route planner

读取包含 effective model metadata 的 immutable config snapshot，返回 `RoutePlan`；不解析 secret 值，
也不重新执行目录合并。它只消费已固定的目录版本和 constraint revision。

### Provider adapter

接收 deployment snapshot 和短时 credential lease；不读取任意全局配置。

### Protocol Bridge

接收 RoutePlan 中的协议和 capability decision；不自行重新路由。

### Telemetry sink

只接收完成后的 `CallRecord` 和有界聚合更新；不影响 fallback 或 terminal ownership。

## 11. 最小推进顺序

下列顺序仅用于减少依赖和过度设计；只有第 1 项是当前焦点。每完成一项，都必须更新实施现状、
删除已完成计划并重新检查实际代码基线，才可决定是否开始下一项。

| 顺序 | 候选最小行为 | 前置条件与停止点 |
|---|---|---|
| 1（当前） | deployment `model_constraints`：针对现有 `input/output`、reasoning、参数集合实施只收窄 overlay。 | 见[当前开发焦点](current-focus.md)。完成后先停止，不直接开始同步。 |
| 2（待重新评估） | 外部目录的离线 fixture/规范化器；仅产出可比较的 `ExternalCatalogSnapshot`，不接入运行时。 | 只有第 1 项证明约束合并模型足够清晰，且确实需要减少手工模型维护时才创建新焦点。此时再决定是否引入 `context_length.total`。 |
| 3（待重新评估） | 显式 `sync --dry-run` 与人工接受的目录 artifact。 | 只有目录 fixture 稳定、diff 可审阅且失败不会覆盖旧 artifact 时才规划；不自动 promote。 |
| 4（待重新评估） | 已接受目录 artifact 与 route reload 的原子绑定。 | 只有实际需要让同步数据影响路由/展示，且前述 artifact 有版本与回滚证据时才规划。 |

`local_correction`、自动 `catalog promote`、真实 Provider 批量验证、价格/吞吐排序、管理 API 与第二
Provider Family 均不在当前队列；出现可观察需求后再按上述生命周期创建独立焦点。

## 12. 关联文档

- [产品范围](../functional-requirements/product-scope.md)
- [调用统计与可观测性](../functional-requirements/observability.md)
- [服务架构](service-architecture.md)
- [Provider 适配与数据流](provider-adapters-and-dataflow.md)
- [客户端兼容](client-compatibility.md)
- [Hosted tool 与 MCP](../functional-requirements/hosted-tools-mcp.md)
