# OpenRouter 接入进度与边界

注册与能力事实见 `src/providers/openrouter/`；当前接线见[映射](../model-provider-mapping.md)。

## 当前边界

- MiniMax 图片输入没有模型级真实 Provider 证据；Gemma 的历史 probe 只证明单张 PNG data URL，不能支撑共享 profile 的 JPEG、
  remote URL、4-part 与大小上限；两者当前 executable interface 均保持 text-only。
- Muse Spark 1.2 Contributor 的文本 Chat/Responses 与 Hermes `obc`/`obr` 已真实 probe；图片、音频、视频与文件输入尚未 probe，
  当前 executable interface 保持 text-only。
- GLM-5.3-Flash 已验证 Chat/Responses streaming、non-streaming、PNG data URL、Auto function tool、parallel 请求开关与
  Hermes `obc`/`obr`；named tool choice 和 Responses structured output 经 probe 后显式不公开。
- GLM 的 OpenRouter file input、remote image/JPEG、video、更多图片数量/大小、长上下文、负载和长期运行仍未证明。
- Gemini/Grok file/audio/video、Grok 小图尺寸边界、更多图片格式/数量/大小、强制 DeepSeek fallback、Gemma reasoning、
  MiniMax/NVIDIA failover、Provider routing 偏好、外部 SDK/Agent、负载与长期运行未证明；公开目录字段不自动成为 executable capability。

## 验证与证据

- [2026-08-27 OpenRouter GLM-5.3-Flash 接入验证](../evidence/2026-08-27-openrouter-glm-5-3-flash-integration.md)
- [2026-08-10 OpenRouter Gemma strict schema 差异](../evidence/2026-08-10-openrouter-gemma-strict-schema-mismatch.md)
- 官方模型事实来源见 [references/providers/openrouter-api.md](../../references/providers/openrouter-api.md)。

## 代码 owner

`src/providers/openrouter/`。
