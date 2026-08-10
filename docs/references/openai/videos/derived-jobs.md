# OpenAI 派生 Video jobs 调研

## 来源、范围与快照

本文只记录以已有 Video resource 或上传媒体创建新异步 Video resource 的 edit、extension 与 remix operations。初始 generation、资源
查询/download/delete 与 character resource 分别由其他文档维护。

- 官方来源：[Video edits](https://developers.openai.com/api/reference/resources/videos/methods/edit)、[Video extensions](https://developers.openai.com/api/reference/resources/videos/methods/extend)、[Video remix](https://developers.openai.com/api/reference/resources/videos/methods/remix)、[Video generation guide](https://developers.openai.com/api/docs/guides/video-generation)；
- 官方资料复核日期：2026-08-10；动态 eligibility、input constraint、model 与 limit 使用前仍须重核；
- **弃用边界：**官方指南已将 Sora 2 Videos API 及其 models 标为 deprecated，并计划于 **2026-09-24** 关闭。

## 1. Endpoint map

| Method | Path                                  | 当前 operation |
|--------|---------------------------------------|----------------|
| `POST` | `/v1/videos/edits`                    | 编辑已有 Video id，或按资格上传 video，返回新 Video resource |
| `POST` | `/v1/videos/extensions`               | 延展符合条件的已完成 source Video，返回新 Video resource |
| `POST` | `/v1/videos/{video_id}/remix`         | 以 source Video 与新 prompt 创建 remix Video resource |

这些 operation 创建新的 async identity，不是对 source resource 的原地 mutation。调用方需要单独保存 source id 与 result id，并使用
[resource lifecycle](resource-lifecycle.md) 跟踪 result。

## 2. Request encoding 与选择边界

edit 可以使用引用已有 Video id 的 JSON request；直接上传 video 的 edit 使用 multipart，且官方指南把 upload edit 标为仅向符合资格的
客户开放。extension 与 remix 的 source 状态、时长、尺寸和 model 约束属于各自 contract，不能从 create 或 edit success 外推。

截至快照日期，官方 guide 已说明 remix 正在被弃用，并建议新集成优先使用 edits。这个 operation 级迁移提示不能抵消整个 Sora 2
Videos API 的 2026-09-24 关闭期限。

## 3. 安全与证据边界

- source/result id、prompt 与上传媒体可能敏感，不能写入普通日志或固定 fixture；
- edit success 不证明 extension/remix、上传资格或任意 source asset 可接受；
- mock job 只能证明 request/resource wire，不能证明媒体变换质量、生成时长、费用或真实账户 access；
- 因官方已经公布关闭日期，任何新实现都应先调研替代 API，而不是把这些 endpoints 作为长期目标。
