# 上游模型发现与能力探测

## 状态与证据边界

当前实现提供管理员显式运行的 `openbridge-probe`。它只对代码注册表中的固定 Upstream Target 发起观察请求， 并按 target 已注册的
Upstream API 选择协议；probe report 是一次真实环境观察，不是动态配置或能力自动发现。

- 下游 `GET /v1/models` 只返回代码注册的 Public Model，与 probe 结果无关；
- probe 复用 target 的固定 endpoint、upstream model、Provider adapter、transport 与 credential pool；普通 target 从 API-key
  TOML 加载首个 member，ChatGPT target 只从管理员显式指定的 Codex auth file 构造一个临时 OAuth member；
- 每次 probe 的 credential snapshot 都只属于本次进程，不参与生产 round-robin，也不修改生产 cursor、cooldown 或源 credential；
- CLI 不接受 URL、model、header 或 credential 覆盖，不加载下游用户 API Key；
- OpenBridge 不调用 Codex executable 或 app-server；Codex 源码只用于固定 compatibility profile、`User-Agent` 与请求行为；
- report 不修改 `RuntimeRegistry`、`ModelConfig`、capability 或 Route；
- 真实 Provider probe 会产生网络请求，可能消耗额度、触发限流或受账号状态影响，因此不属于默认验证基线。

## 代码注册的模型事实

`src/models/<developer>/` 下的模型叶模块各自拥有一个完整 `ModelConfig`，研发者模块只负责聚合。当前类型可记录：

- canonical model id 与展示元数据；
- 已核实的总上下文/输入上限、最大输出上限、输入/输出模态、tokenizer 和 knowledge cutoff；
- 支持参数集合；
- reasoning 三态；
- 支持的 canonical reasoning level 子集。

未知事实保持为空或 `Unknown`。OpenRouter 没有独立的输入上限字段，因此其模型级 `context_length` 同时
投影为总上下文和输入上限；这不是把最大输出从总上下文中扣除后的残差。`context_length.output` 在请求显式携带 输出上限时参与候选筛选；
`context_length.input` 当前只作为元数据，因为运行时没有 model-specific tokenizer。Upstream API capability、 served
limit、state affinity 与 reasoning wire 映射由 Provider 注册项中的 typed Rust 值声明，并且只能收窄 Provider contract 和
canonical model 上界。

## CLI 与固定观察项

普通 Provider probe 与服务共享 `OPENBRIDGE_CONFIG` 选择的 bootstrap，并从 bootstrap 指定的私有
`upstream-credentials.toml` 按代码注册的 pool id 加载 API key：

```powershell
cargo run --bin openbridge-probe -- --target openai-main --list-models
cargo run --bin openbridge-probe -- --target openai-main --chat --responses --function-calling
```

普通 target 的可选项为 `--list-models`、`--chat`、`--responses`、`--function-calling` 和 `--all`；没有选择项时等同
`--all`，并且 target 必须是已启用的代码注册项。若 target 没有对应协议的 Upstream API，相关观察不会被解释为 Provider 支持。

| 探测项                     | 固定上游请求                             | `supported` 条件                                           |
|----------------------------|------------------------------------------|------------------------------------------------------------|
| `list_models`              | `GET /v1/models`                         | 返回 JSON `data[]`，并报告注册的 upstream model 是否存在。 |
| `chat`                     | 最小 Chat Completions 请求               | 返回非空 `choices[]`。                                     |
| `responses`                | 最小 Responses 请求                      | 返回 `object: "response"`。                                |
| Chat function calling      | 固定无副作用 function call/result replay | call identity、arguments 与 replay 形状有效。              |
| Responses function calling | 固定 function call/output replay         | call ID、名称、arguments 与 replay 形状有效。              |

明确的 404、405、501 可记为 `unsupported`。认证失败、限流、网络错误、响应超限、无效 JSON 或 400/422 只能记为 `unknown`
；一次成功也只证明本次 target、账号、时间点和固定 payload 的观察结果。

ChatGPT subscription 使用唯一的默认禁用 target 和独立 selector：

```powershell
cargo run --locked --bin openbridge-probe -- `
  --target chatgpt-gpt-5-6-sol `
  --codex-auth-file "C:\path\to\.codex\auth.json" `
  --list-models --responses
```

它必须显式选择 `--list-models`、`--responses` 中至少一项，不接受 `--all`、Chat/function probe、Codex executable、任意
URL/model/header 或 User-Agent 覆盖。auth loader 只接受 ChatGPT OAuth 模式、未过期 access-token JWT 与一致的账户绑定；只读提取
access token、account ID、FedRAMP claim 和 expiry，不保留 refresh token，不写回文件。compatibility profile 固定为
`0.145.0`：模型目录请求为 `GET models?client_version=0.145.0` 并解析 `models[].slug`；Responses 使用固定文本 payload、
`stream=true` 和有界 SSE framing，只有恰好一个合法 `response.completed` 才为 `supported`。report 只显示 profile version、平台、
source-profile match 布尔值、HTTP status、model ID 与 probe state，不显示完整 User-Agent、账户、认证 header 或请求/响应正文。

2026-08-05 15:38+08:00 使用本机只读 Codex auth file 完成一次真实验收：Codex 源码基线为
`1fe6be9719ac4a18ad08f8341b89f9a0f386105e`，profile `0.145.0`，Windows identity match 为 `true`；模型目录 HTTP 200 且包含
`gpt-5.6-sol`，Responses HTTP 200 且观察到唯一成功 terminal。验收前后 auth file 内容 hash 相同；hash、credential、账户、完整
header 与正文均未写入 report 或本文。该结果只证明当时该账户、固定 target 与最小文本 payload 可用，不证明 refresh、其他账号、
更复杂 payload 或 endpoint 长期稳定性。

## 当前 Provider 的可执行入口

填充相应 TOML `api_keys` credential pool 后，可以显式运行：

```powershell
cargo run --bin openbridge-probe -- --target deepseek-v4-pro --list-models --chat --function-calling
cargo run --bin openbridge-probe -- --target deepseek-v4-flash --list-models --chat --function-calling
cargo run --bin openbridge-probe -- --target mimo-v2-5-pro --all
cargo run --bin openbridge-probe -- --target mimo-v2-5 --all
```

DeepSeek target 只注册 Chat Upstream API，因此 probe 不能直接验证原生 Responses；下游 Responses→Chat Bridge 由确定性 Rust
测试覆盖。MiMo target 注册 Chat 与 Responses Upstream API，`--all` 会观察模型列表、两个协议的 最小文本请求与 function
call/result replay。上述命令在本次文档更新中没有执行，不能据此宣称真实 Provider 已经验收。

## 不做的推断

- 不从 `/v1/models` 推断 tools、reasoning、视觉、context、streaming 或 Bridge 能力；
- 不从一次失败推断永久不支持，也不从一次成功推断其他账号、模型或 payload 可用；
- 不自动修改 Model、Upstream API capability、Route 或 Public Model；
- 不在普通服务启动时联网探测；
- 不自动遍历 credential pool，也不把首个 member 的结果描述成整个 pool 可用；
- 不把 probe report 当成 SDK、Agent、负载、长期运行或生产验收。

## 关联文档

- [当前实现说明](current-implementation.md)
- [当前代码架构](current-architecture.md)
- [交付与证据要求](../functional-requirements/delivery-and-evidence.md)
