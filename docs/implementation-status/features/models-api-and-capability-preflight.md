# 功能：Models 接口、Public Model 契约与能力预检

## 当前行为

- `GET /v1/models` 与 retrieve 输出标准四字段对象；扩展 Models 输出 task、operation、modality、reasoning、state、
  streaming/non-streaming、typed multimodal contract 与 `supported_parameters` 等下游安全事实。
- 扩展 list 的 `native_protocol=chat_completions|responses` 只筛选对应协议存在 Native candidate 的 Public Model；省略保持
  完整目录，非法/空/重复/未知 query 返回 typed 400。筛选不改变 DTO、Route 或 fallback。
- 每个 Public Model operation interface 在启动期对全部可执行 candidate 保守求交集；未知事实保持 unknown，不猜测为 supported。
- Chat/Responses analyzer 先按协议级闭合顶层字段目录分类。目录外字段返回 `unknown_parameter`；已知但不在固定 interface 的字段
  返回 `unsupported_model_capability`；两者都在 Provider egress 前失败。
- Generation capability 拒绝以闭合内部 reason 与标准顶层 `param` 定位失败字段，并按固定 family 顺序返回唯一首错；JSON key、集合和 candidate 顺序不改变归因。Chat 双 output-limit 字段保留实际最大值来源，内部 reason 不进入下游 wire。
- Generation reasoning 由 canonical `ReasoningProfile` 单源保存。Public Model 另保存 `Strict | ClampPositiveFloor` 输入策略；
  `none` 独立于正向档位，只有 interface 明确包含时才接受。planning 在 candidate 展开前只规范化一次有效 effort。
- Function tool、tool choice 与 structured output 使用闭合 typed profile；固定 candidate 没有共同 mode 时不公开相应参数。
- `stream_options` 只建模 Chat streaming 的 `include_usage`：省略、`{}` 与 `false` 是 no-op 并从 egress 移除；`true` 才要求
  完整固定 candidate 都能履行 usage 输出合同。
- Responses `include` 解析为闭合逐值集合；公开 accepted set 与 candidate 私有 forwarded set 在启动期分别编译。
  `reasoning.encrypted_content` 是唯一 `ForwardOrOmit` 值：原生支持的 Responses API 精确转发，其他 Native/Bridge candidate
  在 planning 中只删除该 hint；空数组随顶层字段一起删除。未知值和其他不在公共 accepted set 的值继续 fail closed。
- `prompt_cache_key` 只表示 exact forwarding，不承诺 cache hit、成本或延迟；options/retention/breakpoint 未实现。
- 图片、音频与文件 capability 使用带完整 payload/limit 的 typed profile；Models flat JSON 只是只读投影，preflight 直接读取 owned typed contract。
- Responses 当前只接受省略或 `store:false`；`store:true` 拒绝。continuation/state 只有全部 candidate 对 issuing Target/API/credential
  affinity 有共同保证时才公开；opaque state 不能盲投到另一 Provider。
- Preflight 对选定 Public Model 只执行一次能力/限制/state 校验；通过后按静态 Route 资格和顺序规划，不做 capability routing。

## 所有权

下游 DTO/accessor 位于 `src/registry/public_model.rs`，私有 execution/compile/aggregate 位于同名子模块；Generation 与 Embeddings
的 analysis/preflight/planning 分别位于 `src/pipeline/generation/` 与 `src/pipeline/embeddings/`。Analyzer 不解析 registry entity 或选择 Route。

## 确定性证据

- `tests/forwarding_contract/models.rs`：标准/扩展 Models、native filter、task/capability 投影与拓扑不泄漏。
- `tests/forwarding_contract/admission.rs`、`tests/ingress_contract.rs`：unknown/unsupported、Generation 字段级 param、确定性首错、instructions/store/state 与 zero egress。
- `tests/forwarding_contract/native.rs`、`tests/forwarding_contract/resilience.rs`：include Native exact-forward/omission、fallback
  candidate body 隔离与固定顺序。
- `tests/forwarding_contract/mimo.rs`：图片、音频、tool、structured output 和非法组合。
- `tests/bridge_forwarding_contract.rs`、`tests/bridge_conversion_contract.rs`：Router-owned include omission、direct Bridge
  active-include fail-closed，以及其他 Bridge 可表达性、cache/usage 参数。
- `tests/credential_store_contract.rs`、`tests/forwarding_contract/resilience.rs`：state affinity、credential 与 fallback。
- `tests/embedding_forwarding_contract.rs`：Embeddings 输入、budget、成功体、retry/cancel。

## 外部证据与未证明范围

Provider 定向证据见 [Provider 状态目录](../providers/README.md)，文字正常路径见 [evidence](../evidence/README.md)。真实请求只支持
对应 Target/账号/日期的能力收窄，不证明完整 OpenAI API、缓存效果、Provider 内部并行、强制 fallback、外部 SDK/Agent、负载或
长期运行。当前没有动态 capability negotiation、request-selected Route 或通用 continuation ledger。

## 相关文档

- [Public Model 需求](../../functional-requirements/model-capability/README.md)
- [注册表与模型目录](provider-registry-and-model-catalog.md)
- [Native 图片](native-image-input.md)
- [Native 文件](native-file-input.md)
- [MiMo 音频](native-mimo-audio.md)
- [HTTP 网关与认证](gateway-http-api-and-auth.md)
