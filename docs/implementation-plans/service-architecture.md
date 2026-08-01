# OpenBridge 服务架构与扩展边界

## 状态

本文以当前源码为基线，说明稳定分层以及尚未实现能力的接入约束。当前运行时只有
OpenAI-compatible Chat/Responses Native Path；Protocol Bridge、独立 AttemptManager、动态
availability/cooldown 和私有模型信息接口均未实现。当前源码事实以[当前代码架构](../implementation-status/current-architecture.md)为准。

## 1. 当前服务形态

OpenBridge 是单进程、单用户、单配置所有者的 headless 服务，不拆分独立控制面和数据面，也不提供
GUI、Web 控制台、tenant、下游配额或合规审计系统。

```text
OpenAI-compatible client
          ↓
HTTP ingress (JSON/SSE)
          ↓
RequestRequirements
          ↓
Public Model → ordered Routes → RoutePlan
          ↓
selected Upstream Target + Upstream API
          ↓
Provider adapter
          ↓
shared HTTP/SSE transport
          ↓
Upstream Provider
```

当前分层：

| 层次 | 当前职责 | 不承担 |
|---|---|---|
| 接入层 | 下游认证、body/content-type 限制、HTTP/JSON/SSE 错误表达 | Provider 注册和模型能力推断 |
| 请求分析层 | 一次解析协议、Public Model、feature、limits 和 state indicator | 选择 Provider 或改写 wire |
| 路由计划层 | 对有序完整 Route 做协议、能力和限制门控 | 网络 I/O 或协议转换 |
| Provider 适配层 | path、模型字段、认证 header、响应终态和错误分类 | 决定 Public Model 和候选顺序 |
| Transport 层 | endpoint 合成、连接池、timeout、取消和流式 body | 理解模型能力或业务协议语义 |
| 注册表层 | 构建并提供不可变 `RuntimeRegistry` | 网络探测或动态健康写回 |

Ingress 当前仍持有有限的首输出前 retry/fallback 循环；尚无独立执行管理器。

## 2. 当前数据模型

### 2.1 Provider Descriptor

`ProviderKind/ProviderContract` 是编译期闭合目录，声明 adapter dispatch、endpoint profile、credential
kind 和 native capability 上界。运行时注册表不能增加 Provider 行为。

### 2.2 Model

`ModelConfig` 保存与具体上游调用入口无关的模型事实：稳定 ID、展示信息、context、参数和
reasoning 元数据。同名但 revision 或语义不同的模型应使用不同 ID。

### 2.3 Upstream Target

`UpstreamTargetConfig` 表示共享调用边界：

```text
provider kind
model
endpoint base
credential binding
quota/fault scope
request timeout
enabled
native upstream APIs
```

只有 endpoint、credential/account、Model identity、quota/fault 和启停边界相同时，多个 Upstream API
才应共享一个 target。

### 2.4 Upstream API

`UpstreamApiConfig` 表示一个 target 的单协议供应，独立拥有：

```text
protocol
upstream model
endpoint profile
transport profile
model rules
protocol capability evidence
state affinity
```

同一 target 可以同时拥有 Chat 与 Responses Upstream API；二者的 upstream model、上下文/输出限制、能力
和状态绑定方式可以不同。

### 2.5 Public Model 与 Route

`PublicModelConfig` 保存下游稳定模型名和有序 Route ID。`RouteConfig` 固定
Upstream Target、Upstream API、下游协议和当前 `Native` mode。route 优先级只由 Public Model 中的顺序
表达。

### 2.6 RequestRequirements 与 RoutePlan

`analyze_request` 生成与注册表无关的 `RequestRequirements`；`plan_request` 再对每条完整 route 独立门控，
生成有序 `RouteCandidate`：

```text
route id
upstream target id
upstream API id
validated request
```

不同 route 的能力不能按字段求并集。`previous_response_id` 当前关闭跨 target fallback。

## 3. 当前执行与安全边界

- adapter 只生成相对 URI，transport 与受信 target endpoint 合成最终 URL；
- endpoint 必须是经过 builder 验证的 HTTPS base，redirect 被禁用；
- credential locator 存于 `RuntimeRegistry`，secret 只在请求准备阶段解析；
- Native adapter 只做 Provider 所需的最小改写，未知合法字段保持原样；
- streaming bytes 不重渲染，SSE decoder 只观察 framing、大小和 terminal；
- retry/fallback 只发生在首个下游 body 输出之前；
- 下游取消会 drop 上游 stream；
- 上游内部 target、Upstream API、endpoint 和 credential 不通过标准 API 暴露。

## 4. 尚未实现的扩展

### 4.1 Protocol Bridge

未来若实现 Chat ↔ Responses 转换，只能通过已编译 converter 和 route-local policy 接入：

```text
downstream wire
→ protocol parser
→ explicit bridge representation
→ target protocol renderer
→ selected Upstream API
```

配置只能选择和收窄已实现 converter，不能注入模板或脚本。转换必须区分 exact、结构保持、显式允许
的 approximate 和 unsupported；identity、ordering、terminal 或 continuation 无法安全保持时应在输出前
拒绝。当前 `RouteMode` 只有 `Native`，没有 converter 或 `BridgePlan` 类型。

### 4.2 AttemptManager 与 availability

未来可从 Ingress 抽取执行管理器，区分同 route retry 与跨 route fallback，并统一次数、等待和总耗时
预算。动态 cooldown/availability 应是 target 或明确 quota/fault scope 的运行时 overlay，不能写回静态
capability view。

上游已产生任何可观察业务输出、有 Provider-bound continuation、tool identity 或潜在副作用时，不得
透明切换 target。

### 4.3 模型信息扩展接口

未来可从同一不可变 `RuntimeRegistry` 投影 Public Model 的完整 route、Upstream API 协议、配置限制、能力证据和
转换 fidelity。当前没有该内部视图或 HTTP endpoint。任何实现都不得暴露 endpoint、credential locator、
认证 header、secret 或账号信息；标准 `/v1/models` 继续只返回 Public Model 名称。

### 4.4 其他非当前能力

Responses WebSocket、Anthropic Messages、OAuth、hosted tool facade、MCP Tool Bridge、跨进程 cooldown、
动态 Provider/plugin DSL 和 route 热重载均不属于当前实现。

## 5. 验证边界

| 层次 | 适用范围 |
|---|---|
| Unit/compile-time checks | registry 引用、只收窄、协议方向、请求分类和错误分类 |
| Contract fixture | JSON/SSE/tool/error、model rewrite、retry/fallback 与 cancellation |
| OpenAI SDK | Chat/Responses 客户端可见 HTTP/SSE 行为 |
| Codex CLI | custom Provider 的 Responses transport、错误和 tool loop |
| Bridge property | 仅未来 Bridge：identity、ordering、terminal、state 和 fidelity |
| Security/resource | URL/header/credential 边界、cancel、slow consumer 和有界 buffer |

验证记录必须注明实际代码版本、SDK/CLI 版本和环境。编译成功不等同于测试、真实 Provider 或客户端验收。

## 关联文档

- [当前代码架构](../implementation-status/current-architecture.md)
- [当前实现说明](../implementation-status/current-implementation.md)
- [代码注册表与路由](configuration-and-routing.md)
- [Provider adapter 与数据流](provider-adapters-and-dataflow.md)
- [Chat/Responses Bridge](protocol-bridge.md)
- [客户端兼容](client-compatibility.md)
- [Provider 韧性需求](../functional-requirements/provider-resilience.md)
