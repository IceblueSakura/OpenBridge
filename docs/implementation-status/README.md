# 实施边界与证据

当前工作区只有 v2 `semantic`、`protocol`、`lowering` 和纯 SSE framing。实际结构见[架构](../architecture.md)，分层完成度与具体反例见[Generation 实施基线](../architecture-v2/migration.md)，推进顺序见[下一步目标](../implementation-plans/next-goal.md)，已选定切片的验收范围见[当前焦点](../implementation-plans/current-focus.md)。此目录不再维护另一份当前功能清单。

没有生产 Router、Provider registry、credential/OAuth、MCP、观测或服务 binary；旧路线已整体移入 [Git 归档](../archive.md)，而非由 v2 功能对等接替。

离线 codec 与 SDK synthetic loopback 仅证明被执行场景，不证明完整协议、真实 Provider、SDK/Agent 全面兼容、负载或长期服务。受版本管理的旧配置/语料/测试不再参与当前验证。

[evidence/](evidence/README.md) 保留独立历史执行证据，按其原始版本、日期、环境与 payload 范围解释，不刷新验证日期，不将旧结果表述为当前 v2 能力。
