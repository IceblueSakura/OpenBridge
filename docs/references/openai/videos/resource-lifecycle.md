# OpenAI Videos resource lifecycle 调研

## 来源、范围与快照

本文只记录已创建 Video resource 的 retrieve/poll、terminal status、content download 与 cancel/delete 边界。Create request 由独立文档
维护。

- 官方来源：[Video generation](https://developers.openai.com/api/docs/guides/video-generation)、[Videos API](https://developers.openai.com/api/reference/resources/videos)
- 原始资料复核日期：2026-08-04；本次结构整理未重新在线复核 method、status enum、retention 或 media format。

## 1. Status 与 polling

resource status 可经历 queued、processing、completed 或 failed；cancel/expiry 是否存在及其 terminal 语义必须按当期 API 确认。
polling 需要明确 interval、timeout、backoff、rate limit、cancel 与进程重启后的恢复。

## 2. Content download

completed 后通过独立 content/download operation 获取 binary video bytes 或短期 URL。`Content-Type`、container/codec、extension、byte
limit、streaming/backpressure、signed query 与 retention 都属于 download contract。

## 3. Cancel/delete

cancel/delete 是有副作用的 lifecycle operation。已 terminal、不存在、过期或无权限 resource 的错误语义不能与 retrieve 合并；
重复调用的结果也必须按正式 contract 处理。

## 4. 数据与证据边界

- video bytes、preview/signed URL、resource id 与状态可能敏感；
- retrieve success 不证明 download 或 cancel/delete；
- mock polling 不证明真实生成时长、长期运行、费用或 media validity。
