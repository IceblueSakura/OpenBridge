# TensorZero Provider-native capability 与 semantic types 调研

## 文档元数据

| 字段 | 值 |
|---|---|
| Source snapshot | [`tensorzero/tensorzero` `main` @ `62eb8f63e8ec62018d70420dbf1a8c5d1c026315`](https://github.com/tensorzero/tensorzero/tree/62eb8f63e8ec62018d70420dbf1a8c5d1c026315) |
| Last reverified | 2026-08-30，本地只读源码与测试源码复核 |
| Scope | inference semantic types、Provider conversion、function/custom/provider tools、reasoning/opaque data、stream chunks、capability/config 与 tests |
| Evidence boundary | 未构建或启动 TensorZero，未运行 ClickHouse/Provider E2E；静态类型和测试不能证明真实 Provider 当前行为 |
| Recheck trigger | `tensorzero-inference-types`、Provider config/converter、provider tools、reasoning、stream chunk、MCP 或 license 变化时 |

## 1. Architecture 与 canonical types

TensorZero 在 application input、model inference request、Provider wire 和存储/observability 之间有明确转换层。Provider 接收 `ModelInferenceRequest`，返回 `ProviderInferenceResponse` 或 `ProviderInferenceResponseChunk`；请求保存 semantic messages、system、tool config，响应同时保留 normalized output、usage、finish reason 和 raw request/response：`crates/tensorzero-inference-types/src/lib.rs:289-373`、`1106-1114`。

其 static semantic model 以 message + ordered content blocks组织。输入 `ContentBlock` 包含 text、tool call/result、file、thought 和 unknown；输出包含 text、tool call、thought、unknown：同文件 `181-205`、`463-490`。这比 Chat-only DTO 富，但仍是 Agent/inference-oriented model，不覆盖 Responses 的全部 item lifecycle、stored response 或 server-side tool result 类型。

Provider response中的 unknown block被捕获为 JSON `Value`，stream unknown还带 model/provider identity：同文件 `197-205`、`265-283`。这是保留未知 Provider 信息的机制，不代表该信息可以安全跨 Provider replay。

## 2. Function、custom 与 Provider tools

TensorZero 对三类工具显式分层：

- `Tool::Function` 是 client-executed JSON Schema function；
- `Tool::OpenAICustom` 是 OpenAI custom tool；
- OpenAI web search 一类 Provider tool故意不进入 `Tool`，而使用独立 `ProviderTool`：`crates/tensorzero-inference-types/src/tool.rs:19-44`、`181-200`。

`ProviderTool` 由 `scope` 和 opaque `tool: Value` 构成；scope 可限定 model 和 Provider：`crates/tensorzero-inference-types/src/lib.rs:808-852`。Provider call config将 function、provider 和 OpenAI custom tools保存在不同 vector：同文件 `1009-1017`。Provider config通过穷举 match声明是否支持 Provider tools：`crates/tensorzero-core/src/model.rs:1668-1679`。

这种分层证明“hosted tool 不是普通 function tool”可以落实到类型和 capability gate。限制也很明确：payload仍是 arbitrary `Value`，scope只约束目标，不验证具体 tool schema、输出 lifecycle、数据政策或可跨 Route replay 性。

## 3. Reasoning 与 opaque state

`ThoughtChunk` 分开保存 visible text、signature、summary ID/text、Provider type 和 Provider-specific `extra_data`：`crates/tensorzero-inference-types/src/lib.rs:244-263`。这比单个 `reasoning_content` 字符串更接近富语义模型，也显示 opaque data需要 Provider provenance。

Reasoning E2E 测试允许已知 text/thought 之外的 Provider raw reasoning block，并对 Gemini summary 的非确定性设置明确边界：`crates/tensorzero-core/tests/e2e/providers/reasoning.rs:77-107`。但这些 E2E 依赖真实模型和 ClickHouse，不适合作为逐 commit deterministic oracle；可吸收的是 block classification、storage round-trip 和“unknown 不得污染 visible thought”的场景。

## 4. Streaming

streaming 使用 `ContentBlockChunk`，包含 text、tool-call、thought 和 unknown；tool-call chunk保留 ID、可缺失 raw name 和 raw arguments，thought chunk保留独立 ID和 opaque state：`crates/tensorzero-inference-types/src/lib.rs:211-283`。Provider chunk另带 optional usage、raw usage、raw response、latency 和 finish reason：同文件 `289-300`。

这是一套 provider-neutral incremental payload，但没有统一 start/delta/end lifecycle：text/thought/tool 均以内容 chunk表达，terminal主要依赖 stream结束和 finish reason。若用于严格协议 gateway，还需补充 identity creation、block start/end、唯一 terminal、EOF-before-terminal 和 post-terminal rejection。

## 5. Capability、routing 与 extensions

Provider tool支持使用显式穷举，function strictness保留为调用要求，并承认不同 Provider 对 JSON Schema 有额外限制：`crates/tensorzero-inference-types/src/tool.rs:181-200`。这种 compile-time Provider support gate优于按协议名推断能力。

另一方面，model Provider config中的 `provider_tools: Vec<Value>`、reasoning `extra_data: Value` 和 unknown block都是宽扩展口。TensorZero 的受信配置/Agent场景可以接受这种灵活性；面向不受信 downstream 的 gateway需要 typed namespace、schema validation、Route whitelist、大小限制和 exposure/replay policy。

## 6. State 与 observability

normalized response与 raw request/response同时保存，raw request在入库前尽力替换 file data：`crates/tensorzero-inference-types/src/lib.rs:352-413`。这是 semantic projection与wire evidence分离的例子，但 raw capture属于可观测性/存储策略，不应进入 canonical IR 本身。

Variant和Provider fallback还会记录失败 attempt的 raw response：`crates/tensorzero-core/src/variant/mod.rs:198-214`、`964-978`。这有助于 attempt-level observability，不能代替 continuation/provider-affinity contract。

## 7. 可吸收测试资产

优先自主重写：

1. function、custom、Provider tool不能互相反序列化或静默降级；
2. Provider tool scope匹配/不匹配 model与Provider；
3. unsupported Provider tool在 encode前失败，而非由Provider返回模糊错误；
4. thought text、summary、signature、extra data独立 round-trip；
5. opaque reasoning只允许同 Provider/Target replay；
6. fragmented tool arguments保持 ID和原始 bytes，malformed JSON不丢失原串；
7. unknown block可观察但默认不可跨 Provider encode；
8. normalized output与raw evidence分别测试，media capture保持有界/脱敏。

TensorZero 使用 Apache-2.0。其大量 E2E依赖真实 Provider、数据库和模型采样；测试吸收时应只提炼独立语义并自主编写 deterministic fixture。

## 8. Lessons

### Adopt

- portable function/custom tool与Provider tool类型分离；
- Provider tool scope和穷举 capability gate；
- reasoning text、summary、signature和opaque extra data分层；
- normalized semantic output与raw Provider evidence分离。

### Adapt

- 将 `ProviderTool.tool: Value`收紧为 typed Provider extension registry；
- 将 chunk union扩展为带 start/delta/end/terminal的 Event IR；
- 将 unknown preservation变成明确的 observability、same-target replay或reject policy。

### Avoid

- 允许 downstream arbitrary Provider JSON直接进入上游；
- 把 unknown/extra data存在于 semantic object解释为可移植；
- 将真实模型 E2E pass当成 protocol conversion correctness。

### Open Questions

- Provider tool result和citation是否进入 normalized output，还是只留 raw Provider block；
- MCP tool与Provider-hosted tool的执行 owner、approval和result semantic如何区分；
- thought signature/extra data的route affinity和fallback失效规则；
- stream materialization是否能与non-stream normalized response建立严格等价。
