# 文档迁移说明：控制面、密钥与可观测性

本文原先描述多主体授权、代理签发密钥、配额、审计和控制面模型。OpenBridge 的核心范围现已收敛为**单用户、单服务的多 Provider 聚合代理**，这些企业级能力不再属于核心设计。

当前设计请参阅：

- [本地配置、路由与使用量](local-configuration-routing-and-usage.md)
- [代理需求](../requirements/proxy-requirements.md)
- [架构与收敛路线](architecture-and-roadmap.md)

保留此文件是为了避免旧链接失效。若未来确有多用户或独立控制面的需求，应作为独立扩展重新立项，而不是恢复为核心前置条件。
