# Grok（xAI）订阅 OAuth 登录路径（sub2api 源码核对）

## 文档元数据

| 字段 | 值 |
|---|---|
| Source snapshot | sub2api 本地 checkout `main` @ `5097b31457e6dc9f49e5f5c9c72b925ce79543b3`（2026-09-03）；sub2api README Grok 小节（同 checkout） |
| Last reverified | 2026-09-03，对 sub2api 本地 checkout 做了源码与 git 历史实读核对；同日实抓 `auth.x.ai/.well-known/openid-configuration` 并对 `/oauth2/device/code` 执行了一次匿名授权探测 |
| Scope | Grok（xAI）订阅账号 OAuth 登录路径的端点、参数、旁路授权、配额探测与信息来源；不含推理/媒体 wire 全量字段，不含 OpenBridge 实现 |
| Evidence boundary | 未用真实账号执行过任何一条授权流程；协议细节来自代理项目源码而非官方文档，不证明 xAI 认可或保证稳定性 |
| Recheck trigger | 决定评估或接入该路径；sub2api xAI 常量或端点变化；xAI 计费/订阅政策或设备授权端点变更 |

同批拆分文档：[Antigravity 订阅 OAuth](antigravity-oauth.md)、[OpenAI ChatGPT 订阅 OAuth](openai-chatgpt-oauth.md)。

## 1. 定位与证据性质

与 Antigravity 路径不同，Grok 的 OAuth 是 xAI 官方设计的公共流程（有 OIDC discovery、
环境变量可配置项与 README 级说明），逆向成分主要在订阅代理端点选择、JWT 声明解读与
两条非交互式旁路。本文不构成接入建议；合规风险见 §6。

## 2. 协议事实（`backend/internal/pkg/xai/oauth.go` 核对）

- 发行方：`https://auth.x.ai`，实现了 OIDC discovery（`/.well-known/openid-configuration`，
  `oauth.go:26`），是三家订阅路径中唯一的。
- 流程：Authorization Code + PKCE（S256）+ OIDC `nonce`；纯公共客户端，无 client_secret。
- 客户端身份：xAI 官方 client（`DefaultClientID = "b1a00492-..."`，`oauth.go:34`）。
- Scope：`openid profile email offline_access grok-cli:access api:access`（`oauth.go:35`）。
- Redirect：loopback `http://127.0.0.1:56121/callback`；同样手动粘贴授权码。
- 端点可经 `XAI_OAUTH_*` 环境变量覆盖，但带主机白名单校验（仅 `x.ai`/`*.x.ai`，`oauth.go:50-53`）。
- Session 存储是三家订阅路径中最完善的：一次性消费（`TryConsume`）、Redis 跨实例、本地回落
  （`oauth.go:106-214`）。

## 2.1 Device Authorization Flow（官方能力，2026-09-03 实测）

- 实抓 `https://auth.x.ai/.well-known/openid-configuration`：authority 官方声明
  `device_authorization_endpoint: https://auth.x.ai/oauth2/device/code`，
  `grant_types_supported` 包含 `urn:ietf:params:oauth:grant-type:device_code`，
  `token_endpoint_auth_methods_supported` 包含 `none`（公共客户端）。device flow 是 xAI
  官方标准能力，不是代理项目的逆向。
- 匿名实测：对 `/oauth2/device/code` POST `client_id=b1a00492-...` 与 sub2api 默认
  scope，返回 200 与标准 RFC 8628 响应：`device_code`、`user_code`（形如 `XXXX-XXXX`）、
  `verification_uri: https://accounts.x.ai/oauth2/device`、
  `verification_uri_complete`（携带 user_code）、`expires_in: 1800`、`interval: 5`。
- 标准轮询路径为对 `token_endpoint` 的 device_code grant 轮询；sub2api 的
  `sso_device.go` 只把该流程当作 SSO cookie 自动批准的载体（GET `verification_uri_complete`
  + POST `/oauth2/device/verify` + `/oauth2/device/approve`），其轮询段
  （`authorization_pending`/`slow_down` 语义）与 RFC 8628 一致。人工批准的
  标准用法不需要 verify/approve 两个私有端点。
- 探测仅证明端点接受匿名 device-session 创建，未证明完整授权、token 发行与刷新；
  一次探测不覆盖账号、地域与长期行为。

## 3. 旁路路径（非交互式，均非标准 OAuth 语义）

- **Device Code（RFC 8628）程序化**：`/oauth2/device/code` → `device/verify` → `device/approve`
  → token（`sso_device.go:22-25`）；实现上拿用户 SSO cookie 自动完成并自动批准，
  绕过了 device grant 本意中的第二台设备人工批准。
- **邮箱密码 → SSO**：xAI 私有登录端点换 SSO cookie，再转 token；密码不落盘。

## 4. 授权后与配额探测（`pkg/xai/subscription_tier.go`、`quota.go` 核对）

- 推理走订阅代理 `https://cli-chat-proxy.grok.com/v1`（OpenAI Responses 协议），
  而非 `api.x.ai` 开放端点（`oauth.go:30`）。
- 订阅分层不调接口，直接从 access token JWT 解码 `SubscriptionTier` 数值声明
  （`subscription_tier.go:22-43`：free / supergrok / x_basic / x_premium /
  supergrok_lite / supergrok_plus / supergrok_heavy 等）。
- 图片/视频生成有独立的计费探测门槛：未观察到正付费权益的 OAuth 账号被拒绝，
  免费/异常/缺失账单观测一律 fail-closed（README Grok 小节、`account_grok_media_eligibility`
  相关测试）。

## 5. 信息来源

xAI 的 OAuth 是官方公共流程，sub2api 代码内未标注外部出处，无社区中间层项目的依赖痕迹；
README 的 Grok 小节（`### OAuth Configuration`）把默认 client 细节与 `XAI_OAUTH_*`
环境变量写成公开配置项。与 Antigravity 路径的"官方客户端 → 社区固化 → 下游消费"链条相比，
Grok 路径的知识来源更直接。

## 6. 证据边界

- 本文全部协议事实只证明"代理项目源码当前这样实现"，不证明该实现能持续工作；
  xAI 对订阅代理、计费探测与设备授权的任何收紧都会直接影响该路径。
- Device Code 自动批准与邮箱密码旁路属于对标准流程的滥用，账号处置风险高于标准
  授权码路径。
- 一次源码核对不覆盖其他账号、地域、客户端版本或长期运行行为。
- 该路径属订阅 entitlement 的灰色使用，接入前须自行评估 ToS 与封号风险。
