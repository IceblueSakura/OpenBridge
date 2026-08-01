# 代码注册表与原生路由

## 状态

**当前实现。** OpenBridge 使用 bootstrap-only 运行配置和显式 Rust 注册表；没有 route 配置文件、
动态 schema、reload 或运行时 Provider DSL。

## 1. 注册表模型

```text
RegistryConfig
  version
  models: ModelConfig[]
  upstream_targets: UpstreamTargetConfig[]
    upstream_apis: UpstreamApiConfig[]
  routes: RouteConfig[]
  public_models: PublicModelConfig[]
```

- `ProviderContract` 是代码拥有的 Family/adapter 能力上界；
- `ModelConfig` 只保存与某个调用入口无关的模型事实；
- `UpstreamTargetConfig` 表示共享 endpoint、credential、Model、timeout 与故障边界；
- `UpstreamApiConfig` 表示一个 target 的单协议供应，拥有 upstream model、限制、能力与状态绑定方式；
- `RouteConfig` 固定 target、upstream API、下游协议和执行模式；
- `PublicModelConfig` 保存稳定公开名称与有序完整 route ID。

一个 target 可以同时拥有 Chat 与 Responses Upstream API，且两者的信息不必相同。如果 endpoint、账号、
quota、故障域、Model identity 或启停边界不同，应拆成多个 target。

## 2. 显式注册

`src/models/*` 返回 Model 定义，`src/providers/<provider>.rs` 返回该 Provider 的 target 与 Upstream API，
`src/providers/mod.rs` 显式组合 Route 和 Public Model。注册不依赖目录扫描、动态库或 inventory。

当前示例：

```text
Public Model code-primary
  → code-primary-openai-chat
      → openai-main / chat
  → code-primary-openai-responses
      → openai-main / responses
```

route 优先级只由 `PublicModelConfig.routes` 的顺序表达；Route 不重复保存 public model
或 priority，避免两个排序来源不一致。

## 3. 启动构建与不变量

```text
BootstrapConfigPath::load
→ providers::compiled_config
→ registry::build_registry
→ immutable RuntimeRegistry
```

builder 在监听前验证：

1. version、Model、target、Upstream API、route 与 Public Model 标识有效且唯一；
2. 所有 Model、target、Upstream API 和 route 引用存在；
3. credential kind、环境变量 locator、endpoint profile、HTTPS URL 与 timeout 合法；
4. target 至少有一个 Upstream API，同一 target 的 Upstream API ID 不重复；
5. Upstream API 的 capability 类型与协议一致，且不超过 Provider contract；
6. Upstream API model rules 只能收窄 Model 事实；
7. Native route 的下游协议等于 Upstream API 协议；
8. Public Model 至少有一条存在且不重复的 Route。

## 4. 请求分析与计划

```text
request bytes + downstream protocol
→ analyze_request
→ RequestRequirements
→ Public Model ordered Routes
→ complete-path gates
→ RoutePlan<RouteCandidate>
```

`RequestRequirements` 只解析一次 public model、协议、stream、功能组合、limits、reasoning 和 state affinity。
Planner 对每条完整 route 独立求交，不能把 route A 的 streaming 与 route B 的 tools 合并为虚假支持。

`RouteCandidate` 固定：

- Route ID；
- Upstream Target ID；
- Upstream API ID；
- 保留原始 bytes 的 `ApiRequest`。

当前 mode 只有 `Native`。携带 `previous_response_id` 的计划禁止跨 target fallback。

## 5. 执行所有权

- Provider adapter 从 Upstream API 读取 upstream model，并处理 path、字段、认证、响应终态与错误分类；
- transport 从 Upstream Target 读取 endpoint 和 timeout；
- Ingress 当前仍负责有限的首输出前 retry/fallback；
- pipeline 不按 Provider 名称分支，不执行网络 I/O，也不实现 Chat/Responses 转换。

## 6. 能力与探测

当前 Native route 的有效支持来自同一路径：

```text
ProviderContract upper bound
∩ Model facts
∩ UpstreamApi evidence and served limits
∩ request requirements
```

Probe 只形成显式观察报告，不自动修改注册表。`GET /v1/models` 只枚举 Public Model，不能证明某个
feature、Upstream API 或 route 可用。

## 7. 变更方式

1. 在 `src/models/*` 修改模型内在事实；
2. 在 `src/providers/*` 修改 target、Upstream API 或 adapter；
3. 在 `src/providers/mod.rs` 修改 route 与 Public Model 顺序；
4. 同步 registry、planner、probe 与 forwarding fixture；
5. 格式化、编译并按任务范围运行验证；
6. 重启服务使新的 `RuntimeRegistry` 生效。

## 8. 尚未实现

Converter/Bridge route、独立 AttemptManager、统一 unsupported/availability 和模型信息安全投影均未实现。
这些能力必须由各自的功能需求和专项计划单独驱动。

## 关联文档

- [Provider adapter 与数据流](provider-adapters-and-dataflow.md)
- [当前代码架构](../implementation-status/current-architecture.md)
- [Bootstrap、代码注册表与凭证](../functional-requirements/configuration-and-credentials.md)
