# 当前实现说明

## 状态与范围

本文只记录当前可运行入口、外部行为、Provider 注册和验证状态。模块分层、类型职责与内部数据流统一见
[当前代码架构](current-architecture.md)。OpenBridge 仍是实验性原型；最近一次记录已通过全量 Rust 测试与
Clippy，但不代表真实 Provider、外部 SDK、负载或长期运行验收。

## 当前运行入口

默认启动：

```bash
cp config/users.example.toml config/users.toml
cp config/upstream-credentials.example.toml config/upstream-credentials.toml
# 编辑两份私有 TOML，分别填写下游用户 Key 与编译期 pool 对应的上游 API key。
cargo run --bin openbridge --locked
```

schema v2 的 `bootstrap.toml` 包含 loopback listener、两份私有 credential 文件位置、request/SSE 上限和共享 HTTP client 参数。Provider、Model、
Upstream Target、Upstream API、Route、Public Model、endpoint 和 credential pool binding 均由 Rust 代码注册；
修改后需要重新编译或重启。

运行配置与模板一一对应：`config/bootstrap.toml` 使用 `config/bootstrap.example.toml`，
`config/users.toml` 使用 `config/users.example.toml`，`config/upstream-credentials.toml` 使用
`config/upstream-credentials.example.toml`。

| Endpoint | 当前行为 | 认证 |
|---|---|---|
| `GET /healthz` | 返回 `status` 与 `registry_version` | 无 |
| `GET /v1/models` | 返回代码注册的 Public Model | 静态 Bearer |
| `POST /v1/chat/completions` | 按完整 Route 执行 Chat Native 或 Chat→Responses Bridge 的 JSON/SSE | 静态 Bearer |
| `POST /v1/responses` | 按完整 Route 执行 Responses Native 或 Responses→Chat Bridge 的 JSON/SSE | 静态 Bearer |

下游用户和 API Key 来自启动时读取的私有 `config/users.toml`。五个 Provider 的上游 pool 来自私有
`config/upstream-credentials.toml`，每项只包含编译期 pool id 与有序 `api_keys` TOML 数组。服务与 probe
不读取上游 key 环境变量或 `.env`；上游注册表只保存 pool ID、Provider 和 credential kind。
服务在 listener 绑定前把已启用用户 Key 与全部启用 target 引用的 pool 合并为不可变 `CredentialStore`；
未知、缺失或重复 pool、损坏 TOML、空数组、空白成员或重复 secret 会阻止启动。运行时请求只读取该快照，不重新读取文件；
改变 pool 必须重启。
Store 条目同时冻结 credential type、仅含类别的 source、generation 与可选过期时间；文件路径不进入
这些运行时诊断元数据。上游借用同时匹配 `pool_id + member_id + ProviderKind + CredentialKind`。类型系统已能表达
`OAuth2BearerAccessToken`，但现有 Provider contract 仍只接受 `ApiKey`，当前没有 token 获取、refresh、热更新
或 401 refresh/retry 行为。

## Provider 与请求行为

闭合 `ProviderKind` 当前包含 OpenAI、LongCat、OpenRouter、DeepSeek 与 Xiaomi MiMo，五者都进入 compiled
registry。当前可路由目录如下；“Bridge 候选”只表示已注册的协议转换路径，不表示上游原生支持该协议：

| Provider | Public Model | 固定 Upstream Target | 下游可用 Route surface | Credential pool |
|---|---|---|---|---|
| OpenAI | `code-primary` | `openai-main` | Chat/Responses Native-first，各有指向相反 Upstream API 的 Bridge 候选 | `openai-primary` |
| LongCat | `LongCat-2.0` | `longcat-2` | Chat/Responses Native-first，各有指向相反 Upstream API 的 Bridge 候选 | `longcat-primary` |
| OpenRouter | `nemotron-3-ultra` | `openrouter-nemotron-3-ultra` | Chat 与无状态 Responses 各一条 Native Route；无 Bridge | `openrouter-primary` |
| DeepSeek | `deepseek-v4-pro`、`deepseek-v4-flash` | 同名两个 target | Chat Native；Responses→Chat Bridge；无原生 Responses Upstream API | `deepseek-primary` |
| Xiaomi MiMo | `mimo-v2.5-pro`、`mimo-v2.5` | `mimo-v2-5-pro`、`mimo-v2-5` | Chat/Responses Native-first，各有指向相反 Upstream API 的 Bridge 候选 | `mimo-primary` |

OpenRouter 的 `store`、`previous_response_id` 与 `background` 能力关闭，也未注册 `:free` 变体。五个 Provider
分别拥有独立静态 definition、endpoint profile、upstream model 与能力；当前所有已注册上游仍采用
OpenAI-compatible wire，尚未接入或实测真实异构 wire protocol Provider。

MiMo 的 `mimo-v2.5-pro` 与 `mimo-v2.5` Chat/Responses Native Upstream API 均声明支持
`parallel_tool_calls`、image input 和 structured output；两种协议的 `store` 均关闭，Responses 的
`previous_response_id` 与 `background` 均关闭。两种协议的 `reasoning_output` 保持 `Unknown`，因此这组声明只
控制请求能力 gate 和 Native 原样转发，不证明 Provider 会输出可读 reasoning，也不扩大反向 Bridge 的转换能力。

五个具体 Provider 均以静态 `ProviderDefinition` 聚合自身 contract 与 adapter；
`ProviderKind::definition` 是唯一穷举分派，现有 contract 与 adapter 查询接口都委托给该描述符。
descriptor 不注册 target、Route 或 Public Model，也不读取 endpoint origin 或 credential。

canonical 模型目录当前包含 17 个定义。其中 16 个来自 LiteLLM 部署清单中的唯一 Chat/Responses 模型组，
覆盖 GPT-5.6/5.5/5.3 Codex Spark、DeepSeek V4、MiMo V2.5、Qwen3.7、GLM-5.2、Kimi K3、MiniMax M3、
Hy3 与 Nemotron 3 Ultra；已确认的 context、输出上限、参数、reasoning 状态和 level 保存在各自模型模块。
其中 GPT-5.6 Sol、LongCat 2.0、Nemotron 3 Ultra、两个 DeepSeek V4 和两个 MiMo V2.5 模型已被固定 target 与
Public Model 引用；其余目录项尚未新增 Provider target 或 Public Model route，不构成真实可调用声明。Nemotron
embedding/rerank 因当前没有对应协议模型类型而未纳入 `ModelConfig`。

2026-08-02 已按 OpenRouter 官方目录精确匹配其中 16 个模型，并修订现有 `ModelConfig` 可表达的描述、context、
最大输出、参数和 reasoning efforts。`openai/gpt-5.3-codex-spark` 没有精确匹配，未使用相近的
`openai/gpt-5.3-codex` 代替；其 128,000 context、128,000 最大输出和四档 level 为人工修订值。Nemotron
canonical 配置采用基础模型上界，不采用 `:free` endpoint 的收窄值；完整采集边界见
[OpenRouter 模型目录快照](../references/openrouter/model-catalog-2026-08-02.md)。

请求路径当前会：

- 通过同一个 `CredentialStore` constant-time 匹配下游 Key，并按
  `pool_id + member_id + ProviderKind + CredentialKind` 借用上游 Key 及其非敏感元数据；
- 在 egress 前校验 Public Model、协议、streaming、tools、image、structured output、store、continuation、background、输出限制和 reasoning；
- 识别 `none`、`minimal`、`low`、`medium`、`high`、`xhigh`、`max` canonical reasoning level，并只允许
  当前 Model 显式声明的子集；`none` 保持为显式禁用值，不与字段缺失合并；
- 对 Native Route 按选定 Upstream API 的已校验代码规则映射 reasoning level；映射仅修改候选请求副本，
  Chat 只接受标准 `reasoning_effort`，Responses 只接受标准 `reasoning.effort`，并通过
  `reasoning_level_mapped` tracing event 记录源/目标；不把跨协议字段别名当作标准字段；
- 将 selected Upstream API 的 `upstream_model` 写入请求；
- 经各 Provider 的受信 request-header hook 处理普通 header；OpenAI 与 LongCat 把下游 `User-Agent` 覆盖到上游，OpenRouter 不转发可选 attribution/routing header；hook 容器支持普通 header 增添、替换、转换和删除，同时保持认证、cookie、Host 与 proxy header 隔离；
- 保留同协议下未知但合法的 JSON 字段；
- 对 `Bridged` Route 只转换 allowlist 内的 text/function tool/tool result 与明文 reasoning channel 语义，未知或不可表达字段在 egress 前拒绝；
- 对 `previous_response_id` 关闭跨 target fallback；
- Native Route 保持非流式 status/body 和流式原始 bytes；Bridged Route 转换非流式 JSON 与增量 SSE event；
- 两种路径都检查 SSE UTF-8、framing、event size 与 terminal，并保持有限安全 header；
- LongCat Responses 按 data JSON 顶层 `type` 的 `response.completed`、`response.failed` 与
  `response.incomplete` 识别终态，不要求上游额外发送 `event:` 字段；
- OpenRouter Responses 按真实 Nemotron-3 stream 的 data JSON 顶层 `type` 识别 `response.completed`、
  `response.failed` 与 `response.incomplete`，不把尾随 `[DONE]` 代替语义终态；
- OpenAI terminal 事件词汇与 discriminator 来源由编译期 adapter 分开建模；同一 SSE event 同时携带相互
  冲突的 `event:` 与 data JSON `type` terminal 时失败关闭，各 Provider 不接受未绑定的 discriminator 或其他
  terminal family；
- 在 stream/non-stream 提交下游 response 前，对 transient status/transport error 使用请求级最多 6 次、每候选最多 2 次的有限 retry/fallback，并执行 50～500 ms capped exponential backoff；
- 无状态请求从 Provider pool 的共享 cursor 做 round-robin；只有 HTTP 429 会冷却当前 member 并在同一候选内轮转，5xx/timeout/transport retry 保持当前 member，其他 4xx 不轮转；
- member cooldown 使用 `Retry-After` delta/date，缺失或非法时为 1 秒并封顶 30 秒；同请求不会回绕已经 429 的 member，pool 大小不扩大 6/2 attempt 上限；
- 当前候选耗尽后只沿 RoutePlan 进入同一 Public Model 的其他完整候选；全部失败时返回最后一个安全 HTTP 错误或稳定 transport error；
- 429 只记录 member cooldown；暂时性 5xx 与 transport failure 记录 target `fault_domain` cooldown。后续无状态请求跳过已知受限 member/target，target-bound continuation 要求单成员 pool 并继续尝试原 target；
- 在下游中断 pending send、退避等待或丢弃 response body 时取消相应上游工作，不再启动后续 attempt；
- 认证后将稳定用户身份写入请求上下文，并在 response body 正常 EOF、流错误或下游取消时恰好提交一次终态观测；
  外层 body observer 仅在自身提交 EOF 或错误后报告 end-stream，避免完整单帧 body 被误记为下游取消。

## 遥测指标

请求生命周期 tracing、全局低基数累计值和按 Provider attempt 聚合的性能、usage、cache 指标已拆分到
[遥测指标](telemetry-metrics.md)。本页只保留运行行为和验证范围，不复制容易漂移的指标字段、采集口径或
未接入 exporter 的限制。

## Protocol Bridge

`src/bridge.rs` 是 Protocol Bridge 门面；`chat.rs` 与 `responses.rs` 分别实现两种 stream 状态机，
`conversion/request/*`、`response.rs` 与 `stream/*` 分别实现双向请求、非流式响应和增量 SSE 转换。
`BridgePlan` 与 renderer 按 wire 顺序固定 response/item/call/index identity，累计 text 与 function arguments，
区分 `completed`、`failed`、`incomplete` 和独立 `error` terminal，并在 event/type 冲突、identity 冲突、
不完整 JSON arguments、terminal 后事件、重复 terminal 或 EOF-before-terminal 时失败关闭。

生产 Router 已验证双向 text、function schema、tool call/result、并行 fragmented arguments、非流式 JSON、
流式 terminal 与 invalid stream 关闭。具备模型 reasoning capability 且由 `ReasoningOutput` 确认方向兼容可读输出的
Responses reasoning 与 Chat `reasoning_content` 可在 Bridge 中保留为独立 reasoning channel；`encrypted_content`、
`previous_response_id`、hosted/custom tool、image、structured output、background/store、Provider 私有扩展和其他未建模字段
不做降级转换，会在 egress 前拒绝；MiMo 对 image、structured output 和并行工具的声明仅适用于其 Native API。

## 显式 probe

`openbridge-probe --target <id>` 复用同一 bootstrap、注册表、credential Store、adapter 与 transport，可以观察模型
列表、最小 Chat/Responses 请求和 function call/result replay。它不接受 endpoint、model、header 或
credential 覆盖，只加载选中 target 的 pool 并固定使用首个 member，不遍历或改变生产 cursor/cooldown；它不读取下游用户 Key，不修改注册表，也不自动改变 capability。

## 验证状态

仓库中的 Rust 测试源码覆盖 bootstrap/registry 校验、私有 upstream credential TOML、模型规则、reasoning gate/候选级 level 映射、统一 credential Store、认证、Provider descriptor 单一分派、DeepSeek/MiMo 编译 target 与候选顺序、Provider model 改写、
capability routing、`/v1/models`、stream/non-stream 指数退避、跨 Provider fallback、请求级 attempt 硬上限、
credential round-robin/429 rotation/member cooldown、fault domain cooldown、continuation 单成员约束、retry header、SSE terminal、partial failure、pending
send/backoff/body 取消、canonical bridge request/response/SSE 转换、生产 Router Bridged Route、真实 loopback
HTTP 429 process replay 和 probe。
`tests/sdk_compatibility.rs` 是 ignored integration test，需要外部 Python/Node SDK。日常客户端可见测试优先使用
OpenAI SDK、独立 Python 脚本或 curl，不要求绑定 Codex/Hermes 等 Agent runtime。

2026-08-02 最近一次执行：

```text
cargo fmt -- --check
cargo test --locked
cargo clippy --locked -- -D warnings
git diff --check
```

结果为 134 个测试通过、1 个需要下载 OpenAI Python/Node SDK 的集成测试 ignored，Clippy 零告警，
格式与 diff 检查通过。另使用 `users.toml` 中的本地用户 key，通过逐行消费 SSE 的独立 PowerShell HTTP client
显式调用真实 LongCat、MiMo 与 DeepSeek：三者的 Chat/Responses streaming 均返回 HTTP 200、
`text/event-stream` 和明确终态；LongCat/MiMo 两种协议走 Native，DeepSeek Chat 走 Native、Responses 走
Responses→Chat Bridge。另对真实 OpenRouter `nemotron-3-ultra` 先后执行修复前复现与修复后验收的 Responses
Native streaming：两次均为 HTTP 200，所有 SSE frame 均未携带 `event:`，语义终态为 data JSON 顶层
`type=response.completed`，随后发送 `[DONE]`；修复前网关误记为 `sse_eof_before_terminal`，修复后记录
`outcome=completed`。这条真实证据只覆盖 2026-08-02 的成功流，不证明失败流、其他 OpenRouter 模型或未来
wire 稳定性。没有运行外部 SDK、Codex/Hermes、负载或长期验证。

2026-08-03 补充 DeepSeek V4 Flash high-level tool-call 验证（修复前基线）：

- 确定性回归通过：`bridge_conversion_contract` 5 个、`bridge_forwarding_contract` 7 个、`protocol_bridge_replay` 8 个测试通过；DeepSeek 编译路由、Chat-only Provider contract、DeepSeek/MiMo 原生协议声明三个精确测试各 1 个通过；`process_replay_contract` 1 个通过。
- 真实 DeepSeek Chat Native、显式 high、非流式工具调用返回 HTTP 200，带有 `reasoning_content`、一个 function tool call、`finish_reason=tool_calls`，且 arguments 是合法 JSON。把完整 assistant `reasoning_content`、tool call 和 tool result 带入下一轮后，真实请求返回 HTTP 200、`finish_reason=stop` 和文本结果，证明 high thinking tool loop 在 Chat Native 上可用。
- 真实 Responses 请求分别使用 `reasoning:{"effort":"high"}` 与顶层 `reasoning_effort:"high"` 时均在本地 Bridge preflight 返回 HTTP 400、`unsupported_request`。这是修复前 Bridge 的行为，不是 DeepSeek Responses 原生能力的验收结果。
- 真实 Responses→Chat Bridge 流式工具调用（未显式设置 reasoning，仅作对照）曾返回 HTTP 200，但客户端以 curl 18 结束；收到 `response.created`、`response.output_item.added` 和完整的 `response.function_call_arguments.delta`，未收到 `response.function_call_arguments.done`、`response.output_item.done` 或 `response.completed`。arguments 拼接后是合法 JSON。结合真实 DeepSeek Chat high 的最终 tool-call chunk 为 `content:""` 且 `finish_reason=tool_calls`，修复前 converter 在“已有 tool call 后遇到空 content”分支提前失败；这是本轮修复的触发证据。
- Responses 的 named/required `tool_choice` 触发上游 `Thinking mode does not support this tool_choice` 400；按本轮约束，非 thinking mode 不支持工具是上游预期行为，不将该限制计为 OpenBridge 转换缺陷。无显式 reasoning 的 Responses 对照路径能够产生 function call 并完成续轮，但不构成 high thinking 验收。

修复前结论是：DeepSeek V4 Flash 的 Chat Native high 工具调用和 reasoning continuation 已获真实 Provider 证据；Responses→Chat Bridge 的工具参数分片基本可拼接，但真实流在工具调用终态前中断。测试使用了本地私有配置，未在文档或输出中记录凭证。

本轮未覆盖外部 OpenAI SDK、Codex/Hermes、负载、长期运行或生产环境验收；上述高层真实 Provider 结果属于修复前基线。

2026-08-03 修订 MiMo Native capability contract：

- `tests/provider_boundary_contract.rs` 的 `mimo_contract_declares_tool_output_and_image_capabilities_without_state_or_reasoning`
  通过，确认 Chat/Responses 的并行工具、image input、structured output，以及关闭的 state capability 和 `Unknown` reasoning output。
- `tests/example_config.rs` 的 `mimo_models_are_compiled_with_dual_native_first_routes` 通过，确认两个 MiMo 模型的编译
  Upstream API、复杂请求 Native-first route 和 `store`/`previous_response_id`/`background` 拒绝边界一致。
- 本轮执行 `cargo fmt -- --check` 与上述两个 focused `cargo test --locked --test ...`；这是 deterministic Rust contract
  evidence，不等同于真实 MiMo Provider、外部 SDK、并发负载或长期运行验收。

2026-08-03 完成 MiMo Responses Native 复杂工具流回归：

- `tests/forwarding_contract.rs` 的 `mimo_responses_native_preserves_parallel_tool_stream` 使用实际 compiled registry
  中的 `mimo-v2.5`，提交同时包含 image input、structured output、两个 function tools 和
  `parallel_tool_calls: true` 的 Responses streaming 请求。
- mock upstream 将两个 function-call arguments 交错拆成 17-byte SSE chunks；Router 仍命中 `/v1/responses` Native，
  下游收到与上游完全相同的 bytes，Responses 状态机重建两个合法 arguments，并以唯一 `response.completed` 收口。
- `cargo test --locked --test forwarding_contract` 30 个测试、`cargo test --locked --test native_routing_contract` 11 个测试、
  `cargo test --locked --test provider_boundary_contract` 15 个测试和 `cargo test --locked --test example_config` 7 个测试均通过。
  这仍是 mock/deterministic 测试证据，不等同于真实 MiMo Provider 或外部 SDK 验收。
- 最终执行 `cargo fmt -- --check`、`cargo test --locked`、`cargo clippy --locked -- -D warnings` 和 `git diff --check`；
  全量 Rust 结果为 148 个测试通过、1 个需要外部 OpenAI Python/Node SDK 的集成测试 ignored，未运行该 SDK 测试。

2026-08-03 补充 DeepSeek V4 Flash 与 MiMo V2.5 reasoning output 分类测试：

- `tests/provider_boundary_contract.rs` 的 `deepseek_and_mimo_reasoning_output_types_are_explicit` 固定了 Provider contract：
  DeepSeek Chat 为 `PlainText`，对应 `reasoning_content`；DeepSeek 没有原生 Responses output；MiMo Chat/Responses 均为 `Unknown`。
- `tests/example_config.rs` 的 `compiled_reasoning_output_types_match_deepseek_flash_and_mimo_v25_routes` 核对了具体 compiled
  target：DeepSeek Responses 只能进入 Chat Bridge，MiMo 带 reasoning history 时保留 Responses Native，不能因 `Unknown`
  reasoning output 进入 Bridge。
- `tests/forwarding_contract.rs` 的 `deepseek_v4_flash_chat_native_exposes_plain_text_reasoning_content` 使用实际 compiled
  `deepseek-v4-flash` route 和分片 Chat SSE fixture，验证 `reasoning_content` 与 visible `content` 分离，并保留原始 Native bytes。
  这些是 deterministic contract/wire tests；本轮没有重复真实 Provider 网络调用，因此 MiMo 的 `Unknown` 仍需真实 probe 才能升级为具体 output 类型。
- 随后执行 `cargo fmt -- --check`、`cargo test --locked`、`cargo clippy --locked -- -D warnings` 和 `git diff --check`；全量 Rust
  结果为 151 个测试通过、1 个外部 OpenAI Python/Node SDK 测试 ignored。

2026-08-03 完成 Bridge reasoning 与 DeepSeek thinking 空文本修复：

- `cargo fmt -- --check`、`cargo test --locked`、串行 `cargo test --locked -- --test-threads=1`、`cargo clippy --locked -- -D warnings` 和 `git diff --check` 均通过；全量结果为 144 个测试通过、1 个需要外部 OpenAI Python/Node SDK 的集成测试 ignored。默认并行全量曾有一次观测测试日志捕获波动，单测复跑与串行全量均通过。
- `bridge_conversion_contract` 11 个测试覆盖两种 reasoning request 配置、Responses reasoning item、非流式 tool response、Chat→Responses reasoning/text stream、Responses summary→Chat stream、empty `content`、非成功/迟到终态和 opaque/未知 reasoning 形状拒绝；`bridge_forwarding_contract` 9 个测试包含生产 Router + mock upstream 的同类闭环，并分别证明 `PlainText` 放行与 `Unknown` 在 egress 前拒绝；`native_routing_contract` 11 个测试覆盖 reasoning capability、`none`、冲突配置、输出类型路由门禁和 `reasoning:false` fail-closed；DeepSeek/MiMo/LongCat 编译配置断言通过。
- Chat 上游的 reasoning-only 空 `content` chunk 不再启动或污染 visible message；tool-call arguments 会继续累计并完成 `response.function_call_arguments.done`、`response.output_item.done`、`response.completed`。当 reasoning item 占用 output index 0 时，普通 text/tool output index 会从 1 开始。
- Chat Bridge 仅把 `stop` 与 `tool_calls` 视为可转换的成功 finish reason；`length`、`content_filter` 等非成功终态以及 finish reason 后追加 chunk 均 fail closed，不伪造 `response.completed`。
- Responses 标准 `reasoning.effort` 与 Chat `reasoning_effort` 可在 Bridged Route 间映射；明文 `reasoning_content`、`reasoning_text` 和 `summary_text` 保持在独立 reasoning channel，不进入 user-visible text；opaque `encrypted_content` 和未知 reasoning 形状在 egress/stream state 边界拒绝。
- Responses 顶层 `reasoning_effort` 不是标准字段，Native 与 Bridge 均拒绝；Chat 仅使用标准 `reasoning_effort`，Responses 仅使用标准 `reasoning.effort`，不保留跨协议入站别名。
- Chat Bridge 对 `reasoning_effort` 的布尔等非标准形状 fail closed；显式 `null` 仍按省略处理。Responses 的 `reasoning:false`、非对象和未知子字段同样 fail closed，不再静默降级为无 reasoning。
- 真实 `cargo run --locked --bin openbridge-probe -- --target deepseek-v4-flash --chat --function-calling` 返回 Chat 文本 HTTP 200、非 thinking function-calling HTTP 400，Responses function-calling 为 configured unsupported；这与已确认的 DeepSeek 上游模式/端点限制一致，不作为 Bridge 转换缺陷或 high reasoning 验收。修复后的 reasoning/tool Bridge 结论由 deterministic contract 与 mock HTTP 闭环证明；未在文档或用户可见输出中记录凭证，也没有运行外部 SDK、Codex/Hermes、负载或长期验证。

2026-08-03 追加 reasoning 输出 capability 配置：

- 核心 capability 增加 `ReasoningOutput` 类型：`Unknown` 表示没有可读 wire 证据，`Unsupported` 表示明确无 reasoning 输出，
  `PlainText`/`Summary` 表示可读 channel，`Opaque` 表示包括 Responses `encrypted_content` 在内的不可读 continuation。
- DeepSeek Chat contract 配置为 `PlainText`，对应本轮真实 V4 Flash high Chat 的 `reasoning_content` 证据；DeepSeek Responses
  contract 配置为 `Unsupported` 且没有注册原生 Responses API。MiMo 与 LongCat 的 Chat/Responses 均配置为 `Unknown`，因为现有
  真实测试只证明协议、文本/工具和 streaming 终态，未证明可读 reasoning wire 映射。
- Bridge 规划现在消费选定 Upstream API 的 `ReasoningOutput`：Chat 上游只有 `PlainText` 可进入 Responses Bridge，Responses
  上游允许 `PlainText` 或 `Summary` 进入 Chat Bridge；请求包含 reasoning 配置或 reasoning history 时，`Unknown`、`Unsupported`
  和 `Opaque` candidate 在 egress 前返回能力错误。Native route 不受该 Bridge 输出门禁影响。
- 新增配置与规划契约覆盖 DeepSeek/MiMo/LongCat 的运行时 capability 值、Provider contract 的 reasoning capability 越权拒绝，以及
  `Unknown`/`PlainText` Bridge candidate 的选择差异。该层仍是 deterministic Rust tests 与 mock/high-level evidence，不等同于
  MiMo/LongCat 的 readable reasoning real-provider acceptance。

2026-08-03 完成 Provider attempt 遥测扩展：

- 每个已收口的实际上游 attempt 现在按编译期 Provider、Route、Target、Upstream API、Public Model、协议、
  streaming 和 Native/Bridge 模式聚合独立快照，记录 response-ready、首个上游 body byte、上下游 TTFT、body
  生命周期、明确 usage、token observation、output speed 和 cache read/write 观测；request/user/credential/
  endpoint URL 与正文不进入指标 key。
- `GatewayMetrics::provider_snapshots` 提供进程内只读快照；当前未接入 `/metrics`、Prometheus/OpenTelemetry
  exporter、持久化、分布式聚合或按遥测结果动态重排 Route。
- 新增的 8 个 `observability_contract` 测试覆盖 JSON/streaming usage 与 cache、Provider/route mode 维度、
  retry HTTP failure、SSE terminal/EOF failure 和下游取消；`cargo test --locked`、`cargo fmt -- --check`、
  `cargo clippy --locked -- -D warnings` 与 `git diff --check` 均通过。该证据仍只覆盖 fake transport 的进程内
  采集边界，不证明真实 Provider 性能、cache 语义、外部 SDK、负载或长期运行结果。

2026-08-03 完成 Chat/Responses definition 命名拆分与标准字段预留：

- 原通用 `EndpointCapabilities` 已拆为可注册的 `ChatCompletionsCapabilities`、`ResponsesCapabilities`，以及只供请求
  分析和公共子集判断使用的 `GenerationCapabilities`。现有 routing 字段与行为保持不变。
- canonical `ModelConfig`/`ModelInfo` 增加可选 `ModelMode`、`InputModality` 和 `OutputModality`；输入枚举覆盖
  text/image/audio/file，输出枚举覆盖 text/image/audio。所有 checked-in Model 均保留 `None`，未知不被解释成空集合。
- Chat 预留 custom tool、audio/file input、audio output、predicted outputs、web search、prompt caching、moderation、
  logprobs 与 multiple choices；Responses 预留 custom/hosted tools、file input、conversation、prompt template、prompt
  caching、context management、标准 `include` 枚举、moderation 与 logprobs。所有 Provider contract 和 Upstream API
  definition 均保持新增字段为 `false` 或空集合。
- 本轮没有增加对应 request parser、Bridge、adapter 或 Provider 行为。进入 registry 编译的 Model 或 Upstream API
  definition 若启用任一预留字段，会触发带稳定说明的 `unimplemented!`，不会被发布为运行时可用能力。
- `capability_definition_contract` 4 个测试、`config_contract` 11 个、`native_routing_contract` 11 个和
  `provider_boundary_contract` 16 个测试通过；随后 `cargo fmt -- --check`、`cargo test --locked`、
  `cargo clippy --locked -- -D warnings` 与 `git diff --check` 均通过。全量 Rust 结果为 155 个测试通过、1 个需要外部
  OpenAI Python/Node SDK 的集成测试 ignored；没有修改 protocol corpus，也没有运行外部 SDK、真实 Provider、负载或长期验证。

## 当前未实现

当前 checked-in OpenAI/LongCat/OpenRouter/DeepSeek/MiMo 注册项没有在缺少真实能力证据时预设 reasoning level 映射；功能只在具体
Upstream API 显式声明后生效。Bridged Route 支持明文 reasoning channel 的受限转换，但不支持 opaque
`encrypted_content` continuation 或把 summary/content 伪造成 user-visible text。

- OpenRouter 有状态 Responses、真实异构协议 Provider、可配置 ConversionPolicy 和 Bridge continuation ledger；
- Responses WebSocket、Realtime、Files、Conversations 等资源 API；
- OAuth/subscription 多账号池、keyring、加密 secret 文件、远程 secret manager 和动态 credential 控制面；
- 动态 health/weight、持久化或分布式 cooldown 与后台探测；
- OpenTelemetry/Prometheus exporter、指标 HTTP API、持久化或分布式聚合；
- hosted tool、MCP Tool Bridge 或非 loopback 部署。

## 相关资源

- [当前代码架构](current-architecture.md)
- [能力探测](capability-probing.md)
- [协议测试语料与工具](protocol-test-corpus.md)
- [配置、凭证与受信边界](../functional-requirements/configuration-and-credentials.md)
- [路由与 Provider 韧性](../functional-requirements/provider-resilience.md)
