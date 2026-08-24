# OpenAI Responses continuation 与 state ownership 调研

## 来源、范围与快照

本文只比较 Responses 多轮上下文的三种 owner：`previous_response_id`、Conversations resource 与客户端 manual item replay。
Background/retrieve/cancel 等 resource operations 由另一文档维护。

- 官方来源：[Conversation state](https://developers.openai.com/api/docs/guides/conversation-state)、
  [Conversations API](https://developers.openai.com/api/reference/resources/conversations)、
  [Create a response](https://developers.openai.com/api/reference/resources/responses/methods/create)；
- 协议复核日期：2026-08-10；动态 TTL、retention、pagination 与 metadata limit 使用前仍须重核。

## 1. `previous_response_id`

新 request 提交 input，并指向前一 `response.id`，由服务端 response chain 提供 continuation。官方资料说明，使用该 id 不代表前序
input tokens 不再计入后续 input token 费用。

## 2. Conversations resource

`conversation` 引用长期 conversation object。其 items 会加入 request 上下文，response 完成后新的 input/output items 可写回同一
conversation。conversation identity、权限、retention 与普通 response identity 不能混用。

当前 operation map 为：

| Method | Path | 语义 |
|--------|------|------|
| `POST` | `/v1/conversations` | 创建 conversation |
| `GET`/`POST`/`DELETE` | `/v1/conversations/{conversation_id}` | retrieve、metadata update 或 delete |
| `POST`/`GET` | `/v1/conversations/{conversation_id}/items` | 创建 item 或分页列出 items |
| `GET`/`DELETE` | `/v1/conversations/{conversation_id}/items/{item_id}` | retrieve 或 delete item |

resource method 共享 identity，但各自 request/response shape、分页和错误必须按当期 API Reference 固定。它们不是 Responses create 的
本地 history helper。

## 3. Manual item replay

客户端保存输入和完整 `response.output[]`，在后续 request 的 `input[]` 中回放。该方式适合客户端拥有 state 的场景，但必须保留
message、reasoning、tool call/result 的结构和顺序。

若 reasoning continuity 需要 opaque encrypted content，应按明确 include/profile 获取并绑定兼容边界；不能当作普通文本或跨任意
Provider 重放。

## 4. 不可互换性

三种方式拥有不同 state owner、identity、retention 与迁移语义。兼容层必须选择明确 owner 或证明等价性，不能把它们压缩成一个
字符串 history。

## 5. 证据边界

- response id、conversation id、item id 与 tool call id 不能互换；
- 单轮 success 不证明 continuation、restart recovery 或跨账户迁移；
- 本文不定义 resource retrieve/cancel/compaction；见 [Responses resource lifecycle](responses-resource-lifecycle.md)。
