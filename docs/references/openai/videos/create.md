# OpenAI Videos Create 调研

## 来源、范围与快照

本文只记录 Videos generation create operation 的 request 与初始 async resource。Poll、download、cancel/delete 由 lifecycle 文档
维护。

- 官方来源：[Video generation](https://developers.openai.com/api/docs/guides/video-generation)、[Videos API](https://developers.openai.com/api/reference/resources/videos)
- 原始资料复核日期：2026-08-04；本次结构整理未重新在线复核 model、duration、size、format 或 beta 状态。

## 1. Create request

create 可按当期 endpoint/profile 使用 JSON 或 multipart，并可能引用 prompt、input image 或其他 profile-specific source。model、
duration、size、format、edit/remix/extension 等字段不能从一个 endpoint/model 推断为整个 Videos family 的共同能力。

SDK helper 不能替代实际 JSON/multipart method、field、filename、content type 与 bytes contract。

## 2. Initial resource

success 创建 video job/resource，并返回 opaque id 与初始 status；它通常不是最终 video bytes。id 绑定原服务、账户/项目和生成状态，
不是通用 media URL。

## 3. Retry 与数据边界

create 可能已经排队、计费或生成 resource。transport 结果不确定时不能盲目重复提交；prompt、input media、resource id 与 metadata
也可能敏感。

## 4. 证据边界

- create success 不证明 completed、download、cancel/delete 或媒体有效；
- 一个 model/size sample 不证明 edit/remix/extension 或全部 format；
- mock create 不证明真实生成耗时、费用或容量。
