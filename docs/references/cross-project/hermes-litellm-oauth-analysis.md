# Hermes 与 LiteLLM 的 ChatGPT OAuth 实现调研

## 状态与范围

**外部实现调研；不代表本项目已实现，也不构成上游 OAuth 使用授权。**

**矩阵角色。** 本文只用于识别 OAuth client identity、账户绑定、refresh 与本地 credential 存储不可被 proxy 外推的风险。它不构成 Codex/Hermes 的当前主要参考目标，也不授权 OpenBridge 导入本地 auth file、复用 device flow 或建立账号池。

- Hermes Agent 源码快照：`F:/codespace/hermes-agent`，commit `e598cef87465981fcea1c0339edfcf5d9716c917`。
- LiteLLM 源码快照：`F:/codespace/litellm`，commit `bd44c9e305b89526d4c5d773ee39ca935561b9c8`。
- 调研范围：两个项目对 ChatGPT/Codex subscription 的 device-code 登录、token 持久化、refresh、账户 header 与运行期认证路径。
- 未读取、输出、复制任何本地 credential、token、`auth.json` 内容或 OAuth client ID。
- 行号仅适用于上述快照。上游 endpoint、client registration、token policy 与服务条款必须以当前官方资料重新确认。

## 1. 结论摘要

1. Hermes 与 LiteLLM 都内置 ChatGPT/Codex subscription 的上游 OAuth client：device code → 用户授权 → authorization code + PKCE verifier → token exchange → 本地持久化。它们不是为 proxy 下游用户提供登录的 OAuth authorization server。
2. 两者均向 ChatGPT/Codex backend 使用 bearer token，并从 JWT/id token 提取 `chatgpt_account_id` 后附加 `ChatGPT-Account-Id`。所以有效 route context 不只是 access token 字符串。
3. Hermes 的实现面向本地 Agent：其 `auth.json` 受跨进程 auth-store lock 保护，并有 token rotation 同步、credential pool、终态 credential 隔离和来自 Codex CLI auth file 的 best-effort self-heal。
4. LiteLLM 的 `Authenticator` 是单一 JSON 文件、按需 refresh 的实现。当前类中可观察到普通 `open()`/`json.dump()` 读写，但没有 file lock、atomic replace、credential version/CAS 或跨 worker refresh single-flight。
5. 两者均依赖 Codex/ChatGPT 专用的 client/endpoint/header 行为。它们证明已有客户端的实现方式，**不证明**第三方多租户 proxy 可以复用 OAuth client identity、device flow、私有 endpoint 或 credential 文件。

## 2. 已验证机制对比

| 方面 | Hermes `openai-codex` | LiteLLM `chatgpt` |
|---|---|---|
| 登录方式 | Device-code flow；取得 authorization code 与 PKCE verifier 后 exchange | Device-code flow；同样取得 authorization code 与 PKCE verifier 后 exchange |
| 默认本地状态 | `~/.hermes/auth.json` 的 provider state，另有 `credential_pool` | `~/.config/litellm/chatgpt/auth.json`；可由环境变量改路径 |
| refresh 提前量 | JWT `exp` 前 120 秒 | JWT `exp` 前 60 秒 |
| refresh 并发 | 读写与 refresh re-check 使用跨进程 auth-store lock | 当前 `Authenticator` 中未见文件锁、原子写或跨进程协调 |
| 多账号 | credential pool 支持多条独立 entry、优先级和轮换 | 当前 `Authenticator` 读取一个 auth file，未见 account pool |
| stale token 恢复 | 可从 `~/.codex/auth.json` best-effort 重导入有效 token pair | refresh 失败后进入 device-code re-login |
| 请求身份上下文 | bearer、account ID、Codex-shaped originator/User-Agent（特定路径） | bearer、account ID、originator/User-Agent、session ID |
| 终态失败 | pool entry 可标记 `dead` 并退出轮换 | refresh 错误记录 warning，回退到重新登录 |

## 3. Hermes：本地 Agent 的 credential lifecycle

### 3.1 登录与状态写入

`_login_openai_codex()` 优先尝试现有 Hermes credential；然后可经用户确认导入 Codex CLI 的 `~/.codex/auth.json`；否则运行新的 device-code 登录并写入 Hermes 自己的 auth store（`hermes_cli/auth.py:7030-7101`）。

`_codex_device_code_login()` 的实际顺序为：

```text
request device user code
  -> user visits the displayed verification URL and authorizes
  -> poll device token endpoint for authorization code + PKCE verifier
  -> authorization-code exchange
  -> return access/refresh token pair and backend base URL
```

源码在 `hermes_cli/auth.py:7357-7499`。实现包含登录端 429 的有限重试/`Retry-After` 处理，以及 15 分钟 poll timeout；这属于本地交互体验，不是 proxy 用户授权协议。

`_save_codex_tokens()` 在持久化 provider state 时记录 `auth_mode: "chatgpt"`，同步适用的 pool entry，并清理被新 token 覆盖 entry 的错误状态（`hermes_cli/auth.py:3469-3494`）。该同步明确避免以新登录账户覆盖独立手工添加的其他账户。

### 3.2 refresh、rotation 与进程协调

`resolve_codex_runtime_credentials()` 先检查 access token 的 JWT `exp`；默认在 120 秒窗口内触发 refresh（`hermes_cli/auth.py:2295-2300`、`3725-3838`）。需要 refresh 时，它在 auth-store lock 内重新读取 token，再确认是否仍需 refresh，避免两个 Hermes 进程直接使用同一个旧 refresh token。

`refresh_codex_oauth_pure()` 发送标准 `grant_type=refresh_token` 请求，接受 rotated refresh token，并将 OAuth error shape 映射为是否要求 re-login（`hermes_cli/auth.py:3513-3644`）。成功 token pair 会立即写回 auth store（`3647-3688`）。

当 refresh 被拒绝为 re-login required 时，`_refresh_codex_auth_tokens()` 会 best-effort 从 Codex CLI 的 `~/.codex/auth.json` 重导入一对当前有效 token（`3647-3722`）。源码注释明确指出 refresh token rotation/单次使用会使不同本地副本互相变旧。

**边界：** 这是本地文件与本地进程之间的协调；并不包含 proxy 所需的 vault version/CAS、跨节点 lock lease、refresh outcome 传播、审计或租户隔离。

### 3.3 credential pool 与失败隔离

Hermes 的 `CredentialPool` 允许同 provider 多条 credential，记录 priority、request count、最后错误、rate-limit reset 和状态（`agent/credential_pool.py:164-240`）。对于 token invalidated、token revoked、`invalid_grant` 与 `refresh_token_reused` 等已知终态 OAuth 错误，401 entry 会转为 `dead`，而不是在普通 cooldown 后重新投入 rotation（`63-98`、`627-679`）。

对 singleton-seeded `device_code` entry，pool 会在选择前从 auth store 同步发生 rotation 的 token pair，避免重复使用已被另一个进程刷新过的 refresh token（`agent/credential_pool.py:733-795`）。手工添加的独立 device-code credential 不应被 singleton 的新登录覆盖，相关保护在 `hermes_cli/auth.py:3368-3467`。

### 3.4 请求认证与 account context

Hermes 的 Codex auxiliary path 使用 bearer；其 `_codex_cloudflare_headers()` 从 JWT 的 `https://api.openai.com/auth.chatgpt_account_id` claim 提取 `ChatGPT-Account-ID`，同时为特定 backend 设置 Codex-shaped `originator` 与 User-Agent（`agent/auxiliary_client.py:734-770`）。

这表明 provider route 至少需要把以下内容作为同一认证上下文处理：

- credential identity/version；
- issuer 与允许的 endpoint；
- access/refresh token pair；
- account/workspace identity；
- 必要的非 secret header policy。

不能在 refresh、retry、fallback 或 pool rotation 后只保留 bearer 而丢掉 account binding。

## 4. LiteLLM：单机 auth file 的按需 refresh

### 4.1 storage 与 token 获取

LiteLLM `Authenticator` 默认将状态放在 `~/.config/litellm/chatgpt/auth.json`，并支持 `CHATGPT_TOKEN_DIR` 与 `CHATGPT_AUTH_FILE` 覆盖（`litellm/llms/chatgpt/authenticator.py:31-41`）。

`get_access_token()` 的运行期流程为：

```text
read auth file
  -> access token is valid beyond skew: return it
  -> otherwise, refresh token exists: refresh and write file
  -> refresh failed or no credential: start/wait for device-code login
```

实现见 `authenticator.py:43-64`。token expiry 优先使用 auth file 内 `expires_at`；缺失时从 access token JWT `exp` 推导，并以 60 秒 skew 判断过期（`102-130`）。

### 4.2 device-code 与 refresh

LiteLLM 使用 device-auth endpoint 申请 user code，向用户输出 verification URL 和 code，轮询 authorization code/PKCE verifier，再进行 authorization-code exchange（`authenticator.py:143-286`）。成功 token record 包含 access token、refresh token、id token、expiry 和 account ID（`330-341`）。

refresh 使用 `grant_type=refresh_token`；若返回新 refresh token 则替换旧值，否则保留旧值；随后写回同一 auth file（`288-328`）。如果 refresh 失败，`get_access_token()` 记录 warning 后发起 device-code login（`49-64`）。

### 4.3 account/header 与调用路径

`get_account_id()` 优先读取保存的 account ID；否则解析 id token/access token 的 `chatgpt_account_id` claim，并把推导结果写回 auth file（`authenticator.py:66-79`、`132-141`）。

Chat 与 Responses provider 均在 request validation 期间调用 authenticator：

- Chat：取得动态 API base/access token，再形成默认 header（`litellm/llms/chatgpt/chat/transformation.py:26-61`）。
- Responses：取得 token、account ID、session ID，并将默认 header 与调用方 header 合并（`litellm/llms/chatgpt/responses/transformation.py:41-59`）。
- 默认 header 包含 `Authorization: Bearer ...`、`ChatGPT-Account-Id`（可用时）、`originator`、User-Agent、`session_id`（`litellm/llms/chatgpt/common_utils.py:228-246`）。

Responses transform 还强制 stream、`store: false`，并插入 Codex-shaped default instructions（`responses/transformation.py:61-104`）。这不是通用 OpenAI API-key adapter 的等价实现。

### 4.4 并发与服务端适用边界

`Authenticator._read_auth_file()` 使用普通文件读取，`_write_auth_file()` 以普通写入和 `json.dump()` 覆盖文件（`authenticator.py:85-100`）。在这个类的范围内，未观察到：

- 同一 auth file 的 file lock 或 process-wide lock；
- 临时文件 + atomic replace；
- refresh token version/CAS；
- 多 worker/多副本的 refresh single-flight；
- 多账号选择、rotation 或 account isolation。

因此，不能把该类直接视为多副本 proxy 的 credential manager。并发请求同时看到过期 token 时，可能并行 refresh；若 authority rotation/单次使用 refresh token，简单最后写入不能保证正确性。

LiteLLM proxy 中另有实验性的 MCP outbound OAuth credential store；它服务于 MCP server 的每用户 OAuth，不等同于 `chatgpt` provider 的上游 ChatGPT subscription credential，不能混为一条设计依据。

## 5. 对本项目的约束与可借鉴点

### 5.1 不能从两套实现推出的结论

以下均未被本调研证实：

1. OpenAI 是否允许第三方 proxy 作为该 ChatGPT/Codex OAuth flow 的 client；
2. 是否存在公开、可供本项目注册使用的 client、redirect URI、scope/resource、token endpoint 和 refresh contract；
3. 是否允许 server/proxy 使用这些 subscription token，或转发给多个下游 user/tenant；
4. 当前 endpoint 对 originator、User-Agent、account header、模型 allow-list、SSE 和会话 continuation 的完整要求；
5. 上述实现细节能否跨版本稳定。

因此，项目仍必须把真实 Codex/ChatGPT OAuth 置于 OAuth preflight 硬门之后。没有明确许可与独立 client contract 时，只能保留 mock adapter 或标准 API-key upstream。

### 5.2 可借鉴的安全性质，而不是私有实现细节

| 调研观察 | 对 proxy 的正确抽象 |
|---|---|
| token pair 可能发生 rotation | 将 access/refresh token 与 credential version 作为原子 bundle 处理。 |
| account ID 参与请求身份 | 在 `RouteSnapshot` 绑定 account/workspace context，refresh/retry/fallback 不得脱离。 |
| Hermes 先锁定、重读、再 refresh | 多实例版需 distributed single-flight、vault version/CAS 与 outcome propagation。 |
| Hermes 将永久 auth 错误移出 rotation | 认证错误、使用配额、暂态 upstream 故障应使用不同状态和恢复策略。 |
| LiteLLM 每次 validation 读取当前 token | 不要把短期 bearer 永久固化在长生命周期 client；但读取策略必须配合安全并发控制。 |
| 两者均有 Codex-shaped headers | 将 header policy 视为 provider-specific capability/route context；不得伪造、硬编码或依赖未经授权的 client identity。 |

### 5.3 需求映射

本调研强化而不改变以下既有需求：

- `FR-09`：真实 Codex OAuth 必须在明确上游许可、正式 client registration、授权语义与安全存储方案通过 preflight 后才可启用；不得导入/复用 CLI auth file 或模拟其 client/header identity。
- `FR-10`：用户 OAuth、proxy access control 与 upstream credential routing 必须分离；本调研中的 token 是 upstream account credential，不是下游 user session。
- `FR-11`：proxy credential model 必须包含 provider、issuer、version、account/workspace binding、secret reference、状态与审计字段。
- `NFR-03`：access/refresh token 不能出现在普通日志、数据库、trace、错误或 fixture；日志只允许 metadata 与不可逆 fingerprint。

## 6. 验证记录

针对上述源码快照执行的定向测试：

```text
LiteLLM:
uv run pytest -q tests/test_litellm/llms/chatgpt/test_chatgpt_authenticator.py \
  tests/test_litellm/llms/chatgpt/responses/test_chatgpt_responses_transformation.py
22 passed in 3.49s

Hermes:
uv run pytest -q tests/hermes_cli/test_auth_codex_provider.py \
  tests/hermes_cli/test_auth_codex_self_heal.py \
  tests/agent/test_credential_pool.py -k 'codex'
46 passed, 81 deselected in 2.29s
```

这些测试验证的是本地实现的预期行为，不能替代合法 OAuth preflight、实际 upstream 流量验证、授权/条款确认或 proxy 的多实例安全验证。

## 7. 关联文档

- [产品范围](../../functional-requirements/product-scope.md)
- [配置、凭证与受信边界](../../functional-requirements/configuration-and-credentials.md)
- [Codex OAuth 与工具调用源码调研](../codex/codex-oauth-and-tool-call-analysis.md)
- [Hermes Agent Chat/Responses 分析](../hermes/hermes-chat-responses-analysis.md)
- [LiteLLM Chat/Responses 分析](../litellm/litellm-chat-responses-analysis.md)
- [当前代码架构](../../implementation-status/current-architecture.md)
