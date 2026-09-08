# Grok（xAI 订阅）接入进度与边界

注册与能力事实见 `src/providers/grok/`；当前 Target 与 Public Model 关系见[映射](../model-provider-mapping.md)。
OAuth credential 生命周期合同见 [configuration/oauth-grok.md](../../functional-requirements/configuration/oauth-grok.md)；订阅登录协议事实与证据边界见[参考资料](../../references/providers/grok-oauth.md)。

## 特有接线与例外

- Provider 走订阅 CLI proxy（`cli-chat-proxy.grok.com/v1`）Responses-only 出口；`api.x.ai` 开放端点、Chat Completions、Embeddings、媒体生成和计费探测未接入。
- 登录使用 authority 声明的 RFC 8628 device authorization flow，并由管理员人工批准；不实现 SSO cookie 自动批准、`device/verify`、`device/approve` 或邮箱密码旁路。
- `X-XAI-Token-Auth`、`x-grok-client-version`、`x-grok-client-identifier` 和 UA 为编译期身份头，版本漂移需提交升级，不提供运行时覆盖。
- 订阅 proxy 的媒体 wire 没有本地证据，故启用 `grok-cli` pool 时 `grok-4.6` 的公开媒体交集有意收窄为未声明。真实登录、refresh、401 recovery 和长期账号稳定性没有真实长期运行验收。

## 验证入口

- `tests/oauth2_login_cli.rs` 和 `src/oauth2_credentials/` fake transport tests 覆盖 bundle、device token 轮询和刷新语义；没有带日期的真实账号记录。
- 一次 2026-09-03 匿名端点探测与 authority OIDC discovery 只支撑参考资料中的协议边界，不证明订阅推理可用。

## 代码 owner

`src/providers/grok/`、`src/oauth2_credentials/`。
