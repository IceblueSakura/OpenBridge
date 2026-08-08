# pmcp（Pragmatic AI Labs MCP SDK）实现方法调研

## 范围与证据

- 调研对象：`paiml/rust-mcp-sdk` 仓库 README（2026-08-08 抓取）、docs.rs（2026-08-08 抓取）、crates.io 元数据（2026-08-08 查询）。
- 快照：pmcp 2.17.0（crates.io updated_at 2026-07-19，累计下载 7.8 万）；GitHub 最新 release v2.15.0（2026-07-10）。仓库 53 stars、8 forks、1994 commits、MIT。
- 本文是**外部 SDK 事实**，不构成 OpenBridge 的功能承诺。

关键结论：**pmcp 是"完整生态"路线**——同一仓库含 SDK、宏、cargo-pmcp 工具、文档门户（pmcp-book）与课程，宣称覆盖构建/测试/部署全流程；传输面最广（stdio/SSE/WebSocket/WASM），认证支持 OAuth 2.0 + Bearer + OIDC。协议锚定 **2025-11-25**（含 2024-11-05 兼容），未实现 2026-07-28。其与 TypeScript SDK 的对比表含营销性性能声明（16x faster / 50x lower memory），引用时需区分事实与自述。

## 1. 总体架构与特点

- **协议版本**：README 对比表明确 "Protocol Version: 2025-11-25 (+ 2024-11-05 compat)"；docs.rs 导出 `LATEST_PROTOCOL_VERSION` / `SUPPORTED_PROTOCOL_VERSIONS` / `DEFAULT_PROTOCOL_VERSION` 常量。
- **传输**（feature flags 划分）：`http`（裸 hyper 原语）、`streamable-http`（axum + Tower middleware + SSE framing）、`sse`（轻量 server-push）、`websocket`（tungstenite，浏览器场景）；另有 WASM 目标。
- **框架集成**：`axum` module 提供 Router 便捷 API；Tower middleware 内置 DNS rebinding、CORS、安全头。
- **服务器组合**：`composition` module 支持 MCP Server Composition（多 server 组合成一个端点）。
- **客户端**：`client` module 完整客户端实现（含 OAuth/PKCE，docs.rs 导出 `code_challenge_s256`、`generate_code_verifier`、`generate_state`）。
- **其他模块**：`assets`（平台无关资源加载）、`error`（SDK 错误）、`server::ToolHandler` / `ToolOutput` / `StdioTransport`。

## 2. 特性对照（README 自述 vs TypeScript SDK v2.0）

| Feature | pmcp 自述 |
|---|---|
| Transports | stdio、SSE、WebSocket、**WASM** |
| Authentication | OAuth 2.0、Bearer、**OIDC** |
| Tools | ✓（type-safe + outputSchema） |
| Prompts | ✓（含 Workflows） |
| Resources | ✓（Subscriptions） |
| Sampling | ✓ |
| MCP Apps | ✓（Preview + DevTools） |
| Agent Skills（SEP-2640） | ✓（skill + prompt 双面，byte-equal） |
| Tower Middleware | ✓（DNS rebinding、CORS、security headers） |
| Performance | 自述 16x faster / 50x lower memory vs TS SDK（未独立验证） |

## 3. 选型观察（推论）

- **强项**：传输与认证面最全（尤其 WebSocket/WASM/OIDC 是其他 Rust 库没有的）；组合能力（composition）适合多工具服务聚合场景；文档与课程体系完整。
- **弱项**：仓库复杂度高（1994 commits、多子 crate、planning/quality 附属文件多），学习曲线与维护负担高于 rmcp / rust-mcp-sdk；社区规模小（53 stars）；协议仍停在 2025-11-25。
- **对网关类项目**：若需要把多个内部 MCP server 组合成一个对外端点，composition 值得对照研究；若以规范对齐为先，则优先级低于 rmcp。

## 4. 证据边界

- 性能对比（16x/50x）为仓库自述，无独立基准，不做背书。
- "byte-equal" Agent Skills（SEP-2640）双面支持未验证；SEP 本身状态需以 modelcontextprotocol/modelcontextprotocol 的 seps/ 为准。
- OAuth/OIDC 的 profile 细节（RFC 7591 DCR 废弃后的迁移）未逐条验证。
