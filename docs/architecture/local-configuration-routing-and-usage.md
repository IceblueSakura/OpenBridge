# 本地配置、模型路由与使用量

## 状态

**Working hypothesis。** 本文定义单用户 OpenBridge 的配置和可选使用量边界。它取代独立控制面、proxy-issued key、principal ACL、配额和合规审计设计。

## 1. 结论

OpenBridge 的服务所有者就是唯一管理员。核心不需要独立数据库控制面；配置文件、环境变量和可选 keyring/secret store 足以承载：

- deployment；
- credential reference；
- public model alias；
- capability；
- timeout、candidate 顺序和 enable state；
- 可选静态下游 Bearer token；
- 可选 usage sink。

每个请求仍应生成不可变 `RoutePlan`，但它用于实际调用、诊断和使用量记录，不承载 principal 授权或配额。

## 2. 配置模型

建议保持两个核心配置对象：

```text
Deployment
  id
  provider_family
  base_url
  credential_ref
  allowed_headers
  upstream_model
  native_protocols
  native_transports
  native_capabilities
  timeout
  enabled

PublicModelAlias
  name
  candidates: ordered deployment ids
```

Deployment 是一个可实际调用的上游模型目标；Alias 是下游稳定名称。单用户核心优先保持配置直接，未来只有在重复配置成为明确问题时再增加可复用 Provider profile。

示例：

```toml
[[deployments]]
id = "openai-coder"
provider_family = "openai"
base_url = "https://api.openai.com/v1"
credential = "env://OPENAI_API_KEY"
upstream_model = "example-responses-model"
native_protocols = ["responses"]
native_transports = ["http_json", "sse"]

[deployments.native_capabilities]
streaming = "supported"
function_tools = "supported"
parallel_tools = "supported"
continuation = "supported"

[[deployments]]
id = "local-coder"
provider_family = "openai-compatible"
base_url = "http://127.0.0.1:8000/v1"
credential = "env://LOCAL_PROVIDER_KEY"
upstream_model = "example-chat-model"
native_protocols = ["chat_completions"]
native_transports = ["http_json", "sse"]

[deployments.native_capabilities]
streaming = "supported"
function_tools = "supported"
parallel_tools = "unknown"
continuation = "unsupported"

[[aliases]]
name = "code-primary"
candidates = ["openai-coder", "local-coder"]
```

具体字段名仍可调整；关键边界是 Provider 行为由代码实现，deployment 数据由服务所有者配置。

## 3. 配置来源与 secret

### 3.1 普通配置

允许：

- TOML/YAML/JSON 文件；
- 明确的环境变量覆盖；
- 启动参数指定配置路径；
- 原子 reload 路由配置。

不建议第一版实现管理 API 或写回配置文件。

### 3.2 Credential reference

首批建议支持：

```text
env://NAME
keyring://service/account        # 后续
file-secret://absolute/path      # 可选，权限检查后
```

普通配置只保存 reference。解析后的 secret：

- 不写回 snapshot 序列化；
- 不进入 Debug 输出；
- 不进入响应、错误和普通日志；
- 仅在 Provider request 构造阶段短时使用。

### 3.3 受限 header

部分兼容 Provider 需要 version、account 或 routing header。允许值必须由对应 Provider Family 的配置 schema 明确列出；不提供通用 `headers = { ... }` 透传客户端 header 的能力。

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
10. 非 loopback listener 已配置下游静态 token；
11. request/SSE/arguments limits 非零且有上限。

reload 流程：

```text
read files
→ parse
→ resolve references
→ validate complete snapshot
→ compute config version
→ atomic swap
```

任何错误都保留旧 snapshot；不允许部分应用。

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
→ protocol/transport/capability/state filter
→ first eligible deployment
→ immutable RoutePlan
```

第一版不做：

- principal-specific alias filtering；
- weight/percentage split；
- cost optimizer；
- distributed health；
- per-user budget。

最小被动 cooldown 属于 C2 核心：某 deployment 在明确 429 或 adapter 认可的临时错误后短期跳过，优先遵循 `Retry-After`/rate-limit reset，并受本地最大冷却时间约束。它不能覆盖 continuation affinity；主动探测、跨进程共享和自适应权重仍属于增强。

可选 deployment capacity hint 可描述 owner 已知的 RPM/TPM/concurrency 上限，用于保守 admission/pacing；由于 Provider 可能按账号、模型、区域或外部流量共享配额，本地 hint 不是权威配额计数。具体要求见[Provider 韧性需求](../requirements/provider-resilience.md)。

## 6. 最小入站认证

### 本地 loopback

可以配置一个静态 Bearer token，也可以在明确本机信任模型下允许关闭；默认示例继续启用 token，减少客户端误连和未来迁移风险。

### 非 loopback

必须：

- 使用一个静态高熵 Bearer token；
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

需要轮换时，服务所有者更新 secret reference 并 reload/restart。

## 7. 使用量记录

Usage analysis 是核心后的轻量增强，不是合规审计。

建议请求结束后生成：

```text
UsageRecord
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
  outcome
  error_class
  latency_ms
  time_to_first_event_ms
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

首批 sink：

```text
stdout JSON
→ JSONL
→ SQLite（可选）
```

使用有界 channel 或请求结束后的非阻塞提交。sink 故障默认不阻塞模型响应，但应增加 dropped-record 计数/警告。

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

### Route planner

读取 immutable config snapshot，返回 `RoutePlan`；不解析 secret 值。

### Provider adapter

接收 deployment snapshot 和短时 credential lease；不读取任意全局配置。

### Protocol Bridge

接收 RoutePlan 中的协议和 capability decision；不自行重新路由。

### Usage sink

只接收完成后的 `UsageRecord`；不影响 fallback 或 terminal ownership。

## 11. 关联文档

- [核心需求](../requirements/proxy-requirements.md)
- [目标架构与路线](architecture-and-roadmap.md)
- [Rust Provider adapter 与数据流](rust-provider-adapter-dataflow.md)
- [目标客户端契约](../design/target-client-contracts.md)
- [Hosted tool 增强需求](../requirements/hosted-tools-mcp.md)
