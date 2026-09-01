本文拥有普通 function-tool identity、生成链路的不执行边界，以及受限 Provider 私有扩展。

## 1. Function tools

对固定接口声明支持的普通 `type: "function"` tool：

- 保持请求 schema、tool choice、并行调用语义、`call_id`/`tool_call_id`、arguments 分片与 tool result 关联；
- Responses `input` 中标准 message 可以显式携带 `type: "message"`，也可以使用只含 `role` 与 `content`
  的 shorthand；Responses-to-Chat Bridge 对两种写法使用同一转换，缺少 `type` 且有额外字段的模糊对象拒绝；
- arguments 在完成前是不可信字符串，OpenBridge 不执行或授权模型返回的 tool call；
- tool call/result、`item_id`、stream output index 与 request id 是不同 identity，不得相互替代；
- Bridge 只转换两端都能完整表达的 function schema、choice、call/result identity 与 lifecycle，不能丢弃字段、
  修复 arguments 或根据 Provider 名称猜测语义。

## 2. 私有扩展

- Codex 的 `x-codex-turn-state` 与 `response.metadata` 只可在显式启用的 Codex Native Responses profile 中
  透明保留，不能进入 Bridge IR、普通 transcript、业务日志或跨 Target fallback。
- opaque continuation 或 encrypted reasoning 不是普通 text。若目标协议没有等价 wire，必须拒绝或按明确的
  无状态完成响应规则丢弃不可继续提交的 opaque 内容；不得转换成明文 reasoning。
- custom tool、hosted tool、MCP、annotation、image generation 与 Provider 私有字段都不是普通 function tool
  的别名。固定 interface 未声明支持时必须在 egress 前拒绝，不得静默删除。
- 业务请求不能通过 `extra_body`、任意 header 或未建模 tool type 绕过 Public Model 固定能力预检。

## 3. MCP 隔离

[MCP 本地服务](mcp.md)只执行静态 `hello`，与 generation function-tool wire 互不调用。MCP tool catalog 不进入
Public Model 能力；generation tool call 也不会被发送到 `/mcp` 执行。
