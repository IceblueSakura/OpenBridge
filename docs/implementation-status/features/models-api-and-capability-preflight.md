# 功能：Models 接口、Public Model 契约与能力预检

## 状态

**已完成（当前 checkout）。** 标准 Models、扩展 Models 和请求预检共享同一启动期编译的 Public Model execution interface；客户端看到的是
固定公共契约，而不是某条 Route 的能力上限。

## 已完成内容

- `GET /v1/models` 与 `GET /v1/models/{model}` 提供 OpenAI 标准四字段模型对象；扩展 Models 提供 operation、输入/输出 modality、reasoning、
  state、typed `multimodal_input` 和 `supported_parameters` 等下游安全事实。
- 每个 Public Model 的 Chat、Responses、Embeddings interface 在启动期按所有可执行 candidate 的保守交集编译；未知事实保持未知，不被
  猜测为支持。
- 请求先解析 operation-specific requirements，再对选定 Public Model 做一次能力、限制和 state-affinity preflight；不支持的请求在任何
  Provider egress 前以稳定本地错误拒绝。
- 预检通过后仍按注册表的 Route 资格和顺序规划，不会因单条 Route 的额外能力跳过前序 Route、扩大公共契约或自动更换模型。
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

- [`tests/native_routing_contract.rs`](../../../tests/native_routing_contract.rs) 覆盖请求事实、能力预检、Route 顺序和 continuation issuer 安全。
- [`tests/capability_definition_contract.rs`](../../../tests/capability_definition_contract.rs) 覆盖 capability subset 与未知值边界。
- [`tests/embedding_definition_contract.rs`](../../../tests/embedding_definition_contract.rs) 和 [`tests/embedding_registry_contract.rs`](../../../tests/embedding_registry_contract.rs)
  覆盖 Embeddings interface 的独立编译和公开契约。

确定性测试只证明本地 registry、analysis 和 planning 行为，不证明每个公共模型对真实上游均可用。

## 相关文档

- [功能需求：Public Model 与模型能力契约](../../functional-requirements/model-information-and-capability-contract.md)
- [Provider 注册表与模型目录](provider-registry-and-model-catalog.md)
- [`mimo-v2.5` Native 图片输入](native-image-input.md)
- [HTTP 网关接口与下游认证](gateway-http-api-and-auth.md)
