# OpenRouter 模型目录调研（2026-08-09 复核）

## 来源与采集边界

- 采集时间：2026-08-09，Asia/Shanghai。
- 公开目录：[OpenRouter `GET /api/v1/models`](https://openrouter.ai/api/v1/models) 与
  [Models API 字段说明](https://openrouter.ai/docs/api/api-reference/models/get-models)。本次显式使用
  `output_modalities=all&limit=1000`；该接口默认只返回文本输出模型，不能用默认查询代表全量目录。
- endpoint 详情：[List all endpoints for a model](https://openrouter.ai/docs/api/api-reference/endpoints/list-endpoints)。
- Provider 参数路由：[`provider.require_parameters`](https://openrouter.ai/docs/guides/routing/provider-selection)。
- reasoning 解释：[OpenRouter Reasoning Tokens](https://openrouter.ai/docs/guides/best-practices/reasoning-tokens)。
- MiniMax 模型行为：[MiniMax M3 官方发布说明](https://www.minimax.io/blog/minimax-m3)：只声明 thinking 可开/关。
- 本地对照范围：当前 checkout 的 31 个 canonical Model；只接受完整 model id 精确匹配，不以相近名称或同系列模型补齐记录。

本快照只比较 identity、context、最大输出、输入/输出模态、tokenizer、knowledge cutoff、`supported_parameters` 和
reasoning effort。价格、排行、吞吐、数据策略和动态可用性属于其他字段类别。OpenRouter 目录和 endpoint 会变化，本文不是永久事实。

## 全量对照摘要

| 项目 | 结果 | 解释 |
|---|---:|---|
| OpenRouter 全模态公开目录 | 525 | 显式使用 `output_modalities=all` |
| OpenBridge canonical Model | 31 | 当前 checkout 的编译期目录 |
| 完整 id 精确匹配 | 17 | 只比较同一完整 id |
| 无精确匹配 | 14 | 5 个 `chatgpt/*`、5 个 Qwen 专用模型、4 个 MiMo 音频模型 |
| 精确匹配且 `supported_parameters` 完全相同 | 15 / 17 | 两个差异见下文 |

14 个无精确匹配项为：

- `chatgpt/gpt-5.3-codex-spark`、`chatgpt/gpt-5.5`、`chatgpt/gpt-5.6-luna`、
  `chatgpt/gpt-5.6-sol`、`chatgpt/gpt-5.6-terra`；
- `qwen/qwen3.5-livetranslate-flash-realtime`、`qwen/qwen3.7-text-embedding`、
  `qwen/qwen-audio-3.0-asr-flash`、`qwen/qwen-image-3.0`、`qwen/qwen-image-3.0-pro`；
- `xiaomi/mimo-v2.5-asr`、`xiaomi/mimo-v2.5-tts`、`xiaomi/mimo-v2.5-tts-voicedesign`、
  `xiaomi/mimo-v2.5-tts-voiceclone`。

`chatgpt/gpt-5.5` 与三个 `chatgpt/gpt-5.6-*` profile 虽可和对应 `openai/*` 模型作同系列参考，但不是精确 id；
它们相对 OpenRouter 对应记录多声明 `parallel_tool_calls`。`chatgpt/gpt-5.3-codex-spark` 也不能用相近的
`openai/gpt-5.3-codex` 替代。

## 17 个精确匹配记录

下表记录 OpenRouter 侧数值；“未声明”不等于不支持。

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

## `supported_parameters` 精确差异

| model id | OpenBridge 独有 | OpenRouter 独有 | 当前解释 |
|---|---|---|---|
| `moonshotai/kimi-k3` | `n` | 无 | Kimi 官方固定 `n=1`；OpenBridge 继续接受标准字段，但 Kimi CN egress 会删除它 |
| `openai/text-embedding-3-small` | `encoding_format`, `user` | `frequency_penalty`, `logit_bias`, `logprobs`, `max_completion_tokens`, `max_tokens`, `presence_penalty`, `response_format`, `seed`, `stop`, `structured_outputs`, `temperature`, `top_logprobs`, `top_p` | OpenRouter 同一记录明确是 `text -> embeddings`，但列出 generation 参数并遗漏 Embeddings 专用字段；当前把它视为目录元数据不一致，不据此扩张本地类型化 Embeddings 契约 |

其余 15 个精确匹配模型的集合完全相同；这里比较的是集合，不依赖数组顺序。

## 其他精确元数据差异

| model id | OpenBridge | OpenRouter | 处理边界 |
|---|---|---|---|
| `deepseek/deepseek-v4-flash` | reasoning levels `max, high, low` | `xhigh, high` | Provider 词汇不一致；OpenRouter fallback 需要独立映射与真实验收，不能直接等同 |
| `deepseek/deepseek-v4-pro` | reasoning levels `max, high` | `xhigh, high` | 同上；当前 OpenBridge 没有该模型的 OpenRouter target |
| `qwen/qwen3.6-27b` | 最大输出 65,536 | 最大输出 262,144 | 本地仍保持较保守的已确认上限，不由聚合目录自动放宽 |
| `z-ai/glm-5.2` | 最大输出 131,072 | 最大输出 128,000 | 两侧数值不一致，需要回到实际 Provider/官方资料决定具体 target 上限 |
| `openai/text-embedding-3-small` | output modality `embedding`；tokenizer 未知 | output modality `embeddings`；tokenizer `Other` | 前者有单复数词汇差异；`Other` 不是具体 tokenizer 证据 |

除上表外，精确匹配项的 context、模态、tokenizer 和 knowledge cutoff 没有发现其他差异。OpenRouter 未提供 MiMo、LongCat、
Qwen3.7 和 MiniMax 的 `supported_efforts` 属于元数据缺失，不构成对本地官方 Provider 证据的反证。

## 当前 OpenRouter target 的 endpoint 差异

OpenBridge 当前只把 `deepseek/deepseek-v4-flash` 与 `minimax/minimax-m3` 接入 OpenRouter。两者本地 canonical
`supported_parameters` 均与 OpenRouter 模型级集合相同，但 endpoint 交集明显更窄：

| model id | endpoint 数 | 模型级/endpoint 并集 | 所有 endpoint 交集 | 非全 endpoint 共同支持 |
|---|---:|---:|---|---:|
| `deepseek/deepseek-v4-flash` | 20 | 21 | `include_reasoning`, `max_tokens`, `reasoning`, `reasoning_effort`, `temperature`, `tool_choice`, `tools`, `top_p` | 13 |
| `minimax/minimax-m3` | 9 | 19 | `include_reasoning`, `max_tokens`, `reasoning`, `temperature`, `top_p` | 14 |

本次两个模型中，模型级集合都等于 endpoint 并集；不在所有 endpoint 交集中的字段为：

- DeepSeek V4 Flash：`frequency_penalty`、`logit_bias`、`logprobs`、`min_p`、`presence_penalty`、
  `repetition_penalty`、`response_format`、`seed`、`stop`、`structured_outputs`、`top_a`、`top_k`、`top_logprobs`；
- MiniMax M3：`frequency_penalty`、`logit_bias`、`logprobs`、`min_p`、`presence_penalty`、`repetition_penalty`、
  `response_format`、`seed`、`stop`、`structured_outputs`、`tool_choice`、`tools`、`top_k`、`top_logprobs`。

因此，至少对这两个当前 target，模型级 `supported_parameters` 不能解释成每个 endpoint 的共同保证。OpenRouter 的
`provider.require_parameters` 默认为 `false`；默认路由可把请求交给不支持全部字段的 endpoint，并由其忽略未知参数。设置为
`true` 才会排除不支持请求中全部参数的 endpoint。当前 OpenBridge OpenRouter adapter 没有主动注入该开关，所以模型级集合不能单独
证明每次路由都会实际应用对应参数。

## reasoning 解释边界

`mandatory=false` 只说明 reasoning 不是强制模式；它不自动提供 `low`、`medium`、`high` 等强度语义。OpenRouter 可作为
reasoning 支持/可关闭性的交叉证据，但不能用缺失的 `supported_efforts` 扩张 Provider 官方未声明的 level 集合。MiniMax M3 因而仍按
`none/high` 二态模型契约解释：`none` 表示关闭，`high` 表示开启；该归一化不是对中间强度的推断。
