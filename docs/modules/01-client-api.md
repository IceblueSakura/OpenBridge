# M01 客户端 API

## 对外接口

| Endpoint | 用途 | 当前状态 |
|---|---|---|
| `GET /healthz` | 健康状态和配置版本 | 已实现 |
| `GET /v1/models` | 返回 public model aliases | 已实现 |
| `POST /v1/chat/completions` | Chat JSON/SSE | 已实现原生转发 |
| `POST /v1/responses` | Responses JSON/SSE | 已实现原生转发 |

业务接口使用静态 Bearer token。未知模型、无兼容 candidate、无效 JSON 或 capability 不支持时，应在调用上游前返回明确错误。

## 目标客户端

### Codex

- P0 使用独立 custom Provider id；
- `wire_api = "responses"`；
- `supports_websockets = false`；
- 验证 HTTP/SSE 文本、工具、usage、reasoning、错误和取消。

### Hermes Agent

- P0 验证 Chat transport；
- P1 验证 Responses transport；
- 验证流式工具参数、并行工具、tool result replay、Provider 切换和辅助任务。

## 验收

- 固定 Codex/Hermes 版本、平台和配置；
- 两个 P0 Native Path 完成真实多轮 tool loop；
- 成功、错误、EOF、partial stream 和 cancel 有脱敏 fixture；
- 客户端升级后重跑兼容 corpus。

## 详细资料

- [目标客户端契约](../design/target-client-contracts.md)
- [当前实现](../implementation/current-implementation.md)
- [OpenAI 协议规范索引](../specifications/openai/api-specification-catalog.md)
