# OpenAI Videos Create 调研

## 来源、范围与快照

本文只记录 Videos generation create operation 的 request 与初始 async resource。List、poll、download 与 delete 由 lifecycle 文档
维护。

- 官方来源：[Video generation](https://developers.openai.com/api/docs/guides/video-generation)、[Create video](https://developers.openai.com/api/reference/resources/videos/methods/create)；
- 官方资料复核日期：2026-08-10；动态 model、duration、size、format、limit 与 availability 使用前仍须重核；
- **弃用边界：**官方指南已将 Sora 2 Videos API 及其 models 标为 deprecated，并计划于 **2026-09-24** 关闭。

## 1. Create request

`POST /v1/videos` 创建异步 Video job。纯 JSON request 可以引用 prompt 以及 `file_id`/`image_url` 形式的 `input_reference`；需要直接
上传二进制 reference asset 时使用 multipart。两种 encoding 是同一 create operation 的不同 wire contract，不能互换字段和
content type。

model、duration、size、format 等字段必须按目标 model/profile 验证。Edit、extension 与 remix 是返回新 Video resource 的独立
operation，见[派生 Video jobs](videos-derived-jobs.md)；character identity 见[Video characters](videos-characters.md)。

SDK helper 不能替代实际 JSON/multipart method、field、filename、content type 与 bytes contract。

## 2. Initial resource

success 返回新的异步 Video resource 与 opaque id，而不是最终 video bytes。id 绑定原服务、账户/项目和生成状态，
不是通用 media URL。

## 3. Retry 与数据边界

create 可能已经排队、计费或生成 resource。transport 结果不确定时不能盲目重复提交；prompt、input media、resource id 与 metadata
也可能敏感。

## 4. 证据边界

- create success 不证明 completed、download、delete 或媒体有效；
- 一个 model/size sample 不证明 edit/remix/extension 或全部 format；
- mock create 不证明真实生成耗时、费用或容量。
- 因官方已经公布关闭日期，本文不能作为 2026-09-24 之后的实现依据；届时必须重新选择并调研替代 API。
