# 文档索引

本目录按文档用途组织；根目录 [`README.md`](../README.md) 是项目入口和推荐阅读顺序。

| 目录 | 内容 | 主要文档 |
|---|---|---|
| [`requirements/`](requirements/) | 产品范围、外部契约、验收方向与调研缺口 | [初版需求](requirements/proxy-requirements.md)、[Hosted tool MCP 暴露](requirements/hosted-tools-mcp.md) |
| [`architecture/`](architecture/) | 目标架构、实现边界与长期技术决策 | [架构与路线](architecture/architecture-and-roadmap.md)、[控制面](architecture/control-plane-models-keys-and-observability.md)、[Rust adapter 与数据流](architecture/rust-provider-adapter-dataflow.md) |
| [`design/`](design/) | 跨协议转换与 OAuth 凭证等专项设计 | [Chat/Responses 转换](design/chat-responses-conversion.md)、[Codex OAuth 凭证边界](design/codex-oauth-credential-boundary.md) |
| [`implementation/`](implementation/) | 当前代码、API、配置、路由、SSE 语义与验证证据 | [当前实现说明](implementation/current-implementation.md) |
| [`plans/`](plans/) | 已确认、待实施的分阶段工作计划 | [开发计划](plans/development-plan.md) |
| [`research/hermes/`](research/hermes/) | Hermes Agent 源码调研 | [Chat/Responses 分析](research/hermes/chat-responses-analysis.md) |
| [`research/litellm/`](research/litellm/) | LiteLLM 源码调研、调用链与性能观察 | [协议分析](research/litellm/chat-responses-analysis.md)、[调用链](research/litellm/proxy-call-chain-analysis.md)、[性能分析](research/litellm/proxy-performance-bottlenecks.md) |
| [`research/cc-switch/`](research/cc-switch/) | cc-switch 的 Codex Responses ↔ Chat bridge、agent tool context、跨请求 tool-call 恢复与 SSE 状态机调研 | [协议与工具转换分析](research/cc-switch/chat-responses-tool-conversion-analysis.md) |
| [`research/codex/`](research/codex/) | Codex 本地客户端 OAuth 与 Responses tool lifecycle 源码调研 | [OAuth 与工具调用](research/codex/oauth-and-tool-call-analysis.md) |
| [`research/chatgpt-oauth/`](research/chatgpt-oauth/) | Hermes 与 LiteLLM 的 ChatGPT/Codex subscription OAuth 实现调研 | [OAuth 实现对比](research/chatgpt-oauth/hermes-and-litellm-oauth-analysis.md) |
| [`specifications/openai/`](specifications/openai/) | OpenAI 官方 API 的协议与规范快照 | [规范目录](specifications/openai/api-specification-catalog.md)、[Chat Completions](specifications/openai/chat-completions-protocol.md)、[Responses](specifications/openai/responses-protocol.md) |

## 文档使用原则

- `architecture/` 与 `design/` 记录本项目的目标设计，不表示已有实现。
- `plans/` 只记录已确认的实施顺序、退出条件和非目标。
- `research/` 是带源码快照和行号的外部实现参考；不等同于本项目依赖或行为承诺。
- `specifications/` 是带采集日期的协议学习快照。实现或升级前应复核当前官方 API Reference、OpenAPI 与模型能力。
