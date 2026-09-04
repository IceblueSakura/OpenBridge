# Antigravity 订阅 OAuth 登录路径（sub2api 源码核对）

## 文档元数据

| 字段 | 值 |
|---|---|
| Source snapshot | sub2api 本地 checkout `main` @ `5097b31457e6dc9f49e5f5c9c72b925ce79543b3`（2026-09-03）；Antigravity-Manager `docs/proxy/auth.md`（raw.githubusercontent，2026-09-03 抓取）；Google Gemini Code Assist 消费级弃用公告（web search 摘要，2026-09-03） |
| Last reverified | 2026-09-03，对 sub2api 本地 checkout 做了源码与 git 历史实读核对；同日对 Google device flow 端点以该 client 做了一次匿名授权探测 |
| Scope | Antigravity（Google）订阅账号 OAuth 登录路径的端点、参数、授权后动作、对抗时间线与信息来源链；不含推理 wire 全量字段，不含 OpenBridge 实现 |
| Evidence boundary | 未用真实账号执行过任何一条授权流程；协议细节来自代理项目源码而非官方文档，不证明 Google 认可或保证稳定性；社区项目星数与时间线为检索自报数据 |
| Recheck trigger | 决定评估或接入该路径；sub2api/Antigravity-Manager 上游协议常量变化；Google 端点、客户端指纹校验或消费级订阅政策再变更 |

同批拆分文档：[Grok 订阅 OAuth](grok-oauth.md)、[OpenAI ChatGPT 订阅 OAuth](openai-chatgpt-oauth.md)。

## 1. 定位与证据性质

该路径不是官方开放 API：把面向交互式客户端（Antigravity IDE）的订阅 entitlement 当作服务端
凭据使用的社区逆向成果。本文固定协议事实和信息来源链，不构成接入建议；合规风险见 §5。

## 2. 协议事实（`backend/internal/pkg/antigravity/oauth.go`、`client.go` 核对）

- 流程：标准 OAuth 2.0 Authorization Code + PKCE（S256），端点为 `accounts.google.com/o/oauth2/v2/auth`
  与 `oauth2.googleapis.com/token`，完全借用 Google 官方授权面。
- 客户端身份：Antigravity 官方客户端的 OAuth client（`ClientID = "1071006060591-..."`，
  `client_secret` 为 `GOCSPX-` 前缀的内置值，见 `oauth.go:28`/`oauth.go:73`；本文不复制完整值）。
  同一 client_id 也出现在独立项目 `NoeFabris/opencode-antigravity-auth` 的 `src/constants.ts`，
  说明该身份由社区从官方客户端统一提取。
- Scope：`cloud-platform`、`userinfo.email`、`userinfo.profile` 外加两个官方客户端私有 scope
  `cclog` 与 `experimentsandconfigs`（`oauth.go:43-47`）。
- Redirect：固定 `http://localhost:8085/callback`；网关无法接收用户浏览器跳转，用户需手动复制
  授权码回粘贴（OOB 交互）。
- Google device flow 不可用（2026-09-03 实测）：以该 client_id 匿名 POST
  `https://oauth2.googleapis.com/device/code`（scope=cloud-platform+userinfo 子集），
  authority 返回 401 `invalid_client: Invalid client type`。该 client 为 Desktop/Web 类型，
  Google 的 TV/limited-input device grant 需要单独类型的客户端，因此 Antigravity 路径
  无法获得 device flow，只能沿用 OOB 授权码。这也与全部已知社区实现（sub2api、
  opencode-antigravity-auth、Antigravity-Manager）一致。
- User-Agent：`antigravity/<版本号> <平台>` 伪装官方客户端，版本号可配置且需跟随官方升级
  （默认曾从 1.11.9 一路 bump 到 1.23.2，见 §4）。

## 3. 授权后动作（`service/antigravity_oauth_service.go`、`pkg/antigravity/client.go` 核对）

- 调 `cloudcode-pa.googleapis.com/v1internal:loadCodeAssist` 获取 `cloudaicompanionProject`
  （project_id）与订阅档位；失败时自动 `v1internal:onboardUser` 开通（`client.go:440`/`513`）。
- 立即调 `v1internal:setUserSettings` 关闭隐私上报，并用 `fetchUserInfo` 二次验证
  （`service/antigravity_privacy_service.go`）。
- 推理走 `v1internal:generateContent` / `v1internal:streamGenerateContent?alt=sse`，请求信封为
  `{project, requestId: "agent-<uuid>", requestType: "agent", userAgent: "antigravity", model, request}`
  （`request_transformer.go:184-190`、`service/antigravity_gateway_service.go:620-627`）。
- 端点有两组：生产 `cloudcode-pa.googleapis.com` 与 `daily-cloudcode-pa.googleapis.com`；
  daily 域名曾为 `*.sandbox.googleapis.com`，2026-08-13 随官方客户端改为非 sandbox 域名
  （`oauth.go:55-57`，commit `21c07e835`）。

## 4. 信息来源链

1. **事实源：Antigravity 官方客户端**。Antigravity 是闭源 Electron 应用（VS Code fork），
   `app.asar` 解包即得明文 JS；client_id/secret、scope、端点、信封字段均以常量形式存在。
   这是高置信推断：多个独立项目持有逐字符相同的 client_id 与 secret 前缀，但未见作者自述的
   第一手逆向文章（证据边界见 §6）。
2. **社区固化：`lbjlaq/Antigravity-Manager`**（2025-11-26 创建，Tauri/Rust，检索自报约 3 万星）。
   sub2api 代码注释多处写明出处：`schema_cleaner.go:9` 注明参考其
   `src-tauri/src/proxy/common/json_schema.rs`；`client.go:47`、`oauth.go:148`、
   `request_transformer.go:268/300` 标注"与 Antigravity-Manager 保持一致"。
   2026-01-17 commit `cc0fca35e` 标题即"同步 Antigravity-Manager 的请求逻辑"；
   2026-04-15 该仓库甚至被误提交为 sub2api 的 git submodule
   （Subproject commit `a9d96bd54978c22d3033830debfe77aeeeee2500`，次日移除）。
3. **交叉印证：开源 gemini-cli**。`cloudcode-pa.googleapis.com/v1internal:loadCodeAssist` 等端点
   同样出现在 `google-gemini/gemini-cli` 仓库的公开报错里，说明 Antigravity 与 Gemini CLI 共享
   同一 Code Assist 后端，端点真实性几乎零成本确认。
4. **协议边界靠试错收敛**。例：`opencode-antigravity-auth/docs/ANTIGRAVITY_API_SPEC.md`
   （标注 "Verified by Direct API Testing"）汇总了会触发 400 的 JSON Schema 字段；
   sub2api 的 schema 清理逻辑照抄该清单。
5. **产品背景**：Google 于 2026-06-18 停止 Gemini CLI 的个人消费路径（Google AI Pro/Ultra 与
   Code Assist Individual 免费档），官方迁移方向为 Antigravity 家族（含 `agy` CLI）。
   Antigravity 路径因此成为个人订阅的主要代理通道。来源：developers.googleblog.com 与
   developers.google.com 弃用文档的检索摘要。

## 5. 协议对抗时间线（sub2api git 历史，已逐条核对）

| 日期 | commit | 事件 |
|---|---|---|
| 2025-12-28 | `6648e6506` | Antigravity OAuth 首次引入；UA 自报 `sub2api` |
| 2026-01-08 | `6e8188ed6` | 修复频繁 429：UA 改伪装官方、切 `daily-*.sandbox` 端点、补 Accept/Host header |
| 2026-01-17 | `cc0fca35e` | 同步 Antigravity-Manager 的请求逻辑 |
| 2026-02~03 | 多个 | 跟进 429 策略：读上游 `retryDelay`、冷却 5min→30s、credits 耗尽识别 |
| 2026-02-24 起 | 多个 | UA 版本号可配置并持续 bump（1.18.4→1.19.6→1.20.5→1.23.2），对应上游客户端指纹校验收紧 |
| 2026-04-15 | `21f22b509` | 移除误提交的 Antigravity-Manager submodule |
| 2026-08-13 | `21c07e835` | daily 域名去 sandbox，与官方客户端对齐 |

## 6. 证据边界

- 本文全部协议事实只证明"代理项目源码当前这样实现"，不证明该实现能持续工作；
  Google 已有 2026-06-18 一次性砍掉 Gemini CLI 个人路径的先例，该路径同样可能被
  端点迁移、指纹校验或账号处置收紧。
- "通过 asar 提取获得 client_id/secret"为多项目同值指纹支撑的高置信推断，缺一手自述。
- Antigravity-Manager 星数、创建日期等社区数据来自检索摘要，未逐项复核。
- 一次源码核对不覆盖其他账号、地域、客户端版本或长期运行行为。
- 该路径属订阅 entitlement 的灰色使用，接入前须自行评估 ToS 与封号风险。
