# OpenAI Images Edits 与 Variations 调研

## 来源、范围与快照

本文只记录 Images API edit/variation 的 multipart file request 与对应 result 边界。JSON generation 与协议内图片理解不在本文定义。

- 官方来源：[Image generation](https://developers.openai.com/api/docs/guides/image-generation)、[Images API](https://developers.openai.com/api/reference/resources/images)、[Create image variation](https://developers.openai.com/api/reference/resources/images/methods/create_variation)
- 原始资料复核日期：2026-08-04；本次结构整理未重新在线复核 operation availability、file count、mask 或 format limit。

## 1. Multipart request

edit/variation 可使用 multipart image file；edit 还可能包含 mask、prompt 或多个 image，精确 parts 由 endpoint/model profile 决定。
SDK file helper 不能替代 multipart field、filename、content type 与 bytes 的 wire contract。

## 2. Result

result 可沿用 Images family 的短期 URL 或 Base64 形状，但每个 operation/model 的字段与 format 必须单独确认。generation success
不能作为 edit/variation response 的完整证据。

## 3. Resource 与重放

上传 bytes、临时文件、mask、图片尺寸和 decode resource 需要有界处理。operation 可能已产生费用或结果；transport 不确定时不能
假设 multipart request 安全重放。

## 4. 证据边界

- edit 与 variation 也不能互相推断全部 request parts；
- 单图 sample 不证明多图、mask、透明背景、全部格式或错误路径；
- mock multipart 不证明真实 media decoder 或 model behavior。
