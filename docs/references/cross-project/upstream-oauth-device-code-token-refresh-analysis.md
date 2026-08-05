# 上游 OAuth 2.0 设备码登录与 token 刷新综合调研

## 1. 状态、规范与项目级前置文档

本文是标准与四个项目调研结果的综合比较，不记录任何具体网关的实现状态或实施方案。

规范与官方资料：

- [RFC 8628: OAuth 2.0 Device Authorization Grant](https://www.rfc-editor.org/rfc/rfc8628.html)
- [RFC 6749: OAuth 2.0 Authorization Framework](https://www.rfc-editor.org/rfc/rfc6749.html)
- [RFC 9700: Best Current Practice for OAuth 2.0 Security](https://www.rfc-editor.org/rfc/rfc9700.html)
- [Codex authentication](https://learn.chatgpt.com/docs/auth)

项目级调研：

- [Codex 设备登录与 token 刷新](../codex/codex-device-auth-token-refresh-analysis.md)
- [CLIProxyAPI 的 Codex OAuth 与后台刷新](../cliproxyapi/cliproxyapi-codex-oauth-refresh-analysis.md)
- [Hermes Agent 的 Codex OAuth credential lifecycle](../hermes/hermes-codex-oauth-refresh-analysis.md)
- [LiteLLM ChatGPT authenticator](../litellm/litellm-chatgpt-oauth-refresh-analysis.md)

各项目文档固定源码 commit、阅读范围与局部结论；本文只保留跨项目比较。

## 2. 标准设备授权基线

RFC 8628 的设备授权流程是：

1. 客户端向 device authorization endpoint 提交已注册的 client identity 与 scope；
2. authorization server 返回 `device_code`、`user_code`、`verification_uri`、过期时间和 poll interval；
3. 用户在另一台有浏览器的设备完成认证和授权；
4. 客户端以 device-code grant 轮询 OAuth token endpoint；
5. 成功响应直接返回 token，失败则按标准 error 调整或终止。

标准轮询结果：

| 结果                    | 行为                              |
|-------------------------|-----------------------------------|
| `authorization_pending` | 保持当前 interval 继续            |
| `slow_down`             | 本次及后续 interval 至少增加 5 秒 |
| `access_denied`         | 终止本次登录                      |
| `expired_token`         | 终止并重新开始                    |
| 网络超时                | 降低轮询频率，避免紧密循环        |

设备 code 需要只展示给刚刚主动发起登录的用户，并附带防钓鱼提示。

## 3. Codex 产品 flow 与 RFC 8628 的差异

四个项目的 Codex/ChatGPT adapter 都观察到同一类产品 flow：

```text
private device user-code request
  -> display verification URL and code
  -> poll private device-auth endpoint
  -> receive authorization code + PKCE material
  -> authorization-code + PKCE token exchange
```

它与标准 RFC 8628 的主要差异是：

| 维度            | RFC 8628                           | 四个项目中的 Codex flow                      |
|-----------------|------------------------------------|----------------------------------------------|
| poll target     | OAuth token endpoint               | 私有 device-auth endpoint                    |
| poll credential | `device_code`                      | 私有 device auth ID                          |
| pending signal  | OAuth JSON error                   | 当前实现使用 HTTP 403/404                    |
| poll success    | access/refresh token               | authorization code + PKCE verifier/challenge |
| 后续交换        | 无额外 authorization-code exchange | 需要 authorization-code exchange             |

因此不能把 Codex 私有 endpoint、status 和字段当作标准 device authorization adapter 的通用语义。

## 4. ChatGPT 数据面 request identity 快照

request identity 不是 OAuth grant 的标准字段，但三个 ChatGPT/Codex 数据面实现会把 token/account context 与非 secret client header 一起
组装。以下只描述固定源码快照：

| 项目 | 快照中的数据面 header 行为 |
|------|----------------------------|
| Codex stable `rust-v0.146.0` / `e363b08c9175ac1cbe5893615dd2cb9ddf95043b` | workspace version 为 `0.146.0`；default client 使用 `originator: codex_cli_rs`，并按 `originator/version (OS type OS version; architecture) terminal` 构造运行时 UA；Responses HTTP request 另设置 `Accept: text/event-stream`。 |
| LiteLLM `23de7a15d9d40006ee596e617475ba101d60c5e9` | ChatGPT adapter 默认 `originator: codex_cli_rs`，以 originator、LiteLLM version、OS/architecture 与 terminal 信息构造 UA，并与 bearer、content type、SSE accept、可选 session/account header 合并。 |
| Hermes `470cf66b039c73bdd2c21d43094ce41a4db74eae` | Codex auxiliary helper 固定 `originator: codex_cli_rs` 与 `codex_cli_rs/0.0.0 (Hermes Agent)` UA，并在 token claim 可读时增加 account header。 |

一手源码：[Codex 0.146.0 release](https://github.com/openai/codex/releases/tag/rust-v0.146.0)、
[Codex workspace version](https://github.com/openai/codex/blob/rust-v0.146.0/codex-rs/Cargo.toml)、
[Codex default client](https://github.com/openai/codex/blob/rust-v0.146.0/codex-rs/login/src/auth/default_client.rs)、
[Codex terminal detection](https://github.com/openai/codex/blob/rust-v0.146.0/codex-rs/terminal-detection/src/lib.rs)、
[Codex Responses endpoint](https://github.com/openai/codex/blob/rust-v0.146.0/codex-rs/codex-api/src/endpoint/responses.rs)、
[LiteLLM ChatGPT common utils](https://github.com/BerriAI/litellm/blob/23de7a15d9d40006ee596e617475ba101d60c5e9/litellm/llms/chatgpt/common_utils.py)、
[Hermes auxiliary client](https://github.com/NousResearch/hermes-agent/blob/470cf66b039c73bdd2c21d43094ce41a4db74eae/agent/auxiliary_client.py)。

这些 header 只证明各项目快照的客户端行为，不是 OAuth 标准，也不证明第三方 client identity、subscription 用途或 edge policy 获得长期授权。
account header 属于 credential context，不能降级成普通静态 header 或由下游覆盖。

Codex 的 Linux UA 会随实际发行版、kernel/OS version、architecture 与 terminal 环境变化，因此不存在唯一的 Linux 字符串。OpenBridge
选择 `codex_cli_rs/0.146.0 (Linux unknown; x86_64) unknown` 作为固定 headless Linux x86_64 source-compatible profile；它只复用
`rust-v0.146.0` 的格式和版本，不宣称复现任一具体 Linux 主机，也不动态读取部署环境。

## 5. refresh 实现对比

| 方面                       | Codex                  | CLIProxyAPI                    | Hermes                  | LiteLLM                    |
|----------------------------|------------------------|--------------------------------|-------------------------|----------------------------|
| 常态触发                   | 使用时检查             | 后台 scheduler + 使用时恢复    | 使用时检查              | 使用时检查                 |
| access token safety window | 5 分钟                 | Codex provider 设置 5 天 lead  | 120 秒                  | 60 秒                      |
| 后台队列                   | 无                     | 最小堆 + 有界 worker           | 无                      | 无                         |
| 同进程协调                 | 单 permit + reload     | mutex + singleflight           | auth-store lock         | 未见锁                     |
| 跨进程协调                 | 未提供分布式机制       | 进程内                         | 同文件系统 file lock    | 未见                       |
| rotation 写回              | credential bundle      | 新值写回，缺失时保留旧值       | 锁内写回                | 普通 JSON 覆盖写           |
| 401 recovery               | reload + refresh，有界 | 同 credential 至多一次 refresh | pool 可隔离终态认证失败 | 主要在下次 resolution 处理 |

CLIProxyAPI 是唯一持续运行后台调度器的样本；其他三个项目主要在 access token 被需要时检查 expiry。不同 safety window
是项目常量，不是 OAuth 标准推荐值。

## 6. refresh token rotation 的共同风险

RFC 6749 允许 authorization server 在 refresh 成功时返回新 refresh token；客户端必须丢弃旧值。RFC 9700 进一步要求 public
client 使用 sender-constrained refresh token 或 rotation 检测重放。

跨项目可以观察到三个风险：

1. 两个并发 worker 可能同时使用同一个旧 refresh token；
2. 新 token 已签发但响应在网络中丢失时，客户端无法确定旧 token 是否仍有效；
3. 新 access token 与 rotated refresh token 若不是同一原子写入，进程崩溃可能留下无法继续刷新的状态。

Codex 的进程内 semaphore、CLIProxyAPI 的 mutex/singleflight、Hermes 的文件锁分别缓解局部并发；LiteLLM 的简单 JSON
覆盖写清楚展示了缺少协调时的竞态。四个样本都不能单独证明跨主机共享 credential 的一致性。

## 7. 失败分类综合

| 失败                   | 跨项目可确认的含义                                                         |
|------------------------|----------------------------------------------------------------------------|
| device pending         | 登录尚未完成，只能按对应 flow 的 interval 继续                             |
| device denied/expired  | 本次登录终止，需要用户重新发起                                             |
| refresh 429/5xx        | 可能暂态，但 retry 必须受 expiry、Retry-After 和硬预算约束                 |
| `invalid_grant`        | 可能过期、撤销、client 不匹配或 rotation 后重用                            |
| `refresh_token_reused` | 至少 CLIProxyAPI/Hermes 将其视为终态或不可普通重试                         |
| 401                    | 不必然等于 token 过期，也可能是 account、audience 或 header context 不匹配 |
| 写入冲突               | 应重新读取胜出的 credential；不能覆盖较新的 rotated token                  |

结果不确定且 authority 使用 single-use rotation 时，是否允许重试旧 refresh token 只能由该 authority 的正式 contract 决定。

## 8. 共同适用边界

- Codex 官方文档证明的是 Codex CLI 的产品能力，不公开保证第三方客户端可复用相同 registration。
- 其他项目复现私有 flow 不会扩大该 flow 的授权范围。
- 本地 auth file、account pool、CLI cache import 和 Codex-shaped header 都是项目产品行为。
- “自动刷新”必须区分后台调度与使用时检查；四个项目并未使用同一 lifecycle。
- 真实 client registration、scope、audience、subscription 使用资格和 refresh policy 必须以目标 Provider 的正式资料为准。
