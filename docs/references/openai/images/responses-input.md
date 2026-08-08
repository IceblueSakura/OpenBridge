# OpenAI Responses 图片输入调研

## 来源、范围与快照

本文只记录 Responses message content 中的 `input_image` part。Chat `image_url`、Images API 和 hosted image-generation tool 分别由
其他文档维护。

- 官方来源：[Images and vision](https://developers.openai.com/api/docs/guides/images-vision)、[Create a response](https://developers.openai.com/api/reference/resources/responses/methods/create)
- 原始资料复核日期：2026-08-04；本次结构整理未重新在线复核 source union、detail 或 model capability。

## 1. Wire position

`input_image` 位于 Responses ordered item/content 结构中。part type、item/content 顺序、source one-of 与可选 `detail` 都是 wire
语义；不能投影成普通 `input_text`。

## 2. Source

资料快照覆盖 remote URL、data URL 与 hosted `file_id` 等 source：

- remote URL 由上游 fetch 时，redirect、网络范围、最终 media type、大小与时限不由 JSON schema 保证；
- data URL 同时占用 encoded request budget 与 decoded media budget；
- `file_id` 绑定签发服务、账户/项目、权限和 resource lifecycle，不能跨 Provider 猜测迁移。

source union 与 `detail` domain 必须按当期 schema 和目标 model/profile 复核。

## 3. 与其他 operation 的边界

- Chat 图片输入见 [Chat 图片输入](chat-input.md)，两者不是字段级同构；
- Images generation/edit/variation 不由 `input_image` capability 推导；
- hosted image generation 是 Responses tool operation，见 [Responses hosted image generation](responses-hosted-generation.md)。

## 4. 安全与证据边界

- file id、signed URL、Base64 与图片内容可能敏感；
- 一个 source 成功不证明其他 source、detail、format、state affinity 或真实 Provider fetch；
- fixture 只证明被覆盖的 JSON shape。
