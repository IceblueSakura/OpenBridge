# Xiaomi MiMo 协议与图片理解快照（复核于 2026-08-07）

## 来源与范围

本文记录 Xiaomi MiMo 官方文档中的 Chat/Responses endpoint、认证与图片限制，并把官方声明和 2026-08-07 的脱敏真实 wire
观察分开。它不替代 OpenBridge 当前实现状态。

- [Chat Completions API](https://mimo.mi.com/docs/zh-CN/api/chat/openai-api)
- [Responses API](https://mimo.mi.com/docs/zh-CN/api/chat/responses)
- [图片理解](https://mimo.mi.com/docs/zh-CN/quick-start/usage-guide/multimodal-understanding/image-understanding)
- [Models list](https://mimo.mi.com/docs/zh-CN/api/model/list-models)

## 观察事实

- Chat Completions 请求地址为 `https://api.xiaomimimo.com/v1/chat/completions`。
- Responses 请求地址为 `https://api.xiaomimimo.com/v1/responses`。
- 两份文档允许 `api-key` 或 `Authorization: Bearer` 认证。
- Responses 文档明确不支持 `background` 与 `previous_response_id`。
- 模型列表包含 `mimo-v2.5-pro` 与 `mimo-v2.5`，另有不属于本文文本生成范围的 ASR/TTS 变体。
- 图片理解页明确只有 `mimo-v2.5` 支持图片；公开示例使用 Chat user content 中的 `image_url`，来源可为公网 URL 或
  `data:{MIME_TYPE};base64,<payload>`。
- 允许格式为 JPEG、PNG、GIF、WebP、BMP；URL 文件与 Base64 字符串的单图上限均为 50 MB，多图总量还受模型上下文约束；
  当前不支持本地图片文件上传。
- 图片理解页没有声明显式 `detail` domain；Responses API 页也没有明确列出 `input_image`。因此这两个能力不能仅凭文档推导。

## 脱敏真实 wire 观察

2026-08-07 使用已配置的 `mimo-primary` 账号和内存生成的 PNG data URL，分别直连 Chat Completions 与 Responses：

- Chat 返回 HTTP 200、`chat.completion`、`mimo-v2.5`，并产生可见的图片语义文本和 `image_tokens`；
- Responses 返回 HTTP 200、完成态 `response`、`mimo-v2.5`，并产生可见的图片语义文本；
- 测试没有记录 credential、请求正文或模型原文。

同日又经本地 OpenBridge 独立端口复测 PNG data URL 与官方示例公网 PNG URL；Chat/Responses 四个请求均返回 HTTP 200、正确
object/model 和非空图片语义文本，其中内存红蓝图的两个结果都命中预期颜色。该结果同时证明当次 OpenBridge Native 路径与 MiMo
remote/data source 可用。

该观察补足了 Responses 图片 wire 的当次可用性，但只覆盖一个账号、PNG data/remote URL 和一次网络状态；它不把未探测的
`detail`、`file_id`、全部格式/尺寸边界或未来服务行为提升为已确认能力。

## 证据边界

官方文档、一次真实调用和本地确定性测试是不同证据层。endpoint/字段说明不能单独证明完整 wire 行为；当次真实成功也不证明
其他账号、格式、尺寸、SDK、负载、长期运行或未来 Provider 状态。
