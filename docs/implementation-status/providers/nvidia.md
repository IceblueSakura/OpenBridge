# NVIDIA 接入进度与边界

注册与能力事实见 `src/providers/nvidia/`；当前 Target 与 Public Model 关系见[映射](../model-provider-mapping.md)。

## 特有接线与例外

- 已注册 Chat-only MiniMax Target：function tools 全 choice、parallel、strict schema、JSON Object、JSON Schema(strict) 与图片输入。
- 能力来自 2026-08-10 有界 probe 与 OpenAI-compatible convention：单张 PNG data URL 实测，JPEG 按惯例声明；不等于 MiniMax-M3 的图片质量、structured output 或真实推理验收。
- Nemotron 3 Embed 1B 已注册 Embeddings Native；语义质量、MiniMax fallback、其他账号/区域、配额、负载和长期运行未验证。

## 验证与证据入口

- [全模型接入静态审计](../evidence/2026-08-25-model-integration-static-audit.md)（包含 2026-08-10 有界 probe 的注册依据）。
- 官方模型事实来源见 [references/providers/nvidia-api.md](../../references/providers/nvidia-api.md)。

## 代码 owner

`src/providers/nvidia/`。
