# 参考项目调研索引

调研文档用于提供源码事实、实现模式和失败反例，不自动成为 OpenBridge 的产品或架构决定。

| 项目/主题 | 文档 | 主要用途 |
|---|---|---|
| 全局比较 | [参考项目比较矩阵](project-comparison-matrix.md) | Codex、Hermes、LiteLLM、cc-switch、Bifrost、CLIProxyAPI 的研究职责 |
| Codex | [OAuth 与工具调用](codex/oauth-and-tool-call-analysis.md) | Responses tool lifecycle、取消和 credential 边界 |
| Hermes Agent | [Chat/Responses 分析](hermes/chat-responses-analysis.md) | 多 transport Agent loop 和状态语义 |
| LiteLLM | [协议分析](litellm/chat-responses-analysis.md) | 双向 Chat/Responses 转换 |
| LiteLLM | [调用链](litellm/proxy-call-chain-analysis.md) | proxy 入口、认证、路由、Provider 和 usage 链路 |
| LiteLLM | [性能瓶颈](litellm/proxy-performance-bottlenecks.md) | 热路径、缓存、数据库和 bridge 开销反例 |
| cc-switch | [协议与工具转换](cc-switch/chat-responses-tool-conversion-analysis.md) | Codex bridge、tool context 和 SSE 状态机 |
| OAuth 对照 | [Hermes 与 LiteLLM OAuth](chatgpt-oauth/hermes-and-litellm-oauth-analysis.md) | refresh、storage、并发与适用边界 |

每份新增调研应记录 repository、full commit SHA、snapshot date、检查文件、观察事实、推论、适用边界和需要的本地实验。
