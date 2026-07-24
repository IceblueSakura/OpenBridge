# M06 安全与凭证

## 核心边界

- 下游 token 与上游 credential 分离；
- credential 通过受信 reference 解析，普通配置不保存明文；
- secret 不进入响应、普通日志、fixture 和配置错误；
- Provider adapter 只能产生相对路径；
- transport 只连接 allowlist origin，并禁用自动重定向；
- request body、SSE event、timeout 和连接池有上限。

## 部署规则

- 当前只允许 loopback listener；
- 未来非 loopback 必须使用高熵静态 token，并由 TLS 或可信反向代理保护；
- 业务请求不得获得控制面权限；
- 日志默认不记录 prompt、response、tool arguments 或 result 正文。

## Credential

当前核心只支持 API key。OAuth 是可选 credential adapter，必须先明确官方授权契约、client registration、redirect、scope/resource 和产品条款；OAuth 阻塞不得影响 API-key core。

## 验收

- 无认证请求在 egress 前失败；
- 明文 credential 配置失败；
- 日志、Debug、错误和 fixture 无 secret；
- origin/header/redirect 约束通过；
- credential rotation 和失效操作在核心接受阶段形成 runbook。

## 详细资料

- [本地配置、路由与使用量](../architecture/local-configuration-routing-and-usage.md)
- [Codex OAuth 边界](../design/codex-oauth-credential-boundary.md)
- [OAuth 实现调研](../research/chatgpt-oauth/hermes-and-litellm-oauth-analysis.md)
