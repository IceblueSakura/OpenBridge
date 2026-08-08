# DeepSeek 模型目录与定价调研

## 来源与范围

- 官方模型与定价：[Models & Pricing](https://api-docs.deepseek.com/quick_start/pricing/)（2026-08-08 抓取）。
- OpenRouter 目录补充：[OpenRouter 模型目录](../openrouter/models.md)（2026-08-02 采集，精确匹配）。
- 价格单位为每 1M tokens（美元）；本文不构成计费承诺，价格随时可能调整。

## 官方模型表（2026-08-08）

| 字段 | `deepseek-v4-flash` | `deepseek-v4-pro` |
|---|---|---|
| MODEL VERSION | DeepSeek-V4-Flash-0731 | DeepSeek-V4-Pro |
| BASE URL (OpenAI) | `https://api.deepseek.com` | 同左 |
| BASE URL (Anthropic) | `https://api.deepseek.com/anthropic` | 同左 |
| THINKING MODE | 支持非思考/思考（默认思考）两种模式 | 同左 |
| CONTEXT LENGTH | 1M | 1M |
| MAX OUTPUT | 最大 384K | 最大 384K |
| JSON Output | ✓ | ✓ |
| Tool Calls | ✓ | ✓ |
| Responses API | ✓ | ✗（预计 2026 年 8 月初支持） |
| Anthropic API | ✓ | ✓ |
| Chat Prefix Completion（Beta） | ✓ | ✓ |
| FIM Completion（Beta） | 仅非思考模式 | 仅非思考模式 |
| 1M 输入（cache hit） | $0.0028 | $0.003625 |
| 1M 输入（cache miss） | $0.14 | $0.435 |
| 1M 输出 | $0.28 | $0.87 |
| 并发限制 | 2500 | 500 |

注意：官方页标注 API 服务将很快采用峰谷定价，高峰时段（北京时间每日 9:00–12:00、14:00–18:00）价格为常规 2 倍，生效日期以官方公告为准。

## OpenRouter 目录补充（2026-08-02 精确匹配）

| OpenRouter model id | `context_length` | 最大输出 | input modalities | supported efforts |
|---|---|---|---|---|
| `deepseek/deepseek-v4-pro` | 1,048,576 | 384,000 | `text` | `xhigh, high` |
| `deepseek/deepseek-v4-flash` | 1,048,576 | 393,216 | `text` | `xhigh, high` |

该表来源为 OpenRouter 目录而非 DeepSeek 官方页；两处 context 均约 1M，但官方 MAX OUTPUT 384K 与 OpenRouter 的 flash 393,216 并不相同，使用时以各自 endpoint 声明为准。

## 证据边界

官方页模型版本、特性与并发限制可能随发布变化；`deepseek-v4-flash-0731` 等带日期版本号在本地与目录中也可能存在差异（如并行工作流中保留的 upstream model 覆盖）。价格、配额与模型行为不构成长期承诺。
