# 代码注册表与原生路由

## 状态

**M1–M3 已实现的当前基线。** OpenBridge 使用 bootstrap-only 运行配置和显式 Rust 注册表；
`routes.toml`、动态 schema、reload 和旧 Deployment/Alias 兼容入口均不存在。后续迁移只见
[架构迁移总计划](registry-architecture-migration.md)。

## 1. 注册表模型

```text
RegistryDefinition
  version
  real_models: RealModelDefinition[]
  upstream_targets: UpstreamTargetDefinition[]
    offerings: NativeOfferingDefinition[]
  serving_routes: ServingRouteDefinition[]
  public_models: PublicModelDefinition[]
```

- `ProviderDescriptor` 是代码拥有的 Family/adapter 能力上界；
- `RealModelDefinition` 只保存与某个调用入口无关的模型事实；
- `UpstreamTargetDefinition` 表示共享 endpoint、credential、Real Model、timeout 与故障边界；
- `NativeOfferingDefinition` 表示一个 target 的单协议供应，拥有 upstream model、限制、能力与状态政策；
- `ServingRouteDefinition` 固定 target、offering、下游协议和执行模式；
- `PublicModelDefinition` 保存稳定公开名称与有序完整 route ID。

一个 target 可以同时拥有 Chat 与 Responses Offering，且两者的信息不必相同。如果 endpoint、账号、
quota、故障域、Real Model identity 或启停边界不同，应拆成多个 target。

## 2. 显式注册

`src/models/*` 返回 Real Model 定义，`src/providers/<provider>.rs` 返回该 Provider 的 target 与 Offering，
`src/providers/mod.rs` 显式组合 Serving Route 和 Public Model。注册不依赖目录扫描、动态库或 inventory。

当前示例：

```text
Public Model code-primary
  → code-primary-openai-chat
      → openai-main / chat
  → code-primary-openai-responses
      → openai-main / responses
```

route 优先级只由 `PublicModelDefinition.serving_routes` 的顺序表达；Serving Route 不重复保存 public model
或 priority，避免两个排序来源不一致。

## 3. 启动构建与不变量

```text
BootstrapPath::load
→ providers::compiled_definition
→ registry::build_registry
→ immutable RegistrySnapshot
```

builder 在监听前验证：

1. version、Real Model、target、Offering、route 与 Public Model 标识有效且唯一；
2. 所有 Real Model、target、Offering 和 route 引用存在；
3. credential kind、环境变量 locator、endpoint profile、HTTPS URL 与 timeout 合法；
4. target 至少有一个 Offering，同一 target 的 Offering ID 不重复；
5. Offering 的 capability 类型与协议一致，且不超过 Provider descriptor；
6. Offering 模型约束只能收窄 Real Model 事实；
7. Native route 的下游协议等于 Offering 协议；
8. Public Model 至少有一条存在且不重复的 Serving Route。

## 4. 请求分析与计划

```text
request bytes + downstream protocol
→ analyze_request
→ RequestProfile
→ Public Model ordered Serving Routes
→ complete-path gates
→ RoutePlan<ExecutionPlanCandidate>
```

`RequestProfile` 只解析一次 public model、协议、stream、功能组合、limits、reasoning 和 state affinity。
Planner 对每条完整 route 独立求交，不能把 route A 的 streaming 与 route B 的 tools 合并为虚假支持。

`ExecutionPlanCandidate` 固定：

- Serving Route ID；
- Upstream Target ID；
- Native Offering ID；
- 保留原始 bytes 的 `ValidatedRequest`。

当前 mode 只有 `Native`。携带 `previous_response_id` 的计划禁止跨 target fallback。

## 5. 执行所有权

- Provider adapter 从 Offering 读取 upstream model，并处理 path、字段、认证、响应终态与错误分类；
- transport 从 Upstream Target 读取 endpoint 和 timeout；
- Ingress 当前仍负责有限的首输出前 retry/fallback；
- pipeline 不按 Provider 名称分支，不执行网络 I/O，也不实现 Chat/Responses 转换。

## 6. 能力与探测

当前 Native route 的有效支持来自同一路径：

```text
ProviderDescriptor upper bound
∩ RealModel facts
∩ NativeOffering evidence and served limits
∩ request requirements
```

Probe 只形成显式观察报告，不自动修改注册表。`GET /v1/models` 只枚举 Public Model，不能证明某个
feature、Offering 或 route 可用。

## 7. 变更方式

1. 在 `src/models/*` 修改模型内在事实；
2. 在 `src/providers/*` 修改 target、Offering 或 adapter；
3. 在 `src/providers/mod.rs` 修改 route 与 Public Model 顺序；
4. 同步 registry、planner、probe 与 forwarding fixture；
5. 格式化、编译并按任务范围运行验证；
6. 重启服务使新 snapshot 生效。

## 8. 后续边界

M5 才增加 Converter 与 Bridge route；M6 抽取 AttemptManager 并统一 unsupported/availability；M7 仅按独立
需求建立模型信息安全投影。当前结构不代表这些能力已经实现。

## 关联文档

- [架构迁移总计划](registry-architecture-migration.md)
- [Provider adapter 与数据流](provider-adapters-and-dataflow.md)
- [当前代码架构](../implementation-status/current-architecture.md)
- [Bootstrap、代码注册表与凭证](../functional-requirements/configuration-and-credentials.md)
