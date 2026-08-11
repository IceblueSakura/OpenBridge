# 当前开发焦点

## 状态

**待确认。** 2026-08-11 用户指示清空旧焦点并写入本焦点：将 MCP 服务从自研实现迁移到官方 Rust SDK `rmcp`，
目标为同时支持协议版本 `2026-07-28`（无状态）与 `2025-11-25` 及更早（legacy 握手）的 dual-era 行为。
方案已获用户决策确认：删除自研 transport、完全参考官方 rmcp 推荐配置与能力范围。
本文件是计划与授权边界；用户确认前不修改任何代码。

## 需求（可观察行为）

1. **依赖**：`Cargo.toml` 增加 `rmcp`（features：`server`、`transport-streamable-http-server`），
   dev-dependencies 增加 `client`、`transport-streamable-http-client-reqwest`；`Cargo.lock` 同步更新。
2. **挂载**：`src/ingress/router.rs` 中 `/mcp` 由 `post(mcp::endpoint)` 改为
   `nest_service("/mcp", StreamableHttpService)`；外层 `require_user`（无 Bearer → 401）与
   `reject_origin`（带 Origin → 403）中间件保持现有语义不变。
3. **工具契约**：`src/mcp/tools/hello.rs` 移植为 `#[tool_router(server_handler)]` + `#[tool]` +
   schemars 参数结构，保持现有 `hello(name: string) -> "Hi, {name}!"` 契约；删除手写 catalog 与
   `ToolDispatchError`。
4. **双版本行为**：默认 `StreamableHttpServerConfig` 下，`2026-07-28` 客户端走无状态路径
   （`server/discover` + `_meta` 版本声明）；`< 2026-07-28` 客户端走 legacy `initialize` 握手路径；
   两条路径均提供 `server/discover`、`tools/list`、`tools/call`。不启用
   `with_stateless_protocol_metadata_required(true)`（会拒绝旧客户端）。
5. **清理**：删除 `src/mcp/transport.rs` 自研实现；静态搜索确认零残留（无 `mcp::endpoint`、
   `HEADER_MISMATCH`、`PROTOCOL_VERSION_META` 符号引用）。
6. **文档同步**（breaking change 原子更新）：`docs/functional-requirements/product-scope/product-scope.md`、
   `docs/functional-requirements/gateway-api/interfaces-and-auth.md` 的 MCP 描述由"仅 2026-07-28"更新为"2026-07-28 + legacy 握手双版本"；
   `docs/implementation-status/current-architecture.md` 模块图更新。

## 失败测试（先 RED）

1. `tests/mcp_contract.rs`：保留现有 HTTP 层边界测试（401/403/415/405），
   新增 rmcp 客户端集成测试（dev-dependencies 启用 `client` + reqwest transport）：
   - `ClientLifecycleMode::Initialize` 连接 `/mcp`，`tools/list` 返回 hello 工具（legacy 握手路径）；
   - `ClientLifecycleMode::Discover { preferred_versions: [2026-07-28] }` 连接，
     `server/discover` + `tools/list` + `tools/call` 成功（无状态路径）；
   - `ClientLifecycleMode::Auto { preferred: [2026-07-28], legacy: [2025-11-25] }` 连接，双版本均成功。
2. 现有"`2025-11-25` 被拒绝（-32022）"断言反转：改为 legacy 握手成功。
3. 新增：`tools/call` 调用 `hello(name)` 返回 `Hi, {name}!`，响应形状按 rmcp SDK 实际序列化断言。

## 不做项（Non-Goals）

- 不实现 legacy HTTP+SSE（2024-11-05 两端点）传输——rmcp 官方 deliberate non-goal。
- 不引入 tasks / resources / prompts / MRTR / elicitation 等扩展能力；保持 tools-only。
- 不修改下游 OpenAI 兼容 API、Provider 适配、认证与 Origin 策略。
- 不触碰工作区中 DeepSeek/Bailian 能力声明的未提交改动（`src/providers/bailian/registration.rs`、
  `src/providers/deepseek/definition.rs`）。

## 验证边界

- `cargo test`（重点 `tests/mcp_contract.rs` + 全量回归）、`cargo clippy`、`cargo fmt --check`。
- 双版本实测：rmcp 客户端 Initialize / Discover / Auto 三种 lifecycle 各跑一轮 discover/list/call。
- 依赖变更按 AGENTS.md 更新 `Cargo.lock` 并 locked 验证。
- 清理后静态搜索：`rg -n "mcp::endpoint|HEADER_MISMATCH|PROTOCOL_VERSION_META" src/ tests/` 零命中。
- 验证通过后：结果写入 `docs/implementation-status/`，本文件恢复为空焦点。
