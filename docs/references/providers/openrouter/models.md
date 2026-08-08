# OpenRouter 模型目录调研（采集 2026-08-02）

## 来源与采集边界

- 采集时间：2026-08-02，Asia/Shanghai。
- 模型事实：[OpenRouter `GET /api/v1/models`](https://openrouter.ai/api/v1/models)
  与[字段说明](https://openrouter.ai/docs/api/api-reference/models/list-all-models-and-their-properties)。
- reasoning 解释：[OpenRouter Reasoning Tokens](https://openrouter.ai/docs/guides/best-practices/reasoning-tokens)。
- 匹配规则：只接受完整 model id 精确匹配，不以相近名称或同系列模型补齐缺失记录。
- 2026-08-03 补充采集 `architecture`、tokenizer 与 knowledge cutoff；未改变精确匹配规则。

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
| `tencent/hy3`                       |          262,144 |  128,000 | `text`                      | `Other`    | 未声明           | `high, low, none`                     |
| `nvidia/nemotron-3-ultra-550b-a55b` |          512,288 |   未声明 | `text`                      | `Other`    | 未声明           | `high, medium`                        |

`gpt-5.3-codex-spark` 在本次目录中没有精确记录。`openai/gpt-5.3-codex` 是另一个 id，不能用它替代 Spark 或补齐 Spark 的字段。

## Nemotron 变体边界

基础模型 `nvidia/nemotron-3-ultra-550b-a55b` 与 `:free` 变体是两条不同目录记录。2026-08-02 快照中：

- 基础模型公布 512,288 context、未公布最大输出，并列出较完整参数集合；
- `:free` 变体公布 1,000,000 context、65,536 最大输出和较小参数集合。

因此基础模型的目录字段不能覆盖 `:free` endpoint，反向也不能用免费变体限制替代基础模型事实。
