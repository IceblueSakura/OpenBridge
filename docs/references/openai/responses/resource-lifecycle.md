# OpenAI Responses resource lifecycle 调研

## 来源、范围与快照

本文只记录 Responses create 之外围绕 response identity 的 background、retrieve、cancel、input-items、compaction 与 input-token
operation。它们共享 Responses resource/state 边界，但 request method 与成功体必须逐 operation 保持。

- 官方来源：[Responses API](https://developers.openai.com/api/reference/resources/responses)、[Background mode](https://platform.openai.com/docs/guides/background)
- 协议复核日期：2026-08-03；本次结构整理未重新在线复核 endpoint beta 状态或 retention。

## 1. Operation map

| Method | Path                                     | 语义                                      |
|--------|------------------------------------------|-------------------------------------------|
| `POST` | `/v1/responses`                          | 创建同步、流式或 background response      |
| `GET`  | `/v1/responses/{response_id}`            | 读取已存储 response/resource 状态         |
| `POST` | `/v1/responses/{response_id}/cancel`     | 取消可取消 response                       |
| `GET`  | `/v1/responses/{response_id}/input_items`| 分页读取关联 input items                  |
| `POST` | `/v1/responses/compact`                  | 显式上下文压缩                            |
| `POST` | `/v1/responses/input_tokens`             | 计算或预检 input-token 信息               |

该表只做路径导航；每个 operation 的 request/response schema 仍应以当期 API Reference 为准，不能从 create response 推断。

## 2. Background

当前快照的 background guide 要求相应服务端存储行为；background 与 `store`、retrieve/cancel 和轮询共同构成资源生命周期。仅接受
`background: true` 却不提供 resource ownership，会产生不可兑现的 response id。

## 3. Retrieve、cancel 与 input items

retrieve 读取既有 resource；cancel 是有副作用的 operation；input-items 是分页 resource view。它们对不存在、过期、无权限、已
terminal 或不可取消 resource 的错误语义不能合并成普通 create error。

## 4. Compaction 与 input tokens

compaction 改变后续上下文表示，input-tokens 则是计算/预检 operation。两者都不是普通 response generation，也不能由本地文本长度
估算冒充官方 token contract。

## 5. 证据边界

- 仅实现 `POST /responses` 不等于实现完整 Responses resource API；
- 返回看似合法的 `resp_*` 字符串不证明 storage、retrieve、cancel 或 restart recovery；
- mock lifecycle 不证明真实 retention、background duration、并发取消或长期运行。
