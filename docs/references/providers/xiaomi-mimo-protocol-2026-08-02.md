# Xiaomi MiMo 协议入口与文本生成快照（复核于 2026-08-08）

## 来源与范围

本文只记录 Xiaomi MiMo 的公共 API origin、Chat/Responses 入口、认证和模型目录事实。图片与音频协议按功能拆分：

- [图片理解协议与真实观察](xiaomi-mimo-image-protocol-2026-08-07.md)
- [ASR/TTS 协议与真实观察](xiaomi-mimo-audio-protocol-2026-08-08.md)

这些页面是外部 Provider 快照，不替代 OpenBridge 当前实现状态或功能需求。

- [Chat Completions API](https://mimo.mi.com/docs/zh-CN/api/chat/openai-api)
- [Responses API](https://mimo.mi.com/docs/zh-CN/api/chat/responses)
- [Models list](https://mimo.mi.com/docs/zh-CN/api/model/list-models)
- [模型下线说明](https://mimo.mi.com/docs/zh-CN/updates/deprecate)

## 观察事实

- Chat Completions 请求地址为 `https://api.xiaomimimo.com/v1/chat/completions`；Responses 请求地址为
  `https://api.xiaomimimo.com/v1/responses`。
- 两份生成协议文档允许 `api-key` 或 `Authorization: Bearer` 认证。
- Responses 文档明确不支持 `background` 与 `previous_response_id`。
- 当前 Models list 包含文本生成的 `mimo-v2.5-pro`/`mimo-v2.5`、`mimo-v2.5-asr` 与三种 TTS model ID；模型存在不自动
  证明某个 wire、参数或 OpenBridge Public Model 已可用。
- 旧 `mimo-v2-pro`、`mimo-v2-omni`、`mimo-v2-flash` 与 `mimo-v2-tts` 已于 2026-06-30 下线；新接入必须使用当前 model ID。

## 证据边界

endpoint、认证和 Models list 只证明目录/入口事实，不能推导图片、音频、tools、reasoning、streaming、Bridge 或服务限制。动态模型
目录和 Provider 行为会变化；使用前须按功能页的日期与证据层重新复核。
