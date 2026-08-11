# OpenRouter 模型目录调研（2026-08-09 复核）

## 文档元数据

| 字段 | 值 |
|---|---|
| Source snapshot | 2026-08-09 的公开 Models/endpoint API；2026-08-10 的 Gemma 4 31B 定向请求；MiniMax 官方发布说明 |
| Last reverified | 外部目录最后复核 2026-08-09，Gemma 定向观察 2026-08-10；2026-08-12 仅移除本地实现对照，没有刷新 OpenRouter |
| Scope | 全模态目录计数、17 个固定模型样本、DeepSeek/MiniMax endpoint 参数差异、reasoning 元数据与 Gemma 定向 wire |
| Evidence boundary | 聚合目录、模型级字段和一次真实请求不证明所有 endpoint、账户、路由、额度、语义质量或长期可用性 |
| Recheck trigger | Models/endpoint schema、Provider routing、样本模型 metadata、free endpoint 政策或采用的参数集合变化时 |

## 来源与采集边界

- 采集时间：2026-08-09，Asia/Shanghai。
- 公开目录：[OpenRouter `GET /api/v1/models`](https://openrouter.ai/api/v1/models) 与
  [Models API 字段说明](https://openrouter.ai/docs/api/api-reference/models/get-models)。本次显式使用
  `output_modalities=all&limit=1000`；该接口默认只返回文本输出模型，不能用默认查询代表全量目录。
- endpoint 详情：[List all endpoints for a model](https://openrouter.ai/docs/api/api-reference/endpoints/list-endpoints)。
- Provider 参数路由：[`provider.require_parameters`](https://openrouter.ai/docs/guides/routing/provider-selection)。
- reasoning 解释：[OpenRouter Reasoning Tokens](https://openrouter.ai/docs/guides/best-practices/reasoning-tokens)。
- MiniMax 模型行为：[MiniMax M3 官方发布说明](https://www.minimax.io/blog/minimax-m3)：只声明 thinking 可开/关。

本快照只记录 identity、context、最大输出、输入/输出模态、tokenizer、knowledge cutoff、`supported_parameters` 和
reasoning effort。价格、排行、吞吐、数据策略和动态可用性属于其他字段类别。OpenRouter 目录和 endpoint 会变化，本文不是永久事实。

## 目录摘要与固定样本

显式使用 `output_modalities=all` 时，本次公开目录返回 525 条记录。下表保留调研涉及的 17 个固定模型样本；它不是完整目录，
也不表示这些模型对任一账户或 endpoint 当前可用。“未声明”只表示该次目录记录没有给出字段。

| OpenRouter model id | `context_length` | 最大输出 | input modalities | tokenizer | knowledge cutoff | supported efforts |
|---|---:|---:|---|---|---|---|
| `openai/gpt-5.6-sol` | 1,050,000 | 128,000 | `text, image, file` | `GPT` | `2026-02-16` | `max, xhigh, high, medium, low, none` |
| `openai/gpt-5.6-terra` | 1,050,000 | 128,000 | `text, image, file` | `GPT` | `2026-02-16` | `max, xhigh, high, medium, low, none` |
| `openai/gpt-5.6-luna` | 1,050,000 | 128,000 | `text, image, file` | `GPT` | `2026-02-16` | `max, xhigh, high, medium, low, none` |
| `openai/gpt-5.5` | 1,050,000 | 128,000 | `text, image, file` | `GPT` | `2025-12-01` | `xhigh, high, medium, low, none` |
| `deepseek/deepseek-v4-pro` | 1,048,576 | 384,000 | `text` | `DeepSeek` | 未声明 | `xhigh, high` |
| `deepseek/deepseek-v4-flash` | 1,048,576 | 393,216 | `text` | `DeepSeek` | 未声明 | `xhigh, high` |
| `xiaomi/mimo-v2.5-pro` | 1,050,000 | 131,072 | `text` | `Other` | 未声明 | 未声明离散 effort |
| `xiaomi/mimo-v2.5` | 1,050,000 | 131,072 | `text, audio, image, video` | `Other` | 未声明 | 未声明离散 effort |
| `meituan/longcat-2.0` | 1,048,756 | 262,144 | `text` | `Other` | 未声明 | 支持 token budget，未声明离散 effort |
| `qwen/qwen3.6-27b` | 262,144 | 262,144 | `text, image, video` | `Qwen3` | 未声明 | 未声明离散 effort |
| `qwen/qwen3.7-max` | 1,000,000 | 131,072 | `text` | `Qwen` | 未声明 | 未声明离散 effort |
| `qwen/qwen3.7-plus` | 1,000,000 | 131,072 | `text, image` | `Qwen` | 未声明 | 未声明离散 effort |
| `qwen/qwen3.8-max` | 1,000,000 | 131,072 | `text, image, video` | `Qwen` | 未声明 | `xhigh, high, medium, low, minimal` |
| `z-ai/glm-5.2` | 1,048,576 | 128,000 | `text` | `Other` | 未声明 | `xhigh, high` |
| `moonshotai/kimi-k3` | 1,048,576 | 未声明 | `text, image` | `Other` | 未声明 | `max, high, low` |
| `minimax/minimax-m3` | 1,048,576 | 512,000 | `text, image, video` | `Other` | 未声明 | 未声明离散 effort |
| `openai/text-embedding-3-small` | 8,192 | 未声明 | `text` | `Other` | 未声明 | 不适用 |

## 目录字段观察

- `supported_parameters` 是模型级目录元数据，不能解释成每个 endpoint 的共同保证。
- `openai/text-embedding-3-small` 的同一记录将任务描述为 `text -> embeddings`，但参数列表包含 generation 参数并遗漏
  `encoding_format`、`dimensions` 等 Embeddings 专用字段；这是目录内部的信息层不一致，不能据此推导 endpoint contract。
- `supported_efforts` 缺失不等于不支持 reasoning；`mandatory=false` 也只说明 reasoning 不是强制模式，不自动提供离散强度语义。
- `Other` tokenizer 是聚合分类，不是具体 tokenizer 实现证据；单复数 output modality 词汇也不应直接提升为 wire schema。

## 模型级与 endpoint 详情差异

本次对 DeepSeek V4 Flash 与 MiniMax M3 读取独立 endpoint 资源。两者的模型级 `supported_parameters` 都等于 endpoint 并集，
但所有 endpoint 的交集明显更窄：

| model id | endpoint 数 | 模型级/endpoint 并集 | 所有 endpoint 交集 | 非全 endpoint 共同支持 |
|---|---:|---:|---|---:|
| `deepseek/deepseek-v4-flash` | 20 | 21 | `include_reasoning`, `max_tokens`, `reasoning`, `reasoning_effort`, `temperature`, `tool_choice`, `tools`, `top_p` | 13 |
| `minimax/minimax-m3` | 9 | 19 | `include_reasoning`, `max_tokens`, `reasoning`, `temperature`, `top_p` | 14 |

不在所有 endpoint 交集中的字段为：

- DeepSeek V4 Flash：`frequency_penalty`、`logit_bias`、`logprobs`、`min_p`、`presence_penalty`、
  `repetition_penalty`、`response_format`、`seed`、`stop`、`structured_outputs`、`top_a`、`top_k`、`top_logprobs`；
- MiniMax M3：`frequency_penalty`、`logit_bias`、`logprobs`、`min_p`、`presence_penalty`、`repetition_penalty`、
  `response_format`、`seed`、`stop`、`structured_outputs`、`tool_choice`、`tools`、`top_k`、`top_logprobs`。

OpenRouter 的 `provider.require_parameters` 默认为 `false`：默认路由可选择不支持全部所传参数的 endpoint，并由该 Provider
忽略未知参数；设为 `true` 才会排除不支持请求中全部参数的 endpoint。采用模型级参数集合时需要显式决定是否接受这一行为。

## Reasoning 解释边界

MiniMax M3 官方材料只声明 thinking 可开/关；OpenRouter 缺少离散 `supported_efforts` 不能补出中间强度。Qwen3.6 27B 的
Alibaba endpoint 在该快照中给出 262,144 context 和 65,536 最大输出，但模型级记录的最大输出为 262,144，说明聚合层级需要分别记录。
Qwen3.8 Max 的样本记录给出 `minimal` 至 `xhigh` 且标记 mandatory；这只适用于该次 OpenRouter 目录，不应外推到其他 Provider。

## 2026-08-10 Gemma 4 31B 定向观察

对 `google/gemma-4-31b-it:free` 的独立定向请求观察到 Chat、流式 usage 尾块、parallel tool calls、PNG data URL 图片输入和
Responses endpoint 均可完成。reasoning 默认关闭；本次没有建立其他离散 reasoning effort 语义。

同一轮 strict JSON Schema 请求返回 markdown 包裹的 JSON，未可靠遵循 strict schema。因此该结果只支持保守的
`json_object` 结论，不支持 strict JSON Schema。以上观察仅适用于当时的 OpenRouter free endpoint、账户、网络和精确 payload；
不证明其他图片 MIME、remote URL、Provider 路由、额度、语义质量、负载或长期可用性。
