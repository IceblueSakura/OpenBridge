# 阿里云百炼 Models 列表前缀与研发者分类

## 范围与快照

- 快照日期：2026-08-08。
- 观察入口：阿里云百炼北京 OpenAI 兼容 Models endpoint，https://dashscope.aliyuncs.com/compatible-mode/v1/models。
- 观察结果：HTTP 200，返回 236 个模型 ID。
- 观察方式：使用当前项目已配置的 Bailian API key 通过受信的 Models probe 获取；本文不记录 key、请求头或响应正文。
- 一级分类：模型 ID 不包含 / 时归入“百炼原生”；包含 / 时归入“第三方转发”。
- 二级分类：按模型 ID、公开模型目录名称和模型家族归纳研发者；第三方条目按 slash 后的模型家族归组，保留原始转发前缀。

“原生/第三方”是按 ID 命名空间做的本次观察分类；研发者分组是基于模型家族的参考归纳，不是 Models API 明示的供应商字段。模型列表、账号权限、地域、配额和模型生命周期可能变化。

## 排序与日期证据

每个研发者组内按当前可用的日期证据从新到旧排列。优先记录模型 ID、公开模型目录或 OpenRouter 给出的发布时间/版本日期；对于本次截图中可唯一对应且原先未确认的条目，记录百炼模型卡片右下角显示的更新时间。无法确认的条目统一放在该组末尾并标为“未确认”，不把 OpenRouter 的目录创建时间直接当作发布时间。

本次补全依据 2026-08-08 用户提供的百炼“全部模型”页面截图；截图日期是模型市场更新时间，不等同于模型 ID 中的版本日期。截至本次截图，已补全截图中能够与本文 ID 或同名模型卡唯一对应的 43 个原“未确认”条目，其中本次新增 9 个；原有已确认的公开版本日期不改写。

OpenRouter 的模型对象可提供 id、name、created、canonical_slug 等目录字段；本快照只把公开页面的 release 信息或 canonical slug 版本日期作为辅助参考，不把其目录时间当作官方发布时间。

## 分类汇总

| 一级分类 | 数量 | 二级分类说明 |
|---|---:|---|
| 百炼原生 | 210 | 无 /；按研发者/模型家族分组 |
| 第三方转发 | 26 | 有 /；按 slash 后模型家族对应的研发者分组 |
| 合计 | 236 | 本次 Models 响应中的全部 ID |

## 百炼原生（无前缀，210）

### 阿里云 / Qwen、Tongyi、Wan 与 GUI（183）

- 2026-08-04 — `qwen-image-3.0`
- 2026-08-03 — `qwen3.8-max`
- 2026-07-30 — `qwen-audio-3.0-asr-flash`
- 2026-07-27 — `qwen3.7-flash`
- 2026-07-20 — `qwen-image-3.0-pro`
- 2026-07-15 — `qwen3.7-flash-2026-07-15`
- 2026-07-15 — `qwen3.7-text-embedding`
- 2026-07-14 — `qwen-audio-3.0-realtime-flash`
- 2026-07-14 — `qwen-audio-3.0-realtime-plus`
- 2026-06-22 — `qwen-image-2.0-pro-2026-06-22`
- 2026-06-16 — `qwen3.5-ocr`
- 2026-06-08 — `qwen3.7-max-2026-06-08`
- 2026-06-03 — `qwen3.7-plus`
- 2026-05-26 — `qwen3.7-plus-2026-05-26`
- 2026-05-20 — `qwen3.7-max-2026-05-20`
- 2026-05-20 — `qwen3.7-max`
- 2026-05-19 — `qwen3.5-livetranslate-flash-realtime-2026-05-19`
- 2026-05-17 — `qwen3.7-max-2026-05-17`
- 2026-04-27 — `qwen3.6-max-preview`
- 2026-04-22 — `qwen-image-2.0-pro-2026-04-22`
- 2026-04-20 — `qwen3.5-plus-2026-04-20`
- 2026-04-20 — `qwen3.5-plus`
- 2026-04-16 — `qwen3.6-flash-2026-04-16`
- 2026-04-16 — `qwen3.6-flash`
- 2026-04-02 — `qwen3.6-plus-2026-04-02`
- 2026-04-02 — `qwen3.6-plus`
- 2026-04-01 — `wan2.7-image-pro`
- 2026-04 — `qwen3.6-27b`
- 2026-03-30 — `qwen3.5-omni-plus`
- 2026-03-18 — `gui-plus`
- 2026-03-15 — `qwen3.5-omni-flash-2026-03-15`
- 2026-03-15 — `qwen3.5-omni-flash-realtime-2026-03-15`
- 2026-03-15 — `qwen3.5-omni-plus-2026-03-15`
- 2026-03-15 — `qwen3.5-omni-plus-realtime-2026-03-15`
- 2026-03-03 — `qwen-image-2.0-2026-03-03`
- 2026-03-03 — `qwen-image-2.0-pro-2026-03-03`
- 2026-02-28 — `qwen-flash-character`
- 2026-02-26 — `qwen-flash-character-2026-02-26`
- 2026-02-23 — `qwen3.5-27b`
- 2026-02-23 — `qwen3.5-flash-2026-02-23`
- 2026-02-23 — `qwen3.5-flash`
- 2026-02-19 — `qwen3-coder-next`
- 2026-02-15 — `qwen3.5-plus-2026-02-15`
- 2026-02-13 — `qwen3-asr-flash-realtime`
- 2026-02-10 — `qwen3-asr-flash-2026-02-10`
- 2026-02-10 — `qwen3-asr-flash-realtime-2026-02-10`
- 2026-02-10 — `qwen3-tts-instruct-flash`
- 2026-01-26 — `qwen3-tts-instruct-flash-2026-01-26`
- 2026-01-26 — `qwen3-tts-vd-2026-01-26`
- 2026-01-23 — `qwen3-max-2026-01-23`
- 2026-01-23 — `qwen3-max`
- 2026-01-22 — `qwen3-tts-instruct-flash-realtime-2026-01-22`
- 2026-01-22 — `qwen3-tts-vc-2026-01-22`
- 2026-01-22 — `qwen3-vl-flash-2026-01-22`
- 2026-01-21 — `qwen3-tts-instruct-flash-realtime`
- 2026-01-16 — `qwen-image-edit-max-2026-01-16`
- 2026-01-15 — `qwen3-tts-vc-realtime-2026-01-15`
- 2026-01-15 — `qwen3-tts-vd-realtime-2026-01-15`
- 2026-01-09 — `qwen-image-plus-2026-01-09`
- 2026-01-09 — `tongyi-xiaomi-analysis-flash`
- 2026-01-09 — `tongyi-xiaomi-analysis-pro`
- 2025-12-30 — `qwen-image-max-2025-12-30`
- 2025-12-19 — `qwen3-vl-plus-2025-12-19`
- 2025-12-19 — `qwen3-vl-plus`
- 2025-12-18 — `z-image-turbo`
- 2025-12-16 — `qwen3-tts-vd-realtime-2025-12-16`
- 2025-12-15 — `qwen-deep-research-2025-12-15`
- 2025-12-15 — `qwen-image-edit-plus-2025-12-15`
- 2025-12-04 — `qwen3-livetranslate-flash`
- 2025-12-01 — `qwen-plus-2025-12-01`
- 2025-12-01 — `qwen3-livetranslate-flash-2025-12-01`
- 2025-12-01 — `qwen3-omni-flash-2025-12-01`
- 2025-12-01 — `qwen3-omni-flash-realtime-2025-12-01`
- 2025-12-01 — `qwen3-omni-flash`
- 2025-11-27 — `qwen3-tts-flash-2025-11-27`
- 2025-11-27 — `qwen3-tts-flash-realtime-2025-11-27`
- 2025-11-27 — `qwen3-tts-flash-realtime`
- 2025-11-27 — `qwen3-tts-flash`
- 2025-11-27 — `qwen3-tts-vc-realtime-2025-11-27`
- 2025-11-20 — `qwen-vl-ocr-2025-11-20`
- 2025-11-19 — `qwen-mt-lite`
- 2025-11-06 — `qwen-mt-flash`
- 2025-11-05 — `qwen-plus-2025-11-05`
- 2025-10-30 — `qwen-image-edit-plus-2025-10-30`
- 2025-10-27 — `qwen3-asr-flash-realtime-2025-10-27`
- 2025-10-15 — `qwen3-vl-flash-2025-10-15`
- 2025-09-23 — `qwen3-coder-plus-2025-09-23`
- 2025-09-23 — `qwen3-coder-plus`
- 2025-09-23 — `qwen3-livetranslate-flash-realtime`
- 2025-09-23 — `qwen3-max-2025-09-23`
- 2025-09-23 — `qwen3-vl-plus-2025-09-23`
- 2025-09-22 — `qwen3-livetranslate-flash-realtime-2025-09-22`
- 2025-09-22 — `qwen3-s2s-flash-realtime-2025-09-22`
- 2025-09-18 — `qwen3-tts-flash-2025-09-18`
- 2025-09-18 — `qwen3-tts-flash-realtime-2025-09-18`
- 2025-09-15 — `qwen3-omni-flash-2025-09-15`
- 2025-09-15 — `qwen3-omni-flash-realtime-2025-09-15`
- 2025-09-11 — `qwen-plus-2025-09-11`
- 2025-08-05 — `qwen-flash`
- 2025-08-05 — `qwen3-coder-flash`
- 2025-07-22 — `qwen-mt-plus`
- 2025-07-22 — `qwen-mt-turbo`
- 2025-07-22 — `qwen3-coder-480b-a35b-instruct`
- 2025-07-22 — `qwen3-coder-plus-2025-07-22`
- 2025-07-14 — `qwen-plus-2025-07-14`
- 2025-07 — `qwen3-235b-a22b-instruct-2507`
- 2025-07 — `qwen3-235b-a22b-thinking-2507`
- 2025-07 — `qwen3-30b-a3b-instruct-2507`
- 2025-07 — `qwen3-30b-a3b-thinking-2507`
- 2025-06-13 — `qwen-vl-plus`
- 2025-06-03 — `qvq-plus`
- 2025-05-26 — `qwen-vl-max`
- 2025-05-22 — `qwen-tts-2025-05-22`
- 2025-04-28 — `qwen-plus-2025-04-28`
- 2025-03-26 — `qvq-max`
- 2025-03-19 — `qwen-long`
- 2025-03-05 — `qwq-plus`
- 2025-01-25 — `qwen-plus-2025-01-25`
- 2025-01-25 — `qwen-plus`
- 2024-11-12 — `qwen-coder-plus`
- 2024-11-06 — `qwen-coder-plus-1106`
- 2024-11-01 — `qwen-turbo-2024-11-01`
- 2024-11-01 — `qwen-turbo`
- 2024-09-19 — `qwen-coder-turbo`
- 2024-09-19 — `qwen-math-plus-0919`
- 2024-09-19 — `qwen-math-turbo`
- 2024-09-19 — `qwen-turbo-0919`
- 2024-01-07 — `qwen-max-0107`
- 2024-01-07 — `qwen-max`
- 未确认 — `codeqwen1.5-7b-chat`
- 未确认 — `qwen-1.8b-chat`
- 未确认 — `qwen-1.8b-longcontext-chat`
- 未确认 — `qwen-14b-chat`
- 未确认 — `qwen-72b-chat`
- 未确认 — `qwen-7b-chat`
- 未确认 — `qwen-deep-search-planning`
- 未确认 — `qwen-image-2.0`
- 未确认 — `qwen-image-2.0-pro`
- 未确认 — `qwen-image-edit-max`
- 未确认 — `qwen-image-edit-plus`
- 未确认 — `qwen-image-max`
- 未确认 — `qwen-math-plus`
- 未确认 — `qwen-math-plus-latest`
- 未确认 — `qwen-max-1201`
- 未确认 — `qwen-max-longcontext`
- 未确认 — `qwen-omni-turbo`
- 未确认 — `qwen-plus-latest`
- 未确认 — `qwen-vl-ocr`
- 未确认 — `qwen-vl-ocr-latest`
- 未确认 — `qwen1.5-0.5b-chat`
- 未确认 — `qwen1.5-1.8b-chat`
- 未确认 — `qwen1.5-110b-chat`
- 未确认 — `qwen1.5-14b-chat`
- 未确认 — `qwen1.5-32b-chat`
- 未确认 — `qwen1.5-72b-chat`
- 未确认 — `qwen1.5-7b-chat`
- 未确认 — `qwen2-0.5b-instruct`
- 未确认 — `qwen2-1.5b-instruct`
- 未确认 — `qwen2-57b-a14b-instruct`
- 未确认 — `qwen2-7b-instruct`
- 未确认 — `qwen2.5-0.5b-instruct`
- 未确认 — `qwen2.5-1.5b-instruct`
- 未确认 — `qwen2.5-math-1.5b-instruct`
- 未确认 — `qwen3-14b`
- 未确认 — `qwen3-235b-a22b`
- 未确认 — `qwen3-30b-a3b`
- 未确认 — `qwen3-32b`
- 未确认 — `qwen3-8b`
- 未确认 — `qwen3-max-preview`
- 未确认 — `qwen3-next-80b-a3b-instruct`
- 未确认 — `qwen3-next-80b-a3b-thinking`
- 未确认 — `qwen3-omni-flash-realtime`
- 未确认 — `qwen3-vl-flash`
- 未确认 — `qwen3.5-122b-a10b`
- 未确认 — `qwen3.5-35b-a3b`
- 未确认 — `qwen3.5-397b-a17b`
- 未确认 — `qwen3.5-livetranslate-flash-realtime`
- 未确认 — `qwen3.5-omni-flash`
- 未确认 — `qwen3.5-omni-flash-realtime`
- 未确认 — `qwen3.5-omni-plus-realtime`
- 未确认 — `qwen3.6-35b-a3b`
- 未确认 — `qwen3.7-max-preview`
- 未确认 — `wan2.7-image`

### DeepSeek（13）

- 2026-07-31 — `deepseek-v4-flash-0731`
- 2026-04-23 — `deepseek-v4-pro`
- 未确认 — `deepseek-r1`
- 未确认 — `deepseek-r1-distill-llama-70b`
- 未确认 — `deepseek-r1-distill-llama-8b`
- 未确认 — `deepseek-r1-distill-qwen-1.5b`
- 未确认 — `deepseek-r1-distill-qwen-14b`
- 未确认 — `deepseek-r1-distill-qwen-32b`
- 未确认 — `deepseek-r1-distill-qwen-7b`
- 未确认 — `deepseek-v3`
- 未确认 — `deepseek-v3.1`
- 未确认 — `deepseek-v3.2`
- 未确认 — `deepseek-v4-flash`

### 智谱 AI / GLM（5）

- 2026-07-09 — `glm-5.2-fast-preview`
- 2026-06-16 — `glm-5.2`
- 2026-04-06 — `glm-5.1`
- 2026-02-11 — `glm-5`
- 2025-12-22 — `glm-4.7`

### 月之暗面 / Kimi（4）

- 2026-06-12 — `kimi-k2.7-code`
- 2026-04-20 — `kimi-k2.6`
- 2026-01-27 — `kimi-k2.5`
- 未确认 — `kimi-k2-thinking`

### MiniMax（2）

- 2026-02-11 — `MiniMax-M2.5`
- 未确认 — `MiniMax-M2.1`

### 阿里云 / FunASR（1）

- 2026-06-15 — `fun-asr-flash-2026-06-15`

### 百炼平台 / 测试条目（2）

- 未确认 — `sre-gpu-auto-handle`
- 未确认 — `test-sre-gpu-auto-handle`

## 第三方转发（带前缀，26）

### DeepSeek（9）

- 2026-01-28 — `siliconflow/deepseek-v3.2`
- 2025-05-28 — `siliconflow/deepseek-r1-0528`
- 2025-03-24 — `siliconflow/deepseek-v3-0324`
- 未确认 — `siliconflow/deepseek-v3.1-terminus`
- 未确认 — `vanchin/deepseek-ocr`
- 未确认 — `vanchin/deepseek-r1`
- 未确认 — `vanchin/deepseek-v3`
- 未确认 — `vanchin/deepseek-v3.1-terminus`
- 未确认 — `vanchin/deepseek-v3.2-think`

### 月之暗面 / Kimi（5）

- 2026-07-17 — `kimi/kimi-k3`
- 2026-06-12 — `kimi/kimi-k2.7-code-highspeed`
- 2026-06-12 — `kimi/kimi-k2.7-code`
- 2026-04-20 — `kimi/kimi-k2.6`
- 2026-01-27 — `kimi/kimi-k2.5`

### MiniMax（8）

- 2026-05-31 — `MiniMax/MiniMax-M3`
- 2026-03-19 — `MiniMax/speech-2.8-hd`
- 2026-02-11 — `MiniMax/MiniMax-M2.5`
- 未确认 — `MiniMax/MiniMax-M2.1`
- 未确认 — `MiniMax/MiniMax-M2.7`
- 未确认 — `MiniMax/speech-02-hd`
- 未确认 — `MiniMax/speech-02-turbo`
- 未确认 — `MiniMax/speech-2.8-turbo`

### 小米 / MiMo（1）

- 2026-04-22 — `xiaomi/mimo-v2.5-pro`

### 智谱 AI / GLM（3）

- 2026-06-16 — `ZHIPU/GLM-5.2`
- 2026-04-06 — `ZHIPU/GLM-5.1`
- 2026-02-11 — `ZHIPU/GLM-5`

## 研发者与前缀说明

| 研发者/家族 | 本次列表中的表现 | 说明 |
|---|---|---|
| 阿里云 / Qwen、Tongyi、Wan 与 GUI | qwen*、codeqwen*、qvq*、qwq*、gui-plus、wan*、z-image*、tongyi-xiaomi-analysis* | 依据模型家族名称和公开目录描述归纳为 Alibaba/Qwen 生态；其中非 Qwen 名称的归属仍应以官方模型页为准。 |
| DeepSeek | deepseek-*、siliconflow/deepseek-*、vanchin/deepseek-* | 第三方条目保留 siliconflow 或 vanchin 转发前缀，研发者按 slash 后的 DeepSeek 家族归组。 |
| 智谱 AI / GLM | glm-*、ZHIPU/GLM-* | ZHIPU 是返回 ID 中的转发前缀，模型家族归入 GLM。 |
| 月之暗面 / Kimi | kimi-*、kimi/kimi-* | kimi 前缀条目按 Kimi 家族归入 Moonshot。 |
| MiniMax | MiniMax-*、MiniMax/MiniMax-*、MiniMax/speech-* | 无前缀和带 MiniMax 前缀的条目统一按 MiniMax 家族归组。 |
| 小米 / MiMo | xiaomi/mimo-* | 当前列表中以第三方前缀形式出现。 |
| 阿里云 / FunASR | fun-asr-* | 按 FunASR 家族名称归纳；本次没有把音频能力等同于已验证的 API 能力。 |
| 百炼平台 / 测试条目 | sre-gpu-auto-handle、test-sre-gpu-auto-handle | 名称表现为平台或测试条目，未归入具体模型研发者。 |

## 证据边界

- 本文记录一次 Models 列表响应中的 ID、按 / 的一级分类、按模型家族的二级归纳和可追溯的日期排序；没有对每个 ID 发起 Chat 或其它模型能力请求。
- “无前缀 = 百炼原生”和“带前缀 = 第三方转发”是本次任务指定的命名约定；真实供应商归属、转发链路和授权边界不能仅由 ID 证明。
- OpenRouter 只作为公开目录辅助参考；OpenRouter 的目录、价格、可用 Provider 与 Bailian 当前账号的 Models 响应不是同一数据源。
- 列表中出现图像、音频、实时、Embedding 或其它模型 ID，不代表当前兼容入口、当前 key 或项目已验证对应接口能力。

## 参考来源

- OpenRouter Models API: https://openrouter.ai/api/v1/models
- OpenRouter Models API 字段说明: https://openrouter.ai/docs/api/api-reference/models/list-all-models-and-their-properties
- OpenRouter Qwen3.7 Plus model page: https://openrouter.ai/qwen/qwen3.7-plus/uptime
- OpenRouter Qwen3.5 Plus 2026-04-20 model page: https://openrouter.ai/qwen/qwen3.5-plus-20260420
- OpenRouter Qwen3.6 Max Preview model page: https://openrouter.ai/qwen/qwen3.6-max-preview
