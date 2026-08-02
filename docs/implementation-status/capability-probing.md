# 上游模型发现与能力探测

## 状态与边界

当前实现提供管理员显式 probe。它使用代码注册表中的固定 Upstream Target，并按协议选择 Upstream API；它是证据报告，不是动态配置。

- 下游 `GET /v1/models` 只返回代码注册的 Public Model；
- probe 使用固定 endpoint、adapter、upstream model 和 credential pool 的首个确定性 member；
- CLI 不接受 URL、model、header 或 credential；
- report 不修改 `RuntimeRegistry` 或 Rust 注册项；
-真实 Provider probe 可能消耗配额或触发限流。

## 代码注册的模型事实

`src/models/<family>.rs` 聚合根模块下，每个扁平叶模块按版本、checkpoint 或命名变体组织一个完整
`ModelConfig` 声明，记录：

- 逻辑模型 id 和展示元数据；
-已核实的 input/output token 上限；
-支持参数集合；
- reasoning 三态；
-支持的标准 reasoning level。

未知事实应留空或标记 `Unknown`。`context_length.output` 会在请求显式提供输出上限时参与候选筛选；
`context_length.input` 当前只作元数据，因为尚无 model-specific tokenizer。

Upstream API capability 和约束由 Provider 文件中的 typed Rust 值声明，只能收窄
`ProviderContract` 上界。

## 显式 probe CLI

服务和 probe 只共享 `OPENBRIDGE_CONFIG` 与已注册 credential 环境变量：

```powershell
cargo run --bin openbridge-probe -- --target openai-main --list-models

cargo run --bin openbridge-probe -- --target openai-main --chat --responses --function-calling
```

填充 `.env` 中的 `DEEPSEEK_API_KEYS` 与 `MIMO_API_KEYS` JSON 数组后，可分别运行以下真实 Provider 验收用例：

```bash
cargo run --bin openbridge-probe -- --target deepseek-v4-pro --list-models --chat --function-calling
cargo run --bin openbridge-probe -- --target deepseek-v4-flash --list-models --chat --function-calling
cargo run --bin openbridge-probe -- --target mimo-v2-5-pro --all
cargo run --bin openbridge-probe -- --target mimo-v2-5 --all
```

DeepSeek 没有注册 Responses Upstream API，因此 probe 只直接验证 Chat；下游 Responses→Chat Bridge 由确定性
Rust 测试验证。MiMo 的 `--all` 会验证模型列表、两种协议的最小文本请求及 function call/result replay。
这些命令会访问真实 Provider、消耗额度并可能触发限流，不属于默认测试基线。

可选项为 `--list-models`、`--chat`、`--responses`、`--function-calling` 和 `--all`。没有选择项时
等同 `--all`。`--target` 必须引用代码注册项。

| 探测项 | 固定上游请求 | 成功条件 |
|---|---|---|
| `list_models` | `GET /v1/models` | 返回 JSON `data[]`；报告模型 ID 和注册的 upstream model 是否列出。 |
| `chat` | 最小 Chat Completions 请求 | 返回非空 `choices[]`。 |
| `responses` | 最小 Responses 请求 | 返回 `object: "response"`。 |
| Chat function calling | 固定无副作用 function call/result replay | call identity、arguments 和 replay 形状有效。 |
| Responses function calling | 固定 function call/output replay | call ID、名称、arguments 和 replay 形状有效。 |

`supported` 表示本次观察成功；明确 404、405、501 可记为 `unsupported`。认证失败、限流、网络错误、
响应超限、JSON 无效或 400/422 均为 `unknown`。

## 不做的推断

- 不从 `/v1/models` 推断 tools、reasoning、视觉、context 或 streaming；
- 不从一次失败推断永久不支持；
-不自动修改 Model/Upstream API capability；
-不在普通服务启动时联网探测；
-不把 probe report 当成真实设备式长期验收或跨账号结论。
