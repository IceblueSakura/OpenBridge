# Xiaomi MiMo 图片理解协议与真实观察（复核于 2026-08-07）

## 来源与范围

本文只记录 MiMo 图片理解的官方声明与脱敏真实 wire 观察，不包含音频、文本模型的一般能力或 OpenBridge 目标设计。

- [Chat Completions API](https://mimo.mi.com/docs/zh-CN/api/chat/openai-api)
- [Responses API](https://mimo.mi.com/docs/zh-CN/api/chat/responses)
- [图片理解](https://mimo.mi.com/docs/zh-CN/quick-start/usage-guide/multimodal-understanding/image-understanding)

## 官方事实

- 图片理解页明确只有 `mimo-v2.5` 支持图片；公开示例使用 Chat user content 中的 `image_url`。
- 图片来源可为公网 URL 或 `data:{MIME_TYPE};base64,<payload>`。
- 允许格式为 JPEG、PNG、GIF、WebP、BMP；URL 文件与 Base64 字符串的单图上限均为 50 MB，多图总量还受模型上下文约束。
- 当前不支持本地图片文件上传。
- 图片理解页没有声明显式 `detail` domain；Responses API 页也没有明确列出 `input_image`，不能仅凭文档推导这两个能力。

## 脱敏真实 wire 观察

2026-08-07 使用已配置的 `mimo-primary` 账号和内存生成的 PNG data URL，分别直连 Chat Completions 与 Responses：

- Chat 返回 HTTP 200、`chat.completion`、`mimo-v2.5`，并产生可见图片语义文本和 `image_tokens`；
- Responses 返回 HTTP 200、完成态 `response`、`mimo-v2.5`，并产生可见图片语义文本；
- 测试没有记录 credential、请求正文或模型原文。

同日又经本地 OpenBridge 独立端口复测 PNG data URL 与官方示例公网 PNG URL；Chat/Responses 四个请求均返回 HTTP 200、正确
object/model 和非空图片语义文本，其中内存红蓝图的两个结果都命中预期颜色。该结果证明当次 OpenBridge Native 路径与 MiMo
remote/data source 可用。

## 证据边界

观察只覆盖一个账号、PNG data/remote URL 和一次网络状态；它不把未探测的 `detail`、`file_id`、全部格式/尺寸、SDK、负载、长期
运行或未来 Provider 状态提升为已确认能力。
