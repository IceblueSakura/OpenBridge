# NVIDIA 接入进度与边界

注册与能力事实见 `src/providers/nvidia/`；当前接线见[映射](../model-provider-mapping.md)。

## 当前边界

- 已注册 Chat-only MiniMax Target：function tools 全 choice 模式 + parallel + strict schema、JSON Object + JSON Schema(strict)
  与图片输入；能力来自 2026-08-10 有界 probe 与 OpenAI-compatible convention（单张 PNG data URL 实测，JPEG 仅按惯例声明），
  不等于 MiniMax-M3 自身的图片质量、structured output 行为或真实推理验证。
- Nemotron 3 Embed 1B 已注册 Embeddings Native；语义质量未验证。
- MiniMax 强制 fallback、真实 reasoning、其他账号/区域、配额、负载与长期运行未证明。

## 验证与证据

- 2026-08-10 有界 probe 支撑 generation family 的图片与工具能力注册（见 [静态审计](../evidence/2026-08-25-model-integration-static-audit.md)）。
- 官方模型事实来源见 [references/providers/nvidia-api.md](../../references/providers/nvidia-api.md)。

## 代码 owner

`src/providers/nvidia/`。
