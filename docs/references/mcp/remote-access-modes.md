# MCP 远程访问模式调研（互联网访问）

## 范围与证据

- 调研对象：MCP 规范 2026-07-28 的 transports / authorization 相关页面（2026-08-08 抓取）、官方 remote server 指南、Arcade.dev MCP Gateway Pattern 文章（2026-07-23 更新）、Cloudflare remote MCP 官方文档（developers.cloudflare.com，2026-08-08 抓取）、ArcadeAI/mcp-rust-sdk OAUTH_SUPPORT.md（2026-08-08 抓取）。
- 本文是**外部协议事实与部署模式调研**，不构成 OpenBridge 的功能承诺；"推论"小节明确标注。

关键结论：**互联网远程访问 MCP server 的唯一标准传输是 Streamable HTTP**（stdio 是本地子进程传输，不跨网络）。2026-07-28 把协议无状态化是远程化的分水岭——去掉了 session 与长连接 SSE，使 MCP server 可以被网关、负载均衡、CDN 和无服务器平台直接托管。远程 server 的认证按规范是 OAuth 2.1（resource server 模型 + RFC 8707 资源指示器），且公网暴露无认证 server 已成为已观测的安全问题（约 1000+ 个）。

## 1. 传输基础：Streamable HTTP

### 1.1 消息形态（2026-07-28）

- **单端点**：所有消息都是对同一个 URL（如 `https://example.com/mcp`）的 HTTP POST，JSON-RPC 2.0 编码。
- **响应形态**：`application/json`（普通结果）或 `text/event-stream`（请求内 SSE 流——当服务器要推送通知或中间结果时）。
- **无状态**：无 `Mcp-Session-Id`、无独立 GET/DELETE 流、无 `Last-Event-ID` 恢复。协议版本与客户端能力通过请求体 `_meta` 携带；HTTP 层仅做镜像。
- **订阅**：`subscriptions/listen` 是一个长连接 POST，响应流承载服务器到客户端的变更通知（tools list_changed、resources updated、progress、logging）；按类别 opt-in，断连后需重新发起，不跨连接恢复。
- **取消**：客户端关闭请求的响应流即放弃 in-flight 请求（HTTP 层面），不再依赖 `notifications/cancelled` 单独消息。

### 1.2 请求元数据镜像（对网关/代理的关键设计）

Streamable HTTP 把 body 字段镜像到 HTTP 头，使中间件**无需解析 JSON body 即可路由**：

| Header | 来源 | 必填场景 |
|---|---|---|
| `MCP-Protocol-Version` | `_meta.io.modelcontextprotocol/protocolVersion` | 所有请求 |
| `Mcp-Method` | JSON-RPC `method` | 所有请求 |
| `Mcp-Name` | `params.name` / `params.uri` | `tools/call`、`resources/read`、`prompts/get` |
| `Mcp-Param-{Name}` | `inputSchema` 中标注 `x-mcp-header` 的参数 | 有该参数时 |

- 头与 body 不一致时服务器返回 `-32020 HeaderMismatch`（400）；参数缺失/为 null 时客户端必须省略对应头。
- 非 ASCII/空白/换行等无法裸走 header 的值自动 Base64 包装为 `=?base64?...?=`。
- 对网关的意义：代理可以按 `Mcp-Method`/`Mcp-Name` 做白名单、按 `Mcp-Param-*` 做租户/区域路由，不必解析请求体。

### 1.3 安全与端点要求

- 服务器 **MUST 校验 `Origin` 头**（防 DNS rebinding）；本机运行时**应只绑 localhost**；公网部署**应实现认证**。
- `Accept` 需包含 `application/json` 与 `text/event-stream` 之一；流式响应场景可用 `X-Accel-Buffering: no` 关闭中间层缓冲。
- 旧版兼容：`2025-03-26`/`2025-11-25` 客户端若发 `initialize` 或带 `Mcp-Session-Id`，2026-07-28 服务器按"向后兼容"规则处理（`400`/`UnsupportedProtocolVersionError` 引导客户端重试正确版本）；legacy HTTP+SSE（2024-11-05 两端点）已废弃。

## 2. 远程认证与授权：OAuth 2.1 框架

### 2.1 角色模型

- MCP server = **OAuth 2.1 resource server**；MCP client = **OAuth client**；token 由独立 **authorization server** 签发。
- 授权对 HTTP 传输的 MCP 是**可选但强烈建议**——规范允许裸奔，这正是公网大量无认证 server 的根源。
- 2026-07-28 增加 6 项要求（对 2025-06-18 基线），核心是 audience 校验与 token 纪律，OAuth 架构本身未变。

### 2.2 关键机制

| 机制 | 要求 | 目的 |
|---|---|---|
| RFC 8707 resource 参数 | 客户端在 authorization 与 token 请求中都带 `resource`（MCP server 的 canonical URI） | token 绑定到特定 server；AS 把 URI 写入 `aud` claim |
| audience 校验 | server 验证 token 的 `aud` 等于自己的身份；失败按 OAuth 2.1 §5.3 返回 401 | 防 token 被重放到伪装 server |
| token passthrough 禁止 | server 不得把客户端 token 转传给上游 API；上游需各自签发 token | 防 token 横向扩散 |
| scope 协商 | 从 `WWW-Authenticate` → Protected Resource Metadata → AS metadata 依次选择 scope；403 insufficient_scope 可触发 scope 升级（SEP-835） | 最小权限 |

### 2.3 发现机制

- **授权服务器位置**：server 元数据 `authorization_servers` 字段声明。
- **Protected Resource Metadata**（RFC 9728）：401 响应带 `WWW-Authenticate`；资源元数据位于 `https://example.com/.well-known/oauth-protected-resource[/<path>]`。
- **Authorization Server Metadata**（RFC 8414 / OIDC）：`/.well-known/oauth-authorization-server` 或 `/.well-known/openid-configuration`；**必须校验 `issuer`**（防攻击者伪造发现文档）。
- **Client Registration**：DCR（RFC 7591）已废弃，改走 **Client ID Metadata Documents（CIMD，SEP-991，URL 式 client 注册）**。

## 3. 2026-07-28 无状态化对远程部署的意义

| 维度 | 2025-11-25（session 模型） | 2026-07-28（无状态） |
|---|---|---|
| 连接 | initialize 握手 + 连接级 session | 每请求自包含（`_meta` 声明版本/能力） |
| 服务器推送 | 连接级 SSE 长连接 | 仅请求内 SSE / `subscriptions/listen` 显式长连接 |
| 网关/代理 | 需 sticky session / session 同步 | 任意实例、任意负载均衡、CDN 友好 |
| 无服务器平台 | session 状态需外部存储 | 天然适配（每次请求独立执行） |
| 多客户端 | 每客户端一个连接 | 无连接概念，按请求鉴权 |

推论：无状态化让 MCP server 从"进程内服务"变成了"普通 HTTP API"，这是它能够被标准 API 网关（Kong、Tyk、Zuplo 等）托管的前提，也是 OpenBridge 这类网关能够代理 MCP 流量的协议基础。

## 4. 部署模式全景

### 4.1 直接公网托管

- 一个 HTTPS 端点 + OAuth 2.1 授权服务器；TLS 必须，认证必须。
- 参考实现：Cloudflare Workers 官方模板（`workers-oauth-provider` TypeScript 库把 Worker 包装成 OAuth provider + MCP resource server，KV 存 session/token），部署到 `*.workers.dev/mcp`。
- 本地客户端连远程：用 `mcp-remote`（npx 包装器）把远程 URL 包装成本地 stdio server，Claude Desktop 等只支持 stdio 的客户端即可接入。

### 4.2 MCP 网关模式（对网关类项目最相关）

**定义**：网关是一个**单一 MCP 入口**，把多个 MCP server 的工具联合成一个受管工具面。价值不是多一跳，而是把认证、策略、路由、遥测集中化（Arcade.dev 文章）。

架构：`MCP clients/hosts → Gateway（单一 MCP 端点 + 策略 + 遥测）→ 后端 MCP servers → 上游系统/API`

关键设计原则（Arcade 文章，转述为外部观点）：

1. 默认 Streamable HTTP 作为传输主干；实现时必须校验 Origin、本机绑 localhost、实现认证。
2. **分离 front-door identity 与 downstream authorization**：网关鉴权"谁在调用"（API key + 可选 end-user 标识传播），工具级授权"能调什么、什么 scope"——两者是不同控制面。
3. **Token 纪律**：audience 绑定、禁止 passthrough、上游 token 独立签发。
4. **Curated tool surfaces 而非全量聚合**：allowlist、per-agent/per-workflow 视图、surface 使用说明；"proxy everything"是快路径但会再造工具过载。
5. **身份相关的缓存必须 identity-scoped**（包括"可用工具列表"），不能跨调用者共享。
6. 运维控制面：集中认证/策略、路由与请求整形、限流、审计日志（谁、何时、做了什么）、tracing。

**构建顺序建议**（Arcade）：单端点 → Origin 校验 + 安全绑定 → 每请求 front-door 认证（fail closed）→ 工具 allowlist → per-agent 视图 → surface 说明 → token 纪律 → 结构化审计 → 按调用方/工具类别限流 → 按需工具发现。

市场现状（2026）：Microsoft 开源 MCP reverse proxy（Kubernetes session-aware routing + Azure Entra ID）、Kong（OAuth 2.1 原生支持 + MCP Registry）、Pomerium（identity-aware，工具级授权）、Tyk、WSO2、Zuplo、MCPJungle（轻量网关+registry）等；Arcade.dev 是专注 MCP 运行时的商业产品。注册表（discovery）与网关（runtime control）是两个不同层次，不能混为一谈。

### 4.3 隧道 / 内网暴露

- Cloudflare Tunnel + Zero Trust（或 VPN）：适合访问内部数据/有特定托管要求的 server，不直接公网开放。

### 4.4 安全现状事实

- Bitsight TRACE 报告约 1000 个、Pomerium 引用研究称 1862 个公网暴露的无认证 MCP server（可被拉取工具列表与元数据）。
- 推论：任何公网 MCP 端点必须先有真实授权机制；网关是"前门"式统一安全基线。

## 5. Rust 生态对远程访问的支持对照（2026-08-08 快照）

| 能力 | rmcp 3.x | rust-mcp-sdk 1.x | pmcp 2.x |
|---|---|---|---|
| Streamable HTTP | ✅ Tower service 挂任意 router | ✅ `create_axum_server` | ✅ axum 便捷层 |
| 无状态（2026-07-28） | ✅ 默认 | ❌ session 模型 | ❌ |
| DNS rebinding / Origin 校验 | 自行装配（Tower 中间件层） | ✅ 内置（allowed_hosts 自动推导） | ✅ 内置（DnsRebindingLayer 等） |
| CORS / 安全头 | 自行装配 | 部分（依赖 tower-http 惯例） | ✅ 内置 |
| OAuth 2.1 | `auth` feature（OAuth 2.0 支持）；ArcadeAI 分支 OAUTH_SUPPORT 文档最全：PKCE S256、RFC 8707、RFC 9728、RFC 8414、DCR、CIMD(SEP-991)、scope 升级(SEP-835) | ✅ RemoteAuthProvider（DCR 兼容 IdP：Keycloak/WorkOS/Scalekit）+ OAuthProxy（开发中） | ✅ OAuth 2.0 + Bearer + OIDC（未逐条验证 profile） |
| TLS/HTTPS | 交给托管 HTTP 栈 | `rust-mcp-axum` 的 `ssl` feature；文档建议生产 TLS | 文档建议生产 TLS |
| health check | 自行添加 | ✅ 内置 `/health` | 未查证 |
| 与网关配合 | ✅ 请求头镜像自动发出（≥2026-07-28）；`with_json_response(true)` 减少 SSE（对代理友好） | 需自行验证 | 需自行验证 |

对 OpenBridge 的推论：若未来让 OpenBridge 充当 MCP 网关/代理，rmcp 的 Tower service 形态可直接嵌入既有 axum 路由；其自动的 `Mcp-Method`/`Mcp-Name`/`Mcp-Param-*` 头镜像正好服务于网关级路由与审计（无需解析 body）。rust-mcp-sdk 则适合"快速暴露一个自带全部安全项的独立 MCP server"，代价是协议停在 2025-11-25 且绑定其 server 生命周期。

## 6. 证据边界与未验证项

- Authorization 规范主页（`/specification/2026-07-28/basic/authorization`）抓取失败两次，2.x 机制综合自 authorization-server-discovery 页、2025-06-18 版规范页、Descope/WorkOS/Solo.io 文章与 Arcade 文章；与 2026-07-28 正式文本的差异以官方页为准。
- ArcadeAI/mcp-rust-sdk 与官方 modelcontextprotocol/rust-sdk 的关系未深入核实（同名分支/衍生仓库）；其 OAUTH_SUPPORT.md 内容按来源标注。
- 网关市场名单来自 2026 年各厂商博客，未逐一验证其"原生支持"的深度；只用于说明生态方向。
- "1000+ / 1862 个无认证 server"来自 Bitsight 与 Pomerium 引用，数字口径（扫描时间、范围）未独立复核。
- 各库 TLS/CORS 细节未逐一读源码，标注为"需自行验证"处保持开放。
