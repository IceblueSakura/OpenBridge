# 架构迁移总计划：注册表、路由与协议执行

## 状态

**唯一架构迁移总计划。** M1–M4 的代码结构迁移已经完成：目标注册表成为唯一生产来源，请求路径使用 `RequestProfile + RoutePlan`，Native 执行按 target/offering 读取调用信息。按本次要求没有运行测试，因此 M0 回归门和 M1–M4 的行为验收仍待补做；M5–M7 未实现。本文继续统一规划 route-local Protocol Bridge、安全 fallback/availability 与模型信息投影。专项文档只能展开本文的对应切片，不另行定义迁移顺序。

当前事实见[当前代码架构](../implementation-status/current-architecture.md)，目标状态见[目标服务架构](service-architecture.md)。每次实施仍须从本文选择一个可观察行为，在[当前开发焦点](current-focus.md)中先写失败测试。

## 1. 迁移目标

当前注册关系：

```text
RealModelDefinition
UpstreamTargetDefinition (ProviderKind + endpoint + credential + shared boundary)
  NativeOfferingDefinition[] (one native protocol each)
ServingRouteDefinition
PublicModelDefinition (ordered complete routes)
```

目标注册关系：

```text
Code-owned catalogs
  ProviderDescriptor
  ConverterDescriptor

Service registry
  RealModelDefinition
  UpstreamTargetDefinition
    NativeOfferingDefinition[]
  PublicModelDefinition
  ServingRouteDefinition[]

Compiled snapshot
  EffectiveExecutionProfile[]
  ResolvedBridgePlan? per bridge route
```

目标解决四个问题：

1. `Deployment` 名称与职责含混，改为表达实际调用边界的 `UpstreamTarget`；
2. 同一上游目标可同时提供 Chat 与 Responses，但两条协议拥有独立 model id、limits、capability evidence 和 state policy；
3. Public Model 的能力来自至少一条完整 Serving Route 的交集，而不是不同候选字段的并集；
4. Bridge 配置只选择已编译 converter 并以内联政策收窄能力，不建立没有复用需求的顶层 Bridge Profile。

## 2. 总计划与专项文档映射

| 切片 | 当前状态 | 唯一交付边界 | 主要专项材料 | 明确依赖 |
|---|---|---|---|---|
| M0 | 文档存在；本次未运行测试，回归门待验收 | 冻结当前 Native 行为和术语基线 | [当前代码架构](../implementation-status/current-architecture.md)、[当前代码注册表与原生路由](configuration-and-routing.md)、[Provider 适配与数据流](provider-adapters-and-dataflow.md) | live source 与默认测试 |
| M1 | 代码完成；行为验收待补 | 引入 Native 目标定义类型 | [目标服务架构](service-architecture.md) | M0 |
| M2 | 代码完成；行为验收待补 | 切换 builder/snapshot，保持 Native 行为 | [目标服务架构](service-architecture.md)、[当前代码注册表与原生路由](configuration-and-routing.md) | M1 |
| M3 | 代码完成；行为验收待补 | 分离 RequestProfile、Route Planner 和 RoutePlan | [目标服务架构](service-architecture.md) | M2 |
| M4 | Native 消费路径完成；行为验收待补 | Native 执行消费完整计划 | [Provider 适配与数据流](provider-adapters-and-dataflow.md)、[客户端兼容](client-compatibility.md) | M3 |
| M5 | 未开始；目标架构必需 | 接入 route-local Protocol Bridge | [协议桥](protocol-bridge.md)、[Agent Loop Bridge](agent-loop-bridge.md)、[协议测试语料](protocol-test-corpus.md)、[Mock Testkit](protocol-testkit.md) | M4；所选 bridge slice 的 corpus/fixture 已稳定 |
| M6 | 未开始；目标行为必需 | 抽取 AttemptManager，统一 unsupported、fallback 与 availability | [路由与 Provider 韧性](../functional-requirements/provider-resilience.md)、[调用统计与可观测性](../functional-requirements/observability.md) | M4；涉及 bridge route 时另依赖 M5 |
| M7 | 未开始且按需选择 | 保留安全的模型信息内部投影视图 | [网关 API 与客户端兼容](../functional-requirements/gateway-api-compatibility.md) | M3；不依赖 M5/M6，也不实现 HTTP API |

OAuth、hosted tool、MCP、Anthropic Messages 和 Responses WebSocket 不属于 M0–M7。它们不能成为本次迁移的隐含前置条件，也不能借本次迁移顺带实现。

切片编号表示技术依赖，不表示发布时间或自动执行队列。M0–M4 必须顺序完成；M5/M6 是实现“隐藏上游协议差异并在安全候选耗尽后才返回不支持”的目标所必需，但仍须分别以具体可观察行为进入当前焦点；M6 的 Native 错误/可用性部分可在 M4 后实施，Bridge 相关部分必须等待 M5；M7 只建立内部投影边界，可在 M3 后独立选择。

## 3. 目标概念与所有权

### 3.1 代码拥有的目录

```text
ProviderDescriptor
  provider_kind
  endpoint profile upper bound
  credential kind upper bound
  native protocol/capability upper bound
  adapter dispatch

ConverterDescriptor
  converter_kind
  source_protocol
  target_protocol
  verified_features
  fidelity_upper_bound
  state_identity_support
```

配置不能新增 Provider 行为或 converter 实现，只能引用代码已经编译和测试的 descriptor。

### 3.2 服务注册表

```text
RealModelDefinition
  id
  family / revision
  tokenizer/context semantics
  intrinsic facts: Known | Unknown

UpstreamTargetDefinition
  id
  provider_kind
  real_model
  base_url
  credential_ref
  quota_scope
  fault_domain
  timeout
  enabled
  offerings: NativeOfferingDefinition[]

NativeOfferingDefinition
  id
  protocol
  upstream_model
  endpoint_profile
  transport profile
  served limits
  native capability evidence
  state policy / namespace

PublicModelDefinition
  name
  ordered serving_route ids

ServingRouteDefinition
  id
  upstream_target
  offering
  downstream_protocol
  mode: Native | Bridge { converter, conversion_policy }
```

route 优先级只由 `PublicModelDefinition.serving_routes` 的顺序表达；Serving Route 不重复保存
`public_model` 或 `priority`，避免双重所有权和排序冲突。

暂不保留独立 `ProviderDefinition` 配置实体：当前 `ProviderDefinition` 中的 `ProviderKind` 属于代码目录，credential binding 属于 Upstream Target。如果以后出现需要多个 target 共享并独立管理的账号、credential 或 quota 对象，再基于真实复用需求抽取，不在本次迁移中预设。

### 3.3 编译结果

启动 builder 为每条 route 产生路径级结果：

```text
ProviderDescriptor upper bound
∩ RealModel intrinsic facts
∩ UpstreamTarget shared boundary
∩ NativeOffering served evidence
∩ ConverterDescriptor upper bound (bridge only)
∩ route-local ConversionPolicy (bridge only)
= EffectiveExecutionProfile
```

Bridge route 同时生成 `ResolvedBridgePlan`。运行时不得再次从松散配置拼装 converter、fidelity 或 state policy。

## 4. 命名迁移

| 当前名称 | 目标名称 | 处理方式 |
|---|---|---|
| `ModelDefinition` | `RealModelDefinition` | 先收窄为模型内在事实；供应商/协议证据移入 Offering |
| `ProviderKind` / `ProviderDescriptor` | 保留 | 明确为代码拥有的 Provider Family/adapter catalog |
| `ProviderDefinition` | 移除或拆解 | `ProviderKind` 留在代码目录；credential binding 移入 Upstream Target |
| `DeploymentDefinition` | `UpstreamTargetDefinition + NativeOfferingDefinition[]` | 拆分共享调用边界与协议级供应 |
| `ResolvedDeployment` | `ResolvedUpstreamTarget` + `ResolvedNativeOffering` | snapshot 分开保存共享与协议级结果 |
| `AliasDefinition` | `PublicModelDefinition` | 不再直接引用 target，改为引用完整 routes |
| alias candidate | `ServingRouteDefinition` | 显式固定 target、offering、协议和 Native/Bridge 模式 |
| `PreparedNativeCandidate` | `ExecutionPlanCandidate` | 固定完整路径和有效能力 |
| `PreparedNativeRequest` | `RequestProfile + RoutePlan` | 分离请求语义分析与候选选择 |

迁移期文档提到旧类型时使用代码名并标注“当前”；目标文档不再把 `Deployment` 当作未来概念。

## 5. Upstream Target 与 Offering 拆分规则

多个 Offering 只有同时满足以下条件时才可共享一个 Upstream Target：

- 相同 endpoint origin/base 与安全策略；
- 相同 credential/account；
- 相同 quota bucket 或已明确共享的 quota scope；
- 相同故障、health 与 cooldown domain；
- 相同 Real Model 与 upstream identity/revision 语义；
- 兼容的状态所有权和 namespace；
- 无需独立启停或 target 级路由优先级。

协议、upstream model id、served context/output limits、feature evidence、transport 和协议状态政策可以在 Offering 之间不同。

若上述任一共享边界不成立，应配置多个 Upstream Target。不能为了减少对象数量把不同账号、quota、模型 revision 或故障域塞入同一 target。

## 6. 迁移原则

- **先保持行为，再扩展能力。** 类型迁移阶段仍只走 Native Path，不同时引入 bridge 行为。
- **单一编译入口。** 任一阶段只有一个生产 `RegistrySnapshot` builder，避免两套定义在热路径并存。
- **旧模型只收窄，不自动推断。** 从当前 capability 拆 Offering 时按协议复制已明确证据；`Unknown` 保持 fail closed。
- **无外部 schema 兼容负担。** 当前路由来自 Rust 代码且没有 `routes.toml`，因此不增加旧 schema loader；但下游 public model 名、HTTP/SSE 和错误行为在行为保持阶段不得漂移。
- **按完整路径验证。** 任一 capability 结论必须能追溯到同一 target/offering/route，不跨 route 求字段并集。
- **状态优先于 fallback。** `previous_response_id`、tool continuation 或 Provider resource 在没有 ledger 证明时固定 issuing target/offering。

## 7. 实施切片

### M0：冻结当前行为与术语边界

目标：在改类型前建立可靠的回归基线。

已经具备：

- 当前状态文档已经按 live source 记录 OpenAI 与 Meituan/LongCat 注册事实；
- 迁移前测试已经覆盖 `RegistryDefinition → RegistrySnapshot`、两协议 capability gate、model rewrite、候选顺序和 `previous_response_id` fallback boundary；测试源码现已迁移到新 API，但本次未执行。

进入 M1 前仍须：

- 在当时的 live checkout 重新运行默认验证并记录结果；
- 固定当时的 public model ids、target/offering/route ids 和 `/v1/models` 输出；
- 对审计发现但尚未有 characterization test 的行为先补失败测试；
- 禁止在此切片引入 converter、route reload 或新 Provider 行为。

退出条件：当前默认测试全部通过，状态文档与 live registry 一致，后续结构变化能由测试识别行为漂移。

### M1：引入目标定义类型，不改变运行时路径

目标：建立清晰的数据所有权。

工作：

- 增加 `RealModelDefinition`、`UpstreamTargetDefinition`、`NativeOfferingDefinition`、`PublicModelDefinition` 和 `ServingRouteDefinition`；
- 保留 `ProviderKind/ProviderDescriptor` 作为代码目录；
- 在 fixture 或 builder 单元测试中表达一个 target 同时具有 Chat 与 Responses Offering，以及两个 Provider 提供同一 Real Model 但 limits 不同的情况。

退出条件：Native 目标类型的引用、唯一性、协议方向、只收窄和 target/offering 拆分规则有失败测试；生产请求仍走旧 snapshot。Converter 类型推迟到真正实施 Bridge 的 M5，不提前建立空目录。

### M2：切换 Registry builder 与不可变 Snapshot

目标：让目标定义成为唯一生产注册表来源，同时保持 Native 行为。

本次已采用的旧 deployment 机械映射规则：

```text
one DeploymentDefinition
→ one UpstreamTargetDefinition
→ one NativeOfferingDefinition per enabled native protocol
→ one Native ServingRoute per public alias/protocol/target/offering
```

同一旧 deployment 中的 endpoint、credential、timeout 和 model reference 进入 target；`upstream_model`、对应协议 capability 和有效模型限制进入各 Offering。若旧 `CapabilitySet` 同时启用 Chat/Responses，则生成两个 Offering，不把两者能力合并。

工作：

- 构建 `ResolvedUpstreamTarget`、`ResolvedNativeOffering`、`ResolvedServingRoute`；
- 把 Provider credential binding 移到 target；
- 让 `/v1/models` 从 `PublicModelDefinition` 枚举，保持对外 id 不变；
- 删除生产路径对 `ResolvedDeployment` 和 `ResolvedAlias` 的读取；
- 完成切换后删除旧 definition/builder，不长期维护双模型。

退出条件：相同输入请求选择与迁移前等价的上游 endpoint、credential、upstream model 和 Native 协议；注册表越权与坏引用仍在监听前失败。

### M3：分离 RequestProfile、Route Planner 与 Execution Plan

目标：把当前 `PreparedNativeRequest` 拆成请求语义和路由结果。

工作：

- 统一提取一次 `RequestProfile`，包含下游协议、stream、完整 feature combination、limits 和 state-affinity indicators；
- route planner 只遍历 Public Model 的完整 Serving Routes；
- 按路径级 `EffectiveExecutionProfile` 过滤，生成不可变 `RoutePlan/ExecutionPlanCandidate`；
- 固定 target、offering、credential binding、candidate order、state affinity 和 fallback boundary；
- Ingress 只接收计划并交给执行器，不再自行做全局候选解析。

退出条件：不同 route 的独立 capability 不能被错误求并集；同一 target 的 Chat/Responses Offering 可以有不同限制；所有不支持请求在出站前稳定拒绝。

### M4：迁移 Native 执行与 attempt 编排

目标：让 Native Path 消费完整 Execution Plan。

工作：

- Provider adapter 从 selected Offering 读取 `upstream_model` 和协议证据；
- transport 从 selected Upstream Target 读取 endpoint、credential/fault boundary 与 timeout；
- 保持现有首输出前 attempt 语义，并让其遍历 `RoutePlan` 的完整候选；
- 保持首输出后禁止 retry/fallback、SSE bytes 透明和取消传播；
- 错误与日志使用 target/offering identity，但不向下游泄漏内部候选或 credential。

退出条件：现有 HTTP/SSE、tool、retry/fallback 和 cancellation corpus 全部保持；Ingress 不再从旧 deployment/alias 模型拼装调用细节。独立 AttemptManager 的抽取与统一预算归入 M6。

### M5：增加 route-local Protocol Bridge

目标：在 Native 计划稳定后才启用配置驱动的受限协议转换。

工作：

- 为已实现协议对注册 `ConverterDescriptor`；
- Bridge Serving Route 内联 `converter kind + ConversionPolicy`；
- builder 验证政策不超过 converter 的 feature/fidelity/state 上界并生成 `ResolvedBridgePlan`；
- 执行器按计划选择 Native 或 Bridge，converter 不重新路由；
- 为 exact、structure-preserving、显式 approximate 和 unsupported 建立 fixture。

退出条件：只有完整 bridge path 支持请求时才出站；未知或未允许损失在输出前拒绝；不建立顶层 Bridge Profile。

### M6：完善不支持错误、fallback 与运行时可用性

目标：实现“主要路径隐藏差异，安全路径耗尽后才返回不支持”的产品行为。

工作：

- 增加稳定的 `unsupported_protocol_or_capability` 分类；
- 从 Ingress 抽取 AttemptManager，区分 same-route retry 与 cross-route fallback，并统一总预算；
- 上游首输出前明确不支持时，只 fallback 到仍满足同一 RequestProfile 的 route；
- 将 cooldown/availability 作为 target 或明确 quota scope 的运行时 overlay，不写回 capability snapshot；
- 所有安全候选耗尽后，按下游协议返回归一化错误。

退出条件：本地不支持、上游明确不支持、临时失败、有状态禁止切换和已输出失败的行为矩阵有确定性测试。

### M7：仅保留模型信息扩展点

目标：为未来查询真实配置能力留边界，不提前实现 API。

工作：

- 确保 compiled snapshot 能形成 Public Model → complete routes → target/offering/fidelity/limits/evidence 的只读视图；
- 保持标准 `/v1/models` 简单；
- 不确定 endpoint、schema、授权模型或动态探测协议；
- 明确查询结果不得暴露 endpoint、credential locator、header、secret 或账号信息。

退出条件：仅形成内部可安全投影的数据边界；除非另有独立需求和失败测试，不增加新 HTTP endpoint。

## 8. Builder 必须验证的目标不变量

1. Real Model、Upstream Target、Offering、Public Model 和 Serving Route ID 唯一且引用完整；
2. 每个 target 至少有一个 Offering，且同一 target 内 Offering ID/协议组合无歧义；
3. Native route 的下游协议等于 Offering 协议；
4. Bridge route 的 converter source 等于下游协议，target 等于 Offering 协议；
5. Offering capability 不超过 ProviderDescriptor 上界，也不扩大已知 Real Model 上限；
6. ConversionPolicy 不超过 ConverterDescriptor 已验证的 feature/fidelity/state 上界；
7. `Unknown` fail closed，不因同名 Provider、Model 或另一条 route 的证据自动提升；
8. Public Model 只有在至少一条完整 route 同时满足请求全部语义时才声明支持；
9. credential、endpoint、state namespace 与 quota/fault scope 在 RoutePlan 中无歧义；
10. 配置不能注入任意 URL、header、模板、脚本或 converter 实现。

## 9. 兼容、清理与回滚边界

### 9.1 必须保持

- 下游 endpoint、public model id 和 OpenAI-compatible HTTP error envelope；
- Native JSON/SSE 的最小改写与未知合法字段保留；
- credential 不进入 snapshot 明文、日志或响应；
- 首输出后不 retry/fallback；
- `previous_response_id` 默认不跨 issuing target/offering；
- probe 与测试 transport 的可注入性。

### 9.2 明确不兼容但仅限内部

- Rust 类型和内部 ID 字段从 deployment/alias 迁移到 target/offering/route；
- Provider credential binding 从当前 `ProviderDefinition` 下沉到 target；
- pipeline 返回类型由 `PreparedNativeRequest` 改为 `RequestProfile + RoutePlan`；
- registry builder error 名称随实体重命名。

当前没有公开 route 配置 schema，因此不增加 legacy loader、双写 snapshot 或版本迁移文件。若迁移过程中出现行为回归，回滚整个未完成切片；不要在生产路径同时保留两套 registry 选择逻辑。

### 9.3 清理条件

只有在所有生产读取方和测试 fixture 都切到新类型后，才删除：

- `DeploymentDefinition` / `ResolvedDeployment`；
- `AliasDefinition` / `ResolvedAlias`；
- 旧 deployment/alias builder errors；
- Ingress 中按 deployment candidate 直接循环的逻辑；
- 按整个 `CapabilitySet` 同时表达 Chat/Responses Offering 的旧注册方式。

## 10. 验证矩阵

| 范围 | 必须证明 |
|---|---|
| Builder unit tests | 引用、唯一性、只收窄、协议方向、converter policy 和状态边界 |
| Registry snapshot tests | 一个 target 双 Offering、同模型多 target 不同 limits、完整 route 顺序 |
| Pipeline tests | RequestProfile 单次解析、路径交集、Unknown fail closed、无字段级并集 |
| Native contract tests | HTTP/JSON/SSE、model rewrite、headers、tool、terminal 与错误保持 |
| Bridge corpus | 双向 text/tool/usage/terminal、identity、ordering、fidelity 与拒绝 |
| Recovery tests | same-route retry、cross-route fallback、unsupported、cooldown、state affinity |
| Security tests | URL/header/credential/converter 注入失败，私有信息不进入响应 |
| SDK/CLI tests | OpenAI SDK 与 Codex 的既有 Native 行为不回归 |

文档、单元/fixture、SDK/CLI 和真实 Provider 证据必须分别记录。Mock 或单次真实调用不能证明所有 target/offering 或 bridge feature。

## 11. 本次迁移不做

- 动态 Provider/plugin DSL；
- 任意模板或脚本转换；
- 顶层 Bridge Profile 注册表；
- route TOML、热重载或旧 schema 兼容；
- 自动把 probe 结果写入静态 registry；
- 多租户、账号池、credential rotation 或分布式 cooldown；
- 模型信息扩展接口的 endpoint/schema 实现。

## 关联文档

- [当前代码架构](../implementation-status/current-architecture.md)
- [当前实现说明](../implementation-status/current-implementation.md)
- [目标服务架构](service-architecture.md)
- [当前代码注册表与原生路由](configuration-and-routing.md)
- [Provider adapter 与数据流](provider-adapters-and-dataflow.md)
- [Chat/Responses bridge](protocol-bridge.md)
- [网关 API 与客户端兼容需求](../functional-requirements/gateway-api-compatibility.md)
- [路由与 Provider 韧性需求](../functional-requirements/provider-resilience.md)
