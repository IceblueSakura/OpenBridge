# 错误与客户端可见结果

## 状态

本文是[网关 API 域](README.md)的错误模块：定义请求各阶段的失败语义和客户端可见的错误结果。
其他模块见[网关 API 域](README.md)导航。

## 1. 错误时机与必需行为

| 时机                                        | 必需行为                                                                                                                                              |
|---------------------------------------------|-------------------------------------------------------------------------------------------------------------------------------------------------------|
| ingress、Public Model、能力、认证或配置拒绝 | 上游调用前返回安全、稳定的 OpenAI-compatible JSON error；不暴露 URL、credential、候选列表或内部栈。                                                   |
| Chat/Responses 请求含未知顶层字段            | 上游调用前返回稳定 `unknown_parameter`，`param` 指向安全字段名；Native 与 Bridge 行为一致，且不得调用 Provider。                                       |
| 所选 Public Model 的固定接口契约不支持请求  | 在 egress 前返回稳定 `unsupported_model_capability`；Generation 的 `param` 必须定位到标准顶层字段。不得改选模型或筛选 Route。只有普通参数兼容规则明确定义的参数可以在选定 API 的 egress 静默删除字段。 |
| 上游在首输出前返回可重试失败                | 该请求已经通过统一能力预检；是否 retry/fallback 只按静态 Route 顺序、错误分类和状态亲和执行，不重新比较候选能力。                                     |
| 首个业务输出前的上游失败                    | 依[路由与 Provider 韧性](../routing-resilience/README.md)判断有限 retry/fallback，最终保留安全的 status、error code、request id 与 allowlist rate-limit 信息。 |
| 已开始 JSON/SSE body 后的失败               | 只使用目标协议已有的 terminal/error 或关闭语义；不重写已发内容、不注入私有 event、不切换 candidate。                                                  |
| 下游取消                                    | 停止当前请求及可取消的 retry/backoff；终态单列为 client cancellation，而非上游成功或错误。                                                            |

所有错误类别必须稳定、低基数且可用于调用统计；原始上游错误正文只能在受保护诊断中按脱敏规则处理，不能成为对外契约。

## 2. Generation 字段定位与首错

Chat/Responses 的合法字段超出固定 Public Model interface 时，HTTP status、`error.type`、`error.code` 和固定 message 保持不变，
`error.param` 使用下列标准顶层 owner；内部 capability reason 不进入下游响应：

| 失败事实 | `param` |
|---|---|
| streaming 或 non-streaming delivery | `stream` |
| function tool / strict schema | `tools` |
| tool choice | `tool_choice` |
| parallel function calls | `parallel_tool_calls` |
| Chat / Responses structured output | `response_format` / `text` |
| continuation、background、Responses projection | `previous_response_id`、`background`、`include` |
| Chat / Responses multimodal input | `messages` / `input`；独立音频控制使用 `audio` 或 `asr_options` |
| output limit | 实际触发限制的 `max_output_tokens`、`max_completion_tokens` 或 `max_tokens` |
| Chat / Responses reasoning | `reasoning_effort` / `reasoning` |
| ordinary interface parameter | 该字段本身 |

已知字段形状非法继续返回 `invalid_request_error`；未知字段继续返回 `unknown_parameter`，不能误报为 capability。一个响应只返回一个错误，
顺序固定为：JSON envelope/model、unknown field、shape/combination、Public Model/interface、stream、tools/tool choice/parallel/strict、
structured output、state、`include`、multimodal、output limit、reasoning、ordinary parameter。JSON key、集合与 candidate 顺序不得改变首错；
所有本地拒绝必须保持 zero egress，且不得回显字段值或执行拓扑。

## 关联文档

- [网关 API 域导航](README.md)
- [请求、Public Model 与安全边界](request-and-security-boundary.md)
- [Native Path 与流式语义](native-path-and-streaming.md)
- [路由与 Provider 韧性](../routing-resilience/README.md)
- [实施现状](../../implementation-status/README.md)
