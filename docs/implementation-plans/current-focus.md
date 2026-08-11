# 当前开发焦点

## 状态

**已完成。** MCP 服务已从自研实现迁移到官方 Rust SDK `rmcp`（3.1.2），实现 dual-era 兼容：
`2026-07-28` 无状态路径与 legacy `initialize` 握手路径均由 `rmcp` `StreamableHttpService` 原生提供。
验证结果与事实已转入 [current-architecture](../implementation-status/current-architecture.md)；
功能需求文档（[interfaces-and-auth](../functional-requirements/gateway-api/interfaces-and-auth.md)、
[product-scope](../functional-requirements/product-scope/product-scope.md)）已同步为 dual-era 描述。

开始其他行为前，必须重新写明单一可观察行为、失败测试、明确非目标和验证范围。

## 完成证据

- `tests/mcp_dual_era.rs`（新增）：rmcp 客户端三种 lifecycle（Initialize / Discover / Auto）经真实
  HTTP listener 连接 `/mcp`，`tools/list` 与 `tools/call` 均成功。
- `tests/mcp_contract.rs`（重写）：HTTP 边界（401/403/-32020/-32602）与 hello 契约按 rmcp 实际序列化断言；
  `supportedVersions` 为 rmcp SDK 全量（2024-11-05 … 2026-07-28）。
- `cargo test --locked` 全量通过（唯一预存失败 `otlp_trace_contract` 与本次迁移无关，已在干净 HEAD 上确认）；
  `cargo clippy --all-targets --locked` 0 警告；`cargo fmt --check` 干净。
- 静态搜索 `mcp::endpoint|HEADER_MISMATCH|PROTOCOL_VERSION_META` 零残留。
