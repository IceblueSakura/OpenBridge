# Codex 调研索引

本目录记录 Codex 作为外部 Rust CLI/Agent client 的 Responses、tool 与认证行为。许可证见
[Apache-2.0](https://github.com/openai/codex/blob/main/LICENSE)。本索引没有重新拉取 Codex；具体行号、旧快照和复核日期由叶文档维护。

| 主题 | 文档 | 固定证据 |
|---|---|---|
| Responses SSE 与 tool lifecycle | [SSE 与工具调用生命周期](codex-sse-and-tool-lifecycle-analysis.md) | 原始逐行快照 `4c43465133428898aa84f0bfc02c306ed65fb66a`；模块级复核 `ee0247f95a6fe2b094ba2253d82cae2a2b4c2dff`，2026-08-01 |
| Browser OAuth 与 tool invocation | [浏览器 OAuth 与工具调用](codex-oauth-and-tool-call-analysis.md) | 原始逐行快照 `0fb559f0f6e231a88ac02ea002d3ecd248e2b515`；模块级复核 `ee0247f95a6fe2b094ba2253d82cae2a2b4c2dff`，2026-08-01 |
| Device login 与 refresh | [设备登录与 token 刷新](codex-device-auth-token-refresh-analysis.md) | `ee0247f95a6fe2b094ba2253d82cae2a2b4c2dff`，复核 2026-08-05 |
| Protocol tests | [Responses 与工具生命周期测试资产](codex-protocol-test-assets-analysis.md) | 在线复核 2026-07-26；模块级复核 commit 见叶文档 |

这些文档证明固定 Codex client 快照如何消费协议或管理自身 credential，不定义完整 OpenAI server 规范，也不公开保证第三方复用
Codex client registration、account context 或私有 endpoint。升级 Codex、采用新认证 flow，或依赖新的 SSE/tool 行为前必须重新固定证据。
