# 模型与能力契约域

Public Model 与模型能力契约需求按功能模块拆分如下。子文档只定义目标行为、失败语义与安全边界；不记录
"代码已经做到什么"或某次测试结果。

| 功能模块                              | 文档                                                    |
|---------------------------------------|---------------------------------------------------------|
| 事实所有权与公开边界                  | [fact-ownership-and-boundary.md](fact-ownership-and-boundary.md) |
| Public Model 身份、生命周期与可见性   | [identity-and-lifecycle.md](identity-and-lifecycle.md)  |
| 模型事实与固定接口契约                | [model-facts-and-interface-contract.md](model-facts-and-interface-contract.md) |
| 请求预检与禁止能力路由                | [request-preflight-and-routing.md](request-preflight-and-routing.md) |
| Models API 契约                       | [models-api.md](models-api.md)                          |
| 启动时校验                            | [startup-validation.md](startup-validation.md)          |
| 功能验收要求与非目标                  | [acceptance-and-non-goals.md](acceptance-and-non-goals.md) |

## 状态

**当前目标。** 本域集中定义 Public Model 的公共身份、模型信息、固定接口能力、请求预检和 Models API
边界。它是"模型支持什么"的唯一功能需求入口；Route 执行、retry、fallback 与 cooldown 见
[路由与 Provider 韧性](../routing-resilience/provider-resilience.md)，当前实现事实见
[当前实现总览](../../implementation-status/current-implementation.md)链接的功能专题。

## 域目标（用户结果）

客户端只需选择一个稳定 Public Model 和 Chat Completions、Responses 或 Embeddings 接口，即可在发起模型请求前读取同一份
静态能力契约。若所选模型不支持请求能力，OpenBridge 必须在任何上游调用前返回稳定错误；不得自动改选模型或寻找能力更强的 Route。
只有[普通参数上游兼容规则](../gateway-api/tools-continuation-and-extensions.md)中的闭合、已验证字段可以在选中 Upstream API 的
egress 边界静默删除，其他请求字段不得被隐式降级。

模型信息用于能力展示和正确拒绝，不承担模型推荐、质量排序、成本优化或运行时调度。

## 关联文档

- [产品范围](../product-scope/product-scope.md)
- [网关 API 与客户端兼容](../gateway-api/README.md)
- [扩展能力导航及共同规则](../extended-capabilities/embedding-and-native-multimodal.md)
- [Embeddings 能力](../extended-capabilities/embeddings.md)
- [Native 图片能力](../extended-capabilities/native-image.md)
- [Native 文件能力](../extended-capabilities/native-file.md)
- [Native 音频能力](../extended-capabilities/native-audio.md)
- [待定 Model 目录与 Provider 接入配置](../pending/model-catalog-configuration.md)
- [配置、凭证与受信边界](../configuration-credentials/README.md)
- [路由与 Provider 韧性](../routing-resilience/provider-resilience.md)
- [LiteLLM/OpenRouter 模型信息综合调研](../../references/cross-project/model-information-comparison.md)
- [当前实现总览](../../implementation-status/current-implementation.md)
