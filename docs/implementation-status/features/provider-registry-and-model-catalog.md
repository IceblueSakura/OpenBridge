# 功能：Provider、Model、Target、API、Route 与 Public Model 注册表

## 当前行为

- Rust catalog 显式注册闭合 Provider family、canonical Model、Provider instance、credential pool、Upstream Target/API、Route
  与 Public Model，并在启动期编译为 immutable `RuntimeRegistry`；没有动态 DSL、名称推断或自动发现写回。
- 每个 canonical Model 必须选择 `Generation | Embedding | SpeechRecognition | SpeechSynthesis | VoiceDesign | VoiceClone`
  task variant；operation、task 和 executable audio profile 不兼容时启动失败。
- Provider contract 是 capability ceiling；每个 Target/API 必须在 ceiling 内显式收窄。ceiling 不自动打开图片、音频、工具、
  structured output、state 或参数。
- active credential pool 只能禁用静态 Target，并据此收窄可执行 Public Model；不能新增 Provider、endpoint、Route 或能力。
- Public Model compiler 从全部固定 candidate 保守聚合唯一 interface。请求能力不筛选、跳过或重排 candidate。
- Generation registration 显式选择 `NativeFirst` 或 `SourceFirst`；缺少某个 downstream protocol 的全局 Native coverage 时才为
  允许的单协议 source 自动补充 Bridge。Embeddings 与专用音频 task 不生成 generation Bridge。
- Canonical identity、Provider routing identity、upstream model 与下游 Public Model identity 分层保存；下游不接触执行拓扑。
- `gpt-5.6-sol` 聚合 ChatGPT/OpenAI，DeepSeek V4 Pro 聚合 DeepSeek/Bailian，DeepSeek V4 Flash 聚合
  DeepSeek/Bailian/OpenRouter，MiniMax M3 聚合 OpenRouter/NVIDIA；各自顺序由注册策略固定。其他当前 Public Model 的 source
  见 [Provider 状态目录](../providers/README.md)。
- 只有 canonical Model/Target、但没有 Public Model/Route 的条目不可调用；Models 可见性也仍受 active pool 影响。

## 所有权

- Catalog composition：[`src/providers/catalog.rs`](../../../src/providers/catalog.rs)与 `src/providers/catalog/`。
- Canonical facts：[`src/models/`](../../../src/models/)；Provider definitions/registrations：[`src/providers/`](../../../src/providers/)。
- Validation、compiler 与 runtime entity：[`src/registry/`](../../../src/registry/)。
- Provider adapter 拥有 wire/auth/error；pipeline 不按 Provider 名称分支，也不创建 Route。

## 确定性证据

- `tests/config_contract.rs`：引用、task/operation/capability 与启动 fail-closed。
- `tests/upstream_credential_config.rs`：active pool、Target/Public Model 过滤和 credential ownership。
- `tests/example_config.rs`：两个 checked-in Bootstrap profile 的 registry compile smoke test，不复制完整目录。
- `tests/provider_contract.rs`、`tests/provider_boundary_contract.rs`：Provider operation、认证、模型和安全出站。
- `tests/forwarding_contract.rs`、`tests/embedding_forwarding_contract.rs`：从客户端入口保护实际 Public Model/Route 行为。

## 外部证据

文字模型正常首选路径见 [2026-08-09 矩阵](../evidence/real-provider/2026-08-09-text-generation-none-high-matrix.md)与
[2026-08-10 Qwen3.6 矩阵](../evidence/real-provider/2026-08-10-qwen36-none-high-matrix.md)。这些记录不证明强制后备 source；
逐 Provider 定向证据和未证明范围由 [Provider 状态目录](../providers/README.md)解释。

## 未证明范围

Registry compile 与 mock forwarding 不证明动态 Provider、真实目录/账号可用性、强制多 source fallback、外部 SDK/Agent、负载或
长期运行。当前 Provider 多为 OpenAI-compatible wire，不构成真实异构协议验收。

## 相关文档

- [Models 与能力预检](models-api-and-capability-preflight.md)
- [启动配置与凭证](startup-configuration-and-credentials.md)
- [当前代码架构](../current-architecture.md)
- [OpenRouter 状态](../providers/openrouter.md)
