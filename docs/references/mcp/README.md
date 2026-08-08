# MCP Rust 生态调研索引

## 文档定位

本目录记录 Rust 场景下实现 MCP（Model Context Protocol）server 的可选库的外部事实：来源仓库、版本、维护状态、协议支持范围、传输与框架集成方式。这里不记录 OpenBridge 当前实现、目标类型、实施计划或已运行验证。

调研快照日期：**2026-08-08**。所有版本号、下载量、协议支持状态均为该日期的 crates.io / GitHub 阅读结果；MCP 协议版本以官方 [specification/versioning](https://modelcontextprotocol.io/specification/versioning) 页面为准（当前协议版本 **2026-07-28**，上一修订 2025-11-25）。SDK 升级后须重新复核协议支持声明，不能把"某日某版本支持某协议"当作永久事实。

关键结论先行：**截至快照日，Rust 生态中唯一完整实现 2026-07-28 现行规范的库是官方 rust-sdk（crate 名 `rmcp`）**；其余活跃库（rust-mcp-stack、pmcp）仍锚定 2025-11-25。若目标是与最新规范对齐，选型空间实际上是"用官方 `rmcp`"还是"接受旧规范用社区库"。

## 1. 生态全景

| 库 | 维护方 | 最新版（快照日） | 下载量 | 协议支持 | 运行时 | 传输 | HTTP 集成 |
|---|---|---|---|---|---|---|---|
| [rmcp](rmcp-official-sdk.md) | 官方 modelcontextprotocol | 3.1.2（2026-08-07） | 1923 万 | **2026-07-28**（含 2025-11-25 兼容） | tokio | stdio、Streamable HTTP、SSE（作为 HTTP 实现细节）；明确不支持 legacy HTTP+SSE | Tower service，可挂 axum/hyper |
| [rust-mcp-sdk](rust-mcp-sdk-community.md) | rust-mcp-stack（社区） | 1.0.1（2026-07-26） | 22 万 | 2025-11-25（conformance 100%） | tokio | stdio、Streamable HTTP、SSE（向后兼容） | 原生 axum / actix，BYO server |
| [pmcp](pmcp.md) | paiml（Pragmatic AI Labs） | 2.17.0（2026-07-19） | 7.8 万 | 2025-11-25（+2024-11-05 兼容） | tokio | stdio、SSE、WebSocket、WASM | axum 便捷层 + Tower middleware |
| [fastmcp_rust](fastmcp-rust.md) | Dicklesworthstone（个人） | 0.3.2（2026-06-18） | 2.2 千 | 声明 2024-11-05，2026-07-28 "实现中未验证" | asupersync（非 tokio） | stdio、SSE、WebSocket、HTTP（fail-closed） | 自带 HTTP，不支持外部框架 |
| mcp_rust_sdk | 未知（docs.rs） | 0.1.1（2024-12-01） | 5.6 千 | 2024-11-05 时代 | tokio | stdio、WebSocket | 无（HTTP 不作为主要路径） |

版本/下载量来源：crates.io API（`max_stable_version`、`downloads`、`updated_at`），2026-08-08 查询。

## 2. 协议支持矩阵（对照 2026-07-28 重大变更）

| 2026-07-28 特性 | rmcp 3.x | rust-mcp-sdk 1.x | pmcp 2.x | fastmcp_rust |
|---|---|---|---|---|
| 无状态协议（无 initialize 握手，`_meta` 声明版本） | ✅ 原生 | ❌ 仍用初始化握手 | ❌ 仍用初始化握手 | ❌ |
| `server/discover` | ✅ | ❌ | ❌ | ❌ |
| `subscriptions/listen`（替代 resources/subscribe） | ✅ | ❌ | ❌ | ❌ |
| MRTR（InputRequiredResult / requestState） | ✅ | ❌ | ❌ | ❌ |
| Tasks 扩展（io.modelcontextprotocol/tasks） | ✅（TaskManager） | ✅（作为 2025-11-25 特性） | ❌ | ❌ |
| 响应缓存（SEP-2549 ttlMs/cacheScope） | ✅ 客户端透明缓存 | ❌ | ❌ | ❌ |
| 标准 HTTP 路由头（Mcp-Method/Mcp-Name） | ✅ | ❌ | ❌ | ❌ |
| 协议版本协商常量 | `ProtocolVersion::{LATEST, V_2026_07_28, KNOWN_VERSIONS}` | `ProtocolVersion::V2025_11_25` | `LATEST_PROTOCOL_VERSION` 等 | 单一 PROTOCOL_VERSION |

## 3. 选型观察（推论，非实现建议）

- **规范对齐优先**：只有 `rmcp` 一条路；其余库至少差一个大修订，且无公开迁移时间表（fastmcp_rust 有实现计划但明确"未验证、发布被隔离"）。
- **生态成熟度**：`rmcp` 下载量比其他三者之和还高两个数量级，且是官方维护，未来规范演进优先落地。
- **社区库的差异化价值**：`rust-mcp-sdk` 提供开箱即用的 axum server（`create_axum_server`）、DNS rebinding 防护、health check、OAuth（DCR）；`pmcp` 有 WebSocket/WASM 传输和 MCP Server Composition。这些在 `rmcp` 里要自行组装。
- **运行时约束**：`fastmcp_rust` 绑死 asupersync，与 tokio/axum 技术栈不兼容，选型上基本出局。
- **官方一致性测试**：modelcontextprotocol/conformance 是官方 conformance 套件；rust-mcp-sdk 声称 100% 通过，`rmcp` 仓库自带 conformance 目录。声称不等于独立验证，引用时标注来源。

## 4. 证据边界与未验证项

- crates.io 下载量/版本号为 2026-08-08 快照；rmcp 更新日期 2026-08-07（快照前一日），版本节奏很快，可能随时变化。
- pmcp 与 TypeScript SDK 的 "16x faster / 50x lower memory" 对比为仓库自述性能声明，未见独立基准，文档中只转述不背书。
- fastmcp_rust 的 "2026-07-28 under implementation" 是其 README 自述；仓库自带多份计划文档（COMPREHENSIVE_PLAN_TO_SUPPORT_MCP_2026-07-28_SPEC 等），但没有 conformance 或 release 证据。
- 各库的 OAuth 支持深度（DCR、RFC 7591 废弃后的 Client ID Metadata Documents 迁移）未逐一验证，引用时以各自 docs/OAUTH 文档为准。

## 5. 外部链接

- MCP 规范：<https://modelcontextprotocol.io/specification/2026-07-28>（当前版）、<https://modelcontextprotocol.io/specification/2025-11-25>
- 官方 conformance：<https://github.com/modelcontextprotocol/conformance>
- 官方 Rust SDK：<https://github.com/modelcontextprotocol/rust-sdk>、<https://crates.io/crates/rmcp>
- rust-mcp-stack：<https://github.com/rust-mcp-stack/rust-mcp-sdk>、<https://crates.io/crates/rust-mcp-sdk>
- pmcp：<https://github.com/paiml/rust-mcp-sdk>、<https://crates.io/crates/pmcp>
- fastmcp_rust：<https://github.com/Dicklesworthstone/fastmcp_rust>、<https://crates.io/crates/fastmcp_rust>
