# MCP 本地服务

MCP endpoint 与 Chat/Responses 中的 function-tool wire 转发相互独立。它只提供显式注册的本地 tool，不把
Public Model、Provider、Target、Route 或上游 credential 暴露为 MCP tool。

## 1. Dual-era transport contract

- `/mcp` 使用与业务 API 相同的静态 Bearer 认证。所有 browser `Origin` 一律在认证与 JSON-RPC dispatch 前
  以 HTTP `403` 拒绝；本服务没有 browser Origin allowlist。
- `2026-07-28` 客户端使用无状态 `server/discover` 路径；每个 POST 请求自带完整 protocol、client 与
  capability metadata，不创建 session。
- legacy 客户端使用 `initialize`/`initialized` handshake，并由同一 `/mcp` endpoint 管理
  `Mcp-Session-Id`、GET SSE stream 与 DELETE session lifecycle。legacy session compatibility 是当前契约，
  不是非目标。
- 两种 lifecycle 都必须发现同一个静态 tool catalog，并受相同的认证、Origin、request body、request id、
  敏感 header 与终态观测边界保护。
- `server/discover` 必须声明 `tools` capability 并返回支持的 protocol version 列表；version negotiation 失败
  必须返回稳定 JSON-RPC error，不能猜测或降级到未声明协议。

## 2. Stateless metadata

`2026-07-28` 请求必须：

- 使用 `POST /mcp` 和 `application/json` body，并同时接受 `application/json` 与 `text/event-stream`；
- 携带 `MCP-Protocol-Version` 与 `Mcp-Method`，并与 JSON-RPC body 和 `_meta` 中的 protocol version、method、
  client info/capabilities 一致；
- 对 `tools/call` 携带与 body tool name 一致的 `Mcp-Name`。

缺失、畸形或不一致的 metadata 必须在 tool 执行前失败。该 header contract 不得被误用于拒绝合法 legacy
initialize/session lifecycle。

## 3. `hello` tool

- `tools/list` 按确定性顺序返回唯一的 `hello`；其 closed `inputSchema` 只接受一个必需字符串 `name`。
- 有效 `tools/call` 返回一个 text content block：`Hi, {name}!`。
- `hello` 不读取配置、registry、文件、网络或 Provider，也不产生外部 side effect。
- 无效 argument 返回 `isError: true` 的 tool result；未知 tool 返回 JSON-RPC `-32602`。
- 未实现 JSON-RPC method 返回 `-32601`；认证、Origin、session 和 transport 错误保持各自 HTTP/JSON-RPC
  边界，不能执行 tool 后再伪造失败。

## 4. 非目标

- `hello` 之外的本地 tool、动态 tool catalog、resource、prompt、notification 或业务 side effect；
- MCP-to-Provider Bridge、generation tool execution 或把 Chat/Responses tool call 交给本 endpoint 执行；
- browser Origin allowlist、远程公网部署或绕过 loopback/Bearer 边界；
- 将 stateless metadata 规则强加给 legacy session 请求，或移除当前 legacy lifecycle。

## 关联文档

- [Endpoint 与认证](endpoints-and-auth.md)
- [tool 与私有扩展](tools-and-extensions.md)
- [MCP 外部协议参考](../../references/mcp/README.md)
- [实施现状](../../implementation-status/README.md)
