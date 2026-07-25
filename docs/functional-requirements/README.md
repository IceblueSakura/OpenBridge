# 功能需求

本目录定义当前有效的产品行为、边界和交付证据要求，不规定按模块拆分的实现方式或预先固定的实施顺序。功能需求是产品目标的唯一入口；实施计划只能解释某项需求准备如何落地，不能反向扩大兼容承诺。

| 功能域 | 当前需要回答的问题 | 文档 |
|---|---|---|
| 产品范围 | 服务为谁解决什么问题，哪些能力不做 | [产品范围](product-scope.md) |
| 网关 API 与兼容 | 客户端可调用什么、JSON/SSE/tool/continuation 如何表现 | [网关 API 与客户端兼容](gateway-api-compatibility.md) |
| 配置与凭证 | 配置文件、private secret、header、网络和 reload 如何受信管理 | [配置、凭证与受信运行边界](configuration-and-credentials.md) |
| 路由与可用性 | alias 如何选择 deployment，限流、重试、cooldown、fallback 和状态亲和如何处理 | [路由与 Provider 韧性](provider-resilience.md) |
| 调用统计与观测 | usage、TTFT/TTFB、终态错误率和 headless 输出如何定义 | [调用统计与可观测性](observability.md) |
| 交付与证据 | 如何以 TDD、fixture、SDK/CLI 和真实环境证据约束兼容声明 | [交付与证据要求](delivery-and-evidence.md) |
| 条件性后续能力 | Provider-hosted tool facade 与 MCP 在何种用户结果下才进入范围 | [Hosted tool 与 MCP](hosted-tools-mcp.md) |

阅读时先确认“状态”：`当前目标`是需保持的行为，`后续方向`不构成待办或兼容承诺，`【需根据实际情况完善】`表示尚未获服务所有者决定。当前代码、测试或真实运行已经证明的内容只写入 `../implementation-status/`。

需求变更先在本目录明确用户可观察的结果、失败语义、资源/安全边界与非目标；具体实现方案再写入 `../implementation-plans/`。当目标或证据冲突时，以产品范围、对应功能需求和实施现状为准，而非历史设计假设。
