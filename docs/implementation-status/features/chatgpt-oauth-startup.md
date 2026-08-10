# 功能：ChatGPT OAuth2 生命周期与 Responses 数据面

## 状态

**已完成（当前受限范围）。** OpenBridge 已具备独立 ChatGPT Provider 的 OAuth2 bundle 加载、显式 device/PKCE 登录、到期驱动
refresh，以及 Spark、GPT-5.5、GPT-5.6 Luna/Terra 和作为 `gpt-5.6-sol` pool source 的 Sol 五个固定 target profile 的注册与数据面。
五个模型当前均有 Chat/Responses、stream on/off 与 omitted/high 的真实最小文本证据。

## 已完成内容

- `openbridge-auth login chatgpt` 使用固定注册的 device interaction、authorization-code + PKCE 流程，完成 token bundle 校验后事务性写入
  OpenBridge-owned auth 文件。
- 启动时为不存在的 OpenBridge-owned auth 文件创建空的待登录文件；对存在且非空文件校验完整性、Provider/context 绑定、token 类型和过期信息，
  并将可用 bundle 放入独立 `OAuth2CredentialManager`。
- 到期前 refresh 在进程内 gate 和文件锁内重新加载持久化文档；成功后校验新 bundle、原子写回并发布新的 credential generation。
- ChatGPT 的五个 target profile（包括 `chatgpt-gpt-5-6-sol`）各自编译一个 Responses Native Route 和一个 Chat→Responses Bridge Route，
  固定到同一受信 Codex backend 和共享 OAuth pool；Sol 的两个 Route 作为 source 归入下游 `gpt-5.6-sol` Provider 池。
- GPT-5.6 Luna/Terra 的下游 Public Model id 分别为 `gpt-5.6-luna` 和 `gpt-5.6-terra`；`chatgpt/gpt-5.6-*` canonical identity 与
  `chatgpt-gpt-5-6-*` target/Route identity 仍保持 Provider-qualified。
- GPT-5.5 与 GPT-5.6 的 Responses upstream contract 声明 function tools、parallel tool calls 和 structured outputs；ChatGPT 的
  Chat→Responses Bridge 对应转换 function tools、parallel tool calls 以及 `response_format`/`text.format` 的 text、JSON object 和
  JSON Schema 形状。Spark 仍保持文本-only capability。
- ChatGPT adapter 固定 SSE `Accept`、`originator` 和 headless Codex CLI UA；它要求 `stream: true`，把字符串 `input` 转为 user
  message 数组、强制 `store: false`，并在 Native 或 Chat→Responses Bridge 完成后注入 Bootstrap 的 `chatgpt_instructions`。只有 active
  pool 保留 ChatGPT Target 时该字段才必须非空且非纯空白；未激活 ChatGPT 不阻止其他 Provider 启动。adapter 仍在 egress 前拒绝且不公开
  当前 backend 不接受的输出 token limit 参数。真实 backend 的成功 SSE
  可以缺失 `Content-Type`；静态 ChatGPT media profile 识别该形状、执行完整 lifecycle 校验并向下游规范化 SSE media type。
- GPT-5.5 与 GPT-5.6 的 `seed` 仍作为下游可接受普通提示公开，并在四个 advanced ChatGPT Responses API 的 candidate egress 删除。
  `include_reasoning` 会改变 reasoning 可见性，当前 API 将其显式禁用，Chat/Responses 固定 interface 不再公开并在 egress 前拒绝。
  输出 token 上限、reasoning level、tools、state 与其他能力字段同样不进入普通参数忽略例外。
- GPT-5.5、GPT-5.6 Luna/Terra/Sol 的 canonical profile 在 Chat 与 Responses interface 公开 `service_tier`；Responses Native
  与 Chat→Responses Bridge 保留下游提交的 JSON 值，不在网关内新增枚举、改写值或宣称真实 fast/priority 服务质量。
- 请求只从 manager 借用短生命周期、账户绑定的当前 generation。首个预提交 `401` 先 guarded reload，persisted generation 未变化时才
  refresh，然后只重放一次；第二个 `401` 把仍被拒绝的 generation 标记为 `reauth_required`。
- 管理员可通过 `openbridge-probe --target <chatgpt-target> --list-models` 或 `--responses` 对已激活的 ChatGPT target 执行固定 Models
  manifest 或 streaming Responses 基础 probe；它只借用选定 manager 的短期 lease，不启动服务、不打开未选中的 auth 文件，也不参与
  生产请求调度。Responses probe 不携带工具，并以 adapter 识别的正常 SSE 终态作为成功条件。
- 服务不会读取本机 Codex auth/cache、terminal identity 或隐式登录；登录、refresh、存储、请求诊断和验收记录都不输出 token、账户、
  locator 或业务响应正文。

## 实现边界

- 登录与 manager 位于 [`src/oauth2_credentials/`](../../../src/oauth2_credentials/)，ChatGPT 注册与 wire 规则位于
  [`src/providers/chatgpt/`](../../../src/providers/chatgpt/)，请求级恢复位于
  [`src/ingress/forwarding.rs`](../../../src/ingress/forwarding.rs)。
- 当前 ChatGPT 上游只公开 streaming Responses 文本、function tool、parallel tool calls 和 structured output；下游 Responses/受限 Chat
  Bridge 均可选择 JSON 或 SSE，非流式模式由有界 SSE takeover 完成。WebSocket、Batch、Embeddings、hosted/custom tool、MCP、多模态、
  background、stateful response 和完整 Agent loop 都未开放。
- 当前只有一个账户绑定 OAuth pool，不进行账户轮换或跨 Provider fallback；服务请求和显式 ChatGPT 基础 probe 都只借用该账户的短生命周期
  lease，`429` 只进入 target cooldown。
- 当前不提供运行中换账户。换账户需要停止服务，手动删除 private upstream binding 指向的 OpenBridge-owned `auth_json_file` 及同一登录
  流程明确创建的其他 OpenBridge-owned 授权文件（如有），再显式登录并重启；本机 Codex auth cache 始终不在操作范围内。
- 真实调用只证明当前账户、当前网络、当前 backend 与本次 payload；不构成其他账户、entitlement、SDK、工具、负载、长稳或生产兼容承诺。

## 验证证据

- [`tests/example_config.rs`](../../../tests/example_config.rs) 覆盖五个 target/Public Model/Route 的编译、能力和固定规划。
- [`tests/forwarding_contract.rs`](../../../tests/forwarding_contract.rs) 覆盖五个模型的模型改写、streaming 请求 envelope、账户绑定
  header、Native/Chat Bridge 共用的 startup instructions、首次 `401` reload/replay、第二次 `401` fail-closed，以及非流式/输出限制请求的
  pre-egress 拒绝；`tests/config_contract.rs` 覆盖 active ChatGPT 条件下缺失、空值和纯空白启动失败以及 inactive ChatGPT 的可选行为。
- [`tests/bridge_conversion_contract.rs`](../../../tests/bridge_conversion_contract.rs) 覆盖 Chat/Responses structured output request shape 的双向
  转换和未知字段拒绝。
- [`src/oauth2_credentials/manager.rs`](../../../src/oauth2_credentials/manager.rs) 的单元测试覆盖 rejected generation 的强制 refresh、
  single-flight 与 stale caller 复用胜出 generation。
- [`tests/oauth2_login_cli.rs`](../../../tests/oauth2_login_cli.rs)、[`tests/startup_contract.rs`](../../../tests/startup_contract.rs)、
  [`tests/upstream_credential_config.rs`](../../../tests/upstream_credential_config.rs) 继续覆盖登录、启动和 auth 文件生命周期。
- [上游模型发现与基础 API 探测](../capability-probing.md)记录 ChatGPT Models/Responses probe 的固定路径、OAuth lease 边界和观察
  规则；该基础观察不等同于工具、SDK、模型语义或长期 Provider 验收。

2026-08-09 使用当前 private 配置和同一最小文本，通过修复后本地 OpenBridge 完成五个 GPT 模型的 40 单元矩阵：

- Chat/Responses × `stream:false/true` × reasoning omitted/high 全部为 HTTP 200，并具有合法 JSON/SSE 完成终态；
- 0 个 HTTP、协议或传输错误，0 个单元触发 429/503 重试；
- 测试只保存状态、终态、reasoning-present 布尔值和耗时，不保存 credential、账户、opaque continuation、响应正文或 request ID；
- GPT-5.3 Codex Spark 与 GPT-5.5 的完成 output 含不可读 `encrypted_content` reasoning item；非流式 Chat 与既有流式 Chat 一致，只保留
  可表示输出，不泄露或伪造 opaque reasoning。

同日最新严格参数复测以 GPT-5.6 Luna 为真实代表：Chat/Responses `seed` 2/2 返回合法 HTTP 200 JSON 终态，
`include_reasoning` 2/2 在 egress 前返回带精确 `param` 的 `unsupported_model_capability`；全部一次完成，没有最终 429/503 或
传输错误。确定性 RecordingTransport 测试覆盖 GPT-5.5 与三个 GPT-5.6 advanced profile，确认 seed 不进入上游 body 且
`include_reasoning` 不产生 transport attempt。

以下表格保留 2026-08-06 历史调用当时的 Public Model 名称；当前 GPT-5.3 Codex Spark、Luna 和 Terra 名称已按注册契约改为
`gpt-5.3-codex-spark`、`gpt-5.6-luna` 和 `gpt-5.6-terra`。该历史记录通过同一固定 target 证明上游数据面，不代表旧名称仍可用。

2026-08-06 使用当前 private 配置和已有 OpenBridge-owned `auth.json`，通过本地 OpenBridge 的 `/v1/responses` 发出同一最小 streaming
文本请求；验收只记录 HTTP 状态、SSE terminal 类别和耗时，没有记录 credential、账户或响应正文：

| Public Model                    | HTTP | SSE 终态             | 本次耗时 |
|---------------------------------|------|----------------------|----------|
| `chatgpt-gpt-5.3-codex-spark`  | 200  | `response.completed` | 1848 ms  |
| `chatgpt-gpt-5.6-luna`         | 200  | `response.completed` | 1393 ms  |
| `chatgpt-gpt-5.6-terra`        | 200  | `response.completed` | 1133 ms  |
| `gpt-5.6-sol` (ChatGPT source) | 200  | `response.completed` | 1601 ms  |

同日最终脱敏 preflight 再次确认该 bundle 完整、access token 未过期且未进入 120 秒 safety window。最终验证结果：

上表中 Sol 的历史调用使用的是同一 ChatGPT target source；当前公共名称和五模型完整最小矩阵以 2026-08-09 最终结果为准。

- `cargo fmt -- --check`：通过；
- 变更 Rust 文件的 `rustfmt --edition 2024 --check`：通过；
- `cargo test --locked`：通过；
- `cargo clippy --locked -- -D warnings`：通过；
- `git diff --check -- README.md config/upstream-credentials.example.toml docs src tests`：通过；仓库级 `git diff --check` 只报告本轮未修改的
  `.gitignore:30` 末尾新增空行。

本轮新增 capability 与 Bridge 的验证是确定性 Rust 证据；真实登录/refresh authority、真实 ChatGPT 工具/structured-output 调用、
外部 SDK compatibility、多模态、负载和长稳测试未执行。本轮没有修改 `testdata/` 或 `tools/corpus/`，因此未运行 Python corpus 基线。

2026-08-09 `service_tier` 下游透传变更的确定性验证：

- `forwarding_contract::chatgpt::chatgpt_service_tier_is_advertised_and_preserved_across_native_and_bridge` 通过，覆盖四个目标模型
  的 Chat/Responses `supported_parameters`，以及 `priority` 在 Responses Native 和 Chat→Responses Bridge 上游 body 中的原值保留；
- `cargo test --locked --test forwarding_contract chatgpt`：8 项通过；`cargo test --locked`：通过；
  `cargo fmt -- --check`、`cargo clippy --locked -- -D warnings` 和 `git diff --check`：通过；
- 未运行真实 ChatGPT Provider、外部 SDK、负载或长期运行验收，因此本轮只证明网关契约和合成 transport 的 wire 透传。

2026-08-10 `chatgpt_instructions` 启动约束与上游注入的确定性验证：

- `config_contract` 覆盖无 active ChatGPT Target 时字段可缺失，以及 active ChatGPT Target 下缺失、空字符串和纯空白均启动失败；
- `forwarding_contract` 覆盖 Responses Native 与 Chat→Responses Bridge 在 Provider adapter 处注入同一启动值，`probe` 单元测试覆盖同一
  Responses request preparation 路径，`startup_contract` 覆盖真实进程组合夹具；
- `cargo test --locked --test config_contract`：21 项通过；`cargo test --locked --test forwarding_contract chatgpt`：8 项通过；相关
  `probe` 与 `startup_contract` 聚焦测试通过；
- `cargo test --locked -- --skip configuration::checked_in_examples_compile_into_a_closed_runtime_registry`：通过；未跳过的仓库级测试只有该既有
  示例一致性检查失败，原因是 `config/bootstrap.toml` 与 example 中本轮未修改的 telemetry enabled 值不同；
- `cargo fmt -- --check`、`cargo clippy --locked -- -D warnings` 和 `git diff --check`：通过。未运行真实 ChatGPT Provider、外部 SDK、负载或
  长期运行验收；本轮未修改 `testdata/` 或 `tools/corpus/`，因此未运行 Python corpus 基线。

## 相关文档

- [功能需求：ChatGPT subscription OAuth credential lifecycle](../../functional-requirements/upstream-oauth-credential-lifecycle.md)
- [功能需求：Bootstrap、代码注册表、凭证与受信边界](../../functional-requirements/configuration-and-credentials.md)
- [Provider 注册表与模型目录](provider-registry-and-model-catalog.md)
