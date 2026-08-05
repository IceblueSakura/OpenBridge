# OpenAI Images API 协议调研

## 1. Endpoint family

Images API 包含 generation、edit 与 variation 等不同 operation。generation 常见为 JSON；edit/variation 可使用 multipart file，具体 shape 由 endpoint 和 model 决定。

资料：[Image generation](https://developers.openai.com/api/docs/guides/image-generation)、[Images API](https://developers.openai.com/api/reference/resources/images)、[Create image variation](https://developers.openai.com/api/reference/resources/images/methods/create_variation)。

## 2. Request/response 维度

- prompt、model、size、quality、format、background/streaming 等字段不是所有 model 都支持；
- edit 可能包含一个或多个 image/mask file；
- response 可返回 short-lived URL 或 base64 image data；
- streaming image generation 使用自己的 progress/partial-image event，不等于 Chat/Responses text SSE。

## 3. 边界

- URL 可能含签名 query 和短 TTL，不能视为永久资源 identity。
- base64 会扩大 JSON body，decode 后还有独立 media size。
- image generation/edit 具有成本与副作用，retry 需要区分请求是否已被服务接受。
- SDK helper 的 file 参数不能替代对应 endpoint 的 wire schema；使用前需固定 SDK/API 版本。

