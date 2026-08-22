# Responses `reasoning.encrypted_content` 兼容提示设计

> **状态：候选执行前设计，不构成实施授权。** 本文记录 `include: ["reasoning.encrypted_content"]` 的目标契约、当前实现差距、候选结构、风险与验证边界。真正实施前必须重新读取 live source 和工作树，只将一个可观察切片提升到 [`implementation-plans/current-focus.md`](../implementation-plans/current-focus.md)。当前实现事实仍以 live source、确定性测试和 [`implementation-status/`](../implementation-status/README.md) 为准。

## 1. 目标

OpenBridge 是严格、固定契约、fail-closed 的多 Provider OpenAI-compatible 网关。严格不等于把每个下游字段都要求为所有 Provider 的原生 wire 共同子集；当且仅当一个**精确字段值**已经有证据证明可以在不改变该 Provider 既有行为的前提下被网关消费时，OpenBridge 可以把它建模为兼容提示：

- 下游统一接受该精确标准值；
- 当前 candidate 的 Provider 原生支持时原样转发；
- Provider 不支持且该值被明确证明为 omitted-equivalent 时删除；
- 不伪造 Provider 未返回的输出；
- 不把该例外推广到同字段的其他值或其他参数。

本文只为 Responses Create 的以下精确值定义该候选契约：

```json
{
  "include": ["reasoning.encrypted_content"]
}
```

## 2. 已决定边界

以下是当前产品决定，不是从 Provider capability 自动推导出的实现事实：

1. `reasoning.encrypted_content` 是唯一明确批准的 Responses `include` 兼容提示。
2. 所有 Responses Public Model 都应能安全接受该精确值。
3. Provider 声明原生支持时，OpenBridge 原样向上转发。
4. Provider 不支持时，OpenBridge 删除该数组元素；删除后数组为空则删除顶层 `include`。
5. OpenBridge 不因下游请求该 hint 而合成 `encrypted_content`、明文 reasoning、summary 或任何 output item。
6. 其他已知 `include` 值继续服从严格的原生能力交集；未知值和非法形状继续 fail closed。
7. 该决定不授权删除 `parallel_tool_calls`、`prompt_cache_key` 或其他 Hermes 常发字段。
8. 该决定不授权 opaque `encrypted_content` 跨 Provider、Target、API 或 credential owner 重放。

决定依据是当前真实 Provider 观察：除 OpenAI 路径外，被测模型即使不接收该字段，也会按各自协议默认返回其既有 reasoning content。因此对这些 Provider 删除该精确 hint 不改变它们本来会返回的 reasoning 行为。该观察只支持“请求 hint 可 omitted-equivalent”，不证明 plaintext reasoning 与 opaque encrypted continuation 等价。

## 3. 关键语义区分

### 3.1 下游接受不等于 Provider 原生支持

必须分别建模三个事实：

| 事实 | 含义 | 所有者 |
|---|---|---|
| 下游安全接受 | OpenBridge 是否能处理某个标准 include 值而不改变请求的功能语义 | Public Model interface / gateway policy |
| Provider wire 支持 | 某个固定 Upstream API 是否接受该值并应收到原样字段 | `ProviderResponsesCapabilities` / Upstream API |
| 实际响应观察 | Provider 是否返回 plaintext reasoning、summary 或 opaque encrypted content | 真实 Provider evidence |

当前代码把前两项压缩为同一个 `response_includes` 集合，导致只要一个 candidate 的 Provider ceiling 不声明原生支持，整个 Public Model 就在 preflight 拒绝请求。

### 3.2 plaintext reasoning 不等于 opaque continuation

`reasoning_content`、reasoning summary 和 `encrypted_content` 不是可互换表示：

- plaintext reasoning 或 summary 是可读输出；
- `encrypted_content` 是 Provider/issuer 颁发的 opaque continuation；
- 网关不得把明文包装成假的 encrypted state；
- 网关不得把一个 Provider 的 opaque state 发给另一个 Provider。

因此本文只定义**请求 hint 的条件转发/删除**，不定义 response synthesis 或跨 Provider replay。

### 3.3 Public Models 投影不承诺输出存在

扩展 Models 中的 `response_includes` 应继续表示：

> 该 Public Model 能安全接受并处理哪些下游 `include` 值。

它不表示：

> 每次响应都保证包含对应 projection。

这一语义与当前 [`Models 接口、Public Model 契约与能力预检`](../implementation-status/features/models-api-and-capability-preflight.md)一致：公开 include 值表示请求可安全处理，不保证对应 output item。

## 4. 当前实现基线

### 4.1 请求分析

`ResponseInclude` 在 `src/core/capability/generation.rs` 中是闭合枚举，逐个保存标准 wire 值。`ResponseInclude::from_wire` 精确识别：

- `web_search_call.action.sources`；
- `code_interpreter_call.outputs`；
- `computer_call_output.output.image_url`；
- `file_search_call.results`；
- `message.input_image.image_url`；
- `message.output_text.logprobs`；
- `reasoning.encrypted_content`。

`analyze_response_includes` 位于 `src/pipeline/generation/analysis.rs`：

- 省略和 `null` 视为未请求；
- 非数组、非字符串元素和未知值返回能力错误；
- 合法值进入 `RequestedCapabilities.response_includes`；
- analyzer 只提取请求事实，不解析 registry 或选择 Route。

该闭合解析边界应保留。

### 4.2 Public Model 编译和拒绝点

`RouteContractContribution::from_binding` 通过 `protocol_specific_capabilities` 读取每条 Route 的 include 能力。当前 Native Responses Route 直接复制 `ProviderResponsesCapabilities.include`；`aggregate_generation_contract` 再对所有 candidate 的 `response_includes` 求交集。交集为空时，`include` 也从公共 `supported_parameters` 中删除。

请求阶段的 `validate_interface_request` 检查每个请求 include 是否位于编译后的 Public Model 交集中；不在交集时返回 `RequestPlanningError::UnsupportedCapabilities`。Ingress 将其映射为 HTTP 400 `unsupported_model_capability`。此时尚未生成 candidate body，也没有 Provider egress。

因此近期 Hermes 请求中的 400 是 OpenBridge 本地固定接口拒绝，不是 Provider 返回的 400。

### 4.3 当前 Provider 声明

| Provider | 当前 Responses 声明 | 目标 egress 行为 |
|---|---|---|
| OpenAI | 支持 `reasoning.encrypted_content` | 原样转发 |
| ChatGPT | 支持该值 | 原样转发 |
| OpenRouter | 支持该值 | 原样转发 |
| MiMo | 支持该值 | 原样转发 |
| LongCat | 支持该值 | 原样转发 |
| DeepSeek | `include: &[]` | 删除该精确 hint |
| Bailian | `include: &[]` | 删除该精确 hint |

声明 owner 分别位于 `src/providers/<provider>/definition.rs`。Provider ceiling 仍只描述 wire 支持，不应为实现下游兼容而虚假提升 Bailian 或 DeepSeek ceiling。

### 4.4 已有局部先例

Responses→Chat Bridge 已经实现相同原则：

- 下游 Responses 接受 `reasoning.encrypted_content`；
- Chat wire 没有 `include`，Bridge 在 egress 前消费它；
- Bridge 不承诺或合成 opaque reasoning output。

对应 owner 是 `protocol_specific_capabilities`、`BridgePlan` 和 `tests/bridge_forwarding_contract.rs` 中的 `responses_bridge_consumes_reasoning_include_before_chat_egress`。Native 路径目前没有按 candidate 的 include 值过滤机制。

## 5. 当前设计缺口

当前 `response_includes` 同时承担“下游安全接受集合”和“Provider 原生转发集合”，产生三个问题：

1. **错误拒绝**：Bailian、DeepSeek 或混合 candidate Public Model 会在 preflight 拒绝本可安全消费的 hint。
2. **无法表达候选差异**：同一个不可变 canonical request 无法为支持与不支持 include 的 fallback candidate 生成不同 egress body。
3. **未来容易错误泛化**：若简单把 `include` 当作普通 ignored parameter，会丢失逐值语义，并错误删除同数组中真正影响输出的 projection。

现有 `IgnorableGenerationParameter` 不适合作为解决方案。它按顶层参数名删除整个字段，只覆盖显式批准的普通 generation 参数；`include` 是带多个独立语义值的 capability 字段，必须逐值处理。

## 6. 候选领域模型

### 6.1 保留闭合 wire 枚举

继续由 `ResponseInclude` 表示已知标准值。未知值不进入兼容策略，不能以字符串 passthrough 绕过 closed catalog。

### 6.2 增加逐值 gateway handling policy

概念上需要一个闭合策略：

```rust
pub enum ResponseIncludeHandling {
    NativeOnly,
    ForwardOrOmit,
}
```

当前映射应仅为：

```text
ReasoningEncryptedContent -> ForwardOrOmit
其他 ResponseInclude      -> NativeOnly
```

名称可以调整，但 owner 应位于 generation capability/policy 域，而不是散落到各 Provider adapter。该策略表达 OpenBridge 已批准的下游兼容语义，不能由 Provider capability 自动推导。

### 6.3 分离 accepted set 与 forwarded set

每条 Route contribution 应贡献“该 Route 可安全接受的 include 集合”：

```text
accepted = provider_native_includes ∪ gateway_forward_or_omit_includes
```

Public Model 继续对所有 Route 的 **accepted set** 求交集，保持固定候选、保守契约和不按请求 capability routing 的架构。

每个 `RouteExecutionCandidate` 另需冻结私有的 **forwarded set**：

```text
forwarded = selected Upstream API 的 ProviderResponsesCapabilities.include
```

这两个集合具有不同含义，不能再次共用一个字段。Public Models 只投影 accepted set；forwarded set 不序列化，也不泄漏执行拓扑。

### 6.4 Candidate-specific egress 过滤

`plan_request` 已从同一个 immutable canonical body 为每个固定 candidate 独立构造请求。应在 candidate materialization 阶段、Provider adapter 之前执行 typed include 过滤：

1. 请求值位于 candidate forwarded set：保留原始元素；
2. 不在 forwarded set，但 handling 为 `ForwardOrOmit`：删除该元素；
3. 不在 forwarded set 且 handling 为 `NativeOnly`：防御性返回错误；正常情况下应已被 Public Model preflight 拒绝；
4. 过滤后数组为空：删除顶层 `include`；
5. 不改变其他 candidate 的 body；
6. 保持其余请求字段和被保留 include 元素的原始 wire 顺序。

示例：

```json
// 下游
{
  "include": [
    "reasoning.encrypted_content",
    "file_search_call.results"
  ]
}
```

若某 candidate 原生支持 `file_search_call.results`、但不支持 encrypted-content hint，其 egress 应为：

```json
{
  "include": ["file_search_call.results"]
}
```

前提是整个 Public Model 的固定 accepted contract 已允许 `file_search_call.results`；否则请求必须在 candidate 展开前零 egress 拒绝。

### 6.5 Bridge 统一

现有 Responses→Chat Bridge 特例应收敛到同一个 accepted/forwarded policy：

- Responses downstream accepted set 包含 approved hint；
- Chat upstream forwarded set 为空；
- candidate filtering 删除 hint；
- Bridge 不再拥有一套语义重复但仅适用于该方向的 capability 特例。

是否在首个实现切片中立即移除旧特例，应由 live source 和失败测试决定；不应保留长期双路径。

## 7. 请求与响应合同

### 7.1 接受

以下形状应被所有 Responses Public Model 接受，只要请求的其他字段满足其固定接口：

```json
{"include": ["reasoning.encrypted_content"]}
```

省略、`null` 和空数组继续按当前 no-op 规则处理。

### 7.2 拒绝

以下情况继续在 Provider egress 前拒绝：

- `include` 不是数组；
- 任一元素不是字符串；
- 任一值不在 `ResponseInclude` 闭合目录；
- 任一已知值既不被所有固定 candidate 原生支持，也没有显式 `ForwardOrOmit` policy；
- 请求其他 capability 不满足固定 Public Model interface。

错误应尽量定位到 `param: "include"`，而不是只返回无字段信息的泛化 capability 错误。是否拆分完整 generation error enum 可作为同一 current focus 的错误合同切片，也可另立切片；不能为了 include 转发而扩大到所有错误重构。

### 7.3 Provider egress

支持 Provider 收到原始精确值；不支持 Provider 看不到该 hint。过滤不能改变：

- `reasoning` effort/summary；
- tools、tool choice 或 parallel policy；
- output budget；
- prompt cache 字段；
- input/history；
- streaming 或 state 字段。

### 7.4 Provider response

OpenBridge 保留 Provider 实际返回的合法响应，不因为请求 hint 是否被删除而执行补偿：

- 不生成假的 encrypted content；
- 不把 plaintext reasoning 改写为 encrypted content；
- 不因缺少 encrypted content 把成功响应改为失败；
- 不承诺被测 Provider 未来仍默认返回相同 reasoning 形状。

## 8. 为什么不能泛化整个 `include`

其他标准 include 值可能直接改变客户端可见输出：

- web search sources；
- code interpreter outputs；
- file search results；
- output text logprobs；
- computer output image URL；
- input image URL projection。

静默删除这些值可能丢失客户端明确请求的数据、破坏 tool lifecycle 或改变后续处理。因此新值只有在分别获得协议依据、目标 Provider 证据、响应观察和回归测试后，才能独立加入 `ForwardOrOmit`；不能按字段名、Provider 家族或“OpenAI-compatible”标签批量放宽。

## 9. Hermes 请求的直接影响

### 9.1 `qwen3.8-max`

近期 Hermes Responses 请求携带：

```json
{
  "include": ["reasoning.encrypted_content"],
  "parallel_tool_calls": true,
  "prompt_cache_key": "..."
}
```

Bailian 当前 Responses ceiling 不声明 include，但声明 parallel tool calls 和 prompt cache key。按本文策略删除 encrypted-content hint 后，该请求不再因 include 被拒绝；其他字段仍由固定接口独立校验。

### 9.2 `deepseek-v4-pro`

DeepSeek 当前不声明：

- `include`；
- `parallel_tool_calls`；
- `prompt_cache_key`。

本文只解决第一个阻断。`parallel_tool_calls: true` 可能改变工具执行语义，未经真实证据不能删除。`prompt_cache_key` 可能影响缓存隔离、计费和延迟，当前合同将其解释为 exact forwarding，也不能借本设计顺带吞掉。修复 include 后，该请求仍应对下一项不满足的字段 fail closed，并尽可能返回字段级错误。

## 10. Opaque replay 与 fallback 风险

请求 hint 可 omitted-equivalent，不表示响应中的 `encrypted_content` 可以跨执行边界重放。

Hermes 会把 opaque reasoning item 与 issuer 标记保存在 sidecar，并只向它认为相同的 endpoint/provider replay。对 Hermes 而言，OpenBridge 是单一 endpoint；但 OpenBridge 内部可能在固定 fallback 中切换 Provider、Target、Upstream API 或 credential owner。因此后续请求可能把一个内部 issuer 颁发的 opaque state 带回 OpenBridge，而客户端无法表达内部 affinity。

严格边界必须保持：

- Responses→Chat Bridge 继续拒绝把 opaque continuation 当作 plaintext reasoning；
- Native 多 Provider fallback 不能盲转 foreign opaque state；
- 在没有完整 issuer ledger/affinity 设计前，应对携带 `encrypted_content` 的 input 禁止不安全 fallback、绑定到可证明的 issuer，或零 egress 拒绝；
- 本文不选择具体 replay 方案，也不授权实现通用 continuation ledger。

该风险应作为实现前阻断检查，但不应阻止安全消费当前轮的 request include hint。

## 11. 非目标

本文不设计或授权：

- 按请求 capability 跳过较弱 Route；
- 动态 Provider negotiation；
- 任意 `include` passthrough；
- response synthesis；
- `parallel_tool_calls` 或 `prompt_cache_key` 静默删除；
- opaque continuation 跨 Provider 转换；
- state ledger、`previous_response_id`、background 或 stored Responses；
- Provider capability 自动探测或热更新；
- Models schema v2；
- 与该 hint 无关的 capability/error 全面重构。

## 12. 候选实施切片

真正实施时应只把以下一个可观察切片提升到 `current-focus.md`：

> 所有 Responses Public Model 接受 `reasoning.encrypted_content` hint；每个固定 candidate 按原生 include ceiling 原样转发或安全删除；其他 include 值和其他 capability 行为不变。

建议依赖顺序：

1. 先用失败测试固定 Public Models accepted projection、Native 支持/删除和 fallback candidate body 差异；
2. 分离 Route accepted include 与 candidate forwarded include；
3. 在 planning 中增加逐值、candidate-specific 过滤；
4. 收敛 Bridge 特例，避免长期双路径；
5. 更新错误参数定位、功能需求、OpenAPI/Models fixture 和 implementation status 中受影响的唯一事实 owner；
6. 最后执行 focused tests 与仓库基线。

该顺序只是执行准备，不构成当前实施授权。

## 13. 验证矩阵

### 13.1 Analyzer 与 preflight

| 场景 | 预期 |
|---|---|
| `include` 省略、`null`、`[]` | 接受；no-op 规范化后不向上游发送空字段 |
| singleton encrypted-content | 所有 Responses Public Model 在其他字段合法时接受 |
| 非数组、非字符串元素 | `param=include` 的 400；零 egress |
| 未知 include 值 | `param=include` 的 400；零 egress |
| encrypted-content + 不安全且不受支持值 | 整体拒绝；不能只删除不安全值后继续 |

### 13.2 Native exact egress

| Candidate | 预期 |
|---|---|
| OpenAI/ChatGPT/OpenRouter/MiMo/LongCat 支持 API | 精确保留 `reasoning.encrypted_content` |
| Bailian/DeepSeek 不支持 API | 删除该元素；数组为空后删除顶层 `include` |
| 混合数组 | 只删除已批准 hint，保留原生支持值和原顺序 |

### 13.3 Fallback 隔离

至少验证：

- candidate A 支持、B 不支持时生成两个不同的 immutable egress body；
- A 在首输出前失败后，B 收到不含 hint 的 body；
- 反向 candidate 顺序也不产生 mutation 泄漏；
- 请求事实不筛选、跳过或重排 candidate；
- partial response commit 后不执行 fallback。

### 13.4 Response

至少验证：

- 支持 Provider 的 encrypted content 原样保留；
- 不支持 Provider 的 plaintext reasoning/summary 原样保留；
- hint 被删除时不合成 output item；
- JSON 与 SSE terminal/error 行为不因该策略改变。

### 13.5 Models 和隐私

至少验证：

- Responses interface 的 `response_includes` 投影包含 approved hint；
- `supported_parameters` 与 typed accepted set 同源；
- Models 不泄漏 candidate forwarded set、Provider、Route 或内部 omission policy；
- candidate 顺序不改变公共 accepted 集合。

## 14. 证据和验证边界

当前已有的静态证据：

- `ResponseInclude` 闭合枚举和 analyzer；
- Public Model candidate 交集与 preflight 拒绝路径；
- Provider include ceiling 差异；
- Responses→Chat Bridge 消费 hint 的现有测试；
- ChatGPT Native 原样转发 hint 的现有测试；
- 空 include 被规范化删除的现有测试。

当前没有证明：

- Native Bailian/DeepSeek 条件删除 hint 的实现；
- 混合 include 数组的逐值过滤；
- 支持/不支持 candidate fallback body 隔离；
- opaque encrypted input 的完整内部 issuer affinity；
- 所有 Provider、账号、区域和未来版本都保持当前 reasoning 输出观察。

分析期间曾因并行中的 Images 焦点尚未完成而无法运行既有 Bridge 与 ChatGPT focused tests；该临时阻塞现已随 Images 焦点完成而解除，当前仓库全量 Rust 基线通过。本文仍未实施 Responses compatibility 行为；真正进入该切片前必须重新运行对应 focused tests，并将其回归与 Images 合同保持隔离。

## 15. 执行前检查

进入 current focus 前必须重新确认：

- [ ] live source、工作树和当前需求未发生冲突变化；
- [ ] 当前 crate 可编译，或既有失败已由其 owner 单独记录；
- [ ] approved hint 仍只有 `reasoning.encrypted_content`；
- [ ] Provider native include ceiling 与最新真实证据一致；
- [ ] Public accepted 与 candidate forwarded 两层命名和 owner 已明确；
- [ ] RED 覆盖 Native 删除、Native 转发、Bridge、混合数组和 fallback body 隔离；
- [ ] opaque replay 明确保持在本切片之外且没有被意外放宽；
- [ ] requirements、Models/OpenAPI、fixtures 和 implementation status 的更新 owner 已列出；
- [ ] focused tests 先通过，再执行 `cargo fmt -- --check`、`cargo test --locked`、`cargo clippy --locked -- -D warnings` 和 `git diff --check`；
- [ ] 真实 Provider、Hermes runtime、负载与长期行为作为独立更高层证据报告，不能由静态或 synthetic tests 代替。
