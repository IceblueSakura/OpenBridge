# Hermes Agent 调研索引

本目录记录 Hermes Agent 作为外部 OpenAI-compatible consumer 的协议、credential 与插件行为。许可证见
[MIT](https://github.com/NousResearch/hermes-agent/blob/main/LICENSE)。本索引只整理已有固定快照，没有启动 Hermes 或刷新外部来源。

| 主题 | 文档 | 固定证据 |
|---|---|---|
| Chat/Responses 请求合同 | [上游请求合同](hermes-chat-responses-analysis.md) | `main` @ `a31be48030f60383bf4c1d96ba46bd4b48430218`，本地 checkout 复核 2026-08-11 |
| Codex OAuth lifecycle | [OAuth credential lifecycle](hermes-codex-oauth-refresh-analysis.md) | 原始 `e598cef87465981fcea1c0339edfcf5d9716c917`；模块级复核 `470cf66b039c73bdd2c21d43094ce41a4db74eae`，2026-08-05 |
| Models consumer schema | [`/models` 完整 Schema](hermes-models-endpoint-schema.md) | Hermes `v0.20.0 (2026.8.3)` 安装快照；阅读于 2026-08-08 |
| Provider plugin 与 aux 分派 | [model-provider 插件能力](hermes-provider-plugin-capabilities.md) | Hermes `v0.20.0 (2026.8.3)` 安装快照；阅读于 2026-08-08 |
| Gateway-facing plugin surface | [插件化扩展能力全景](hermes-gateway-plugin-capabilities.md) | Hermes `v0.20.0 (2026.8.3)` 安装快照；阅读于 2026-08-08 |

Hermes 的宽松 fallback、字段消费和插件默认值是该客户端的产品行为，不是通用 gateway contract。升级 Hermes，依赖新的
`/models` 字段、ProviderProfile hook、auxiliary routing 或 credential lifecycle 前应重新固定版本并核对对应叶文档。
