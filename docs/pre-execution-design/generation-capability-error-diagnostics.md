# Generation capability 错误定位设计

> **状态：候选执行前设计，不构成实施授权。** 本文记录 Generation 请求在 analysis/preflight 阶段返回字段级错误的目标合同、当前差距、候选结构和验证矩阵。真正实施前必须重新读取 live source 和工作树，只将一个可观察切片提升到 [`implementation-plans/current-focus.md`](../implementation-plans/current-focus.md)。

## 1. 背景与目标

OpenBridge 是严格网关：未知字段、非法形状、未实现能力和 Public Model 固定接口无法满足的请求都必须在 Provider egress 前失败。严格拒绝本身没有问题，但错误必须足够精确，让客户端和 OpenBridge 开发者能够区分：

- 哪个标准字段无效；
- 哪个字段是已知协议字段、但当前 Public Model 不支持；
- 是字段值、组合、资源上限还是协议接口不满足；
- 请求是否已发生 Provider egress；
- 修正第一个问题后，下一个确定性阻断项是什么。

2026-08-22 Hermes 日志暴露了当前缺口：`qwen3.8-max` 与 `deepseek-v4-pro` 的 Responses 请求均收到：

```json
{
  "error": {
    "code": "unsupported_model_capability",
    "message": "The selected model does not support the requested capability"
  }
}
```

错误没有 `param`，无法区分 `include`、`parallel_tool_calls`、structured output、state、tool choice 或其他 capability。对 DeepSeek 请求，移除已确认可安全消费的 `reasoning.encrypted_content` 后，仍会依次遇到 `parallel_tool_calls:true` 和 `prompt_cache_key` 边界；泛化错误会迫使开发者重复修改和重放请求才能发现下一项。

本文目标是：

> 保持现有 fail-closed、固定接口、零 egress 和稳定错误码，只让 Generation 错误精确定位到一个确定性的标准请求字段，并在必要时用内部闭合 reason 分类支持测试和观测。

## 2. 与相关设计的关系

- [`Responses reasoning.encrypted_content 兼容提示设计`](responses-reasoning-encrypted-content-compatibility.md)决定该精确 include 值何时接受、转发或删除；本文只决定剩余拒绝如何表达。
- [模型能力功能需求](../functional-requirements/model-capability/README.md)拥有产品级固定接口与 fail-closed 合同；本文不能替代或修改需求。
- [Models 与 capability preflight 当前状态](../implementation-status/features/models-api-and-capability-preflight.md)拥有当前实现事实；本文不应被引用为“已经实现”。
- Embeddings 和 Images 已使用带 `param` 的 typed error，可作为结构先例，但本文不自动修改它们的既有合同。

## 3. 当前源码事实

### 3.1 Generation 错误类型仍以无字段 variant 为主

`src/pipeline/error.rs` 中的 `RequestPlanningError` 同时包含：

- 可定位字段的 `UnknownParameter(String)`；
- 可定位字段的 `UnsupportedParameter(&'static str)`；
- 无字段的 `UnsupportedCapabilities`；
- 无字段的 `StreamingUnsupported` / `NonStreamingUnsupported`；
- 无字段的 `OutputLimitExceeded`；
- 无字段的 `ReasoningUnsupported` / `ReasoningLevelUnsupported`；
- 无字段的 `MultimodalInputLimitExceeded`。

相比之下，同文件中的 `EmbeddingRequestError` 与 `ImagesRequestError` 已经使用：

```rust
InvalidRequest {
    param: Option<&'static str>,
}

UnsupportedModelCapability {
    param: &'static str,
}
```

这证明字段定位可以保持闭合类型，不需要把 Provider 错误字符串或任意 JSON path 暴露到公共响应。

### 3.2 多个不相关检查被合并成一个布尔表达式

`src/pipeline/generation/preflight.rs` 的 `validate_interface_request` 当前把以下事实合并后统一返回 `UnsupportedCapabilities`：

- function tool choice mode；
- `parallel_tool_calls`；
- structured output；
- strict function schema；
- `previous_response_id`；
- `background`；
- Responses `include` 逐值支持。

后续图片、文件和音频检查也大量返回同一 variant。结果是 pipeline 已经知道失败来自哪个 request fact，但在构造错误时丢失了字段身份。

### 3.3 ingress 抹平为无 `param` 的错误

`src/ingress/response.rs` 当前将多个 Generation variant 合并为：

```text
HTTP 400
error.type = invalid_request_error
error.code = unsupported_model_capability
error.param = null / omitted
```

只有 `UnsupportedParameter(parameter)` 使用 `typed_api_error(..., Some(parameter))`。因此当前问题不是 OpenAI-compatible error envelope 不支持 `param`，而是 Generation pipeline 没有保留足够的 typed location。

### 3.4 已有字段级测试资产

仓库已有多个字段级断言：

- unknown top-level parameter 返回 `param`；
- `stream_options` unsupported 返回 `param="stream_options"`；
- output limit 测试返回 `param="max_output_tokens"`；
- Images/Embeddings unsupported capability 返回对应字段；
- Provider-specific unsupported ordinary parameter 返回参数名且零 egress。

因此目标不是引入新的错误格式，而是把 Generation capability checks 全部收敛到同一已存在格式。

## 4. 目标公共错误合同

### 4.1 稳定 envelope

Public Model 已知但无法满足字段要求时继续返回：

```json
{
  "error": {
    "type": "invalid_request_error",
    "code": "unsupported_model_capability",
    "message": "The selected model does not support the requested capability",
    "param": "parallel_tool_calls"
  }
}
```

约束：

- HTTP status 保持 400；
- `type` 保持 `invalid_request_error`；
- `code` 保持 `unsupported_model_capability`；
- `param` 必须是闭合协议目录中的标准字段名；
- 不暴露 Provider、Route、Target、credential pool、fallback 顺序或内部 capability 结构；
- 不复制或回显任意用户输入值；
- 错误仍发生在零 egress 边界。

### 4.2 invalid、unknown、unsupported 必须分开

| 类型 | 例子 | code | param |
|---|---|---|---|
| 未知字段 | `future_parameter` | `unknown_parameter` | 原顶层字段名 |
| 已知字段但形状非法 | `include: "x"` | `invalid_request_error` | `include` |
| 已知字段且形状合法，但模型接口不支持 | `parallel_tool_calls:true` | `unsupported_model_capability` | `parallel_tool_calls` |
| 值超过模型/接口限制 | `max_output_tokens` 超限 | `unsupported_model_capability` 或需求已批准的 limit code | 实际输出上限字段 |
| 模型不存在或无 route | 未注册 Public Model | `model_not_found` | `model` |
| 内部配置无法兑现已发布接口 | 编译不变量被绕过 | `configuration_error`，5xx | 不暴露内部字段 |

不能为了统一实现而把 malformed value 误报为模型能力不足，也不能把已知但不支持的参数误报为 unknown。

### 4.3 一个响应只返回一个确定性失败

不返回动态错误数组，也不按 `HashMap`/`BTreeSet` 的偶然遍历顺序选择。原因：

- OpenAI-compatible 客户端通常预期单个 error object；
- 多错误响应会扩大公共 schema；
- 客户端修复循环需要稳定的“第一个失败”；
- 内部完整失败集合可以进入受控 trace，不应成为公共合同。

## 5. 推荐的闭合错误模型

概念上将 Generation 错误收敛为与 Images/Embeddings 一致的结构：

```rust
enum GenerationRequestError {
    InvalidRequest {
        param: Option<GenerationErrorParam>,
    },
    ModelNotFound,
    UnsupportedProtocol,
    UnsupportedModelCapability {
        param: GenerationErrorParam,
        reason: GenerationCapabilityReason,
    },
    RouteUnavailable,
}
```

其中 `GenerationErrorParam` 应是闭合 enum 或可证明来自 `GenerationRequestField::as_wire_name()` 的静态值，而不是任意 `String`。未知字段仍需要保留经过安全限制的原字段名，继续走 `UnknownParameter` 专用路径。

候选内部 reason：

```rust
enum GenerationCapabilityReason {
    Streaming,
    NonStreaming,
    ToolChoice,
    ParallelToolCalls,
    StrictToolSchema,
    StructuredOutput,
    PreviousResponse,
    Background,
    ResponseInclude,
    ImageInput,
    FileInput,
    AudioInput,
    OutputLimit,
    Reasoning,
    ReasoningLevel,
    OrdinaryParameter,
}
```

`reason` 的用途是：

- 确定性测试；
- 低基数 metrics/trace；
- 维护稳定 validation order；
- 未来在不改公共 envelope 的前提下改进内部诊断。

默认不把 `reason` 序列化给下游，以免把内部能力模型固化为公共 API。

## 6. 字段定位规则

### 6.1 顶层控制字段

| 请求事实 | param |
|---|---|
| streaming unsupported | `stream` |
| non-streaming unsupported | `stream` |
| function tool choice mode | `tool_choice` |
| parallel function calls | `parallel_tool_calls` |
| strict function schema | `tools` |
| Chat structured output | `response_format` |
| Responses structured output | `text` |
| previous response continuation | `previous_response_id` |
| background mode | `background` |
| Responses projection | `include` |
| unsupported ordinary parameter | 该字段本身 |

### 6.2 reasoning

- Responses reasoning object：`param="reasoning"`；
- Chat reasoning effort：`param="reasoning_effort"`；
- conflicting reasoning sources：应归为 invalid request，而不是 unsupported capability；
- 不在 message 中回显未知 effort 原值；允许使用固定描述“unsupported reasoning level”。

### 6.3 multimodal

- Responses 图片/文件输入定位到 `input`；
- Chat 多模态输入定位到 `messages`；
- 独立顶层 `audio` 参数定位到 `audio`；
- 第一阶段不公开 `input[3].content[1]` 等动态 JSON path，避免形成未设计的 path grammar；
- 内部 reason 可以继续区分 source、format、detail、cardinality 和 byte limit。

### 6.4 output limit

当前 analysis 只保留多个候选输出限制字段中的最大值。若要稳定返回正确 `param`，必须同时冻结触发最大值的源字段，不能在 preflight 中重新解释原始 JSON。

目标：

- Responses 使用 `max_output_tokens`；
- Chat 按实际参与限制判断的字段返回 `max_completion_tokens` 或 `max_tokens`；
- 同时出现多个字段时，analysis 使用协议定义的固定 precedence，并把最终字段身份写入 immutable request facts；
- 不以对象 key 顺序决定错误字段。

## 7. 确定性 validation order

推荐保持显式、有文档和测试保护的顺序：

1. JSON envelope 与 model；
2. unknown top-level field；
3. 已知字段形状与字段组合；
4. Public Model / operation interface；
5. streaming mode；
6. tool type、tool choice、parallel 与 strict schema；
7. structured output；
8. state：`store`、`previous_response_id`、`background`；
9. Responses `include`；
10. multimodal source/format/detail/limit；
11. output token limit；
12. reasoning support 与 level；
13. ordinary supported-parameter 集合。

这不是要求一次请求验证所有失败，而是要求相同请求、相同 registry 在任意构建和 candidate 顺序下都返回同一首个错误。

如果实施时发现现有客户端依赖另一顺序，应先以请求 dump 和测试建立兼容基线，再调整顺序；不得仅为代码结构美观改变公共首错行为。

## 8. Hermes 日志案例的目标结果

### 8.1 Qwen3.8 Max

在 [`reasoning.encrypted_content` 兼容提示设计](responses-reasoning-encrypted-content-compatibility.md)实施后，该精确 include 不再失败。当前 Qwen/Bailian interface 还支持 Hermes 请求中的 parallel 与 prompt cache，因此该案例预期继续规划，而不是返回 capability error。

### 8.2 DeepSeek V4 Pro

相同 include 被安全消费后，当前首个剩余阻断应明确为：

```json
{
  "error": {
    "type": "invalid_request_error",
    "code": "unsupported_model_capability",
    "param": "parallel_tool_calls",
    "message": "The selected model does not support the requested capability"
  }
}
```

`parallel_tool_calls:true` 会影响工具调用行为，不能在缺少 Provider 证据时静默删除。

如果未来经真实 Provider 证据确认该字段可接受或安全忽略，则下一个阻断可能是 `prompt_cache_key`。后者影响缓存隔离、费用和延迟，不应仅以“生成文本可能不变”为由自动归入 ignorable。

## 9. observability 边界

建议记录低基数属性：

- operation；
- protocol；
- error code；
- standard `param`；
- internal closed `reason`；
- zero-egress 标志或 attempts count。

禁止记录：

- 任意 include 原值；
- tool schema；
- messages/input；
- model prompt；
- credential 或内部 route topology。

同一失败只能记一次 request failure；不能因 pipeline、ingress 和 HTTP response 三层都观察到同一 error 而重复增加 counter。

## 10. 测试矩阵

### 10.1 pure analysis/preflight

- 每个 typed capability failure 返回正确 param 和 reason；
- unknown、invalid、unsupported 三类不互换；
- 多个同时失败的字段按固定顺序返回首错；
- candidate 顺序不改变错误；
- malformed include 返回 invalid `param=include`；
- 已知但不接受的 include 返回 unsupported `param=include`；
- encrypted-content safe hint 不产生错误。

### 10.2 in-process forwarding

- 每个字段级 400 envelope 精确断言 status/type/code/param/message；
- 所有本地拒绝均为零 Provider egress；
- 不泄漏 Provider、Route、credential 或请求正文；
- DeepSeek Hermes shape 首错为 `parallel_tool_calls`；
- 修正 parallel 后，后续错误稳定定位到 `prompt_cache_key`，除非其合同已另行批准。

### 10.3 Models 一致性

- `supported_parameters` 声明支持的普通字段不能被 preflight 拒绝；
- 未声明字段不能通过 ordinary parameter fallback；
- typed capability subobject 与 error param 使用同一个事实 owner；
- encrypted-content 的“安全接受”与 Provider 原生转发能力不混淆。

### 10.4 外部客户端

- OpenAI Python SDK 能读取 `error.param`；
- Hermes request dump 复现能直接显示阻断字段；
- 只验证错误解析，不发送真实 Provider 请求即可证明零 egress。

## 11. 非目标

本文不批准：

- 放宽任一 Provider capability；
- 为让 Hermes 请求通过而静默删除 `parallel_tool_calls`；
- 把所有 `include` 值变成 best effort；
- 返回所有失败字段数组；
- 暴露内部 Provider/Route/fallback；
- 改变 retry 或 fallback；
- 顺带重构 Images/Embeddings；
- 修改 OpenAPI 或 runtime，除非该切片进入 current focus。

## 12. 候选实施切片

若用户后续批准实施，建议单独进入 current focus：

1. 先为 Hermes DeepSeek request shape 增加 RED，证明旧代码缺少 `param`；
2. 为 Generation error 引入 typed param/reason，不改 capability truth；
3. 拆开合并布尔条件，按固定顺序返回具体错误；
4. 更新 ingress mapping；
5. 覆盖关键 capability、multimodal、limit 和 reasoning 参数；
6. 更新 OpenAPI error examples 和功能需求/实施状态；
7. focused tests 通过后再运行完整 baseline。

该切片与 encrypted-content 条件转发可以先后实施，但测试应明确：include 设计生效后 DeepSeek 的首个剩余错误变成 `parallel_tool_calls`。

## 13. 执行前检查

- [ ] 重新读取 live `RequestPlanningError`、preflight 和 ingress error mapping；
- [ ] 确认相关 Images/observability owner 与工作树没有并行冲突；
- [ ] 记录当前 Generation error fixtures；
- [ ] 确定公共首错顺序；
- [ ] 确定 output-limit 源字段保留方式；
- [ ] 确定 multimodal public param 只使用顶层字段；
- [ ] 建立 Hermes Qwen/DeepSeek 脱敏请求 fixture；
- [ ] 先证明 RED，后修改 runtime；
- [ ] 更新 requirements、OpenAPI、tests 和 implementation status；
- [ ] 不读取或写入私有 Provider 凭据。

## 14. 当前验证边界

本文基于 2026-08-22 的 live source、Hermes 本地日志与 request dump。已确认当前 pipeline 丢失字段身份以及 ingress 可支持 typed `param`；本文没有修改或执行运行时代码。

真正实施时必须重新建立可编译基线，不能把本文的源码位置或当前首错顺序当作永久事实。
