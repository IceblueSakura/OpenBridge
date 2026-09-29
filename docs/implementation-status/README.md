# 实施边界与证据

当前工作区是 v2 Rust 库与最小 loopback 文本网关，具有 synthetic HTTP/SDK 全链验收，不是生产就绪服务。实际结构见[架构](../architecture.md)，分层完成度与具体反例见[Generation 实施基线](../architecture-v2/migration.md)，推进顺序见[下一步目标](../implementation-plans/next-goal.md)，已选定切片的验收范围见[当前焦点](../implementation-plans/current-focus.md)。此目录不再维护另一份当前功能清单。

最小 Router、环境变量凭据绑定、HTTP transport 与 binary 见 [HTTP 网关指南](../http-gateway.md)；没有动态 registry、凭据池/OAuth、MCP 或生产观测体系。旧路线已整体移入 [Git 归档](../archive.md)，而非由 v2 功能对等接替。

离线 codec 与 SDK synthetic loopback 仅证明被执行场景，不证明完整协议、真实 Provider、SDK/Agent 全面兼容、负载或长期服务。受版本管理的旧配置/语料/测试不再参与当前验证。

[evidence/](evidence/README.md) 保留独立执行证据，包括[显式 adapter 重构后的 Flash 真实验收](evidence/2026-09-29-flash-provider-adapter-acceptance.md)。按各记录的代码版本、日期、环境与 payload 范围解释，不刷新历史验证日期；codec 消费成功、模型输出符合请求与生产接线分别判断。
