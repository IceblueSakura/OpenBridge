# M07 工具与增强

本模块不阻塞 Native Path、Provider 聚合和最小 Protocol Bridge。

## 增强顺序

1. Usage/成本记录；
2. C2 最小 cooldown 之上的主动健康观测、跨进程状态和自适应路由；
3. Provider-hosted tool facade；
4. 本地/MCP Tool Bridge；
5. 可选 OAuth credential adapter；
6. 简单管理 UI。

## 边界

- Provider-hosted tool 不伪装成普通 function tool；
- Tool Bridge 与 Protocol Bridge 使用不同状态机；
- OpenBridge 核心不执行 Agent 返回的任意通用 function tool；
- MCP/本地工具需要 allowlist、sandbox、timeout 和输出上限；
- usage 默认不记录请求、响应和工具正文；
- 本模块不得重新定义 C2 已要求的最小 deployment cooldown、有限 retry 和最终错误传播；
- UI 不持有上游 secret，也不演化为多租户控制面。

## 启动条件

每个增强需要独立说明用户价值、数据和安全边界、测试以及退出条件。只有真实核心需求依赖时，增强才可提前进入排期。

## 详细资料

- [Hosted tool 与 MCP 需求](../requirements/hosted-tools-mcp.md)
- [Provider 韧性需求](../requirements/provider-resilience.md)
- [本地配置、路由与使用量](../architecture/local-configuration-routing-and-usage.md)
- [Codex OAuth 边界](../design/codex-oauth-credential-boundary.md)
