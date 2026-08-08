# 功能：Models 接口、Public Model 契约与能力预检

## 状态

**已完成（当前 checkout）。** 标准 Models、扩展 Models 和请求预检共享同一启动期编译的 Public Model execution interface；客户端看到的是
固定公共契约，而不是某条 Route 的能力上限。

## 已完成内容

- `GET /v1/models` 与 `GET /v1/models/{model}` 提供 OpenAI 标准四字段模型对象；扩展 Models 提供 operation、输入/输出 modality、reasoning、
  state、typed `multimodal_input` 和 `supported_parameters` 等下游安全事实。
- 每个 Public Model 的 Chat、Responses、Embeddings interface 在启动期按所有可执行 candidate 的保守交集编译；未知事实保持未知，不被
  猜测为支持。
- generation interface 使用 typed function-tool 与 structured-output profile：分别公开 `tool_choice` mode 集合、parallel/strict
  约束，以及 `json_object`/`json_schema` mode 与 strict 约束；集合逐候选相交，不再由布尔支持值推导整组 mode。
- 请求 analyzer 冻结精确的 function `tool_choice` 与 structured-output mode/strict facts；缺失证据或未建模值 fail closed，preflight
  直接读取同一 fixed interface contract。function tool 未显式指定 `tool_choice` 时按协议默认的 `auto` 进行预检。
- Chat capability 不再同时维护 `audio_input`/`audio_output` 布尔值与 typed audio profile；输入/输出模态由具体 audio profile 推导。
  Provider family 可以保留 `AudioTask::Any` 作为 ceiling 标记，但每个可执行 Upstream API 必须绑定确定 task。
- MiMo 四个音频专用 target 将 Provider-wide function-tool ceiling 收窄为 `None`；扩展 Models 公开 tools `unsupported`，并在 egress
  前拒绝带 function tool 的合法音频 task。通用 `mimo-v2.5` 与 Pro 的工具契约不受影响。
- 请求先解析 operation-specific requirements，再对选定 Public Model 做一次能力、限制和 state-affinity preflight；不支持的请求在任何
  Provider egress 前以稳定本地错误拒绝。
- 预检通过后仍按注册表的 Route 资格和顺序规划，不会因单条 Route 的额外能力跳过前序 Route、扩大公共契约或自动更换模型。
- 细粒度能力只用于一次公共契约预检，不参与候选筛选；同一 Public Model/operation 的全部静态候选以原配置顺序进入 RoutePlan，fallback
  仍只处理既有的首输出前可重试可用性失败。
- Responses `previous_response_id` 只在所有可执行 Responses candidate 绑定同一且唯一的 issuing Target/API 时公开；潜在签发者不唯一时，
  在上游调用前拒绝，避免把 opaque continuation 盲投到错误 Provider。
- 当前实现把 `previous_response_id`、`background` 与 `store: true` 作为受限状态能力的安全 gate，而不是完整的有状态服务；
  当前 Public Model 注册不提供通用 response storage、retrieve/cancel、conversation lifecycle 或 continuation ledger。它们是次要目标，
  当前支持不完整，默认客户端和验证仍应使用每次携带完整历史的无状态请求。

## 实现边界

- Public Model projection 位于 [`src/registry/public_model.rs`](../../../src/registry/public_model.rs)，编译逻辑位于
  [`src/registry/public_model/compiler.rs`](../../../src/registry/public_model/compiler.rs)。
- generation 与 Embeddings analyzer 分开；analyzer 只提取请求事实，不解析 registry entity，也不选择 Route。
- 当前不包含动态目录、通用 capability negotiation、continuation ledger 或请求级 Route 选择 API。

## 验证证据

- [`tests/native_routing_contract.rs`](../../../tests/native_routing_contract.rs) 覆盖 typed mode 交集、能力预检、Route 顺序和 continuation issuer 安全。
- [`tests/capability_definition_contract.rs`](../../../tests/capability_definition_contract.rs) 覆盖 capability subset 与未知值边界。
- `cargo test --locked --lib core::capability::generation::tests` 验证 typed generation subset 与 audio profile presence 推导；
  `cargo test --locked --test native_routing_contract` 验证交集外 mode 在 egress 前拒绝且通过预检后候选顺序不变。
- [`tests/embedding_definition_contract.rs`](../../../tests/embedding_definition_contract.rs) 和 [`tests/embedding_registry_contract.rs`](../../../tests/embedding_registry_contract.rs)
  覆盖 Embeddings interface 的独立编译和公开契约。

最终本地验证运行 `cargo fmt -- --check`、`cargo test --locked`、`cargo clippy --locked -- -D warnings` 和 `git diff --check`；这些检查只证明
本地 registry、analysis、planning 与静态 Provider 定义，不证明每个公共模型对真实上游均可用。

## 相关文档

- [功能需求：Public Model 与模型能力契约](../../functional-requirements/model-information-and-capability-contract.md)
- [Provider 注册表与模型目录](provider-registry-and-model-catalog.md)
- [MiMo Provider 多模态与工具调用状态](../providers/mimo.md)
- [`mimo-v2.5` Native 图片输入](native-image-input.md)
- [HTTP 网关接口与下游认证](gateway-http-api-and-auth.md)
