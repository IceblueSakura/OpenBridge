# OpenAI Videos resource lifecycle 调研

## 来源、范围与快照

本文只记录已创建 Video resource 的 list/retrieve/poll、terminal status、content download 与 delete 边界。Create request 由独立文档
维护。

- 官方来源：[Video generation](https://developers.openai.com/api/docs/guides/video-generation)、[Videos API](https://developers.openai.com/api/reference/resources/videos)；
- 官方资料复核日期：2026-08-10；动态 status、retention、content variant 与 media format 使用前仍须重核；
- **弃用边界：**官方指南已将 Sora 2 Videos API 及其 models 标为 deprecated，并计划于 **2026-09-24** 关闭。

## 1. Resource operations

| Method   | Path                                    | 当前 operation |
|----------|-----------------------------------------|----------------|
| `GET`    | `/v1/videos`                            | 列出 Video resources |
| `GET`    | `/v1/videos/{video_id}`                 | 读取状态与 metadata |
| `GET`    | `/v1/videos/{video_id}/content`         | 下载 video、thumbnail 或 spritesheet bytes |
| `DELETE` | `/v1/videos/{video_id}`                 | 永久删除已 completed/failed 的 Video resource 及其 stored assets |

截至快照日期，当前 Videos API reference 没有列出 cancel operation。实现不能把 Responses 等其他 resource family 的 cancel method
外推到 Video；若需要停止 queued/in-progress job，必须重新核对当期 API 是否提供正式能力。

## 2. Status 与 polling

resource status 的当前 enum 是 `queued`、`in_progress`、`completed` 或 `failed`。客户端可轮询 retrieve，也可按官方 guide 使用 webhook
观察完成状态；polling 仍需明确 interval、timeout、backoff、rate limit、调用方取消等待与进程重启后的恢复。

## 3. Content download

completed 后通过独立 content operation 获取 bytes；当前 variant 包含 `video`、`thumbnail` 与 `spritesheet`。`Content-Type`、
container/codec、extension、byte limit、streaming/backpressure 与 retention 都属于 download contract，不能把资源 metadata response
当作媒体 body。

## 4. Delete

delete 是有副作用的 lifecycle operation。不同状态、不存在、过期或无权限 resource 的错误语义不能与 retrieve 合并；重复调用的
结果也必须按正式 contract 处理。

## 5. 数据与证据边界

- video bytes、preview、resource id 与状态可能敏感；
- retrieve success 不证明 download 或 delete；
- mock polling 不证明真实生成时长、长期运行、费用或 media validity。
- 因官方已经公布关闭日期，本文不能作为 2026-09-24 之后的实现依据；届时必须重新选择并调研替代 API。
