# 网关 API 域

网关 API 与客户端兼容需求按功能模块拆分如下。子文档只定义目标行为、失败语义与安全边界；不记录
"代码已经做到什么"或某次测试结果。

| 功能模块                                    | 文档                                                    |
|---------------------------------------------|---------------------------------------------------------|
| 接口与认证（含 MCP 本地工具入口）           | [interfaces-and-auth.md](interfaces-and-auth.md)        |
| 请求、Public Model 与安全边界               | [request-and-security-boundary.md](request-and-security-boundary.md) |
| Native Path 与流式语义                      | [native-path-and-streaming.md](native-path-and-streaming.md) |
| tools、continuation 与扩展                  | [tools-continuation-and-extensions.md](tools-continuation-and-extensions.md) |
| 错误与客户端可见结果                        | [errors-and-client-results.md](errors-and-client-results.md) |
| 运行期观测与 OpenTelemetry 导出             | [observability-and-otel.md](observability-and-otel.md)  |
| 功能验收要求与非目标                        | [acceptance-and-non-goals.md](acceptance-and-non-goals.md) |

## 状态

**当前目标。** 网关 API 域定义 OpenBridge 对下游客户端可见的 API、认证、原生 HTTP/SSE 语义和兼容边界；
不规定内部模块、converter 形态或实现顺序。当前已经由代码和测试证明的范围以
[当前实现总览](../../implementation-status/current-implementation.md)链接的功能专题为准。

## 域目标（用户结果）

受信用户应能把本地 Agent 或 OpenAI-compatible SDK 指向一个稳定的 OpenAI-compatible base URL，使用私有用户表中分配的
Bearer API Key 与 Public Model 调用服务。主要调用路径不得要求客户端知道上游 Provider、真实模型、URL、凭证或候选切换细节。

初期的兼容目标按优先级为：

1. OpenAI SDK 的 Chat Completions 与 Responses HTTP JSON/SSE；
2. OpenAI-compatible Embeddings、Chat/Responses 同协议 Native 多模态输入，以及按任务分离的 Chat Native 音频理解、ASR/TTS；
3. 独立 Python 脚本或 curl 的最小 HTTP/header/SSE 复现；
4. 只有在明确声明时，才验证 Codex、Hermes 等具体客户端的 profile、transport 与 tool-loop 行为。

"某个请求能被转发"不等于"某个 Agent 已完整兼容"。每项声明必须限定 endpoint、stream、tool、continuation、Provider 与实际验证版本。

## 关联文档

- [产品范围](../product-scope/product-scope.md)
- [Public Model 与模型能力契约](../model-capability/README.md)
- [配置与凭证](../configuration-credentials/README.md)
- [路由与 Provider 韧性](../routing-resilience/provider-resilience.md)
- [交付与证据要求](../delivery-evidence/delivery-and-evidence.md)
- [MCP 2026-07-28 外部协议与 Rust 生态调研](../../references/mcp/README.md)
- [当前实现总览](../../implementation-status/current-implementation.md)
