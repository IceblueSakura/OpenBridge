# 当前代码注册表与原生路由

## 状态

**已实现基线。** OpenBridge 使用 bootstrap-only 运行配置和显式 Rust 代码注册表。旧的
`routes.toml`、route schema、reload 和 `ArcSwap` 已删除，不保留兼容入口。

## 1. 注册表模型

```text
RegistryDefinition
  version
  providers: ProviderDefinition[]
  models: ModelDefinition[]
  deployments: DeploymentDefinition[]
  aliases: AliasDefinition[]
```

```text
ProviderDefinition
  id
  ProviderKind
  CredentialDefinition

ModelDefinition
  stable logical id
  display metadata
  context input/output
  supported parameters
  reasoning: supported | unsupported | unknown
  reasoning_levels: minimal | low | medium | high | xhigh

DeploymentDefinition
  provider
  model
  upstream_model
  endpoint profile/base
  request timeout
  protocol capability profile
  typed model constraints

AliasDefinition
  public name
  ordered deployment candidates
```

Model 表示模型事实；Deployment 表示在一个具体 Provider endpoint 上如何调用该模型。同一个 Model
可以被多个 Deployment 引用，deployment constraint 只能收窄 context、reasoning 和参数集合。

## 2. 显式注册

模型目录和 Provider 目录分别返回 typed definition：

```rust
pub fn longcat_definition() -> ModelDefinition {
    ModelDefinition { /* canonical model facts */ }
}

pub fn definition() -> OpenAiDefinition {
    OpenAiDefinition {
        provider: ProviderDefinition { /* ... */ },
        deployments: vec![/* ... */],
    }
}
```

顶层注册表明确组合 Provider 和 alias：

```rust
pub fn compiled_definition() -> RegistryDefinition {
    let openai = openai::definition();
    RegistryDefinition {
        providers: vec![openai.provider],
        models: models::compiled_definitions(),
        deployments: openai.deployments,
        aliases: vec![/* ordered candidates */],
        /* ... */
    }
}
```

不使用 `inventory`、`linkme`、动态库或目录扫描。新增 Provider 必须修改顶层注册函数并增加契约测试。

## 3. 启动构建

```text
BootstrapPath::load()
→ BootstrapPolicy
→ providers::compiled_definition()
→ registry::build_registry()
→ RegistrySnapshot
→ Arc<RegistrySnapshot>
```

builder 校验：

1. registry version 非空；
2. Provider、credential、Model、Deployment、Alias ID 唯一；
3. 所有引用存在；
4. credential kind 被 adapter 接受，环境变量名称合法；
5. Model 字段、context、参数集合和 reasoning 一致；
6. reasoning level 规范且不重复；
7. deployment constraint 不扩大模型事实；
8. deployment capability 不超过 Provider descriptor；
9. endpoint profile 被 Provider 接受；
10. endpoint URL 与 path prefix 安全；
11. timeout 非零；
12. alias 非空、candidate 存在且不重复。

构建失败时服务不监听。构建完成后 snapshot 不再变化。

## 4. 路由

```text
client model alias
→ ordered deployment candidates
→ protocol/capability filter
→ model context/reasoning filter
→ first eligible candidate
→ Provider adapter encodes upstream request
```

Pipeline 只负责：

- 解析 public alias；
- 分类请求实际需要的功能；
- 选择兼容 candidate；
- 保留原始请求 bytes；
- 固定 provider-bound continuation 的 fallback 边界。

Provider adapter 负责：

- 写入 deployment 的 `upstream_model`；
- Provider-specific 字段转换；
- path、header 和认证；
- response/SSE terminal；
- 错误分类。

Pipeline 不包含 provider-name 分支，也不修改上游 model。

## 5. Reasoning

Model 同时声明 reasoning 三态和接受的标准 level 集合。请求：

- 没有 reasoning 字段时不要求 reasoning 能力；
- 只要求 reasoning、不指定 level 时，需要 `reasoning = supported`；
- 显式 `reasoning_effort` 或 `reasoning.effort` 时，还必须命中模型 level 集合；
- 未知 level、`unknown` 或 `unsupported` 均在 egress 前 fail closed；
- 不自动降级、升级或选择 reasoning level。

具体 Provider 如何编码这些标准 level 属于该 Provider adapter。OpenAI 原生路径当前保留原字段。

## 6. 能力与探测

有效路由能力当前为：

```text
Provider descriptor compile-time upper bound
∩ DeploymentDefinition capability
∩ Model metadata/reasoning facts
∩ request requirement
```

Probe 是额外证据，不自动修改注册表。`GET /v1/models` 只能证明模型 ID 在一次观察中可见，不能证明
tools、reasoning、image、structured output 或 streaming terminal。

## 7. 变更方式

增加或修改 Provider/Model：

1. 模型事实修改对应 `src/models/<model>.rs`；
2. Provider 行为和 deployment 修改对应 `src/providers/<provider>.rs`；
3. 必要时修改 `src/models/mod.rs`、`src/providers/mod.rs` 的显式聚合和 alias；
4. 添加 model、descriptor、adapter、registry、routing 和 probe fixture；
5. 运行默认验证；
6. 重启服务。

没有 route migration、reload 或旧 schema 兼容步骤。

## 8. 目标架构迁移边界

本文件保留当前 `ModelDefinition + ProviderDefinition + DeploymentDefinition + AliasDefinition` 的原生路由基线，不在这里混写目标注册表。目标概念、类型映射、实施切片、退出条件和回滚边界统一见[注册表与路由架构迁移计划](registry-architecture-migration.md)。

迁移完成前，源码和当前状态文档继续使用 `DeploymentDefinition`/`AliasDefinition` 指代现有类型；`UpstreamTarget`、`NativeOffering`、`PublicModel`、`ServingRoute` 和 `ResolvedBridgePlan` 只用于目标架构与迁移计划。

## 关联文档

- [Provider adapter 与数据流](provider-adapters-and-dataflow.md)
- [Bootstrap、代码注册表与凭证](../functional-requirements/configuration-and-credentials.md)
- [网关 API 与客户端兼容需求](../functional-requirements/gateway-api-compatibility.md)
- [服务架构](service-architecture.md)
- [注册表与路由架构迁移计划](registry-architecture-migration.md)
- [当前代码架构](../implementation-status/current-architecture.md)
- [当前实现说明](../implementation-status/current-implementation.md)
