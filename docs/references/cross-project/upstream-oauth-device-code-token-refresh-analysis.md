# 上游 OAuth 2.0 设备码登录与 access token 刷新调研

**状态：2026-08-05 外部参考调研。本文不代表 OpenBridge 已实现 OAuth，也不构成复用任何上游 client registration、私有端点、订阅账户或本地凭证的授权。**

## 1. 调研问题与证据边界

本文回答两个问题：

1. headless gateway 如何让运维人员通过设备码为一个受信任的上游 Provider 建立 OAuth credential；
2. 登录完成后，如何在 access token 到期前安全刷新，并处理 refresh token rotation、并发、进程重启和 401。

重点比较 Codex、CLIProxyAPI、Hermes Agent 与 LiteLLM。源码观察固定在以下本地快照；本次没有更新或运行这些外部项目，也没有读取本地 credential 文件内容：

| 项目 | 固定提交 | 本文使用的证据 |
| --- | --- | --- |
| Codex | `ee0247f95a6fe2b094ba2253d82cae2a2b4c2dff` | 设备登录、token exchange、认证存储、主动 refresh 与 401 recovery |
| CLIProxyAPI | `bc71c77f5cc42f3fbe1bf040cf14d4f166894835` | Codex 设备登录、后台 refresh scheduler、单 credential 并发控制与 401 后刷新 |
| Hermes Agent | `470cf66b039c73bdd2c21d43094ce41a4db74eae` | Codex 设备登录、跨进程文件锁、按需 refresh 与 rotated token 写回 |
| LiteLLM | `23de7a15d9d40006ee596e617475ba101d60c5e9` | ChatGPT authenticator 的设备登录、按需 refresh 与本地 JSON 存储 |

证据分为三类：

- **规范事实**：OAuth 2.0 RFC 对标准 device authorization grant、refresh grant 与 refresh token rotation 的要求；
- **官方产品事实**：Codex 官方文档对 Codex CLI 登录方式的说明；
- **源码观察**：四个项目在固定提交中的具体实现，只能说明这些客户端如何工作，不能外推第三方 gateway 获得了同样的 OAuth 使用资格。

## 2. 结论先行

1. **Codex 官方设备登录是 Codex CLI 的 beta 功能，不是公开的通用 Provider OAuth contract。** 官方文档说明 `codex login --device-auth` 可用于无法打开浏览器的环境，但需要用户或 workspace 管理员先启用 device code authentication；文档同时说明 ChatGPT 登录会在使用期间于过期前自动刷新。
2. **Codex 当前 wire flow 不是 RFC 8628 的原样实现。** Codex、CLIProxyAPI、Hermes 和 LiteLLM 都先调用 Codex 私有 device-auth 端点，轮询得到 `authorization_code` 与 PKCE verifier，再走 authorization-code token exchange。RFC 8628 则要求客户端直接拿 `device_code` 轮询 token endpoint，并处理 `authorization_pending`、`slow_down` 等标准错误。
3. **“定期刷新”应是按过期时间调度，不是无条件固定周期换 token。** 正确组合是 `expires_at - safety_window - jitter` 的到期队列、请求路径上的 single-flight，以及 401 时至多一次的受限恢复。四个样本中只有 CLIProxyAPI 有持续运行的后台 scheduler；Codex、Hermes 与 LiteLLM 都主要在 credential 被使用时检查和刷新。
4. **refresh 的原子单位是完整 credential bundle，不是单独的 access token。** 新 access token、可能轮换的 refresh token、到期时间、issuer、account/workspace identity、scope 与 credential version 必须一起校验和写回。服务器返回新 refresh token 时，RFC 6749 要求客户端丢弃旧值。
5. **进程内锁只解决单进程重复刷新。** 多实例 OpenBridge 若共享 credential，需要以非 secret credential ID 为键的 lease/single-flight，加存储版本或 CAS；否则两个实例可能同时消费一个已轮换的 refresh token。
6. **登录只能是显式的运维控制面操作。** 不应在业务请求因 token 缺失或刷新失败时自动进入设备登录，也不应导入 Codex/Hermes 的 `auth.json`、复制内置 client identity，或从下游请求选择 OAuth endpoint、scope 或 header。

## 3. 标准 OAuth 2.0 基线

### 3.1 RFC 8628 设备授权

标准设备授权流程是：

1. 客户端向 Provider 的 device authorization endpoint 提交已注册的 `client_id` 和 scope；
2. Provider 返回 `device_code`、`user_code`、`verification_uri`、过期时间与建议轮询间隔；
3. gateway 只把 verification URI 和一次性 user code 展示给发起登录的运维人员；
4. 运维人员在另一台有浏览器的设备上完成认证和授权；
5. gateway 按 Provider 返回的 interval 轮询 token endpoint，使用 `urn:ietf:params:oauth:grant-type:device_code`；
6. 成功后保存 access token、refresh token、到期时间和已确认的身份上下文。

轮询必须区分：

| token endpoint 结果 | 客户端动作 |
| --- | --- |
| `authorization_pending` | 保持当前 interval，继续轮询 |
| `slow_down` | 本次及后续轮询至少再增加 5 秒 |
| `access_denied` | 终止本次登录并告知操作者拒绝了授权 |
| `expired_token` | 终止并要求显式重新开始设备登录 |
| 网络超时 | 降低轮询频率；不能形成紧密重试循环 |
| 登录会话被取消 | 立即停止轮询，清除内存中的临时 device state |

`user_code` 的用途是把浏览器中的授权绑定到操作者刚刚发起的登录。界面需要像 Codex 一样明确提示：只有在本人主动发起登录时才继续，避免被诱导输入他人提供的 code。

### 3.2 RFC 6749 refresh grant

refresh 请求使用 token endpoint，表单至少包含 `grant_type=refresh_token` 和 refresh token；机密客户端还必须按其 registration 要求完成 client authentication。客户端必须验证响应，再替换 credential：

- access token 必须存在且类型、scope、audience 符合 Provider contract；
- 若响应含新 refresh token，必须在同一次持久化中替换旧 token；
- 若 Provider 合同允许不返回新 refresh token，则保留当前 refresh token；
- `invalid_grant` 可能表示 token 已过期、撤销、轮换后被重复使用、签发给其他 client，或与当前 redirect/client 不匹配，不能归类为普通 5xx cooldown。

### 3.3 RFC 9700 rotation 与重放边界

OAuth 2.0 Security Best Current Practice 要求 public client 的 refresh token 使用 sender-constrained token 或 rotation 来检测重放。采用 rotation 时，旧 refresh token 在每次成功刷新后失效；发现旧 token 再次出现时，authorization server 可能撤销当前 grant。

这使“请求已被上游接受，但响应在网络中丢失”成为重要的歧义状态：客户端不能假定旧 refresh token 仍可安全重试。是否可以重试、能否查询当前 grant、或必须重新登录，必须由具体 Provider contract 决定。

## 4. Codex：设备登录与刷新生命周期

### 4.1 官方支持范围

[Codex authentication](https://learn.chatgpt.com/docs/auth) 当前说明：

- 默认 `codex login` 使用浏览器完成 ChatGPT OAuth；
- headless 环境可使用 beta 的 `codex login --device-auth`，前提是该账户或 workspace 已启用 device code authentication；
- ChatGPT 登录的会话会在使用期间于过期前自动刷新；
- `auth.json` 等认证缓存应按密码对待，自动化场景仍优先建议使用 API key。

这些是 Codex 产品的登录说明。页面没有公开一个允许第三方 multi-provider gateway 复用的 OAuth client registration、scope、私有 device endpoint 或 subscription backend contract。

### 4.2 当前源码的实际设备 flow

Codex 的 [`device_code_auth.rs`](https://github.com/openai/codex/blob/ee0247f95a6fe2b094ba2253d82cae2a2b4c2dff/codex-rs/login/src/device_code_auth.rs) 实际执行：

1. 从私有 `deviceauth/usercode` endpoint 获取 verification URL、user code、内部 device auth ID 与轮询 interval；
2. 向操作者显示 URL、code、15 分钟有效期和防钓鱼提示；
3. 在 15 分钟上限内轮询私有 `deviceauth/token` endpoint；该实现把 HTTP 403/404 当成“仍在等待”，而不是解析 RFC 8628 的 `authorization_pending`；
4. 成功轮询得到 authorization code、PKCE verifier/challenge；
5. 使用 authorization-code + PKCE 和固定 callback 完成 token exchange；
6. 检查 workspace 限制，随后持久化 id/access/refresh token 与身份信息。

因此 OpenBridge 必须把两类 adapter 分开：

| 维度 | 标准 RFC 8628 adapter | Codex 当前产品 flow |
| --- | --- | --- |
| 轮询目标 | OAuth token endpoint | 私有 device-auth endpoint |
| 轮询凭据 | `device_code` | 私有 device auth ID |
| 等待信号 | OAuth JSON error，如 `authorization_pending` | 当前源码把 HTTP 403/404 视为 pending |
| 轮询成功结果 | access/refresh token | authorization code + PKCE material |
| 后续交换 | 无额外 authorization-code exchange | 再执行 authorization-code + PKCE exchange |
| 可实现前提 | Provider 公开 discovery/registration/授权 | Codex/OpenAI 提供允许 gateway 使用的正式 contract |

不能把 Codex 的状态码、端点和字段名称放入通用 RFC 8628 adapter，也不能因为其他项目复现了这个 flow 就认为它是公共协议。

### 4.3 当前源码的 refresh 策略

Codex 的 [`auth/manager.rs`](https://github.com/openai/codex/blob/ee0247f95a6fe2b094ba2253d82cae2a2b4c2dff/codex-rs/login/src/auth/manager.rs) 展示了两层恢复：

- 每次解析当前认证时，若 access token 的 JWT `exp` 距当前不超过 5 分钟，就尝试主动 refresh；只有无法读取 `exp` 时才退回到 `last_refresh` 超过 8 天的陈旧性判断；
- refresh 由进程内单 permit semaphore 合并。取得锁后重新加载存储，若其他调用已经更新 credential 就跳过重复刷新；
- refresh 成功后一次更新 access token、可选的 rotated refresh token 与 `last_refresh`，再重新加载当前 auth；
- 401 recovery 是有界状态机：先尝试重新加载已有 credential，再尝试 refresh，不会无限循环。

这属于**使用时主动刷新**，不是常驻后台定时器。它适合 CLI 进程，但不能保证一个长期空闲、随后突然接收请求的 gateway 始终提前完成刷新。

## 5. 四个项目的实现对比

| 方面 | Codex | CLIProxyAPI | Hermes Agent | LiteLLM |
| --- | --- | --- | --- | --- |
| Codex 设备登录 wire flow | 私有 device polling → authorization code + PKCE | 与 Codex 相同的私有 flow | 与 Codex 相同的私有 flow | 与 Codex 相同的私有 flow |
| 登录超时 | 15 分钟 | 15 分钟 | 15 分钟 | 15 分钟 |
| 常态 refresh 触发 | 使用时，`exp` 前 5 分钟 | 后台 scheduler + 使用时恢复 | 使用时，`exp` 前 120 秒 | 使用时，`exp` 前 60 秒 |
| 后台定时调度 | 无 | 有：到期最小堆、约 5 秒检查、受限 worker pool | 无 | 无 |
| Codex 提前量 | 5 分钟 | Provider 实现返回 5 天；这是项目策略，不是 OAuth 标准 | 120 秒 | 60 秒 |
| 同进程重复 refresh | 单 permit + reload/re-check | per-credential mutex；底层还有按 refresh token 合并调用 | runtime 路径受 auth-store lock 保护 | 当前 authenticator 未见锁 |
| 跨进程/多实例 | 未解决共享 store 的分布式协调 | 当前协调仍是进程内 | 同一主机文件锁 + reload/re-check | 当前未见文件锁、CAS 或 lease |
| rotated refresh token | 整体写回 credential | 写回新值；上游未返回时保留旧值 | 锁内写回；上游未返回时保留旧值 | 直接写回 JSON；上游未返回时保留旧值 |
| 401 恢复 | reload，再 refresh，有界结束 | 同 credential 至多一次 refresh 后再进入 fallback | credential pool 可隔离终态 auth 错误 | 主要依赖下一次 token resolution |
| 对 OpenBridge 的主要价值 | 到期检查、guarded reload、身份绑定、有界 401 recovery | 到期队列、受限 worker、per-credential single-flight、一次 401 恢复 | 锁内 reload/re-check 与 rotation 写回 | 简单按需路径的反例和并发缺口 |

### 5.1 CLIProxyAPI：唯一的后台 scheduler 样本

CLIProxyAPI 的 [`auto_refresh_loop.go`](https://github.com/router-for-me/CLIProxyAPI/blob/bc71c77f5cc42f3fbe1bf040cf14d4f166894835/sdk/cliproxy/auth/auto_refresh_loop.go) 和 [`conductor_refresh.go`](https://github.com/router-for-me/CLIProxyAPI/blob/bc71c77f5cc42f3fbe1bf040cf14d4f166894835/sdk/cliproxy/auth/conductor_refresh.go) 使用按下一次到期时间排序的最小堆、受限 worker pool 和不同结果的重新调度间隔。它还在业务请求 401 后允许同一 credential 至多刷新一次，避免无限 refresh/retry。

值得借鉴的是调度形状和硬边界，不是具体常量。当前 Codex provider 在 [`codex.go`](https://github.com/router-for-me/CLIProxyAPI/blob/bc71c77f5cc42f3fbe1bf040cf14d4f166894835/sdk/auth/codex.go) 中返回的 5 天 refresh lead、约 5 秒 scheduler tick，以及 retry 分类都是 CLIProxyAPI 的产品选择；它们没有成为 OpenBridge 默认值的标准依据。其 mutex/singleflight 也只覆盖一个进程，不能代替共享 secret store 的 lease 与 CAS。

CLIProxyAPI 的设备登录见 [`codex_device.go`](https://github.com/router-for-me/CLIProxyAPI/blob/bc71c77f5cc42f3fbe1bf040cf14d4f166894835/sdk/auth/codex_device.go)，refresh 与 credential 更新入口见 [`codex_executor_auth.go`](https://github.com/router-for-me/CLIProxyAPI/blob/bc71c77f5cc42f3fbe1bf040cf14d4f166894835/internal/runtime/executor/codex_executor_auth.go)。这些仍是对 Codex 私有 flow 的客户端复现，不是授权依据。

### 5.2 Hermes：跨进程本地锁样本

Hermes 的 [`hermes_cli/auth.py`](https://github.com/NousResearch/hermes-agent/blob/470cf66b039c73bdd2c21d43094ce41a4db74eae/hermes_cli/auth.py) 在 credential resolution 时检查 120 秒 safety window，并在跨进程 auth-store 文件锁内重新读取、重新判断、refresh 和写回。这能减少同一主机上两个进程同时使用旧 refresh token。

其限制是文件锁只保护同一文件系统，不能覆盖多主机实例。Hermes 在某些失败路径会尝试从 Codex CLI credential store 重新导入 token；这是本地 Agent 的自愈策略，OpenBridge 不应跨越 credential ownership boundary 采用。多 credential pool 与账户轮换也不属于本调研建议的 OpenBridge 范围。

### 5.3 LiteLLM：按需刷新与存储反例

LiteLLM 的 [`authenticator.py`](https://github.com/BerriAI/litellm/blob/23de7a15d9d40006ee596e617475ba101d60c5e9/litellm/llms/chatgpt/authenticator.py) 在获取 access token 时检查 60 秒 safety window，过期时 refresh；若没有可用 refresh token，还可能在 token resolution 路径进入交互式设备登录。

当前实现通过普通 JSON 读取和覆盖写持久化 credential，未见文件锁、atomic replace、credential version/CAS 或跨 worker single-flight。它适合作为两个明确反例：业务请求路径不能自动触发交互登录；rotated refresh token 不能依赖无并发保护的覆盖写。

## 6. OpenBridge 可评估的目标契约

以下是基于标准和样本得出的**研究推论**，不是已批准设计、当前实施计划或兼容承诺。

### 6.1 先通过 Provider OAuth preflight

每个 OAuth Provider adapter 在进入实现前必须有可引用的正式资料，确认：

- authorization server、device authorization endpoint、token endpoint 与 issuer；
- OpenBridge 自己的 client registration 类型、允许的 grant、redirect/device flow 和 client authentication；
- scope、resource/audience、token lifetime、refresh token rotation/revocation 语义；
- account/workspace/organization 绑定以及请求所需的非 secret header；
- subscription/API 使用资格、自动化访问和 gateway/proxy 场景是否被允许；
- reauthorization、用户撤销、管理员禁用与 credential 删除流程。

若 Codex/OpenAI 没有提供允许第三方 gateway 使用的正式 registration 和 contract，OpenBridge 不应实现对私有 Codex device endpoint 的模拟。

### 6.2 登录职责

设备登录应是显式命令或受保护的运维 API，并遵循：

1. 从编译期注册的 Provider adapter 选择受信任 endpoint；业务请求不能传入 URL、client identity 或 scope；
2. 建立有 15 分钟或 Provider 指定 TTL 的临时 login session，只向发起者显示 verification URI 与 user code；
3. 标准 Provider 严格实现 RFC 8628 poll/error 语义；非标准 Provider 必须使用独立、明确命名的 adapter；
4. token exchange 后校验 issuer、audience、scope 与 account/workspace allow-list；
5. 将 credential bundle 写入 secret backend 后才宣布登录成功；
6. 取消、拒绝、过期或校验失败时清除临时 state，不保存半成品 token。

### 6.3 Credential bundle

一个可刷新 credential 至少应把以下信息作为同一版本管理：

```text
credential_id          非 secret 稳定标识，用于锁、调度和审计
provider / issuer      只能来自受信任注册表
client_registration    OpenBridge 获授权的 registration 引用，不是下游可选值
subject / account      token 与 route 必须保持的身份绑定
workspace / org        Provider 要求时参与 allow-list 与请求 header policy
access_token           secret
refresh_token          secret，可选且可能每次轮换
expires_at             绝对时间；避免把 JWT decode 当作唯一依据
scope / audience       用于刷新响应与请求前校验
version                CAS、reload 与审计边界
status                 active / refresh_backoff / reauth_required / revoked
refreshed_at           非 secret lifecycle metadata
```

日志、metric、lock key 和错误响应只使用 `credential_id` 或脱敏 hash；不得记录 bearer、refresh token、authorization code、PKCE verifier 或完整 credential 文件内容。

### 6.4 到期驱动的 refresh scheduler

推荐的行为不是“每 N 分钟刷新所有账户”，而是：

```text
due_at = expires_at - provider_safety_window - bounded_jitter

到达 due_at：
  1. 取得 credential_id 对应的 refresh lease；
  2. 从 secret store 重新加载 bundle 与 version；
  3. 若其他 worker 已刷新且 token 仍在安全窗口外，直接结束；
  4. 按 Provider contract 执行一次 refresh grant；
  5. 校验完整响应；
  6. 以 version CAS 原子写入 access token、rotated refresh token、expiry 和 identity；
  7. 发布新 snapshot，并按新 expires_at 安排下一次 due_at；
  8. 释放 lease，将等待同一 credential 的请求唤醒。
```

实现需要同时覆盖：

- **启动恢复**：从存储重建所有 active credential 的 due queue；已进入 safety window 的条目立即进入受限 worker queue，而不是并发洪峰；
- **有界并发**：全局 worker limit + 每 credential 单飞；不同 Provider 可有独立 rate limit；
- **抖动**：对大量同到期 credential 分散刷新，但不能把 due time 推到 access token 已失效之后；
- **空闲账户**：是否为 refresh token inactivity policy 主动换 token，只能依据 Provider 文档，不能猜测心跳周期；
- **进程重启**：下一次 due time 来自持久化 expiry，不依赖仅存在于内存的 timer；
- **多实例**：lease 和 version/CAS 必须由共享 store 提供；单进程部署也应保留 version/reload 语义，便于发现外部撤销或人工重登录。

### 6.5 请求路径与 401

业务请求取得当前 credential snapshot 时：

1. 若 token 已在 safety window 内，加入或等待同一 refresh single-flight，而不是各自刷新；
2. 若 token 仍有效，直接发请求，不能为了固定周期强制刷新；
3. 收到 401 后先 reload；若 credential version 已变化，可用新 token 至多重试一次；
4. 若 version 未变且 Provider 允许，可触发一次 refresh，再至多重试一次；
5. 一旦下游 response body 已开始，不得通过刷新后重放制造第二个上游响应；
6. 第二次 401 或终态 OAuth 错误把 credential 转为 `reauth_required`，不进入普通 provider fallback/账号轮换循环。

401 可能来自 token audience、account/workspace header 或授权策略错误，不等于 access token 过期。刷新前后的 identity 和 route binding 必须一致，否则应失败关闭。

## 7. 失败分类与调度结果

| 失败 | credential 状态 | 下一步 |
| --- | --- | --- |
| device `authorization_pending` | login pending | 按当前 interval 继续 |
| device `slow_down` | login pending | 增加 interval 后继续 |
| device denied/expired | 无 credential | 终止本次登录，要求人工重新发起 |
| refresh 429 / 明确暂态 5xx | active 或 `refresh_backoff` | 受 Retry-After/expiry 约束的 bounded backoff |
| refresh 网络错误且确认请求未送达 | active 或 `refresh_backoff` | Provider policy 允许时有界重试 |
| refresh 结果不确定，且 Provider 使用 single-use rotation | ambiguous / `reauth_required` | 不盲目重用旧 refresh token；按 Provider contract 协调或重新登录 |
| `invalid_grant` / reused / revoked | `reauth_required` 或 revoked | 停止自动 refresh，通知运维 |
| 401 且存储已有新 version | active | 新 token 至多重试一次 |
| 401 且一次 refresh 后仍失败 | `reauth_required` | 停止循环；保留脱敏诊断 |
| CAS 失败 | active | reload 新 version；不要覆盖胜出者的 rotated token |
| secret store 写入失败且 refresh 可能已轮换 | ambiguous | 不发布仅在内存中的新 bundle；要求安全恢复 |

不能仅按 HTTP status 决定 retry。OAuth JSON error、是否收到响应、token rotation policy、当前 access token 剩余时间和下游是否已开始输出共同决定结果。

## 8. 可借鉴与不可复制的边界

| 来源 | 可借鉴 | 不可直接复制 |
| --- | --- | --- |
| Codex | 设备登录防钓鱼提示、workspace 检查、guarded reload、身份绑定、有界 401 recovery | 内置 client identity、私有 endpoint、credential cache 或 Codex 产品授权假设 |
| CLIProxyAPI | expiry queue、worker limit、per-credential single-flight、一次 401 refresh | 5 天 lead 常量、账号聚合/轮换、进程内锁充当分布式一致性 |
| Hermes | 锁内 reload/re-check、rotated token 整体写回、终态 auth error 隔离 | 导入 Codex CLI credential、多账号 pool、客户端专用 header identity |
| LiteLLM | 小型按需路径便于理解 | 请求路径触发交互登录、无锁 JSON 覆盖写、私有 flow 被包装成通用 Provider 能力 |

OpenBridge 的 trusted-egress 边界仍然成立：endpoint、client registration、scope、auth header policy 和 credential binding 都必须由编译期 Provider adapter 或受信任启动配置决定，不能由下游业务请求提供。

## 9. 验证边界与后续入口

本次只完成：

- OAuth RFC 与 Codex 官方认证文档的资料核对；
- 四个固定源码快照的静态阅读；
- 不同 device flow、refresh trigger、锁、存储和 401 recovery 的交叉比较。

本次没有：

- 使用任何真实账号执行 Codex/device login；
- 向真实 Provider 发送 token 或 refresh 请求；
- 验证当前服务条款、商业授权或第三方 proxy registration；
- 运行四个外部项目的测试；
- 修改 OpenBridge credential model、runtime、配置、OpenAPI 或测试。

若将来用户批准实现，仍应先把一个经授权 Provider 的一个可观察行为写入 `docs/implementation-plans/current-focus.md`，以失败测试固定标准 device polling 或 refresh rotation 的单一边界，再进入代码变更。

## 10. 一手资料

规范与官方文档：

- [RFC 8628: OAuth 2.0 Device Authorization Grant](https://www.rfc-editor.org/rfc/rfc8628.html)
- [RFC 6749: OAuth 2.0 Authorization Framework](https://www.rfc-editor.org/rfc/rfc6749.html)
- [RFC 9700: Best Current Practice for OAuth 2.0 Security](https://www.rfc-editor.org/rfc/rfc9700.html)
- [Codex authentication](https://learn.chatgpt.com/docs/auth)

固定源码入口：

- Codex [`device_code_auth.rs`](https://github.com/openai/codex/blob/ee0247f95a6fe2b094ba2253d82cae2a2b4c2dff/codex-rs/login/src/device_code_auth.rs)、[`auth/manager.rs`](https://github.com/openai/codex/blob/ee0247f95a6fe2b094ba2253d82cae2a2b4c2dff/codex-rs/login/src/auth/manager.rs)、[`device_code_login.rs`](https://github.com/openai/codex/blob/ee0247f95a6fe2b094ba2253d82cae2a2b4c2dff/codex-rs/login/tests/suite/device_code_login.rs)
- CLIProxyAPI [`codex_device.go`](https://github.com/router-for-me/CLIProxyAPI/blob/bc71c77f5cc42f3fbe1bf040cf14d4f166894835/sdk/auth/codex_device.go)、[`codex.go`](https://github.com/router-for-me/CLIProxyAPI/blob/bc71c77f5cc42f3fbe1bf040cf14d4f166894835/sdk/auth/codex.go)、[`auto_refresh_loop.go`](https://github.com/router-for-me/CLIProxyAPI/blob/bc71c77f5cc42f3fbe1bf040cf14d4f166894835/sdk/cliproxy/auth/auto_refresh_loop.go)、[`conductor_refresh.go`](https://github.com/router-for-me/CLIProxyAPI/blob/bc71c77f5cc42f3fbe1bf040cf14d4f166894835/sdk/cliproxy/auth/conductor_refresh.go)、[`codex_executor_auth.go`](https://github.com/router-for-me/CLIProxyAPI/blob/bc71c77f5cc42f3fbe1bf040cf14d4f166894835/internal/runtime/executor/codex_executor_auth.go)
- Hermes Agent [`hermes_cli/auth.py`](https://github.com/NousResearch/hermes-agent/blob/470cf66b039c73bdd2c21d43094ce41a4db74eae/hermes_cli/auth.py)、[`agent/credential_pool.py`](https://github.com/NousResearch/hermes-agent/blob/470cf66b039c73bdd2c21d43094ce41a4db74eae/agent/credential_pool.py)
- LiteLLM [`authenticator.py`](https://github.com/BerriAI/litellm/blob/23de7a15d9d40006ee596e617475ba101d60c5e9/litellm/llms/chatgpt/authenticator.py)

相关 OpenBridge 材料：

- [Codex OAuth 与工具调用源码调研](../codex/codex-oauth-and-tool-call-analysis.md)
- [Hermes 与 LiteLLM 的 ChatGPT OAuth 实现调研](hermes-litellm-oauth-analysis.md)
- [参考项目比较矩阵](../project-comparison.md)
- [产品范围](../../functional-requirements/product-scope.md)
