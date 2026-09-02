# Kimi CN 接入进度与边界

注册与能力事实见 `src/providers/kimi_cn/`；当前接线见[映射](../model-provider-mapping.md)。

## 当前边界

- 已注册 Chat-only Target：function tools 全 choice 模式 + parallel + strict schema、JSON Object + JSON Schema(strict)、
  `prompt_cache_key` 与图片输入；图片能力来自 2026-08-10 有界 probe（单张 PNG data URL 实测，JPEG 仅按
  OpenAI-compatible 惯例声明，remote URL ≤ 8192 字符，inline ≤ 20 MiB encoded / 15 MiB decoded，最多 4 张），
  不证明多图、视觉质量或长期稳定。
- `logprobs`、`n`、`top_logprobs` 在该 Target 禁用；四个常规采样参数按 ignored-parameter 合同静默忽略。
- 其他 Moonshot endpoint、原生 Responses、更多参数组合、账号权限、外部 SDK/Agent、负载与长期运行未证明。
- 历史 `none` 结果不证明当前可关闭 reasoning。

## 验证与证据

- 官方模型事实来源见 [references/providers/kimi-api.md](../../references/providers/kimi-api.md)。

## 代码 owner

`src/providers/kimi_cn/`。
