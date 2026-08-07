# 功能：ChatGPT OAuth2 生命周期与 Responses 数据面

## 状态

**已完成（当前受限范围）。** OpenBridge 已具备独立 ChatGPT Provider 的 OAuth2 bundle 加载、显式 device/PKCE 登录、到期驱动
refresh，以及 Spark、GPT-5.5 和 GPT-5.6 Luna/Terra/Sol 五个固定 Responses-native Public Model 的注册与数据面。此前的真实
Provider 证据仍只覆盖 Spark 和 GPT-5.6 三个模型。

## 已完成内容

- `openbridge-auth login chatgpt` 使用固定注册的 device interaction、authorization-code + PKCE 流程，完成 token bundle 校验后事务性写入
  OpenBridge-owned auth 文件。
- 启动时为不存在的 OpenBridge-owned auth 文件创建空的待登录文件；对存在且非空文件校验完整性、Provider/context 绑定、token 类型和过期信息，
  并将可用 bundle 放入独立 `OAuth2CredentialManager`。
- 到期前 refresh 在进程内 gate 和文件锁内重新加载持久化文档；成功后校验新 bundle、原子写回并发布新的 credential generation。
- `chatgpt-gpt-5.3-codex-spark`、`chatgpt-gpt-5.5`、`chatgpt-gpt-5.6-luna`、`chatgpt-gpt-5.6-terra` 与
  `chatgpt-gpt-5.6-sol` 各自编译一个 Responses Native Route 和一个 Chat→Responses Bridge Route，固定到同一受信 Codex backend
  和共享 OAuth pool。
- GPT-5.5 与 GPT-5.6 的 Responses upstream contract 声明 function tools、parallel tool calls 和 structured outputs；ChatGPT 的
  Chat→Responses Bridge 对应转换 function tools、parallel tool calls 以及 `response_format`/`text.format` 的 text、JSON object 和
  JSON Schema 形状。Spark 仍保持文本-only capability。
- ChatGPT adapter 固定 SSE `Accept`、`originator` 和 headless Codex CLI UA；它要求 `stream: true`，把字符串 `input` 转为 user
  message 数组、强制 `store: false`，并在 egress 前拒绝且不公开当前 backend 不接受的输出 token limit 参数。
- 请求只从 manager 借用短生命周期、账户绑定的当前 generation。首个预提交 `401` 先 guarded reload，persisted generation 未变化时才
  refresh，然后只重放一次；第二个 `401` 把仍被拒绝的 generation 标记为 `reauth_required`。
- 管理员可通过 `openbridge-probe --target <chatgpt-target> --list-models` 对已激活的 ChatGPT target 执行固定 Models manifest probe；
  它只借用选定 manager 的短期 lease，不启动服务、不打开未选中的 auth 文件，也不参与生产请求调度。
- 服务不会读取本机 Codex auth/cache、terminal identity 或隐式登录；登录、refresh、存储、请求诊断和验收记录都不输出 token、账户、
  locator 或业务响应正文。

## 实现边界

- 登录与 manager 位于 [`src/oauth2_credentials/`](../../../src/oauth2_credentials/)，ChatGPT 注册与 wire 规则位于
  [`src/providers/chatgpt/`](../../../src/providers/chatgpt/)，请求级恢复位于
  [`src/ingress/forwarding.rs`](../../../src/ingress/forwarding.rs)。
- 当前 ChatGPT 只公开 streaming Responses 文本、function tool、parallel tool calls 和 structured output，以及要求 SSE 的受限 Chat
  Bridge；WebSocket、Batch、Embeddings、hosted/custom tool、MCP、多模态、background、stateful response 和完整 Agent loop 都未开放。
- 当前只有一个账户绑定 OAuth pool，不进行账户轮换或跨 Provider fallback；服务请求和显式 ChatGPT Models probe 都只借用该账户的短生命周期
  lease，`429` 只进入 target cooldown。
- 当前不提供运行中换账户。换账户需要停止服务，手动删除 private upstream binding 指向的 OpenBridge-owned `auth_json_file` 及同一登录
  流程明确创建的其他 OpenBridge-owned 授权文件（如有），再显式登录并重启；本机 Codex auth cache 始终不在操作范围内。
- 真实调用只证明当前账户、当前网络、当前 backend 与本次 payload；不构成其他账户、entitlement、SDK、工具、负载、长稳或生产兼容承诺。

## 验证证据

- [`tests/example_config.rs`](../../../tests/example_config.rs) 覆盖五个 target/Public Model/Route 的编译、能力和固定规划。
- [`tests/forwarding_contract.rs`](../../../tests/forwarding_contract.rs) 覆盖五个模型的模型改写、streaming 请求 envelope、账户绑定
  header、首次 `401` reload/replay、第二次 `401` fail-closed，以及非流式/输出限制请求的 pre-egress 拒绝。
- [`tests/bridge_conversion_contract.rs`](../../../tests/bridge_conversion_contract.rs) 覆盖 Chat/Responses structured output request shape 的双向
  转换和未知字段拒绝。
- [`src/oauth2_credentials/manager.rs`](../../../src/oauth2_credentials/manager.rs) 的单元测试覆盖 rejected generation 的强制 refresh、
  single-flight 与 stale caller 复用胜出 generation。
- [`tests/oauth2_login_cli.rs`](../../../tests/oauth2_login_cli.rs)、[`tests/startup_contract.rs`](../../../tests/startup_contract.rs)、
  [`tests/upstream_credential_config.rs`](../../../tests/upstream_credential_config.rs) 继续覆盖登录、启动和 auth 文件生命周期。
- [上游模型发现与能力探测](../capability-probing.md) 记录 ChatGPT Models probe 的固定路径、OAuth lease 边界和真实列表观察；该观察不等同于
  数据面、SDK 或长期 Provider 验收。

2026-08-06 使用当前 private 配置和已有 OpenBridge-owned `auth.json`，通过本地 OpenBridge 的 `/v1/responses` 发出同一最小 streaming
文本请求；验收只记录 HTTP 状态、SSE terminal 类别和耗时，没有记录 credential、账户或响应正文：

| Public Model                    | HTTP | SSE 终态             | 本次耗时 |
|---------------------------------|------|----------------------|----------|
| `chatgpt-gpt-5.3-codex-spark`  | 200  | `response.completed` | 1848 ms  |
| `chatgpt-gpt-5.6-luna`         | 200  | `response.completed` | 1393 ms  |
| `chatgpt-gpt-5.6-terra`        | 200  | `response.completed` | 1133 ms  |
| `chatgpt-gpt-5.6-sol`          | 200  | `response.completed` | 1601 ms  |

同日最终脱敏 preflight 再次确认该 bundle 完整、access token 未过期且未进入 120 秒 safety window。最终验证结果：

- `cargo fmt -- --check`：通过；
- 变更 Rust 文件的 `rustfmt --edition 2024 --check`：通过；
- `cargo test --locked`：通过；两个需要独立 Python loopback/下载 OpenAI SDK 的测试保持 ignored；
- `cargo clippy --locked -- -D warnings`：通过；
- `git diff --check -- README.md config/upstream-credentials.example.toml docs src tests`：通过；仓库级 `git diff --check` 只报告本轮未修改的
  `.gitignore:30` 末尾新增空行。

本轮新增 capability 与 Bridge 的验证是确定性 Rust 证据；真实登录/refresh authority、真实 ChatGPT 工具/structured-output 调用、ignored
SDK compatibility、多模态、负载和长稳测试未执行。本轮没有修改 `testdata/` 或 `tools/corpus/`，因此未运行 Python corpus 基线。

## 相关文档

- [功能需求：ChatGPT subscription OAuth credential lifecycle](../../functional-requirements/upstream-oauth-credential-lifecycle.md)
- [功能需求：Bootstrap、代码注册表、凭证与受信边界](../../functional-requirements/configuration-and-credentials.md)
- [Provider 注册表与模型目录](provider-registry-and-model-catalog.md)
