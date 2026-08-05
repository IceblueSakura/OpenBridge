# OpenAI Videos API 协议调研

## 1. 异步资源模型

Video generation 是异步 resource workflow，不是一次 JSON response 即返回最终媒体。典型流程包含
create、retrieve/poll、list、download content 与 delete/cancel 类操作，具体 method 以当期 API 为准。

资料：[Video generation](https://developers.openai.com/api/docs/guides/video-generation)、[Videos API](https://developers.openai.com/api/reference/resources/videos)。

## 2. Identity 与状态

- create 返回 video job/resource id；
- status 可能经历 queued、processing、completed 或 failed；
- completed 后通过独立 content/download method 获取媒体；
- model、duration、size、format、input image 等字段受 profile 限制。

## 3. 边界

- resource id 与生成状态绑定原账户和服务；不能当成跨 Provider 通用 media URL。
- create 可能已产生费用或任务，网络结果不确定时不能盲目重复提交。
- polling 要有 interval、timeout、cancel 和 terminal error 语义。
- video bytes、预览 URL、签名 query 与 retention 都需要独立大小/隐私/生命周期处理。

