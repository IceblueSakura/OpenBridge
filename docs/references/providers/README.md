# Provider 调研索引

本目录按 Provider 保存外部 API、模型目录和专项能力快照。叶文档拥有来源、日期和证据边界；本索引只提供导航，不表示在
2026-08-12 重新请求过任何 Provider。

## Provider 文档

| Provider | 协议入口 | 模型与专项能力 | 固定日期摘要 |
|---|---|---|---|
| 阿里云百炼 | [API](bailian/api.md) | [Models](bailian/models.md) | Models 快照 2026-08-08；API 的地域与协议面以叶文档来源为准 |
| ChatGPT Codex backend | — | [模型目录说明](chatgpt/models.md)、[脱敏原始 JSON](chatgpt/models-2026-08-10.json) | 采集于 2026-08-10；原始资产不含 token、账户标识或 Authorization 值 |
| DeepSeek | [API](deepseek/api.md) | [Models 与定价](deepseek/models.md) | API 复核 2026-08-09；模型页保留其各来源日期 |
| Kimi | [API](kimi/api.md) | [Kimi K3 参数](kimi/models.md) | 复核于 2026-08-09 |
| LongCat | [API 与 reasoning](longcat/api.md) | [LongCat 2.0](longcat/models.md) | 复核于 2026-08-08 |
| NVIDIA API Catalog / NIM | [API](nvidia/api.md) | [Models](nvidia/models.md) | Models 快照 2026-08-08 |
| OpenRouter | [API 与字段语义](openrouter/api.md) | [模型目录与 endpoint 样本](openrouter/models.md) | wire 观察 2026-08-02；目录复核 2026-08-09；Gemma 定向观察 2026-08-10 |
| Xiaomi MiMo | [API](xiaomi/api.md) | [Models](xiaomi/models.md)、[图片](xiaomi/image.md)、[语音](xiaomi/audio.md) | API 2026-08-09；Models/语音 2026-08-08；图片 2026-08-07 |

## 资产所有权

[ChatGPT 模型目录说明](chatgpt/models.md) 是
[`models-2026-08-10.json`](chatgpt/models-2026-08-10.json) 的 Markdown owner。该 owner 记录采集时间、endpoint、请求
header 形状、脱敏范围、模型数与文件大小；不得从 JSON 反推出长期可用性，后续快照也不得静默覆盖旧采集日期。

## 阅读边界

- Provider 的 Models 目录只证明目录事实，不自动证明具体 endpoint、账户、区域或参数组合可用。
- 聚合 Provider 的模型级字段不自动等于所有下游 endpoint 的交集；endpoint 详情和用户过滤视图需要单独解释。
- 脱敏真实请求只证明固定日期、账户、网络和 payload；不证明 SDK、负载、长期运行、语义质量或未来服务。
- OpenBridge 当前 Target、Route、Public Model 与验证结果由 implementation status 维护，不在本目录复制。
