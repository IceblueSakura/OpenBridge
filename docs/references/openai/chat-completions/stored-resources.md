# OpenAI Stored Chat Completions resource 调研

## 来源、范围与快照

本文只记录以 `store: true` 创建后可管理的 Chat Completion resource。普通 create request、非流式 result 与 data-only SSE 分别由本目录
其他 owner 文档维护。

- 官方来源：[Chat Completions API](https://developers.openai.com/api/reference/resources/chat/subresources/completions)、
  [Retrieve](https://developers.openai.com/api/reference/resources/chat/subresources/completions/methods/retrieve)、
  [Update](https://developers.openai.com/api/reference/resources/chat/subresources/completions/methods/update)、
  [Delete](https://developers.openai.com/api/reference/resources/chat/subresources/completions/methods/delete)、
  [List](https://developers.openai.com/api/reference/resources/chat/subresources/completions/methods/list)、
  [Messages](https://developers.openai.com/api/reference/resources/chat/subresources/completions/subresources/messages/methods/list)；
- 官方资料复核日期：2026-08-10；retention、filter、pagination field 与存储资格使用前仍须重核。

## 1. Operation map

| Method | Path                                                  | 语义 |
|--------|-------------------------------------------------------|------|
| `POST` | `/v1/chat/completions`                                | 使用 `store: true` 创建可后续管理的 completion |
| `GET`  | `/v1/chat/completions`                                | 分页/筛选已存储 completions |
| `GET`  | `/v1/chat/completions/{completion_id}`                | 读取一个已存储 completion |
| `POST` | `/v1/chat/completions/{completion_id}`                | 更新已存储 completion 的 metadata |
| `DELETE` | `/v1/chat/completions/{completion_id}`              | 删除已存储 completion |
| `GET`  | `/v1/chat/completions/{completion_id}/messages`       | 分页读取该 completion 的 messages |

这些 method 围绕同一 opaque completion identity，但不是 `POST /v1/chat/completions` 的参数别名。未以可存储方式创建的普通 completion
不能仅凭返回 id 推定可以 retrieve/update/delete/list messages。

## 2. Storage、metadata 与 pagination

Update operation 当前只修改 metadata，不是重新生成 completion，也不允许把原 prompt、choices 或 model 当作可编辑字段。List 与
Messages list 使用资源分页语义；cursor、limit、order 和 filter 必须按各 operation 的当期 reference 验证，不能复用 Models list 的固定
四字段响应或无分页数组。

completion id、message identity、创建账户/项目和实际 storage issuer 共同决定资源归属。聚合层不能把后续 GET/POST/DELETE 随机发送到
另一个 Provider、region 或 credential pool member。

## 3. Error 与删除边界

不存在、未存储、已删除、过期和无权限可以在下游表现为相似错误，但在内部不能被当作可跨 route 重试的普通生成失败。Delete 成功也不
自动证明所有衍生日志、计费记录或 Provider retention 已同步清除；只能按官方 resource contract 表述。

## 4. Fake 与真实证据边界

F2 fake 至少应验证：

- 只有明确存储的 create 才进入后续 resource lifecycle；
- opaque id 固定回原 issuer/account；
- list/messages 的 cursor、顺序、空页和无效 cursor；
- metadata update 不改写生成内容；
- delete 后 retrieve/update/messages 的稳定错误；
- unauthorized 与 not-found 的安全、无拓扑泄露映射。

fake resource store 不证明真实 retention、跨进程恢复、上游数据政策、并发一致性或 Provider 实际支持 stored Chat。真实 Provider 的 create
成功也不能替代 retrieve/update/delete/list/messages 的逐 operation 探测。
