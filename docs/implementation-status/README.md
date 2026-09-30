# 实施边界与证据

当前工作区是 v2 Rust 库与最小 loopback 文本网关，具有 synthetic HTTP/SDK 全链验收，不是生产就绪服务。实际结构见[架构](../architecture.md)，分层完成度与具体反例见[Generation 当前能力与边界](generation.md)，推进顺序见[下一步目标](../implementation-plans/next-goal.md)，已选定切片的验收范围见[当前焦点](../implementation-plans/current-focus.md)。`generation.md` 是当前 Generation 状态的唯一汇总；本索引不重复功能清单，状态相对当前产品合同判断，不相对旧版本。

最小 Router、环境变量凭据绑定、HTTP transport 与 binary 见 [HTTP 网关指南](../http-gateway.md)；没有动态 registry、凭据池/OAuth、MCP 或生产观测体系。旧路线已整体移入 [Git 归档](../archive.md)，而非由 v2 功能对等接替。

离线 codec 与 SDK synthetic loopback 仅证明被执行场景，不证明完整协议、真实 Provider、SDK/Agent 全面兼容、负载或长期服务。受版本管理的旧配置/语料/测试不再参与当前验证。

[evidence/](evidence/README.md)仅保留固定非 Provider 研究记录。当前 Provider/模型、协议准入和实例启用状态按 [AGENTS.md](../../AGENTS.md#current-provider-model-and-compatibility-information)现场查询；不在本目录保留结果矩阵、账号状态或当前适配清单。测试结论在当次交付中说明，授权产生的原始脱敏产物留在 ignored run 目录；codec 消费、模型输出遵循与生产接线分别判断。
