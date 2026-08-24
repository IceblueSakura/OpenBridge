# Xiaomi MiMo 图片协议与固定观察

- Last reverified：外部来源与固定观察最后复核 2026-08-07；2026-08-24 仅整理本地文档，未刷新外部来源或重跑请求。
- Recheck trigger：图片 content part、Chat/Responses 支持、格式/大小限制或媒体 endpoint 变化。

## 来源与范围

- [Chat Completions API](https://mimo.mi.com/docs/zh-CN/api/chat/openai-api)
- [Responses API](https://mimo.mi.com/docs/zh-CN/api/chat/responses)
- [图片理解](https://mimo.mi.com/docs/zh-CN/quick-start/usage-guide/multimodal-understanding/image-understanding)

本文只保留图片 request wire 与 2026-08-07 的脱敏观察；支持模型、格式、尺寸和限制以官方文档为准。

## 协议与观察

官方 Chat 示例使用 user content 中的 `image_url`，来源可为公网 URL 或 data URL。

2026-08-07 使用受控账号和内存生成的 PNG data URL 直连 Chat Completions 与 Responses，两条路径均返回 HTTP 200 和可见图片语义文本。测试没有保存 credential、请求正文、图片或模型输出原文。

## 证据边界

观察只覆盖一个账号、PNG data URL、remote URL 和一次网络状态；不证明其他格式、尺寸、`detail`、`file_id`、SDK、负载、长期运行或未来 Provider 状态。OpenBridge 当前映射见[Model 与 Provider 映射](../../implementation-status/model-provider-mapping.md)。
