# 请求、Public Model 与安全边界

## 状态

本文是[网关 API 域](README.md)的请求处理模块：定义 Public Model 与 routes 的关系、请求输入保护和安全拒绝边界。
其他模块见[网关 API 域](README.md)导航。

## 1. Public Model 与 routes

- 下游只能提供已配置的 Public Model；它表示 OpenBridge 对下游提供的稳定服务契约，而不是某个上游模型名的透明别名。身份、生命周期、固定能力计算、Models
  API 和错误语义统一由[Public Model 与模型能力契约](../model-capability/README.md)定义。
- 请求能力只在所选 Public Model 边界预检一次，不参与选模、Route 候选资格、顺序或 fallback。预检通过后，Route 仍按配置顺序固定
  Upstream Target、Upstream API、下游 operation 和执行模式；generation `Native` 要求协议相同，`Bridged` 要求协议相反且通过完整
  `BridgePlan` preflight，Embeddings 只允许同 operation Native。
- 每个 generation Public Model 必须显式声明一种类型化 Route ordering strategy。`NativeFirst` 对每个 downstream protocol 先按
  source 顺序排列全部 Native，再排列 Bridge；`SourceFirst` 对每个 downstream protocol 先保持 source 顺序，再在同一 source 内将
  Native 排在 Bridge 前。自动 Bridge 只补全整个 Public Model 缺失的 Native protocol coverage；显式 Bridge surface 可以在其他
  source 已有 Native coverage 时保留。两种策略都在启动期冻结，运行时不得因请求能力、价格、健康或 Provider 名称重新打分或重排。
- 服务对上游只使用选中 route 的真实模型名、协议、endpoint 与 credential；下游不能通过 body、query 或 header 指定上游
  URL、模型、credential、provider family、route、转换脚本或 header 转换规则。Provider 的受信代码 hook 可以按编译期规则增添、替换、转换或删除普通
  header，但认证、cookie、Host 与 proxy header 始终隔离。
- 请求开始后，Public Model、RoutePlan、credential pool binding 与注册表版本保持固定；无状态 attempt 可按策略选择 pool
  member。

## 2. 输入保护

- 仅接受端点契约允许的 content type、JSON body 和受配置约束的大小；无法安全解析的请求在 egress 前返回稳定错误。
- 请求分类必须先识别 operation，再按 operation 解析 `stream`、input form、function/custom/hosted
  tool、并行工具、结构化输出、multimodal、reasoning、`previous_response_id`、background/store 与相应限制等会影响固定契约或状态边界的特征。
- Chat/Responses 下游请求的顶层字段必须先按源协议的代码内类型化目录分类；未知字段即使值为 `null` 也必须在 egress 前以稳定
  `unknown_parameter` 拒绝，不能因"目标 Provider 也许支持"而进入 Native 或 Bridge。已知字段的 `null`/`false` 是否表示未请求能力，
  只由该字段的类型化语义决定，不形成通用绕过规则。
- 服务为每个请求生成或传播安全的 request id，用于响应和受控诊断；该 id 不是 client identity、tool identity 或聚合指标
  label。

## 关联文档

- [网关 API 域导航](README.md)
- [Public Model 与模型能力契约](../model-capability/README.md)
- [Native Path 与流式语义](native-path-and-streaming.md)
- [路由与 Provider 韧性](../routing-resilience/README.md)
- [实施现状](../../implementation-status/README.md)
