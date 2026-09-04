# Grok（xAI 订阅）接入进度与边界

注册与能力事实见 `src/providers/grok/`；当前接线见[映射](../model-provider-mapping.md)。
OAuth credential 生命周期合同见 [configuration/oauth-grok.md](../../functional-requirements/configuration/oauth-grok.md)；
订阅登录路径的协议事实与证据边界见[参考资料](../../references/providers/grok-oauth.md)。

## 当前边界

- Provider 走订阅 CLI proxy（`cli-chat-proxy.grok.com/v1`）Responses-only 出口；`api.x.ai` 开放端点、
  Chat Completions、Embeddings、媒体生成与计费探测均未接入。
- 登录使用 xAI authority 官方声明的标准 RFC 8628 device authorization flow（管理员人工批准）；
  不实现参考实现中的 SSO cookie 自动批准、`device/verify`、`device/approve` 与邮箱密码旁路。
- 请求身份头（`X-XAI-Token-Auth`、`x-grok-client-version`、`x-grok-client-identifier`、UA）为编译期
  常量，版本漂移需通过提交升级，不提供运行时覆盖。
- 图像输入未声明：订阅 proxy 的媒体 wire 无本地证据，启用 `grok-cli` pool 时 `grok-4.6` 公开能力的
  媒体交集会收窄至未声明，属有意保守。
- 真实登录、refresh 与 401 recovery 的长期稳定性只有确定性测试覆盖，未形成真实账号长期运行验收。

## 验证与证据

- 启动期 OAuth bundle 校验、登录与刷新由 `tests/oauth2_login_cli.rs`、`src/oauth2_credentials/` 内
  确定性测试（fake transport）覆盖；无真实账号长期运行记录。
- device authorization 与 token 轮询语义的协议事实来自 authority OIDC discovery 与一次匿名端点探测
  （2026-09-03），证据边界见参考文档。

## 代码 owner

`src/providers/grok/`、`src/oauth2_credentials/`。
