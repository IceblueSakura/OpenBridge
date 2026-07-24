# 功能模块索引

本目录从产品功能角度组织 OpenBridge。模块文档只描述当前目标、边界、接口和验收，不记录需求变更过程。

| 编号 | 模块 | 主要职责 | 当前状态 |
|---|---|---|---|
| M00 | [产品边界](00-product-scope.md) | 用户、部署模型、核心范围、优先级和非目标 | 范围已定义 |
| M01 | [客户端 API](01-client-api.md) | Chat、Responses、Models 与 Codex/Hermes 契约 | 接口原型完成，真实客户端待验收 |
| M02 | [配置与路由](02-configuration-and-routing.md) | bootstrap、deployment、alias、capability、snapshot、fallback | 原型完成，第二 Family 待实现 |
| M03 | [Provider Adapter](03-provider-adapters.md) | Family、认证、路径、响应、错误和 conformance | 仅 OpenAI Family |
| M04 | [原生转发与流式处理](04-native-forwarding-and-streaming.md) | Native Path、HTTP/SSE、终态、取消、错误 | 原型完成，真实 corpus 待验收 |
| M05 | [协议桥](05-protocol-bridge.md) | Chat ↔ Responses、Bridge IR、工具身份与转换等级 | 设计完成，尚未实现 |
| M06 | [安全与凭证](06-security-and-credentials.md) | 入站认证、secret、出站边界、OAuth 范围 | API-key 基线完成 |
| M07 | [工具与增强](07-tools-and-enhancements.md) | hosted tool、MCP、usage、health、UI | Deferred |

实施顺序和各阶段测试见[实施阶段索引](../phases/README.md)。
