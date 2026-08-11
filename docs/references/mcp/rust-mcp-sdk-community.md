# rust-mcp-sdk（rust-mcp-stack 社区 SDK）实现方法调研

## 范围与证据

- 调研对象：`rust-mcp-stack/rust-mcp-sdk` 仓库 README（2026-08-08 抓取）、crates.io 元数据（2026-08-08 查询）。
- 快照：rust-mcp-sdk 1.0.1（crates.io updated_at 2026-07-26，累计下载 22 万）；rust-mcp-axum 1.0.1；rust-mcp-actix / rust-mcp-schema / rust-mcp-extra 同生态。
- 仓库状态：188 stars、31 forks、311 commits、MIT；README 标记 v1.0.0 "stable and production-ready"。
- 本文是**外部 SDK 事实**，不构成 OpenBridge 的功能承诺。

关键结论：**rust-mcp-sdk 是"开箱即用的 axum MCP server"路线**——提供 `create_axum_server()` 一站式托管（多客户端并发、session 管理、DNS rebinding 防护、health check、OAuth），且声称通过官方 conformance 测试 100%。但它锚定 **2025-11-25** 规范，未实现 2026-07-28 的无状态协议。

## 1. 总体架构

```
用户代码（#[mcp_tool] 结构体 + ServerHandler trait 实现）
        │
   rust-mcp-sdk 核心（rust-mcp-schema 类型、协议编解码、session、observer）
        │
   ┌─────────────┴──────────────┐
   │  StdioTransport            │  create_axum_server / create_actix_server
   │  (create_server)           │  (rust-mcp-axum / rust-mcp-actix)
   │                            │  mcp_routes(state, opts, handler) ← BYO server
   └────────────────────────────┘
```

- 服务端 handler trait 有两层：`ServerHandler`（推荐，预实现初始化/ping 等，只覆盖业务方法）与 `ServerHandlerCore`（底层 `request`/`notification`/`error` 三方法细粒度控制）。
- 工具定义方式：`#[mcp_tool]` 过程宏从 struct 生成 `Tool` 与 JSON Schema；`tool_box!(Enum, [Tool1, Tool2])` 宏批量组织工具。
- 与 rmcp 的关键差异：这里**保留** `initialize` 握手与 session 模型（2025-11-25 形态），`InitializeResult` 显式构造并传给 runtime。

## 2. 快速上手（Streamable HTTP + axum）

```rust
#[mcp_tool(name = "say_hello", description = "returns \"Hello from Rust MCP SDK!\" message")]
#[derive(Debug, serde::Deserialize, serde::Serialize, macros::JsonSchema)]
pub struct SayHelloTool {}

#[async_trait]
impl ServerHandler for HelloHandler {
    async fn handle_list_tools_request(
        &self, _request: Option<PaginatedRequestParams>,
        _runtime: Arc<dyn McpServer>,
    ) -> Result<ListToolsResult, RpcError> { /* ... */ }

    async fn handle_call_tool_request(
        &self, params: CallToolRequestParams,
        _runtime: Arc<dyn McpServer>,
    ) -> Result<CallToolResult, CallToolError> { /* ... */ }
}

#[tokio::main]
async fn main() -> SdkResult<()> {
    let hello_handler = HelloHandler::default().to_mcp_server_handler();
    let server = create_axum_server(
        initialize_result, hello_handler,
        AxumServerOptions {
            host: "127.0.0.1".into(),
            port: 8080,
            event_store: Some(Arc::new(InMemoryEventStore::default())), // resumability
            task_store: Some(Arc::new(InMemoryTaskStore::new(None))),   // MCP Tasks
            auth: Some(Arc::new(...)),                                  // OAuth
            health_endpoint: Some("/health".into()),                    // health check
            sse_support: true,                                          // 向后兼容 SSE
            ..Default::default()
        },
    );
    server.start().await?;
    Ok(())
}
```

要点：
- `AxumServerOptions` / `ActixServerOptions` 字段一一对应，可移植。
- `event_store`（SSE resumability）与 `task_store`（MCP Tasks）默认关闭，传入 `Some(..)` 才启用。
- `sse_support` 默认 `true`——为旧 SSE-only 客户端保留兼容入口。
- 无需手工管理连接：一个 axum server 天然支持多客户端并发。

## 3. BYO server（嵌入既有应用）

| 后端 | 函数 | 用法 |
|---|---|---|
| Axum | `mcp_routes(state, &mount_opts, http_handler)` | 返回 route 合并进既有 Router |
| Actix-web | `mcp_scope(state, http_handler, &mount_opts)` | 返回 scope 合并进既有 app |

对已有 HTTP 服务的项目，这是把 MCP 作为子路径挂载的标准方式；也可以按官方文档"Custom HTTP Framework Integration"把核心逻辑适配到 Rocket/Salvo/Warp 等（框架无关设计，适配请求/响应类型即可）。

## 4. 特性清单（README 自述）

- ✅ 最新协议支持：2025-11-25（**非 2026-07-28**）
- ✅ 100% MCP Conformance：通过官方 client/server conformance tests（CI workflow 链接）
- ✅ Transports：stdio、Streamable HTTP、向后兼容 SSE
- ✅ 框架无关：Axum、Actix、BYO
- ✅ 多客户端并发、DNS Rebinding Protection、Resumability、MCP Tasks、Batch Messages、Streaming & non-streaming JSON、Message Observer、HTTP Health Checks
- ✅ OAuth：RemoteAuthProvider（DCR 兼容 IdP：Keycloak / WorkOS / Scalekit 经 rust-mcp-extra）；OAuthProxy（非 DCR 提供方，**开发中，官方建议暂用 RemoteAuthProvider**）
- ✅ 客户端 OAuth：metadata discovery、DCR、PKCE、token refresh、pluggable storage

### 4.1 宏

| 宏 | 用途 |
|---|---|
| `#[mcp_tool]` | struct → Tool + JSON Schema；支持 icons、destructive/idempotent/open_world/read_only hints、execution(task_support) 元数据 |
| `tool_box!` | 工具枚举批量管理 |
| `#[mcp_elicit]` | 类型安全 elicitation（form/URL 模式），`request_elicitation()` + `from_elicit_result_content()` |
| `#[mcp_resource]` / `#[mcp_resource_template]` | 静态/参数化资源声明 |
| `mcp_icon!` | Implementation/tool icon 构建 |

### 4.2 可观测性

- `McpObserver` trait：非阻塞钩子，拦截全部进出 MCP 消息，适用于 telemetry/logging/debugging；可在 server/client 初始化时挂载。
- `health_endpoint`：非规范扩展，面向 LB/容器编排；可自定义 handler 返回指标。

### 4.3 安全默认值

- DNS rebinding 防护默认开启；`allowed_hosts` 未设置时从 `host:port` 自动推导；绑定 `0.0.0.0`/`::` 时**必须**显式配置 `allowed_hosts`。
- 文档建议生产环境 TLS/HTTPS、本机运行只绑 127.0.0.1。

## 5. Cargo features

默认全部开启；可按需裁剪（示例：`default-features = false, features = ["server", "macros", "stdio"]`）。

- `server` / `client`：两侧能力
- `macros`：过程宏
- `sse` / `streamable-http` / `stdio`：传输
- `auth`：OAuth
- `tls-no-provider`：TLS 而不引入 aws-lc crypto provider

## 6. 与 rmcp 的对比要点（推论）

| 维度 | rmcp 3.x | rust-mcp-sdk 1.x |
|---|---|---|
| 协议 | 2026-07-28（现行） | 2025-11-25 |
| 状态模型 | 无状态（默认）/ 旧版兼容 | session 化（2025-11-25 形态） |
| HTTP 托管 | 只给 Tower service，router 自备 | `create_axum_server` 一站式（含 health/OAuth/rebinding） |
| 与现有 axum host 结合 | axum 挂载即可，控制面由 host 保留 | 开箱即用但绑定其 server 生命周期 |

若短期目标是快速让 axum 应用暴露 MCP 工具且可接受 2025-11-25 协议，rust-mcp-sdk 的集成成本最低；若要求与现行规范对齐，
则需要选择支持现行规范的 SDK。rmcp 的 Tower service 和无状态语义更适合由既有 axum host 保留路由与控制面。

## 7. 证据边界

- "100% conformance" 为仓库 CI 自述，未在本调研中独立运行。
- rust-mcp-axum 下载量仅 372（2026-08-08 快照），说明 `create_axum_server` 路线采用率远低于核心 crate；不代表质量问题，但生态验证案例较少。
- OAuthProxy 标记 "still in development"；DCR 相关能力依赖上游 IdP 支持。
