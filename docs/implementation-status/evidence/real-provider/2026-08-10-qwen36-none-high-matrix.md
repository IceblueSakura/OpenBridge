# 2026-08-10 Qwen3.6 27B `none/high` 真实 Provider 矩阵

本文是一次不可变验证记录，只描述 `qwen3.6-27b` 接入后的定向请求和结果，不代表当前 checkout 已重新验收。

## 执行边界

- checkout：`main@ed515cc` 叠加当时尚未提交的实现 worktree。
- 重新构建并启动本地 OpenBridge；扩展 Models 单模型查询返回 HTTP 200。
- 当时 Chat/Responses interface 均公开 `none/high`，reasoning output 为 `plain_text`。
- 使用真实下游用户 key 和既有 Bailian credential，对正常首选 Route 执行 Chat/Responses × JSON/SSE ×
  `none/high`。
- 只保存状态码、终态、语义布尔值和耗时；未保存正文、Provider request ID、账户或 credential。

## 结果

| Model | C none JSON | C none SSE | R none JSON | R none SSE | C high JSON | C high SSE | R high JSON | R high SSE |
|---|---|---|---|---|---|---|---|---|
| `qwen3.6-27b` | OK R- | OK R- | OK R- | OK R- | OK R+ | OK R+ | OK R+ | OK R+ |

- 8/8 均为 HTTP 200、Content-Type 正确、文字非空且协议终态完整；Chat SSE 收到 `[DONE]`，Responses SSE
  收到 `response.completed`。
- 单请求耗时为 260–2915 ms。
- 四个 `none` 单元均没有 reasoning item、可读 reasoning 或正 reasoning token 证据。
- Chat `high` JSON/SSE 均有可读 `reasoning_content` 和正 reasoning token 计数；Responses-via-Chat `high`
  JSON/SSE 均有 reasoning item 和可读 reasoning 文本，但没有 reasoning token 计数。

## 证据边界

本次没有执行外部 SDK、强制 fallback、其他 reasoning level、tools、多模态、负载、并发稳定性、长期运行或生产
验收。结果只证明当时账号、网络、Bailian endpoint、模型、OpenBridge checkout 和固定 payload。

当前实现解释见[Provider 注册](../../current-state.md#7-provider-注册摘要)。
