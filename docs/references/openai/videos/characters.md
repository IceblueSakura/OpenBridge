# OpenAI Video characters 调研

## 来源、范围与快照

本文只记录 Videos API 的 character create 与 retrieve resources。Video generation、edit/extension/remix 及通用 lifecycle 由其他文档
维护。

- 官方来源：[Create character](https://developers.openai.com/api/reference/resources/videos/methods/create_character)、[Retrieve character](https://developers.openai.com/api/reference/resources/videos/methods/get_character)、[Video generation guide](https://developers.openai.com/api/docs/guides/video-generation)；
- 官方资料复核日期：2026-08-10；动态 eligibility、sample、likeness、model 与 usage constraint 使用前仍须重核；
- **弃用边界：**官方指南已将 Sora 2 Videos API 及其 models 标为 deprecated，并计划于 **2026-09-24** 关闭。

## 1. Endpoint map

| Method | Path                                         | 当前 operation |
|--------|----------------------------------------------|----------------|
| `POST` | `/v1/videos/characters`                      | multipart 上传 sample video 并创建 character resource |
| `GET`  | `/v1/videos/characters/{character_id}`       | 读取一个 character resource |

character id 是独立 opaque identity，可在符合条件的 video request 中被引用；它不是 source video id、filename 或人物名称。create 返回
resource metadata，不等价于完成一个 Video generation job。

## 2. Availability 与 identity 边界

截至快照日期，当前 API Reference 明确展示 create 与 retrieve。不能据此推断 list/update/delete 等未列出的同构 CRUD。官方 guide 对
human likeness 与账户资格另有门槛；文档存在 endpoint 不证明任意账户、地区、组织、Provider 或人物样本可使用。

## 3. 安全与证据边界

- sample video、character id、预览与 likeness metadata 可能涉及身份、生物特征、授权与隐私，不能写入普通日志或 fixture；
- synthetic/mock resource 只能验证 wire shape，不能证明授权、合规性、角色一致性或真实账户 access；
- character create/retrieve success 不证明 generation、edit、retention 或删除行为；
- 因官方已经公布关闭日期，本文不能作为 2026-09-24 之后的实现依据；届时必须重新选择并调研替代 API。
