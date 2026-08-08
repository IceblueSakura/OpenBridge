# OpenRouter 模型目录调研（reasoning 复核 2026-08-08）

## 来源与采集边界

- 采集时间：2026-08-02，Asia/Shanghai。
- 模型事实：[OpenRouter `GET /api/v1/models`](https://openrouter.ai/api/v1/models)
  与[字段说明](https://openrouter.ai/docs/api/api-reference/models/list-all-models-and-their-properties)。
- reasoning 解释：[OpenRouter Reasoning Tokens](https://openrouter.ai/docs/guides/best-practices/reasoning-tokens)。
- 匹配规则：只接受完整 model id 精确匹配，不以相近名称或同系列模型补齐缺失记录。
- 2026-08-03 补充采集 `architecture`、tokenizer 与 knowledge cutoff；未改变精确匹配规则。
- 2026-08-08 再次精确查询本页涉及的 Qwen3.7、LongCat 2.0 与 MiMo V2.5 记录，复核 `supported_parameters`
  与 `reasoning` 子对象；OpenRouter 仍未给这些记录声明 `supported_efforts`。

本快照只转录目录公开的 description、context、输入/输出模态、tokenizer、knowledge cutoff、最大输出、supported parameters 和
reasoning effort。价格、排行、吞吐、endpoint 数据策略和动态可用性是其他字段类别。目录会变化，本文不是永久事实。

## 精确匹配结果

| OpenRouter model id                 | `context_length` | 最大输出 | input modalities            | tokenizer  | knowledge cutoff | supported efforts                     |
|-------------------------------------|-----------------:|---------:|-----------------------------|------------|------------------|---------------------------------------|
| `openai/gpt-5.6-sol`                |        1,050,000 |  128,000 | `text, image, file`         | `GPT`      | `2026-02-16`     | `max, xhigh, high, medium, low, none` |
| `openai/gpt-5.6-terra`              |        1,050,000 |  128,000 | `text, image, file`         | `GPT`      | `2026-02-16`     | `max, xhigh, high, medium, low, none` |
| `openai/gpt-5.6-luna`               |        1,050,000 |  128,000 | `text, image, file`         | `GPT`      | `2026-02-16`     | `max, xhigh, high, medium, low, none` |
| `openai/gpt-5.5`                    |        1,050,000 |  128,000 | `text, image, file`         | `GPT`      | `2025-12-01`     | `xhigh, high, medium, low, none`      |
| `deepseek/deepseek-v4-pro`          |        1,048,576 |  384,000 | `text`                      | `DeepSeek` | 未声明           | `xhigh, high`                         |
| `deepseek/deepseek-v4-flash`        |        1,048,576 |  393,216 | `text`                      | `DeepSeek` | 未声明           | `xhigh, high`                         |
| `xiaomi/mimo-v2.5-pro`              |        1,050,000 |  131,072 | `text`                      | `Other`    | 未声明           | 未声明离散 effort                     |
| `xiaomi/mimo-v2.5`                  |        1,050,000 |  131,072 | `text, audio, image, video` | `Other`    | 未声明           | 未声明离散 effort                     |
| `meituan/longcat-2.0`               |        1,048,756 |  262,144 | `text`                      | `Other`    | 未声明           | 支持 token budget，未声明离散 effort  |
| `qwen/qwen3.7-max`                  |        1,000,000 |  131,072 | `text`                      | `Qwen`     | 未声明           | 未声明离散 effort                     |
| `qwen/qwen3.7-plus`                 |        1,000,000 |  131,072 | `text, image`               | `Qwen`     | 未声明           | 未声明离散 effort                     |
| `z-ai/glm-5.2`                      |        1,048,576 |  131,072 | `text`                      | `Other`    | 未声明           | `xhigh, high`                         |
| `moonshotai/kimi-k3`                |        1,048,576 |   未声明 | `text, image`               | `Other`    | 未声明           | `max, high, low`                      |
| `minimax/minimax-m3`                |        1,048,576 |  512,000 | `text, image, video`        | `Other`    | 未声明           | 未声明离散 effort                     |

## 2026-08-08 reasoning 精确复核

| OpenRouter model id | `supported_parameters` 中的 reasoning 字段 | `reasoning` 子对象 | 可得结论 |
|---|---|---|---|
| `qwen/qwen3.7-max`、`qwen/qwen3.7-plus` | 有 `reasoning`，无 `reasoning_effort` | `mandatory=false`、`default_enabled=true`，无 `supported_efforts` | 支持可选 reasoning；不能从 OpenRouter 推导离散 effort |
| `meituan/longcat-2.0` | 有 `reasoning`，无 `reasoning_effort` | `mandatory=false`、`default_enabled=true`、`supports_max_tokens=true`，无 `supported_efforts` | 支持开关与 token budget；不能把 budget 推导成离散 effort |
| `xiaomi/mimo-v2.5`、`xiaomi/mimo-v2.5-pro` | 有 `reasoning`，无 `reasoning_effort` | `mandatory=false`，无 `supported_efforts` | 支持可选 reasoning；离散取值必须以 Xiaomi 官方 API 为准 |

`mandatory=false` 只说明 reasoning 不是强制模式；它不提供 `low`、`medium`、`high` 等强度语义。本文因此只把
OpenRouter 作为 reasoning 支持/可关闭性的交叉证据，不用它扩张任何 Provider 官方未声明的 level 集合。
