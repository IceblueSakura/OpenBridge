# 上游模型发现与能力探测

## 状态与证据边界

当前实现提供管理员显式运行的 `openbridge-probe`。它只对代码注册表中的固定 Upstream Target 发起观察请求，
并按 target 已注册的 Upstream API 选择协议；probe report 是一次真实环境观察，不是动态配置或能力自动发现。

- 下游 `GET /v1/models` 只返回代码注册的 Public Model，与 probe 结果无关；
- probe 复用 target 的固定 endpoint、upstream model、Provider adapter、transport 与 credential pool；
- 每次 probe 为选中 target 单独加载 pool 快照，并确定性使用第一个 member，不参与生产 round-robin，也不修改
  生产 cursor 或 cooldown；
- CLI 不接受 URL、model、header 或 credential 覆盖，不加载下游用户 API Key；
- report 不修改 `RuntimeRegistry`、`ModelConfig`、capability 或 Route；
- 真实 Provider probe 会产生网络请求，可能消耗额度、触发限流或受账号状态影响，因此不属于默认验证基线。

## 代码注册的模型事实

`src/models/<family>/` 下的版本叶模块各自拥有一个完整 `ModelConfig`，家族模块只负责聚合。当前类型可记录：

- canonical model id 与展示元数据；
- 已核实的 input/output token 上限；
- 支持参数集合；
- reasoning 三态；
- 支持的 canonical reasoning level 子集。

未知事实保持为空或 `Unknown`。`context_length.output` 在请求显式携带输出上限时参与候选筛选；
`context_length.input` 当前只作为元数据，因为运行时没有 model-specific tokenizer。Upstream API capability、
served limit、state affinity 与 reasoning wire 映射由 Provider 注册项中的 typed Rust 值声明，并且只能收窄
Provider contract 和 canonical model 上界。

## CLI 与固定观察项

服务与 probe 共享 `OPENBRIDGE_CONFIG`、可选 `.env` 加载规则和代码注册的 credential 环境变量：

```powershell
cargo run --bin openbridge-probe -- --target openai-main --list-models
cargo run --bin openbridge-probe -- --target openai-main --chat --responses --function-calling
```

可选项为 `--list-models`、`--chat`、`--responses`、`--function-calling` 和 `--all`；没有选择项时等同
`--all`。`--target` 必须引用一个已启用的代码注册项。若 target 没有对应协议的 Upstream API，相关观察不会被
解释为 Provider 支持。

| 探测项 | 固定上游请求 | `supported` 条件 |
|---|---|---|
| `list_models` | `GET /v1/models` | 返回 JSON `data[]`，并报告注册的 upstream model 是否存在。 |
| `chat` | 最小 Chat Completions 请求 | 返回非空 `choices[]`。 |
| `responses` | 最小 Responses 请求 | 返回 `object: "response"`。 |
| Chat function calling | 固定无副作用 function call/result replay | call identity、arguments 与 replay 形状有效。 |
| Responses function calling | 固定 function call/output replay | call ID、名称、arguments 与 replay 形状有效。 |

明确的 404、405、501 可记为 `unsupported`。认证失败、限流、网络错误、响应超限、无效 JSON 或 400/422
只能记为 `unknown`；一次成功也只证明本次 target、账号、时间点和固定 payload 的观察结果。

## 当前 Provider 的可执行入口

填充相应 JSON-array credential pool 后，可以显式运行：

```powershell
cargo run --bin openbridge-probe -- --target deepseek-v4-pro --list-models --chat --function-calling
cargo run --bin openbridge-probe -- --target deepseek-v4-flash --list-models --chat --function-calling
cargo run --bin openbridge-probe -- --target mimo-v2-5-pro --all
cargo run --bin openbridge-probe -- --target mimo-v2-5 --all
```

DeepSeek target 只注册 Chat Upstream API，因此 probe 不能直接验证原生 Responses；下游 Responses→Chat Bridge
由确定性 Rust 测试覆盖。MiMo target 注册 Chat 与 Responses Upstream API，`--all` 会观察模型列表、两个协议的
最小文本请求与 function call/result replay。上述命令在本次文档更新中没有执行，不能据此宣称真实 Provider
已经验收。

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
