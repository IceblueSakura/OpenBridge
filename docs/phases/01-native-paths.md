# C1 双 Native Path

## 阶段目标

完成并验收：

```text
Codex Responses HTTP/SSE → Responses Provider
Hermes Chat → OpenAI-compatible Chat Provider
```

## 当前状态

`In progress`。原生 JSON/SSE 转发、模型改写、SSE framing、取消、错误和 SDK loopback 已实现；真实 Agent tool loop 和真实 Provider corpus 尚未完成。

## 实现范围

- stream/non-stream 文本；
- 单个与并行 function tool calls；
- arguments delta 和 tool result replay；
- usage、reasoning、terminal 和错误；
- unknown native fields/events；
- client disconnect；
- request/SSE 资源上限。

## 测试条目

| ID | 测试 |
|---|---|
| C1-01 | JSON 字段除明确改写项外保持不变 |
| C1-02 | UTF-8、CRLF、多 event、多行 `data:` 和任意 chunk 分片 |
| C1-03 | Chat `[DONE]` 与 Responses completed/failed/incomplete |
| C1-04 | EOF 不伪造成功，partial output 后不 retry |
| C1-05 | 下游取消释放上游 stream |
| C1-06 | Codex 单/并行工具多轮 E2E |
| C1-07 | Hermes Chat 单/并行工具多轮 E2E |
| C1-08 | usage、reasoning、unknown event 和 Provider error corpus |

## 退出条件

- 两个目标客户端完成真实多轮 tool loop；
- Native Path 不进入 Bridge IR；
- unknown 合法字段不因内部 schema 丢失；
- cancel、error、EOF 和 terminal 有唯一结果；
- fixture 明确记录证明与不证明。

## 关联模块

- [M01 客户端 API](../modules/01-client-api.md)
- [M04 原生转发与流式处理](../modules/04-native-forwarding-and-streaming.md)
