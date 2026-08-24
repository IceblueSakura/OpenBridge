# OpenAI Responses hosted image generation 调研

## 来源、范围与快照

本文只记录 Responses 中由服务执行的 image-generation hosted tool 边界。它不是 Images API create endpoint，也不是
`input_image` 图片理解。

- 官方来源：[Image generation](https://developers.openai.com/api/docs/guides/image-generation)、[Create a response](https://developers.openai.com/api/reference/resources/responses/methods/create)、[Responses streaming events](https://platform.openai.com/docs/api-reference/responses-streaming)
- 原始资料复核日期：2026-08-03 至 2026-08-04；本次结构整理未重新在线复核 tool schema 或 event catalog。

## 1. Tool ownership

hosted image generation 由服务执行。request tool declaration、output call/result item、include 选项与相关 stream event 都属于 Responses
item lifecycle；不能降级为 client function tool。

## 2. Streaming 与 result

image generation progress/partial output event 需要在对应 output item 下组装。item done 不等于整个 response completed；核心 terminal
规则见 [Responses SSE](responses-streaming.md)。

媒体结果的 URL/Base64、大小、format 与 retention 仍需按 tool/model profile 确认，不能从 Images API response 机械继承。

## 3. 证据边界

- hosted tool、Images API 与 `input_image` 是三种独立 operation；
- 单个 tool call 不证明 SSE partial event、retry、费用或全部 model capability；
- Provider 私有 image tool 不能自动写成 OpenAI Responses 标准。
