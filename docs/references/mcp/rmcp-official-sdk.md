# rmcp（官方 Rust SDK）实现方法调研

## 范围与证据

- 调研对象：`modelcontextprotocol/rust-sdk` 仓库 main 分支 README（2026-08-08 抓取）、crates.io 元数据（2026-08-08 查询）。
- 快照：rmcp 3.1.2 / rmcp-macros 3.1.2（crates.io updated_at 2026-08-07），累计下载 1923 万。
- 本文是**外部 SDK 事实**：描述该库提供的能力与惯用法，不构成 OpenBridge 的功能承诺。

关键结论：**rmcp 是唯一完整实现 MCP 2026-07-28 现行规范的 Rust 库**。其架构是"服务端 = 实现 `ServerHandler` trait + 选择 transport + 挂到任意 Tower router"；2026-07-28 的无状态协议（无握手、无 session、`_meta` 声明版本）是默认行为，旧版客户端自动走兼容路径。

## 1. 总体架构

```
用户代码（ServerHandler 实现 + #[tool] 宏）
        │
   rmcp 核心（JSON-RPC 编解码、协议版本协商、能力协商、MRTR、缓存）
        │
   Transport（stdio / Streamable HTTP Tower service / child-process / worker）
        │
   axum / hyper / 任意 Tower router（HTTP 场景）
```

- 服务端核心 trait 是 `ServerHandler`（含 `get_info()` 返回 `ServerInfo` + `ServerCapabilities`）；tools-only 服务器可用 `#[tool_router(server_handler)]` 跳过手写 handler 样板。
- `StreamableHttpService` 是 Tower service，`axum::Router::new().nest_service("/mcp", service)` 即可挂载。
- 生命周期：`server.serve(transport).await` 后 `waiting().await` 阻塞；`cancel()` 可停止。
- 客户端与服务器同一 crate 内（`server` / `client` feature 分开）。

## 2. 2026-07-28 协议能力（区别于 2025-11-25 的重点）

| 能力 | 实现方式 | 备注 |
|---|---|---|
| 无状态 Streamable HTTP | 默认开启：无 `Mcp-Session-Id`、无 GET/DELETE 流、无 `Last-Event-ID` | `legacy_session_mode`（原 `stateful_mode`）只影响 `< 2026-07-28` 旧客户端；`with_json_response(true)` 让简单工具返回纯 JSON 而非 SSE 流 |
| 版本协商 | `_meta` 携带 `io.modelcontextprotocol/protocolVersion`；客户端 `ClientLifecycleMode::{Initialize, Discover, Auto}` | `Auto` 自动先 `server/discover` 再按需初始化；`ProtocolVersion::V_2026_07_28` / `LATEST` / `KNOWN_VERSIONS` |
| `server/discover` | 服务端必须实现（SDK 自动处理） | 通告支持的协议版本、能力、身份 |
| `subscriptions/listen` | 服务端实现 `listen(context)` + `accepted_subscription_filter()`；客户端 `listen(filter)` 返回流 | 替代旧 `resources/subscribe`；按类别 opt-in；缓冲默认 64 条，落后报 `SubscriptionEnd::Lagged`；断连后不恢复，需重发 |
| MRTR | 服务端返回 `InputRequiredResult`（`CallToolResponse` 枚举的变体）；客户端高层 `call_tool` 自动完成多轮（上限 10） | `requestState` 视为不可信数据；`request-state` feature 提供 `RequestStateCodec`（HMAC 密封/开启） |
| Tasks 扩展 | `rmcp::task_manager::TaskManager` 管理生命周期；客户端声明 `enable_tasks()` 后服务端按请求决定是否物化为 task | 客户端轮询 `tasks/get`、`tasks/update`、`tasks/cancel` |
| 响应缓存 | 客户端透明缓存带 `ttlMs`/`cacheScope` 的响应；按 scope 分区、list_changed 通知自动失效 | `ClientCacheConfig` 可调（默认 TTL、上限、私有分区、stale-on-error） |
| 标准路由头 | 协商 ≥ 2026-07-28 后自动发出/校验 `Mcp-Method`、`Mcp-Name`、`Mcp-Param-*` | 工具 schema 顶层属性标 `x-mcp-header: "Region"` 即提升为路由头 |

## 3. 服务端实现惯用法

### 3.1 最小 tools-only 服务器（stdio）

```rust
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct AddParams { a: i32, b: i32 }

#[derive(Clone)]
struct Calculator;

#[tool_router(server_handler)]
impl Calculator {
    #[tool(description = "Add two numbers")]
    fn add(&self, Parameters(AddParams { a, b }): Parameters<AddParams>) -> String {
        (a + b).to_string()
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let server = Calculator;
    server.serve(stdio()).await?;
    server.waiting().await?;
    Ok(())
}
```

要点：
- `#[tool]` 参数必须包在 `Parameters<T>` 中；`T` 派生 `JsonSchema`（schemars），工具名/description 从宏参数来。
- `inputSchema` / `outputSchema` 由字段名、类型、doc comment 自动生成，dialect 为 JSON Schema 2020-12（规范要求）。
- 多能力（tools + prompts）或要自定义 `ServerInfo` 时用显式 `#[tool_handler]` + `impl ServerHandler`。

### 3.2 工具返回内容与错误哲学

- 返回类型：`String`（纯文本）或 `CallToolResult`（文本/图片/音频/嵌入资源混合，`ContentBlock` 枚举）。
- 两种失败模式：
  - **tool-level error**：`Ok(CallToolResult::error(vec![...]))` —— 工具执行了但结果不好，客户端渲染内容给用户看（"no rows matched"、上游 500 等）。
  - **protocol error**：`Err(McpError::invalid_params(..))` 等 —— 请求无法路由/处理，客户端不透出消息内容。
- 这一区分直接指导错误码设计：业务失败走 tool-level，协议/参数错误走 JSON-RPC 错误码。

### 3.3 多能力服务器

```rust
impl ServerHandler for MyServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder()
            .enable_tools()
            .enable_resources()
            .enable_prompts()
            .enable_tool_list_changed()
            .build())
    }
    // list_resources / read_resource / list_resource_templates / complete / set_level ...
}
```

- Resources：`ResourceContents::text(..)` / `blob(base64, uri)`；模板 `users://{user_id}/profile` 由客户端展开。
- Prompts：`#[prompt_router]` + `#[prompt]` 宏，支持 typed arguments；可返回 `Vec<PromptMessage>` / `GetPromptResult` / `Result<..>`。
- 通知：`peer.notify_tool_list_changed()` / `notify_resource_updated(..)` / `notify_progress(..)` / `notify_cancelled(..)`。
- 已废弃特性（SEP-2577，仍可用）：Sampling、Roots、Logging；SDK 保留实现并标注 Deprecated。

## 4. 传输矩阵

| Transport | Feature | 说明 |
|---|---|---|
| stdio（server + client） | `transport-io` | 本地子进程标准方式 |
| child-process（client） | `transport-child-process` | `TokioChildProcess` 启动 server 二进制 |
| Streamable HTTP（server） | `transport-streamable-http-server` | Tower service，任意 router 挂载 |
| Streamable HTTP（client） | `transport-streamable-http-client-reqwest` | 单 URI 连接，默认 `allow_stateless: true` |
| worker / in-process | `transport-worker` | 嵌入/测试无真实 I/O |

明确**不提供** legacy HTTP+SSE（2024-11-05 两端点传输）——维护方定位为"deliberate non-goal"，需要时要求对端走 Streamable HTTP 或由代理转换。

## 5. 能力协商与兼容

- 客户端/服务端在连接时交换 capabilities；协商结果决定 2026-07-28 特有行为（路由头、无状态、订阅模型）是否生效，旧客户端保持兼容。
- 无状态模式下 `service_factory` 每次请求都跑：共享状态（DB 池、缓存）放 `Clone` 句柄，**不能依赖进程内状态跨请求存活**——这是 2026-07-28 与旧 session 模型最本质的架构差异。
- `with_stateless_protocol_metadata_required(true)` 可拒绝缺少 per-request 协议信号的兼容性回退（同时应把 `supported_protocol_versions` 只通告 ≥ 2026-07-28）。

## 6. 其他事实

- Feature flags：`server`（默认）、`client`、`macros`（默认）、`schemars`、`auth`（OAuth 2.0）、`elicitation`、`request-state`、`transport-*`。
- 宏生态：`#[tool]` / `#[tool_router]` / `#[tool_handler]` / `#[prompt]` / `#[prompt_router]` / `#[prompt_handler]`。
- Elicitation：form 模式（`elicit::<T>()` + `elicit_safe!` 白名单）与 URL 模式（`elicit_url`）；2026-07-28 下由 MRTR 承载。
- 分页：`list_all_tools()` 等 helper 自动翻页；手动翻页跟 `next_cursor`。
- 扩展生态：`rmcp-actix-web`（actix 后端，GitLab lx-industries）、`rmcp-openapi`（OpenAPI → MCP tools）。
- 已知使用者：block/goose、apollo-mcp-server、containerd-mcp-server、nvim-mcp 等（README "Built with rmcp" 列表）。
- 版本策略：crate 主版本 3.x；仓库有 3.x migration guide（讨论 #969），升级跨大版本需查迁移文档。

## 7. 证据边界

- 以上 API 形态与行为来自 README 示例（含宏展开后的省略写法）；精确签名以 docs.rs/rmcp 为准。
- crates.io 显示下载 1923 万（2026-08-08 快照），是生态内的主导实现；该数字含 client/server 两侧与各版本累计。
- OAuth 支持细节在 `docs/OAUTH_SUPPORT.md`，本次未逐条验证；需要时以该文档与 RFC 相关章节为准。
