# fastmcp_rust（FastMCP Rust）实现方法调研

## 范围与证据

- 调研对象：`Dicklesworthstone/fastmcp_rust` 仓库 README（2026-08-08 抓取）、docs.rs（2026-08-08 抓取）、crates.io 元数据（2026-08-08 查询）。
- 快照：fastmcp_rust 0.3.2（crates.io updated_at 2026-06-18，累计下载 2.2 千）；仓库 29 stars、11 forks、457 commits。
- 本文是**外部 SDK 事实**，不构成 OpenBridge 的功能承诺。

关键结论：**fastmcp_rust 是实验性项目，现阶段不构成生产选型**。它是 Python FastMCP（PrefectHQ，已并入官方 Python SDK）的 Rust 移植，主打"cancel-correct"结构化并发，但存在三大约束：运行时绑死 asupersync（不支持 tokio）、协议仍为 2024-11-05（2026-07-28 支持"实现中且未验证"，发布被隔离）、维护者为个人且不接受外部贡献、许可证状态未定。

## 1. 核心设计

- **运行时**：基于作者自研 `asupersync`（context-aware async / 结构化并发）；README 明确 "asupersync only; Tokio adapters are unsupported"。
- **取消语义**：`#[tool]` 处理器通过 `McpContext` 的 `checkpoint()` 显式声明取消点；另有 request/handler 两级 budget（timeout）面。
- **结果模型**：四值 `Outcome`——success、expected error、cancellation、panic；`ResultExt` 将 `Result` 转 `Outcome`。
- **安全**：workspace crate 全部 `#![forbid(unsafe_code)]`。
- **宏**：`#[tool]`、`#[resource]`、`#[prompt]` 属性宏生成 PascalCase handler 值（如 `#[tool] fn greet` → `tool(Greet)`）。

## 2. 快速上手形态

```rust
#[tool]
async fn greet(ctx: &McpContext, name: String) -> McpResult<String> {
    ctx.checkpoint();  // 取消点
    Ok(format!("Hello, {name}!"))
}

fn main() {
    Server::new("my-server", "1.0.0")
        .tool(Greet)   // 宏生成
        .build()
        .run_stdio();
}
```

## 3. 协议状态（README 2026-08-01 自述）

> MCP 2026-07-28 support is under implementation and remains unverified. The current public `PROTOCOL_VERSION` is `2024-11-05`.

- 仓库含 `COMPREHENSIVE_PLAN_TO_SUPPORT_MCP_2026-07-28_SPEC_IN_FASTMCP_RUST.md` 等计划文档，但无 conformance/release 证据；README 明确"release publication remains quarantined"。
- 限定边界（README 自述）：wire cancellation 仅主 stdio 路径部分合格；custom/SSE/WebSocket 入口仍走旧顺序循环；bidirectional calls（sampling/elicitation/roots）未合格；HTTP 公开路径 fail-closed；response caching 仅保守分区。
- 认证：static-token/OAuth/OIDC 实现代码存在，但 OAuth/OIDC 生产安全性与 profile conformance 未验证。

## 4. 选型观察（推论）

- 与既有 tokio/axum host **不兼容**，除非整体切换运行时或等待受支持的 adapter。
- 协议落后两个大修订，且维护模式（个人、不接受 PR、发布隔离）使落地风险不可控。
- 价值在于研究意义：cancel-correct 结构化并发与 checkpoint 式取消是可借鉴的模式，若未来要评估"长任务取消"设计可参考其思路。

## 5. 证据边界

- 所有"未验证/隔离/限定边界"表述均转述 README 原文；未独立构建或运行。
- 许可证：Cargo metadata 声明 MIT，但仓库 LICENSE 含额外 OpenAI/Anthropic rider，LICENSE-MIT 为纯 MIT 文本——README 自述 "release-license representation is unresolved"，发布前引用其代码需先确认授权。
