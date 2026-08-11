# 错误与客户端可见结果

## 状态

本文是[网关 API 域](README.md)的错误模块：定义请求各阶段的失败语义和客户端可见的错误结果。
其他模块见[网关 API 域](README.md)导航。

## 1. 错误时机与必需行为

| 时机                                        | 必需行为                                                                                                                                              |
|---------------------------------------------|-------------------------------------------------------------------------------------------------------------------------------------------------------|
| ingress、Public Model、能力、认证或配置拒绝 | 上游调用前返回安全、稳定的 OpenAI-compatible JSON error；不暴露 URL、credential、候选列表或内部栈。                                                   |
| Chat/Responses 请求含未知顶层字段            | 上游调用前返回稳定 `unknown_parameter`，`param` 指向安全字段名；Native 与 Bridge 行为一致，且不得调用 Provider。                                       |
| 所选 Public Model 的固定接口契约不支持请求  | 在 egress 前返回稳定 `unsupported_model_capability`；不得改选模型或筛选 Route。只有普通参数兼容规则明确定义的参数可以在选定 API 的 egress 静默删除字段。 |
| 上游在首输出前返回可重试失败                | 该请求已经通过统一能力预检；是否 retry/fallback 只按静态 Route 顺序、错误分类和状态亲和执行，不重新比较候选能力。                                     |
| 首个业务输出前的上游失败                    | 依[路由与 Provider 韧性](../routing-resilience/provider-resilience.md)判断有限 retry/fallback，最终保留安全的 status、error code、request id 与 allowlist rate-limit 信息。 |
| 已开始 JSON/SSE body 后的失败               | 只使用目标协议已有的 terminal/error 或关闭语义；不重写已发内容、不注入私有 event、不切换 candidate。                                                  |
| 下游取消                                    | 停止当前请求及可取消的 retry/backoff；终态单列为 client cancellation，而非上游成功或错误。                                                            |

所有错误类别必须稳定、低基数且可用于调用统计；原始上游错误正文只能在受保护诊断中按脱敏规则处理，不能成为对外契约。

## 关联文档

- [网关 API 域导航](README.md)
- [请求、Public Model 与安全边界](request-and-security-boundary.md)
- [Native Path 与流式语义](native-path-and-streaming.md)
- [路由与 Provider 韧性](../routing-resilience/provider-resilience.md)
- [当前实现总览](../../implementation-status/current-implementation.md)
