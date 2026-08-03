# OpenRouter 模型目录快照（2026-08-02）

## 来源与采集边界

- 采集时间：2026-08-02，Asia/Shanghai。
- 模型事实：[OpenRouter `GET /api/v1/models`](https://openrouter.ai/api/v1/models) 及其
  [字段说明](https://openrouter.ai/docs/api/api-reference/models/list-all-models-and-their-properties)。
- reasoning 解释：[OpenRouter Reasoning Tokens](https://openrouter.ai/docs/guides/best-practices/reasoning-tokens)。
- 匹配规则：只接受完整 model id 精确匹配；不以相近名称或同系列模型补齐缺失项。
- 2026-08-03 复核：补充 `architecture`、tokenizer、knowledge cutoff 和 OpenBridge 输入上限投影；
  未改变“精确匹配、未知不猜测”的边界。

本快照只记录 OpenBridge 当前 `ModelConfig` 可表达的 description、context、输入/输出模态、tokenizer、
knowledge cutoff、最大输出、`supported_parameters`、reasoning 状态和 `supported_efforts`。OpenRouter 的
`context_length` 没有对应的独立输入字段，OpenBridge 将它作为 `max_context_tokens` 和 `max_input_tokens`；
不从总上下文减最大输出。价格、排行、吞吐、endpoint 数据策略和动态可用性不属于 canonical 模型事实。
OpenRouter 目录会变化，后续更新必须重新采集，不得把本文件当成永久事实。

## 精确匹配结果

| OpenBridge model id | OpenRouter 精确 id | context / max input projection | 最大输出 | input modalities | tokenizer | knowledge cutoff | supported efforts |
|---|---|---:|---:|---|---|---|---|
| `openai/gpt-5.6-sol` | 同左 | 1,050,000 | 128,000 | `text, image, file` | `GPT` | `2026-02-16` | `max, xhigh, high, medium, low, none` |
| `openai/gpt-5.6-terra` | 同左 | 1,050,000 | 128,000 | `text, image, file` | `GPT` | `2026-02-16` | `max, xhigh, high, medium, low, none` |
| `openai/gpt-5.6-luna` | 同左 | 1,050,000 | 128,000 | `text, image, file` | `GPT` | `2026-02-16` | `max, xhigh, high, medium, low, none` |
| `openai/gpt-5.5` | 同左 | 1,050,000 | 128,000 | `text, image, file` | `GPT` | `2025-12-01` | `xhigh, high, medium, low, none` |
| `deepseek/deepseek-v4-pro` | 同左 | 1,048,576 | 384,000 | `text` | `DeepSeek` | 未声明 | `xhigh, high` |
| `deepseek/deepseek-v4-flash` | 同左 | 1,048,576 | 393,216 | `text` | `DeepSeek` | 未声明 | `xhigh, high` |
| `xiaomi/mimo-v2.5-pro` | 同左 | 1,050,000 | 131,072 | `text` | `Other` | 未声明 | 未声明离散 effort |
| `xiaomi/mimo-v2.5` | 同左 | 1,050,000 | 131,072 | `text, audio, image, video` | `Other` | 未声明 | 未声明离散 effort |
| `meituan/longcat-2.0` | 同左 | 1,048,756 | 262,144 | `text` | `Other` | 未声明 | 支持 token budget，未声明离散 effort |
| `qwen/qwen3.7-max` | 同左 | 1,000,000 | 131,072 | `text` | `Qwen` | 未声明 | 未声明离散 effort |
| `qwen/qwen3.7-plus` | 同左 | 1,000,000 | 131,072 | `text, image` | `Qwen` | 未声明 | 未声明离散 effort |
| `z-ai/glm-5.2` | 同左 | 1,048,576 | 131,072 | `text` | `Other` | 未声明 | `xhigh, high` |
| `moonshotai/kimi-k3` | 同左 | 1,048,576 | 未声明 | `text, image` | `Other` | 未声明 | `max, high, low` |
| `minimax/minimax-m3` | 同左 | 1,048,576 | 512,000 | `text, image, video` | `Other` | 未声明 | 未声明离散 effort |
| `tencent/hy3` | 同左 | 262,144 | 128,000 | `text` | `Other` | 未声明 | `high, low, none` |
| `nvidia/nemotron-3-ultra-550b-a55b` | 同左 | 512,288 | 未声明 | `text` | `Other` | 未声明 | `high, medium` |

`gpt-5.3-codex-spark` 在本次 OpenRouter 目录中没有精确记录。目录中的
`openai/gpt-5.3-codex` 是另一个 id，不能据此覆盖 Spark。OpenBridge 中 Spark 的 128,000 context、
128,000 最大输出及 `xhigh, high, medium, low` levels 为人工修订值，不归因于 OpenRouter 本次快照；LiteLLM
配置只额外确认了 `reasoning` 支持。

## Nemotron 变体边界

LiteLLM 配置的上游字符串是 `openrouter/nvidia/nemotron-3-ultra-550b-a55b:free`。2026-08-02 的 OpenRouter
目录中，基础模型与 `:free` 变体元数据不同：基础模型公布 512,288 context、未公布最大输出并支持更完整的参数；
`:free` 变体公布 1,000,000 context、65,536 最大输出和较小参数集合。

`ModelConfig` 保存 provider-independent canonical 模型上界，因此采用基础模型记录。若未来注册 `:free`
Upstream API，应通过 `UpstreamApiModelRules` 和 API capabilities 收窄，不得把该 endpoint 的限制提升为所有
Provider 共用的模型事实。
