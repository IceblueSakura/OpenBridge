# LongCat 接入进度与边界

注册与能力事实见 `src/providers/longcat/`；当前接线见[映射](../model-provider-mapping.md)。

## 当前边界

- 已注册 Chat + Responses 双 Native（含双向 Bridge）：function tools 全 choice 模式 + parallel + strict schema、
  JSON Object + JSON Schema(strict)，Responses 公开 `reasoning.encrypted_content` include；能力来自 2026-08-10
  有界 probe 与 OpenAI-compatible convention，不证明推理质量或长期稳定。
- 强制 Bridge/fallback、外部 SDK/Agent、负载与长期运行未证明。

## 验证与证据

- 官方模型事实来源见 [references/providers/longcat-api.md](../../references/providers/longcat-api.md)。

## 代码 owner

`src/providers/longcat/`。
