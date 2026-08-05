# 当前开发焦点

## 状态

**活动焦点：ChatGPT subscription OAuth 第一阶段——Provider 与只读 Codex credential 真实 probe。**

两阶段产品边界已经写入[ChatGPT subscription OAuth credential lifecycle](../functional-requirements/upstream-oauth-credential-lifecycle.md)。
本文只授权第一阶段；PKCE 登录、refresh token、续约、401 recovery 和常驻数据面接入必须在本焦点完成并清空后另立焦点。

## 行为

管理员显式运行 `openbridge-probe` 选择编译期 ChatGPT target 时，工具从指定的 Codex `auth.json` 只读提取当前 access token 与账户
绑定，从同机 Codex CLI runtime 获得逐字节一致的 `User-Agent`，然后对固定 ChatGPT Codex backend 执行模型目录查询和
`gpt-5.6-sol` Responses SSE 文本 probe，最后输出不含 token、账户、原始 header、请求正文或响应正文的结果。

该 target 默认禁用，不加入 Route/Public Model，常驻 OpenBridge 服务既不读取 Codex auth file，也不要求 ChatGPT credential。

## 对应功能需求

- 主需求：[OAUTH-01 至 OAUTH-05](../functional-requirements/upstream-oauth-credential-lifecycle.md#8-功能验收要求)
- credential 例外：[CFG-13](../functional-requirements/configuration-and-credentials.md#6-验收要求)
- 真实验收分层：[交付与证据要求](../functional-requirements/delivery-and-evidence.md)

## Codex 实现基线

本焦点以本地只读 checkout `F:\codespace\codex` 已抓取的 `origin/main` commit
`1fe6be9719ac4a18ad08f8341b89f9a0f386105e` 为实现事实基线。开始编码和真实验收前重新执行只读 commit/status 检查；若 commit
变化，先重定位下表文件和测试，不能沿用行号或历史假设。

| 需要保持的 Codex 事实          | 当前源码入口                                                                                              |
|-------------------------------|-----------------------------------------------------------------------------------------------------------|
| ChatGPT backend base          | `codex-rs/model-provider-info/src/lib.rs` 的 `CHATGPT_CODEX_BASE_URL`                                      |
| 模型目录 path/query           | `codex-rs/codex-api/src/endpoint/models.rs`                                                               |
| Responses path/SSE            | `codex-rs/codex-api/src/endpoint/responses.rs`                                                            |
| Bearer 与 account header      | `codex-rs/model-provider/src/bearer_auth_provider.rs`                                                     |
| Codex `auth.json` 最小形状    | `codex-rs/login/src/auth/storage.rs`、`codex-rs/login/src/token_data.rs`                                  |
| `User-Agent` 与 `originator`  | `codex-rs/login/src/auth/default_client.rs`                                                               |
| CLI runtime `User-Agent` 出口 | `codex-rs/app-server/src/request_processors/initialize_processor.rs` 的 `InitializeResponse.user_agent`   |

本次规划时同机已安装 Codex CLI 为 `0.145.0`。该版本只作为首次验收记录，不写成长期固定兼容版本；每次真实验收重新读取 CLI
runtime 值。规划期间只确认 `%USERPROFILE%\.codex\auth.json` 存在，未读取其内容。

## 先失败的测试或复现

先增加或命名以下聚焦测试，并确认旧实现失败：

1. `tests/provider_contract.rs::chatgpt_provider_uses_codex_backend_profiles_and_oauth_credential`
   - 旧实现没有 `ProviderKind::ChatGpt`、ChatGPT adapter、OAuth credential contract、`/models` 或 `/responses` profile，应先编译失败。
2. `tests/example_config.rs::chatgpt_probe_target_is_compiled_but_not_publicly_routable`
   - 旧 registry 没有 ChatGPT pool/target；实现后必须同时证明 target 默认禁用、没有 Route/Public Model、常驻服务不要求其 credential。
3. `tests/codex_auth_file_contract.rs`
   - 用纯合成 JSON 覆盖有效 ChatGPT token、FedRAMP claim、API-key auth、缺失 account、空/过期 access token、损坏 JSON 和
     Debug/错误脱敏；旧实现没有只读 loader，应先失败。
4. `src/probe/tests.rs::chatgpt_probe_matches_codex_identity_models_and_responses_sse`
   - fake transport 捕获普通/敏感 header 名、`GET /models?client_version=...` 和 `POST /responses`，返回 Codex models shape 与分片 SSE；
     旧 probe 只理解 OpenAI `/v1/models`、`data[]` 和非流式 JSON，应先失败。
5. `src/bin/openbridge-probe.rs` CLI contract
   - ChatGPT target 必须显式提供 Codex auth path 和受信 Codex executable；普通 target 拒绝这些 selector，任意 header/URL/model 注入仍被拒绝。

当前真实复现也应失败在网络前：

```powershell
cargo run --locked --bin openbridge-probe -- `
  --target chatgpt-gpt-5-6-sol `
  --codex-auth-file "$env:USERPROFILE\.codex\auth.json" `
  --codex-cli codex `
  --list-models --responses
```

旧实现没有该 target 和两个 Codex selector。失败输出不得打印 auth 文件内容、token 或账户 ID。

## 最小实现边界

- 增加独立 `src/providers/chatgpt.rs` 聚合模块及 `chatgpt/definition.rs`、`chatgpt/registration.rs`；不把 ChatGPT 行为塞入
  `openai` definition。
- 增加 `ProviderKind::ChatGpt`，contract 只允许 `OAuth2BearerAccessToken`、Codex backend endpoint profile 与 Responses；
  Chat、Embeddings 和 WebSocket 均关闭。
- 编译一个单 member credential pool 和一个 `gpt-5.6-sol` target；target base 固定为当前 Codex source 的 ChatGPT Codex
  backend，默认 `enabled=false`，不生成 Route/Public Model。
- 扩展 Provider adapter 的固定 path profile，使模型目录使用 `models?client_version=<current-cli-version>`、Responses 使用
  `responses`，不复用 OpenAI `/v1/*` path，也不接受 CLI/credential 中的 URL。
- 在 probe composition root 增加专用 Codex auth loader。它以显式路径一次性只读最小 JSON 字段，将 access token、账户绑定、
  FedRAMP routing claim 和 expiry 放入 purpose-bound 临时 credential；不反序列化为可输出对象，不保留 refresh token，不写回源文件。
- 扩展 OAuth credential material，使 ChatGPT adapter 能把 `Authorization`、`ChatGPT-Account-ID` 与条件性的 `X-OpenAI-Fedramp`
  都放入敏感 header 集；任何 Debug、错误、report 和测试 snapshot 只能观察 header 名或布尔结果。
- 通过有界 Codex app-server `initialize` 从受信本机 executable 取得 runtime `User-Agent`；初始化使用当前源码定义的非 originating
  `codex-backend` client identity，避免测试工具改写默认 `codex_cli_rs` originator 或追加 client suffix。使用隔离的临时 Codex home，
  不能读取或修改真实 auth cache。ChatGPT probe 只能使用该值，不增加任意 `--user-agent`/header selector。
- 固定注入当前 Codex profile 所需的 `originator` 与 `version` 普通 header；测试验证它们与所记录源码基线一致，report 不返回原始值。
- 为 ChatGPT 模型目录解析 `models[]`，只判断编译模型是否存在；不根据返回目录动态注册 Model、Target、Route 或 Public Model。
- Responses probe 使用最小文本请求和 `stream=true`，复用有界 SSE framer，只有观察到合法 `response.completed` 才报告支持；错误、EOF、
  body/event 超限和下游取消均不得形成成功。
- 保持现有 OpenAI/LongCat/OpenRouter/DeepSeek/MiMo、API-key TOML、服务启动和普通 probe 行为不变。

明确不做：PKCE、浏览器 callback、device code、读取/持久化 refresh token、主动 refresh、401 重放、auth file 写回、keyring、
多账号 pool、余额/配额、ChatGPT Public Model/Route、Chat Completions、WebSocket、tool loop、SDK/Codex 反向调用 OpenBridge，或对
ChatGPT 私有协议作生产稳定性承诺。

## 本次验证

### 确定性本地验证

先运行聚焦测试，再运行 Rust baseline：

```powershell
cargo test --locked --test provider_contract chatgpt_provider_uses_codex_backend_profiles_and_oauth_credential
cargo test --locked --test example_config chatgpt_probe_target_is_compiled_but_not_publicly_routable
cargo test --locked --test codex_auth_file_contract
cargo test --locked probe::tests::chatgpt_probe_matches_codex_identity_models_and_responses_sse
cargo test --locked --bin openbridge-probe
cargo fmt -- --check
cargo test --locked
cargo clippy --locked -- -D warnings
git diff --check
```

确定性测试只使用合成 token/JWT 与 fake transport，不读取真实 Codex auth、不调用网络，也不证明真实订阅可用。

### Codex CLI 请求身份验证

- 记录 `F:\codespace\codex` 的实际 source commit 和 `codex --version`；
- 以非 originating `codex-backend` client identity 通过 Codex app-server `initialize` 获得当次 runtime `User-Agent`，与 ChatGPT probe
  fake transport 捕获值逐字节比较；
- 记录 `user_agent_matches_codex_cli=true`、CLI 版本和平台，不在文档/报告中保存完整 User-Agent、账户或认证 header；
- 若 app-server 不可启动、超时或返回非法 header，真实 probe 在 egress 前失败，不能退回猜测字符串。

### 真实 ChatGPT Provider 验收

在用户明确提供的本机 Codex file credential store 上运行前述 probe，并记录：

- Codex source commit、CLI 版本、平台与执行时间；
- 固定 host/profile、`GET models` 与 `POST responses`，但不记录 query 外的私人 URL、header 或正文；
- `gpt-5.6-sol` 是否在模型目录、HTTP status、SSE 是否出现唯一合法 terminal；
- auth 文件运行前后内容 hash 相同；hash 只做本地比较，不写入 report；
- credential/header/body 均未进入 stdout、stderr、trace 或提交文件。

真实 probe 是本焦点必要的外部验收层，但不属于默认 `cargo test`。如果 token 已过期或上游返回 401/403，本阶段如实记录失败并停止；不能
临时实现 refresh 来扩大当前焦点。

### 本焦点不运行的层

- OpenAI SDK、Hermes、LiteLLM 或通用 Codex-as-client compatibility；
- PKCE/device login、refresh/rotation、并发 refresh 和 401 recovery；
- ChatGPT 多账号、负载、长时间运行、生产稳定性或配额验收；
- Python protocol corpus；本焦点不修改 `testdata/` 或 `tools/corpus/`。

## 结果记录

- 已证明的事实：完成后写入[当前实现说明](../implementation-status/current-implementation.md)，只记录 Provider/profile、只读 loader、
  probe 行为、确定性测试和真实上游实际结果。
- 仍未知或需另起焦点：PKCE/device flow、refresh token rotation、持久化 backend、expiry scheduler、401 recovery、常驻 Route/Public
  Model、其他订阅计划与 endpoint 长期稳定性。
- 完成后把本文恢复为无活动焦点；第二阶段必须重新使用模板建立一个新的可观察行为，不能在本文件累积实施历史。

## 关联文档

- [产品范围](../functional-requirements/product-scope.md)
- [ChatGPT subscription OAuth credential lifecycle](../functional-requirements/upstream-oauth-credential-lifecycle.md)
- [配置、凭证与受信运行边界](../functional-requirements/configuration-and-credentials.md)
- [交付与证据要求](../functional-requirements/delivery-and-evidence.md)
- [Codex 设备登录与 token 刷新调研](../references/codex/codex-device-auth-token-refresh-analysis.md)
- [当前实现说明](../implementation-status/current-implementation.md)
