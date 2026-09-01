# 网关 API 合同

本文集中定义 HTTP/MCP 入口、认证、Generation 请求与响应、streaming、tool、state、错误和验收约束。

受信用户把本地 Agent 或 OpenAI-compatible SDK 指向稳定 base URL，使用私有用户表中的 Bearer API key 和
Public Model 调用服务；客户端不需要、也不能选择上游 Provider、真实模型、URL、credential 或候选切换。

当前合同覆盖 OpenAI-compatible Chat Completions、Responses、Embeddings 与 Images HTTP JSON/SSE，同协议 Native
媒体、按任务分离的 Chat Native audio、显式共同语义内的 Chat/Responses Bridge，以及 MCP stateless/legacy lifecycle。
“请求可转发”不等于“某个 Agent 完整兼容”；客户端 profile、transport 和 tool loop 只有在本文明确声明并有对应证据时才构成承诺。

## 叶子文档与唯一职责

| 叶子 | 只回答什么 |
|---|---|
| [Endpoint 与认证](gateway-api/endpoint-and-auth.md) | Endpoint 总览、认证边界 |
| [请求与安全边界](gateway-api/request-boundaries.md) | Public Model 与 Route 固定、输入保护 |
| [Native Path 与流式语义](gateway-api/native-and-streaming.md) | Native 基线、流式语义、遥测计时边界 |
| [参数兼容](gateway-api/parameter-compat.md) | 普通参数上游兼容、Responses include、prompt-cache 与 parallel-tool 控制 |
| [Generation envelope 与状态](gateway-api/generation-state.md) | 统一 instructions、无状态默认、state affinity 不变量 |
| [Function tool 与扩展](gateway-api/tools.md) | Function tools、私有扩展、MCP 隔离 |
| [错误与客户端结果](gateway-api/errors.md) | 错误时机与必需行为、字段定位与首错 |
| [MCP 本地服务](gateway-api/mcp.md) | Dual-era transport、stateless metadata、hello tool |
| [验收与非目标](gateway-api/acceptance.md) | 功能验收要求（API-01..19）与非目标 |
