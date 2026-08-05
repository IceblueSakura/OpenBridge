# Codex OAuth 与工具调用源码调研

## 状态与范围

**外部实现调研；不代表本项目已实现，也不构成上游 OAuth 使用授权。**

**矩阵角色。** 本文现仅保留为 OAuth credential 的安全边界材料，不是 Codex 的主要参考目标；Codex 的 Rust SSE、终态和 tool lifecycle 已拆分到[专门调研](codex-sse-and-tool-lifecycle-analysis.md)。本文固定的 OAuth 证据仍绑定下列旧快照，不能用后续源码更新暗示 OAuth 已获 OpenBridge 采用或授权。

- 源码快照：`F:/codespace/codex`，commit `0fb559f0f6e231a88ac02ea002d3ecd248e2b515`。
- 调研范围：`codex-rs/login` 的 ChatGPT OAuth 登录/refresh/请求认证，以及 `codex-rs/core` 的 Responses tool call 生命周期；后续按固定 commit 补充 SSE 解析、事件生命周期与工具调用回填的 Rust 源码观察。
- 未读取、输出或复制任何本地 credential、`auth.json` 内容、client ID 或 token。
- 本文的行号只适用于上述源码快照；上游接口、client registration 和服务条款必须以当前官方资料重新确认。

**2026-08-01 当前模块级复核。** 本地 `main` 已 fast-forward 至 `ee0247f95a6fe2b094ba2253d82cae2a2b4c2dff`；`run_login_server`、PKCE、`UnauthorizedRecovery`、`refresh_lock`、`AuthManagerAuthProvider`、`ChatGPT-Account-ID`、`ToolRouter::build_tool_call` 与 `ToolInvocation` 仍可定位。login、core tools 与 model-provider 已演进，所以下文详细行号仍只对应固定证据快照，不能据此推断 OAuth 获得授权或形成 OpenBridge 功能承诺。

**2026-08-05 设备登录补充。** 当前 Codex 官方文档与上述提交还包含 beta 的 `codex login --device-auth`。其源码不是原样 RFC 8628：客户端先轮询 Codex 私有 device-auth endpoint 取得 authorization code 与 PKCE material，再执行 authorization-code exchange。当前设备 flow、5 分钟按需 refresh、CLIProxyAPI 后台 scheduler，以及 Hermes/LiteLLM 对照统一记录在[上游 OAuth 2.0 设备码登录与 token 刷新调研](../cross-project/upstream-oauth-device-code-token-refresh-analysis.md)；本文下方旧快照的 loopback 证据仍只说明当时路径。

## 1. 结论摘要

1. Codex 实现的是**本地客户端** OAuth 登录：loopback callback、authorization code + PKCE、`state` 校验、token exchange、工作区限制检查与本地凭证持久化。它证明 Codex 自身的行为，**不证明**第三方 proxy 可以复用其 OAuth client registration、redirect URI、端点或 token exchange。
2. Codex 的 ChatGPT-auth 请求身份不只是 bearer token：请求还可能携带 `ChatGPT-Account-ID`，且 `AuthManagerAuthProvider` 会在发头前检查 account/user/workspace identity 未跨界。对 proxy 而言，account/workspace binding 是 credential route context，不可在 refresh 或 fallback 后丢失。
3. Codex 的 refresh 采用进程内锁、先 guarded reload 再 authority refresh、持久化更新后 reload 的模式。该锁不是分布式锁，不能直接满足多实例 proxy；proxy 仍需 vault version/CAS 与跨实例 single-flight 方案。
4. Codex 的工具闭环以 `call_id` 为不可替代的关联键：模型 `ResponseItem` → `ToolCall` → `ToolInvocation` → `ResponseInputItem::{FunctionCallOutput,CustomToolCallOutput}`，每一步保留同一 `call_id`。
5. Codex 会在工具 output item 完成后调度本地工具，并可以在 `response.completed` 前开始执行；其工具执行、approval、sandbox、hook、取消和输出回填属于 Agent runtime 职责，不应被 proxy 的普通 function-tool bridge 隐式承担。

### 1.1 后续 Rust 源码阅读重点

Codex 是本地正在使用的 Agent，同时使用 Rust 实现；因此可作为 OpenBridge 的同语言参考，但不是需要管理或复用的客户端组件。后续调研必须固定 commit，并把以下事实与本地 fixture 分开记录：SSE bytes 的分帧/解析、事件到 response/tool item 的映射、`call_id` 在请求/响应/tool output 间的传递、并行工具与取消时的生命周期。当前文档尚未据此对 SSE 解析作实现结论。

## 2. OAuth：源码证据

### 2.1 登录流程

Codex 本地登录服务器的已验证流程如下：

```text
run_login_server
  -> generate PKCE verifier/challenge + state
  -> bind localhost ephemeral/fixed port
  -> build authorization URL with redirect_uri
  -> browser sign-in
  -> /auth/callback validates state
  -> authorization-code exchange with code_verifier
  -> optional workspace restriction check
  -> persist ChatGPT auth state locally
```

| 环节 | 源码证据 | 已验证行为 |
|---|---|---|
| PKCE | `codex-rs/login/src/pkce.rs:12-26` | 生成 64-byte 随机 verifier，使用 base64url(no padding) 与 `S256` challenge。 |
| 本地回调 | `login/src/server.rs:150-175` | 启动 local callback server，并构造 `http://localhost:{port}/auth/callback` redirect URI。 |
| 授权请求 | `login/src/server.rs:553-574` | 使用 authorization code flow、`state`、PKCE `code_challenge` 与 `S256`。 |
| callback 防护 | `login/src/server.rs:309-355` | callback 必须命中 `/auth/callback`，并且返回 `state` 与发起时的值精确一致；不一致返回 400。 |
| code exchange | `login/src/server.rs:778-858` | 向 issuer 的 token endpoint 发送 `grant_type=authorization_code`、`code`、`redirect_uri`、`client_id`、`code_verifier`；成功后取得 id/access/refresh token。 |
| workspace 约束与持久化 | `login/src/server.rs:395-424`、`860-899` | exchange 后可校验 workspace，再写入本地 auth storage。 |

### 2.2 本地持久化不是 proxy credential contract

`AuthDotJson` 在 `codex-rs/login/src/auth/storage.rs:38-61` 定义了 Codex 本地认证状态，其中可能包含 API key、id/access/refresh token、刷新时间、agent identity 或 personal access token；默认文件位置为 `$CODEX_HOME/auth.json`（同文件 `150-152`）。实际 backend 还受 Codex 的 credential-store/keyring 配置影响。

这意味着：

- `auth.json` 是 Codex CLI 的私有本地存储格式，不是 proxy 的导入格式或控制面 API；
- 不应通过上传、读取、复制或持续监听该文件来给 proxy 取得 credential；
- proxy 的 secret manager、credential version、审计、撤销和多实例一致性必须独立设计。

### 2.3 refresh 与 401 恢复

`AuthManager` 的实现提供了可借鉴的**安全性质**，而不是可直接复用的分布式实现：

1. `refresh_token` 取得 `refresh_lock`，并先 reload 当前 credential；若磁盘状态已变化则不再 refresh（`login/src/auth/manager.rs:2362-2400`）。
2. API key/PAT 不走 OAuth refresh；ChatGPT auth 才取 refresh token 并调用 authority（`2415-2452`）。
3. refresh 成功会持久化 id/access/refresh token，再 reload cache（`2593-2609`）。
4. workspace/account 不匹配会阻止外部 auth 被提交（`2579-2591`）。
5. `UnauthorizedRecovery` 是有限状态机：managed path 先 reload、再 refresh；external auth path 只走 external refresh；结束后 401 上浮（`1552-1753`）。`core/src/client.rs:2071-2094` 将其描述为一次受控的 401 refresh/retry。
6. 现有集成测试断言 refresh 请求为 `grant_type=refresh_token`，并验证 rotated access/refresh token 和 `last_refresh` 同时写回且 cache 更新（`login/tests/suite/auth_refresh.rs:37-108`）。

**对 proxy 的结论：** 需要实现 single-flight refresh、credential-version CAS、刷新后的完整 credential bundle 原子提交、account/workspace binding 校验，以及最多一次的 pre-output 401 retry。Codex 的 `refresh_lock` 是单进程对象，不能替代 proxy 的跨进程/跨副本协调。

### 2.4 请求认证头与身份绑定

`ModelClient::current_client_setup` 在每次建立请求设置时从当前 provider/auth state 解析认证，而非固定使用 session 起始时的 token（`core/src/client.rs:940-961`）。

对于 bearer path，`BearerAuthProvider` 会写入：

```text
Authorization: Bearer <token>
ChatGPT-Account-ID: <account id>   # 当存在时
X-OpenAI-Fedramp: true             # 特定账户条件下
```

证据：`model-provider/src/bearer_auth_provider.rs:31-46`。更重要的是，`AuthManagerAuthProvider` 会在附加 header 前核对当前 auth 的 account id、ChatGPT user id 和 workspace-account 属性均与请求预期 identity 相同；跨界时不发送头（`model-provider/src/auth.rs:124-150`）。

**对 proxy 的结论：** 若未来获得合法的 Codex OAuth 接入方式，credential 不应只表示 token 字符串。RouteSnapshot 至少要绑定 provider、issuer、credential version、account/workspace identity，以及该 provider 所需的非 secret auth context/header policy；refresh、retry 和 fallback 不能把它们拆散。

## 3. 工具调用：端到端数据流

### 3.1 模型 item 到本地 invocation

```text
Responses SSE / websocket item
  -> ResponseItem::{FunctionCall | CustomToolCall | ToolSearchCall}
  -> ToolRouter::build_tool_call
  -> ToolCall { tool_name, call_id, payload }
  -> ToolInvocation { session, turn, cancellation_token, call_id, tool_name, payload }
  -> ToolRegistry / handler
  -> ResponseInputItem output with the original call_id
  -> next Responses request input
```

`ToolRouter::build_tool_call` 从 `ResponseItem::FunctionCall` 提取 `name`、`namespace`、`arguments`、`call_id`；custom tool 提取 `input`，仅 client-executed tool search 才被转换为本地调用（`core/src/tools/router.rs:111-159`）。`ToolInvocation` 明确保存 `call_id`、tool name、payload、cancellation token 和调用来源（`core/src/tools/context.rs:45-70`）。

这说明三类 ID 的角色不同：

| 标识 | Codex 已验证用途 | proxy 含义 |
|---|---|---|
| `call_id` | tool invocation、approval/hook、执行、output 回填的关联键 | 必须原样保留，不能用 item/response/index 替代。 |
| response/item ID | stream item、显示和会话状态的一部分 | 不能作为 tool output 的唯一关联键。 |
| code-mode runtime tool ID | 仅嵌套 runtime 内唯一，不是 Codex tool `call_id`（`tools/context.rs:45-56`） | 不应暴露成通用 Responses `call_id`。 |

### 3.2 流式 arguments 与启动时机

在 turn loop 中：

- `OutputItemAdded` 对 `CustomToolCall` 按 `call_id` 建立可选 argument-diff consumer；普通 `FunctionCall` 不建立该 consumer（`core/src/session/turn.rs:2152-2169`）。
- `ToolCallInputDelta` 仅将 delta 交给当前 active consumer，且收到的 `call_id` 与 active ID 不匹配时忽略（`2355-2371`）。
- `OutputItemDone` 会完成 consumer、交给 `handle_output_item_done`，并将生成的 tool future 放入 in-flight queue（`2053-2143`）。
- Response stream 未收到 `response.completed` 就 EOF 时，Codex 报 `stream closed before response.completed`（`core/src/client.rs:1952-2060`、`core/src/session/turn.rs:2037-2045`）。

`tool_parallelism` 集成测试构造多个 shell tool call 后才发送 completed event，并验证工具在 response 完成前即可开始执行（`core/tests/suite/tool_parallelism.rs:302-369`）。因此，proxy 的 SSE bridge 不能把工具处理语义错误地绑定到 stream terminal；它必须保留 item/call 生命周期。

### 3.3 调度、取消、执行边界

`ToolCallRuntime` 根据工具自身 `supports_parallel` 用读/写 gate 控制并行；调用取消会中止或等待运行时 teardown，并生成带原 `call_id` 的 aborted output（`core/src/tools/parallel.rs:75-200`、`237-245`）。`ToolRegistry` 还在 handler 执行前后运行 hook、记录 telemetry、处理 unsupported tool、并把输出包装为 `AnyToolResult`（`core/src/tools/registry.rs:399-720`）。其工具 orchestrator 则承担 approval、sandbox、network approval 与升级重试（`core/src/tools/orchestrator.rs:1-143`）。

这些是 Codex Agent runtime 的责任。当前 proxy 的职责仅是**协议层转发/转换与策略 gate**：

- 不执行下游 function tools；
- 不替 Codex 做 approval、sandbox、hooks、shell/MCP runtime 或 tool-result retry；
- provider-native/builtin/custom tool 无法等价转换时必须 `native` 或 `reject`，而不是伪装成普通 function tool。

### 3.4 输出回传与顺序

`AnyToolResult::into_response` 委托 `ToolOutput::to_response_item` 生成下一次 Responses input（`core/src/tools/registry.rs:163-179`）。例如 MCP output 会生成 `ResponseInputItem::FunctionCallOutput { call_id, output }`（`core/src/tools/context.rs:81-100`）；失败的普通/custom tool output 同样带原 `call_id` 与 `success: false`（`core/src/tools/parallel.rs:210-234`）。

现有 `tool_parallelism` 测试验证：三个 function calls 与三个 outputs 均存在；所有 calls 排在 outputs 前；并且 output 按 call 顺序排列，且每个 output 的 `call_id` 与对应 call 相同（`core/tests/suite/tool_parallelism.rs:258-297`）。

**对 proxy 的结论：** bridge 至少要维持 `call_id` 的一对一关系、并行 call 的独立 buffer 与 source ordering。是否需要复现 Codex 的 call-order output 排列，要按目标 provider/Agent 的 wire contract 验证；不能用执行完成顺序错误重配 output。

## 4. 对本项目需求的影响

| 需求主题 | 本轮确认或强化的规则 |
|---|---|
| Codex OAuth | 维持 preflight 硬门：不导入 `auth.json`、不复用 Codex CLI client registration、不模拟其 loopback callback。只有获得公开且允许的 proxy client contract 后才可实现。 |
| Credential model | OAuth credential 必须把 token rotation、issuer、account/workspace identity 和必需 header policy 作为同一原子 route context 管理。 |
| Refresh | 单实例 mutex 不足；proxy 需要 distributed/single-flight refresh + vault version CAS。401 retry 只能在未输出业务 SSE 前进行一次。 |
| Capability Matrix | 增加 `auth_context_headers`、`account/workspace binding`、`provider-native/custom tools`、`tool argument delta` 和 `tool output ordering` 列。 |
| Bridge IR | `call_id` 为必填关联键；`item_id`、`response_id`、`output_index` 不得替代。应表达 function/custom/tool-search/native-tool 的不同类别。 |
| SSE fixtures | 增加：call item added、argument delta、item done、并行 calls、工具启动早于 response completed、tool failure/abort output、EOF before terminal。 |
| Agent compatibility | Codex 场景须验证 function/custom tool 类型、并行度、tool output 顺序、cancel/abort output 与 `call_id` 关联。 |

## 5. 未证实项与下一步

以下均不能从本地 Codex 源码单独得出：

1. OpenAI 是否允许第三方 proxy 作为该 OAuth flow 的 client；
2. 可公开注册/使用的 client ID、redirect URI、scope/resource、token endpoint、refresh policy 和 workspace contract；
3. Codex 当前生产 endpoint 的完整 SSE event 集合、字段、tool type 及 header 必需性；
4. proxy 直接透传 Codex session/response continuation state 是否被支持；
5. Codex 版本升级后上述私有实现细节是否仍稳定。

因此，真实 Codex OAuth 仍需完成[产品范围](../../functional-requirements/product-scope.md)定义的 `Codex OAuth preflight`，并取得官方文档、授权/条款确认和脱敏实测 fixture；在此之前只支持 mock OAuth adapter 或标准 API-key upstream。

## 6. 关联文档

- [产品范围](../../functional-requirements/product-scope.md)
- [配置、凭证与受信边界](../../functional-requirements/configuration-and-credentials.md)
- [网关 API 与客户端兼容需求](../../functional-requirements/gateway-api-compatibility.md)
- [交付与证据要求](../../functional-requirements/delivery-and-evidence.md)
- [Hermes Agent 协议分析](../hermes/hermes-chat-responses-analysis.md)
- [Hermes 与 LiteLLM ChatGPT OAuth 实现调研](../cross-project/hermes-litellm-oauth-analysis.md)
- [上游 OAuth 2.0 设备码登录与 token 刷新调研](../cross-project/upstream-oauth-device-code-token-refresh-analysis.md)
