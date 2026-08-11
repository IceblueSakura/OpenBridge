# 功能验收要求与非目标

## 状态

本文是[模型与能力契约域](README.md)的验收与非目标模块。验收项是功能需求文档的行为约束；"必须""不得""只允许"
是验收约束，不代表当前实现已经满足。代码、测试、probe 或真实运行已经证明的内容只写入
`implementation-status/`。

## 1. 功能验收要求

| ID       | 应被保护的用户可观察行为                                                                                                                                               |
|----------|------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| MODEL-01 | 标准 list/retrieve 只返回四字段对象，且详情与列表元素相同。                                                                                                            |
| MODEL-02 | 扩展 list/retrieve 返回同一个固定能力对象；参数只由目标接口公开，且不包含部署、凭据、价格或运行状态。                                                                  |
| MODEL-03 | active/deprecated 模型可见；retired 或无可执行接口的模型不可见、不可调用。                                                                                             |
| MODEL-04 | 较弱首选 Route 与较强后续 Route 的交集仍拒绝能力请求，且不发生 egress。                                                                                                |
| MODEL-05 | 能力预检通过后保留全部配置 Route 的原顺序，不按请求能力跳过或重排。                                                                                                    |
| MODEL-06 | unknown 能力 fail closed；token 上限与集合按保守交集计算。                                                                                                             |
| MODEL-07 | Chat、Responses 与 Embeddings 能力相互隔离，不能用一个接口的能力扩大另一个接口。                                                                                       |
| MODEL-08 | 未知模型和 retired 模型统一返回安全 `model_not_found`；能力不足返回 `unsupported_model_capability`。                                                                   |
| MODEL-09 | registry 在启动时拒绝非法身份、生命周期、上下文、模态、引用和能力扩大。                                                                                                |
| MODEL-10 | Embeddings dimension domain、Chat/Responses source-aware 输入与 mode-aware 音频输出由 Models projection 和 preflight 共享，不能由 bool、Native passthrough 或请求期 Route 过滤扩大。 |
| MODEL-11 | `capabilities.tasks` 只由唯一 canonical task 按闭合映射产生；不同 task 的 Route 不能编译进同一 Public Model。                                                    |
| MODEL-12 | Provider 完整 audio ceiling、单个 executable profile 与 canonical task 在启动期逐层校验；VoiceClone conditioning 不进入 content-understanding input。             |
| MODEL-13 | Structured Output 的 Provider/Target profile、Public 交集、Models 投影与请求预检共享一个闭合联合；无共同 mode 时不公开幽灵支持或参数。             |
| MODEL-14 | generation reasoning `levels`、`accepted_levels` 与 `input_policy` 共享同一固定接口；正向归一化在 candidate 展开前执行一次，`none` 保持独立，标准 Models 投影不变。 |
| MODEL-15 | Responses `response_includes` 按具体 wire 值保守相交并直接供 preflight 使用；接受值不保证输出 item，Bridge 只能显式安全消费；`prompt_cache_key` 只作为全部固定候选可原样转发的请求参数公开，不产生独立缓存效果字段。 |
| MODEL-16 | 扩展 list 的 `native_protocol` 只命中含对应 Native candidate 的 Public Model；Bridge-only interface 被排除，省略参数保持完整列表，非法、重复或未知 query 显式失败且响应不泄漏拓扑。 |

确定性 Rust/HTTP 测试只证明本地 registry、序列化、预检和 Route 顺序；不证明真实 Provider 当前能力、外部 SDK、负载、长期运行或
LiteLLM/OpenRouter 目录新鲜度。

## 2. 非目标

- 根据能力、质量、成本或 benchmark 自动选模；
- 按请求能力筛选、打分、加权或重排 Route；
- 在 Models API 中暴露 deployment、endpoint、credential、健康、价格、配额、指标或 benchmark；运行指标只通过独立 OTLP metrics
  signal 导出，不属于模型目录或模型能力契约；
- 从 LiteLLM、OpenRouter、Provider `/models` 或 probe 动态发现和注册模型；
- 模型推荐、自动迁移、alias resolution、ACL、分页搜索，或除 `native_protocol` 外的通用 capability query API；
- 在未实现协议语义前，仅因模型本体声称支持就放行 hosted/custom tool、audio/file、state、embedding 参数或 opaque reasoning。

## 关联文档

- [模型与能力契约域导航](README.md)
- [产品范围](../product-scope/product-scope.md)
- [网关 API 与客户端兼容](../gateway-api/README.md)
- [扩展能力导航及共同规则](../extended-capabilities/embedding-and-native-multimodal.md)
- [路由与 Provider 韧性](../routing-resilience/provider-resilience.md)
- [当前实现总览](../../implementation-status/current-implementation.md)
