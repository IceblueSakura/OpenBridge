# 配置与凭证合同

本文集中定义 Bootstrap、私有用户、上游 credential、静态注册、endpoint/egress 和 ChatGPT OAuth 生命周期。

Provider contract、Model、Target、Upstream API、Route、Public Model、endpoint、能力与 wire mapping 由受信 Rust
代码显式注册；运行时配置不提供 Provider DSL 或 Route hot reload。Registry、用户、API-key store 与 OAuth manager
wiring/locator 在启动时校验并冻结；OAuth manager 内部 token snapshot/generation 可以按专有 lifecycle guarded
refresh/rotation，但这不改变 registry、Route、账户 binding 或配置拓扑。业务请求不能选择 endpoint、credential、header
policy 或 routing topology。

## 叶子文档与唯一职责

| 叶子 | 只回答什么 | 验收 |
|---|---|---|
| [所有权与代码注册表](configuration/ownership-and-registration.md) | 配置所有权划分、代码注册表要求 | — |
| [凭证](configuration/credentials.md) | 凭证总则、API-key pool、ChatGPT 隔离、OAuth2 auth 文件 | — |
| [Endpoint、出站与启动生命周期](configuration/egress-and-startup.md) | 出站边界、启动装配、冻结 wiring | — |
| [ChatGPT OAuth credential 生命周期](configuration/oauth-chatgpt.md) | preflight、登录、bundle、refresh、401 recovery | OAUTH-01..12 |
| [Grok OAuth credential 生命周期](configuration/oauth-grok.md) | preflight、device 登录、bundle、refresh、401 recovery | GROK-01..09 |
| [验收](configuration/acceptance.md) | 全局功能验收要求 | CFG-01..19 |
