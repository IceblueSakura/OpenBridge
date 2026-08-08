# 上游模型发现与基础 API 探测

## 状态与定位

**Confirmed。** 当前 `openbridge-probe` 是管理员在服务上线前显式运行的基础观察工具。它针对一个已注册且已启用的
Upstream Target，观察固定 Models 端点以及该 Target 已注册的 Chat Completions、Responses 和 Embeddings Create API。

该 binary 不再测试 function calling 或 tool-result replay，也不承担模型语义、SDK/Agent、retry/fallback、负载或长期运行测试。
后续深入测试 binary 与 Python semantic verifier 的接入方式不属于本阶段实现。

## 受信输入与副作用边界

- CLI 只接受代码注册的 `--target <id>` 和闭集 probe selector，不接受 URL、model、header、credential 或任意请求体覆盖；
- probe 复用 Target 固定的 Provider endpoint、upstream model、adapter、transport 和 credential pool；
- API-key Target 从私有 upstream credential TOML 绑定的 pool 取得首个 member；ChatGPT Target 只从选定
  `auth_json_file` 的 `OAuth2CredentialManager` 借用账户绑定的短期 lease；
- 未选中的 OAuth2 文件不会被打开，CLI 不读取本机 Codex/Agent cache、terminal identity 或 executable；
- Chat/Responses 请求只包含内置文本输入，不包含 `tools`、`tool_choice`、tool call 或 tool result；Embeddings 使用一个固定文本输入；
- report 不包含 credential、请求正文或上游响应正文，不修改 `RuntimeRegistry`、Model、capability、Route、cursor 或 cooldown；
- 真实 probe 会产生 Provider 请求，可能消耗额度、触发限流或按既有 OAuth manager 规则刷新选定的 OpenBridge-owned auth 文件，
  因此不属于默认测试基线。

## CLI 与观察项

probe 与服务共享 `OPENBRIDGE_CONFIG` 选择的 bootstrap 和其中指向的 private upstream credential TOML：

```powershell
cargo run --locked --bin openbridge-probe -- --target openai-main --list-models
cargo run --locked --bin openbridge-probe -- --target openai-main --chat --responses
cargo run --locked --bin openbridge-probe -- --target openai-text-embedding-3-small --embeddings
cargo run --locked --bin openbridge-probe -- --target chatgpt-gpt-5-6-sol --responses
```

可选项为 `--list-models`、`--chat`、`--responses`、`--embeddings` 和 `--all`。没有 selector 时等同 `--all`；`--all`
只运行当前 Target 的四类观察，不遍历其他 Target。Target 未注册某个 operation 时，该项直接报告 `unsupported`，不发起对应请求。

| 报告字段 | 固定请求 | `supported` 的最低判定 |
|---|---|---|
| `list_models` | Provider 注册的 Models GET 路径 | adapter 识别 Provider-specific 模型信封，并提取模型 ID；另报告已注册 upstream model 是否在列表中 |
| `chat` | 无工具的最小 Chat Completions 文本请求 | 非流式 JSON 含非空 `choices[]` |
| `responses` | 无工具的最小 Responses 文本请求 | 普通 Target 的非流式 JSON 为 `object: "response"`；ChatGPT 的固定 streaming profile 返回 `text/event-stream`，且 adapter 识别正常完成终态 |
| `embeddings` | 一个固定字符串的 Embeddings Create 请求 | JSON 是匹配模型、单个可识别 embedding item 和 usage 的 `list` 信封 |

普通 generation probe 将输出 token 上限固定为最多 16；ChatGPT Responses profile 按其注册 contract 使用 `stream: true`、
`store: false`，且不发送该 backend 不接受的 output-token-limit 参数。JSON 体受 `max_json_response_body_bytes` 限制；SSE 同时受总读取
上限和 `max_sse_event_bytes` 单事件上限约束。Models path 与响应信封仍由 Provider adapter 固定：OpenAI-compatible Provider 使用
`data[].id`；LongCat 的路径是 `/openai/v1/models`；ChatGPT 使用固定 manifest path 并从 `models[].slug` 提取 ID。

## 结果语义

- `supported`：本次固定请求收到满足上述最低形状的响应；
- `unsupported`：该 Target 未注册此 operation，或上游明确返回 404、405、501；
- `unknown`：认证失败、限流、其他 HTTP 错误、transport/stream 错误、响应超限、无效 JSON/SSE 或最低形状不匹配。

一次 `supported` 只证明当前 Target、账号、网络、Provider 状态和固定 payload 在该时间点完成了基础交互。一次 `unknown` 也不能推断
模型或端点永久不可用。probe report 与下游 `/v1/models`、Public Model capability 和 Route 编译完全解耦。

## 确定性验证

本次拆分采用先失败测试、后最小实现：最初运行 `cargo test --locked --lib probe::tests` 因新的 `embeddings` selector/report 尚未实现而
编译失败；实现后同一命令通过 11 项，`cargo test --locked --bin openbridge-probe` 通过 2 项。合成 transport 覆盖 Models、无工具
Chat/Responses、Embeddings、ChatGPT completed SSE、响应超限以及 transport/HTTP/JSON 的保守分类；OAuth 场景只使用 synthetic bundle。

随后 `cargo fmt -- --check`、`cargo test --locked`（291 项）、`cargo clippy --locked -- -D warnings` 和 `git diff --check`
均通过。本阶段未修改 `testdata/` 或 `tools/corpus/`，因此未运行 Python corpus/testkit；也未运行真实 Provider、外部 SDK、Agent、
负载或长期运行验收。

## 历史真实 Models 观察

2026-08-07 曾使用当时的私有配置对 12 个已注册 Target 执行 `--list-models`。以下只是历史 Models 端点观察，本次拆分未重跑，
也不能作为当前 Chat/Responses/Embeddings 或工具能力证据：

| Target 范围 | 当时 HTTP / 状态 | 配置模型是否出现在列表 |
|---|---|---|
| `openai-main`、`openai-text-embedding-3-small` | 401 / `unknown` | 未得到列表，认证未通过 |
| `longcat-2` | 200 / `supported` | 是 |
| `openrouter-deepseek-v4-flash` | 200 / `supported` | 是 |
| `deepseek-v4-pro`、`deepseek-v4-flash` | 200 / `supported` | 是 |
| `mimo-v2-5-pro`、`mimo-v2-5` | 200 / `supported` | 是 |
| 四个 `chatgpt-*` Target | 200 / `supported` | 是 |

## 不做的推断

- 不从 Models 或基础文本/向量成功推断 function/custom/hosted tool、并行调用、structured output、reasoning、媒体或状态能力；
- 不评判回答语义正确性、模型质量、上下文长度、tokenizer、配额或吞吐；
- 不把基础 probe 当成 Protocol Bridge、retry/fallback/cooldown、SDK、curl、Agent、Python semantic verifier 或生产验收；
- 不自动遍历 credential pool，不把首个 member 的结果描述成整个 pool 可用；
- 不在服务启动时自动联网，也不根据结果修改代码注册事实。

## 关联文档

- [产品范围](../functional-requirements/product-scope.md)
- [当前代码架构](current-architecture.md)
- [交付与证据要求](../functional-requirements/delivery-and-evidence.md)
