# 待弃用的 Codex 产品登录来源

MorphieCore 将以公开 SIWC 作为 ChatGPT plan usage 的迁移方向。本页只保留待移除的 Codex 产品认证出处、协议识别与有效约束，不再作为新接入指南；“待弃用”指本项目路径，不是 OpenAI 已弃用 Codex 产品。

公开 SIWC 的完整参考与参数规划直接查 [siwc-login.md](siwc-login.md)，无需先阅读本页。本次文档收敛不删除认证代码、修改私有文件或撤销远端 credential。

## 固定来源

当前产品 wire 来源固定为 **Codex CLI 0.160.0**、`openai/codex@a956835d020762cb2b570053af06f643a11c0ecc`（[release][release]、[Apache-2.0][license]）。它不升级其他语义、codec 或客户端上下文基线，也不授予第三方复用该产品 client/backend 的资格。

| 来源 | 用途 |
|---|---|
| 官方 [Authentication][auth-doc]、[Access tokens][access-doc] | 产品登录方式、workspace 管理与独立 automation 认证的入口 |
| [Browser server][server]、[Authorization][authorization] | 产品 client、授权参数、callback 与 code exchange |
| [Device auth][device]、[OAuth client][oauth] | 私有设备步骤及产品 code/refresh wire |
| [Default auth client][default-client]、[Revocation][revoke] | 请求阶段的 UA/originator 与产品撤销 |

仅保留必要 provenance，不复制官方源码、第三方客户端比较、账户状态或运行报告。参考代码的采用须保留适用许可与 attribution。

## 产品协议识别

这些值只标识仍存产品路径，不能作为 SIWC 参数：

- Authority 为 `https://auth.openai.com`；产品 authorize/token/revoke 路径为 `/oauth/authorize`、`/oauth/token`、`/oauth/revoke`。
- 固定 client 为 `app_EMoamEEZ73f0CkXaXp7hrann`，不是 client secret，也不是可用于公共 API 的通用授权。
- 本项目 browser 请求最小 `openid profile email offline_access`，不申请 connectors 权限；产品授权参数另有 `id_token_add_organizations`、`codex_cli_simplified_flow` 与固定 originator。
- Code exchange 为 form encoding；本项目产品 refresh/revoke 为 JSON。默认 auth metadata 使用固定 CLI 的 `codex_cli_rs` 与 UA 格式；不能套用 SIWC 的 resource/scopes 或表单合同。
- 推理目标是 `https://chatgpt.com/backend-api/codex/responses`，不是公共 `api.openai.com/v1/responses`。

精确实现由 [driver](../../src/credential/codex.rs)、[auth metadata](../../src/credential/codex_metadata.rs) 和 [Provider binding](../../src/provider/catalog.rs)拥有；不在此复制命令、存储格式、token schema 或上下文/header 表。

## Codex 设备交互不是标准 device grant

产品申请与轮询使用 JSON：`/api/accounts/deviceauth/usercode` → 用户访问 `/codex/device` → `/api/accounts/deviceauth/token` 返回 authorization code 与 PKCE 材料 → 产品 `/oauth/token` 交换 token。

最后一步使用产品 `https://auth.openai.com/deviceauth/callback` 与返回的 verifier；不是 `urn:ietf:params:oauth:grant-type:device_code`。申请阶段 404 表示设备入口未启用，固定 polling 中的 pending 403/404 不能推广为所有 OAuth 错误可重试。用户/workspace policy 与有界轮询仍须独立检查，不为 issued SIWC client 复用这些端点。

## 移除前仍有效的约束

- Browser 绑定 literal `127.0.0.1` 与准确 `/auth/callback`，当前仅默认 1455 或显式 1457；不取消其他 listener 或自动换端口。State、PKCE、nonce 与事务绑定；错误 callback 同样先验证 state。
- 身份由受信 issuer/client 的 RS256 验证及 subject/workspace 绑定建立；JWT payload decode、邮箱、alias 和预选 workspace 不替代验证。Device/refresh 不冒用 browser nonce。
- 凭据只属于其产品 profile/principal；正常 rotation、不确定结果、退出、local cleanup 与 remote revoke 分开。普通模型请求不进行登录或 refresh，不从 auth cache 搜索/导入材料，也不派生 Platform API key。
- 产品 token、账户 header、client metadata 或 opaque history 不转换/迁移为 SIWC 权限。新 registration 的创建与绑定须独立验证；代码移除、私有凭据清理和远端撤销分别需要对应授权。

共用 store、并发与恢复机制仍由其代码/合同拥有；本页不再转述通用 OAuth 生命周期、缓存/turn、Pi 接线或迁移实施计划。

[release]: https://github.com/openai/codex/releases/tag/rust-v0.160.0
[license]: https://github.com/openai/codex/blob/a956835d020762cb2b570053af06f643a11c0ecc/LICENSE
[auth-doc]: https://learn.chatgpt.com/docs/auth
[access-doc]: https://learn.chatgpt.com/codex/enterprise/access-tokens
[server]: https://github.com/openai/codex/blob/a956835d020762cb2b570053af06f643a11c0ecc/codex-rs/login/src/server.rs
[authorization]: https://github.com/openai/codex/blob/a956835d020762cb2b570053af06f643a11c0ecc/codex-rs/login/src/oauth/authorization.rs
[device]: https://github.com/openai/codex/blob/a956835d020762cb2b570053af06f643a11c0ecc/codex-rs/login/src/device_code_auth.rs
[oauth]: https://github.com/openai/codex/blob/a956835d020762cb2b570053af06f643a11c0ecc/codex-rs/login/src/oauth/client.rs
[default-client]: https://github.com/openai/codex/blob/a956835d020762cb2b570053af06f643a11c0ecc/codex-rs/login/src/auth/default_client.rs
[revoke]: https://github.com/openai/codex/blob/a956835d020762cb2b570053af06f643a11c0ecc/codex-rs/login/src/auth/revoke.rs
