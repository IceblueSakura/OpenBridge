# OpenRouter 接入进度与边界

注册与能力事实见 `src/providers/openrouter/`；当前 Target 与 Public Model 关系见[映射](../model-provider-mapping.md)。

## 特有接线与例外

- MiniMax 图片输入没有模型级真实 Provider 证据，executable interface 保持 text-only。
- GLM-5.3-Flash 已验证 Chat/Responses streaming、non-streaming、PNG data URL、Auto function tool、parallel 请求开关与 Hermes `obc`/`obr`；named tool choice 与 Responses structured output 不公开。
- GLM 的 file input、remote image/JPEG、video、更多图片数量/大小、长上下文未证明；Gemini/Grok file/audio/video、Grok 小图尺寸、DeepSeek fallback、MiniMax/NVIDIA failover、Provider routing 偏好和长期运行也未形成统一验收。公开目录字段不自动成为 executable capability。

## 验证与证据入口

- [2026-08-27 OpenRouter GLM-5.3-Flash 接入验证](../evidence/2026-08-27-openrouter-glm-5-3-flash-integration.md)
- 官方模型事实来源见 [references/providers/openrouter-api.md](../../references/providers/openrouter-api.md)。

## 代码 owner

`src/providers/openrouter/`。
