# OpenAI ChatGPT 订阅 OAuth 登录路径（sub2api 源码核对）

## 文档元数据

| 字段 | 值 |
|---|---|
| Source snapshot | sub2api 本地 checkout `main` @ `5097b31457e6dc9f49e5f5c9c72b925ce79543b3`（2026-09-03）；`backend/internal/pkg/openai/oauth.go`、`service/openai_oauth_service.go`、`service/openai_agent_identity.go`、`service/openai_codex_pat_service.go`、`service/openai_privacy_service.go`、`handler/admin/account_codex_import.go` |
| Last reverified | 2026-09-03，对 sub2api 本地 checkout 做了源码与 git 历史实读核对 |
| Scope | OpenAI/ChatGPT 订阅账号（Codex 路径）的 OAuth 登录、替代凭据导入、授权后动作与信息来源；不含 Responses wire 全量字段，不含 OpenBridge 实现 |
| Evidence boundary | 未用真实账号执行过任何一条授权流程；协议细节来自代理项目源码而非官方文档，不证明 OpenAI 认可或保证稳定性 |
| Recheck trigger | 决定评估或接入该路径；sub2api Codex OAuth 常量、端点或旁路实现变化；OpenAI auth.openai.com / chatgpt.com backend-api 行为变更 |

同批拆分文档：[Antigravity 订阅 OAuth](antigravity-oauth.md)、[Grok 订阅 OAuth](grok-oauth.md)。

## 1. 定位与证据性质

该路径把 ChatGPT 订阅（Plus/Pro/Business 等）的 Codex 登录态当作服务端凭据使用。
与 Antigravity 不同，它复用的是一个**真实存在的开源客户端身份**（Codex CLI），且该身份
被第三方账号中继服务（claude-relay-service，CRS）广泛共享；与 Grok 类似，主授权码流程
本身是官方公共流程，灰色成分集中在旁路凭据导入和 chatgpt.com 内部 API 的调用。

## 2. 协议事实（`backend/internal/pkg/openai/oauth.go` 核对）

- 流程：OAuth 2.0 Authorization Code + PKCE（S256），端点为
  `https://auth.openai.com/oauth/authorize` 与 `https://auth.openai.com/oauth/token`
  （`oauth.go:21-23`）。
- 客户端身份：Codex CLI 官方 client `ClientID = "app_EMoamEEZ73f0CkXaXp7hrann"`
  （`oauth.go:18-19`，源码注释注明 "from CRS project - Codex CLI client"）。
  纯公共客户端，无 client_secret。
- Scope：授权时 `openid profile email offline_access`，刷新时去掉 `offline_access`
  （`oauth.go:28-31`）。
- Redirect：默认 `http://localhost:1455/auth/callback`（loopback），手动粘贴授权码。
- PKCE 私有偏差：code_verifier 用 **hex 编码**（64 随机字节 → 128 hex 字符）而非
  base64url（`oauth.go:155-163`，注释写明 "OpenAI uses hex encoding"）；
  code_challenge 仍按 RFC 7636 用 base64url(SHA256)。
- 私有授权参数：`codex_cli_simplified_flow=true` 与 `id_token_add_organizations=true`
  （`oauth.go:196-202`）。
- 返回 OIDC id_token；`aud` 是数组；OpenAI 专有 claims 位于
  `https://api.openai.com/auth` 命名空间（`chatgpt_account_id`、`chatgpt_user_id`、
  `chatgpt_plan_type`、组织列表，`oauth.go:236-273`）。id_token 只解码校验 `exp`、
  **不验证签名**（`oauth.go:356-380`，注释自认并给出 JWKS 端点）。

## 3. 授权后动作（`service/openai_oauth_service.go`、`openai_privacy_service.go` 核对）

- 用 access token 调 `chatgpt.com/backend-api`（ImpersonateChrome 指纹客户端）补全
  账号信息：plan_type、订阅到期时间（`enrichTokenInfo`，`openai_oauth_service.go:257-307`）。
- 同样调内部 API 关闭训练数据使用（`disableOpenAITraining`，
  `openai_privacy_service.go:40`），与 Antigravity 路径的隐私清理同构。
- 推理出口恒为 `https://chatgpt.com/backend-api/codex/responses`（Responses 协议），
  请求头伪装 Codex CLI（`openai_gateway_service.go:31`、`openai_gateway_forward.go`
  设 `req.Host = "chatgpt.com"`）；实时语音走 `/backend-api/codex/realtime/calls`。

## 4. 替代登录路径（`OAuthAuthorizationFlow.vue`、`handler/admin/*` 核对）

除授权码流程外，sub2api 前端对 OpenAI 提供多种凭据导入方式：

| 方式 | 实现 | 性质 |
|---|---|---|
| 手动输入 refresh_token | `ExchangeCode`/`RefreshTokenWithClientID`（可换 client） | 标准 refresh grant，常用于跨机器迁移登录态 |
| 手动输入 Mobile RT | `mobile_refresh_token` 输入方法（`OAuthAuthorizationFlow.vue:77`） | 复用 ChatGPT 移动端客户端的 refresh token |
| Codex session 导入 | `account_codex_import.go`：解析 JSON/JSONL/行文本，从会话文件取 `session_token`，解码 JWT 补全账号信息与过期（`ImportCodexSession`，`enrichCodexImportAccountFromJWT`） | 从本地 Codex 登录文件批量搬运凭据，标记 `import_source: "codex_session"` |
| Agent Identity | `openai_agent_identity.go`：存 `agent_runtime_id` + PKCS#8 Ed25519 私钥，签名注册 `auth.openai.com/api/accounts/v1/agent/{runtime_id}/task/register`，用 curve25519 box 解密加密 task id，再以运行时签名换请求凭证 | 模拟 Codex Agent 运行时身份，是三家中伪装最深的旁路 |
| Codex Personal Access Token | `openai_codex_pat_service.go`：`at-` 前缀 token，whoami 验证后按 `auth_provider: codex_personal_access_token` 建账号 | 官方 PAT，非订阅登录态 |
| CRS 同步 | `SyncFromCRS`（`account_handler.go:1150`）：从 claude-relay-service 批量拉取/预览账号 | 社区账号中继服务的互操作 |

## 5. 信息来源

1. **Codex CLI（开源，Apache-2.0）**：client_id、`codex_cli_simplified_flow`、
   loopback redirect 与 Codex 请求头均来自官方开源客户端，是协议事实的第一手来源。
2. **claude-relay-service（CRS）**：`oauth.go` 注释直接注明 client 常量来自
   "CRS project"；sub2api 还实现了从 CRS 同步账号的互操作接口，说明该路径的实现
   直接继承了 CRS 的既有逆向成果。
3. **chatgpt.com/backend-api**：非公开内部 API，属抓包/逆向成果（plan_type、
   订阅到期、训练开关端点），代码内无外部出处标注。
4. **Agent Identity / Mobile RT**：来自对 Codex Agent 运行时与移动端登录态的逆向，
   代码内无外部出处标注。

## 6. 证据边界

- 本文全部协议事实只证明"代理项目源码当前这样实现"，不证明该实现能持续工作；
  OpenAI 对 Codex 客户端指纹、backend-api 或 agent 注册的收紧都会直接影响该路径。
- hex code_verifier、simplified_flow 参数等私有偏差由上游行为决定，可能随官方客户端
  版本漂移。
- 一次源码核对不覆盖其他账号、订阅档、地域、客户端版本或长期运行行为。
- 该路径属订阅 entitlement 的灰色使用，接入前须自行评估 ToS 与封号风险。
