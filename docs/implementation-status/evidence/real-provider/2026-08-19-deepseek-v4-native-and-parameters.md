# 2026-08-19 DeepSeek V4 Native 与参数组合探测

本文是一次不可变的真实 Provider 记录，只描述 2026-08-19 的定向请求结果，不代表其他账户或未来行为。

## 执行边界

- 时间：2026-08-19 约 23:05–23:18（UTC+08:00）。
- checkout：`main@60c7bf9`，叠加 DeepSeek 实现与文档变更以及无关的执行前设计 worktree 改动。
- endpoint：`https://api.deepseek.com/chat/completions` 与 `https://api.deepseek.com/responses`。
- 认证：读取本地私有 `deepseek-primary` credential，仅在进程内构造 Bearer header；未输出或保存 credential。
- 客户端：Python 标准库 `urllib`；非流式、无状态、合成短文本请求。
- 记录仅包含 HTTP 状态、响应 object/status、终止原因、内容存在性和结构校验布尔值；未保存正文、reasoning、账户或 Provider request ID。

## 基础可用性

| Model | Chat | Responses |
|---|---|---|
| `deepseek-v4-flash` | HTTP 200，`chat.completion`，非空正文 | HTTP 200，`response`，非空正文 |
| `deepseek-v4-pro` | HTTP 200，`chat.completion`，非空正文 | HTTP 200，`response`，非空正文 |

Pro Chat 首次使用 16-token 输出上限时，reasoning 消耗预算且最终正文为空；提高到 256 后以 `finish_reason=stop` 返回非空正文。这说明输出预算同时约束 reasoning 与最终文本，不能把首次空正文判作接口不可用。

## 参数组合结果

每个模型均执行以下四个组合，共 8 个请求，全部 HTTP 200：

| 组合 | 请求边界 | 可观察校验 |
|---|---|---|
| Chat options | thinking disabled、`max_tokens`、`temperature`、`top_p`、`stop`、`logprobs`、`top_logprobs` | `finish_reason=stop`、正文非空、logprobs 存在 |
| Chat strict function | named function choice、`strict:true`、required integer schema | `finish_reason=tool_calls`、函数被调用、arguments 通过本地 schema 检查 |
| Responses options | reasoning none、`max_output_tokens`、`temperature`、`top_p`、`top_logprobs`、`user`、strict JSON Schema | `status=completed`、输出 JSON 通过本地 schema 检查、logprobs 存在 |
| Responses strict function | named function choice、`strict:true`、required integer schema | `status=completed`、函数被调用、arguments 通过本地 schema 检查 |

## 与官方文档结合后的边界

- 两模型当前都被官方 Chat/Responses reference 列为合法 model；2026-08-13 更新日志也宣布 Pro Responses Native。
- Chat penalties 已被官方标为 deprecated 且无效；Responses 不列出 penalties、`stop` 或 `logprobs`。HTTP 200 不能证明被静默忽略的字段产生效果。
- DeepSeek Responses 是无状态接口，官方不支持 `include`、`prompt_cache_key`、continuation、conversation、store 或 background。
- Chat 只确认 JSON object 响应格式；Responses 确认 JSON object 与 JSON Schema。
- 单次合规 tool arguments 证明固定 payload 可执行，不证明所有 schema、工具质量、并行调用或长期严格约束。
- Pro `reasoning_effort=low` 的官方 reference、thinking guide 与更新日志互相冲突，本次没有做可区分 low/high 的行为测量。

## 未证明范围

未执行 SSE 参数组合、server-side web search、custom `apply_patch`、图片/文件、状态续接、强制 Bailian/OpenRouter fallback、外部 SDK/Agent、并发、负载、长时间运行或生产验收。
