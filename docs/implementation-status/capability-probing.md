# 上游模型发现与能力探测

## 状态与证据边界

当前实现提供管理员显式运行的 `openbridge-probe`。它只对代码注册表中的固定 Upstream Target 发起观察请求， 并按 target 已注册的
Upstream API 选择协议；probe report 是一次真实环境观察，不是动态配置或能力自动发现。

- 下游 `GET /v1/models` 只返回代码注册的 Public Model，与 probe 结果无关；
- probe 复用 target 引用的固定 Provider instance endpoint、upstream model、Provider adapter、transport 与 credential pool，并只从 API-key
  TOML 加载所选 pool 的首个 member；
- 每次 probe 的 credential snapshot 都只属于本次进程，不参与生产 round-robin，也不修改生产 cursor、cooldown 或源 credential；
- CLI 不接受 URL、model、header 或 credential 覆盖，不加载下游用户 API Key；
- probe 只接受已启用 target，但当前只装载 API-key pool；ChatGPT OAuth target 必须通过常驻服务的 manager 数据面验收，通用 probe 不借用
  OAuth credential；
- CLI 不提供本机 Agent auth file、client identity 或 executable selector，也不读取 OS/terminal 状态；
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

Provider probe 与服务共享 `OPENBRIDGE_CONFIG` 选择的 bootstrap，并从 bootstrap 指定的私有
`upstream-credentials.toml` 按代码注册的 pool id 加载 API key：

```powershell
cargo run --bin openbridge-probe -- --target openai-main --list-models
cargo run --bin openbridge-probe -- --target openai-main --chat --responses --function-calling
```

可选项为 `--list-models`、`--chat`、`--responses`、`--function-calling` 和 `--all`；没有选择项时等同
`--all`，并且 target 必须是已启用的代码注册项。若 target 没有对应协议的 Upstream API，相关观察不会被解释为 Provider 支持。

| 探测项                     | 固定上游请求                             | `supported` 条件                                           |
|----------------------------|------------------------------------------|------------------------------------------------------------|
| `list_models`              | Provider 注册的固定模型列表 GET 路径     | 返回 Provider-specific 模型信封，并报告注册的 upstream model 是否存在。 |
| `chat`                     | 最小 Chat Completions 请求               | 返回非空 `choices[]`。                                     |
| `responses`                | 最小 Responses 请求                      | 返回 `object: "response"`。                                |
| Chat function calling      | 固定无副作用 function call/result replay | call identity、arguments 与 replay 形状有效。              |
| Responses function calling | 固定 function call/output replay         | call ID、名称、arguments 与 replay 形状有效。              |

当前模型列表路径和响应信封由 Provider adapter 固定：OpenAI、OpenRouter、DeepSeek、MiMo 及 LongCat 使用
OpenAI-compatible `data[].id`，其中 LongCat 的路径为 `/openai/v1/models`；ChatGPT 使用
`/models?client_version=0.146.0`，并从 Codex manifest 的 `models[].slug` 提取模型 ID。模型列表 probe 仍然只通过
固定 Provider origin 发起，不能由 CLI 覆盖 URL、query 或响应解析规则。

本次模型列表修订的确定性验证已执行：`cargo test --locked --lib
provider_model_list_profiles_bind_paths_and_response_envelopes`、针对变更 Rust 文件的
`rustfmt --check --edition 2024` 与 `git diff --check` 均通过；未执行真实 Provider 网络请求。

明确的 404、405、501 可记为 `unsupported`。认证失败、限流、网络错误、响应超限、无效 JSON 或 400/422 只能记为 `unknown`
；一次成功也只证明本次 target、账号、时间点和固定 payload 的观察结果。

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

四个 ChatGPT OAuth target 虽已启用，但不通过本 CLI 的 API-key snapshot 路径执行。2026-08-06 的真实 ChatGPT 最小 Responses 验收通过
受保护的下游 `/v1/responses` 和 `OAuth2CredentialManager` 完成，证据见
[ChatGPT OAuth2 生命周期与 Responses 数据面](features/chatgpt-oauth-startup.md)。

## 不做的推断

- 不从 `/v1/models` 推断 tools、reasoning、视觉、context、streaming 或 Bridge 能力；
- 不从一次失败推断永久不支持，也不从一次成功推断其他账号、模型或 payload 可用；
- 不自动修改 Model、Upstream API capability、Route 或 Public Model；
- 不在普通服务启动时联网探测；
- 不自动遍历 credential pool，也不把首个 member 的结果描述成整个 pool 可用；
- 不把 probe report 当成 SDK、Agent、负载、长期运行或生产验收。

## 关联文档

- [当前实现总览](current-implementation.md)
- [当前代码架构](current-architecture.md)
- [交付与证据要求](../functional-requirements/delivery-and-evidence.md)
