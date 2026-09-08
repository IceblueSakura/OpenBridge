# Kimi CN 接入进度与边界

注册与能力事实见 `src/providers/kimi_cn/`；当前 Target 与 Public Model 关系见[映射](../model-provider-mapping.md)。

## 特有接线与例外

- 当前只有 Chat-only Target：function tools 全 choice 模式、parallel、strict schema、JSON Object、JSON Schema(strict)、`prompt_cache_key` 和图片输入。
- 图片能力来自 2026-08-10 有界 probe：单张 PNG data URL 实测；JPEG 按 OpenAI-compatible convention 声明；remote URL ≤ 8192 字符，inline ≤ 20 MiB encoded / 15 MiB decoded，最多 4 张。这不证明多图、视觉质量或长期稳定性。
- `logprobs`、`n`、`top_logprobs` 禁用；四个常规采样参数按 ignored-parameter 合同静默忽略。其他 Moonshot endpoint、原生 Responses、更多参数组合与账号权限未验证。
- 历史 `none` 结果不证明当前可关闭 reasoning。

## 验证入口

- 官方模型事实来源见 [references/providers/kimi-api.md](../../references/providers/kimi-api.md)。图片与工具的 probe 只用于上述 target-scoped 收窄，不构成独立长期 Provider 验收。

## 代码 owner

`src/providers/kimi_cn/`。
