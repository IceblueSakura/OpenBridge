# 跨项目综合调研索引

本目录只比较已经存在项目级前置文档的外部事实。综合页不拥有项目源码快照，也不在这里首次引入单项目结论；固定 commit、来源
URL 与原始证据由链接的项目文档维护。本索引没有刷新任何外部来源。

| 主题 | 综合文档 | 项目级前置 |
|---|---|---|
| Chat/Responses、SSE与tools | [协议测试资产综合](chat-responses-sse-tool-test-suite-survey.md) | OpenAI/gpt-oss、Open Responses、Codex、LiteLLM |
| 富语义IR、Provider extensions与server tools | [Protocol IR生态综合](protocol-ir-ecosystem-analysis.md) | Bifrost、LiteLLM、TensorZero、Vercel AI SDK、Portkey、Helicone、OpenRouter |
| Credential retry/cooldown | [Pool、cooldown 与有限重试](credential-pool-retry-analysis.md) | CLIProxyAPI、LiteLLM、cc-switch |
| OAuth device/refresh | [设备登录与 token refresh](upstream-oauth-device-code-token-refresh-analysis.md) | RFC、Codex、CLIProxyAPI、Hermes、LiteLLM |

跨项目的证据角色与互证关系见[参考项目调研总览](../project-comparison.md)。综合结论只说明可比较的共性、差异和未知项，不构成
OpenBridge 需求或实现方案。任一前置项目版本、Provider policy、协议规范或采用场景变化时，应先更新对应项目叶文档，再复核综合页。
