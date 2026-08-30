# 当前开发焦点

## 状态

**已批准：富语义 Generation IR 设计基线；仅文档与设计，不修改运行时。**

## 1. 目标

结合当前 OpenBridge 源码、确定性测试以及已经固定的外部生态调研，形成后续额外 rewrite branch 可采用的
Generation Canonical IR 设计基线。该基线必须回答静态语义、stream event、identity/state、capability/fidelity、
Provider lowering、server-side tools 和测试迁移问题；本焦点完成前不定义生产 Rust API，也不替换现有 Bridge 或
Native Path。

最终设计对象是一次模型交互的内部语义，不是 OpenAI Chat、Responses 或任一 Provider DTO 的重命名版本。

## 2. 使用背景与复杂度预算

OpenBridge 主要是个人使用的可信配置网关，不建设多租户 model/provider 授权系统。设计优先：

1. immutable data + pure transformation；
2. closed enum/newtype 表达 portable semantic；
3. 显式 `Result`、fidelity 和 lowering disposition；
4. 小 facade、按 semantic domain 拆分的 leaf modules；
5. 由可信 Bootstrap/Registry 决定 Target、URL、credential 和 Provider profile。

仍需保留的安全边界只有现有架构已经依赖的技术正确性：业务请求不能选择任意 URL、credential、auth header 或
transform script；Provider-private state 不能错误 replay 到不兼容 Target；bounded body/event/resource limit、
commit 后禁止 fallback、credential secrecy 和 fail-closed unknown semantic 不因个人使用而取消。

明确不引入：

- per-user/per-model ACL；
- 通用规则 DSL、动态策略图或 plugin policy engine；
- 为每个语义对象建立 trait hierarchy；
- 通过 Provider name 分散 `match` 的“简化”；
- portable semantic 中的任意 `serde_json::Value`；
- 长期 Native/IR 双栈、compatibility shim 或 legacy alias。

## 3. 当前事实基线

当前实现已经包含多个彼此独立但不完整的隐式语义层：

| 当前 owner | 已拥有的事实 | 不是 |
|---|---|---|
| `src/core/request.rs` | `OperationKind`、Chat/Responses `ApiProtocol` 和 protocol-bound JSON bytes | semantic IR |
| `src/pipeline/generation/analysis.rs` | registry-independent request facts、unknown/reserved field rejection | Route selector 或完整 decoder |
| `src/pipeline/generation/types.rs` | capability requirements、ordered candidates、fallback/state-affinity flag | request/response content model |
| `src/pipeline/generation/preflight.rs` | Public Model fixed contract 与 value-sensitive capability validation | encoder |
| `src/pipeline/generation/planning.rs` | normalization、candidate lowering、Native/Bridge materialization | canonical semantic owner |
| `src/bridge/conversion/` | Chat↔Responses request/response/stream pairwise conversion | protocol-neutral transform |
| `src/bridge/chat.rs`、`responses.rs` | terminal、identity、fragmented tool arguments 和 stream accumulation | canonical Event IR |
| `src/provider/operation.rs` | fixed Provider operation/path、wire preparation、status/SSE classification | capability planner 或 tool executor |
| `src/registry/public_model/` | immutable execution interface、capability aggregation、continuation issuer constraints | request-time semantic decoder |

直接证据包括：

- `ApiRequest` 明确保存 RoutePlan 确定协议后的 JSON bytes：`src/core/request.rs:124-188`；
- analyzer 不选 Route、不改 body：`src/pipeline/generation/analysis.rs:35-41`；
- `RequestRequirements` 与 `RequestedCapabilities` 只保存规划事实：`src/pipeline/generation/types.rs:15-26`、`74-96`；
- Native candidate 目前仍保留 normalized wire body，Bridge candidate 调用 pairwise `BridgePlan`：
  `src/pipeline/generation/planning.rs:24-34`、`83-145`；
- `previous_response_id` 使 cross-target fallback 关闭：`src/pipeline/generation/planning.rs:150-155`；
- Provider adapter 已按 operation 固定 protocol/path，并拒绝不匹配 request：`src/provider/operation.rs:132-182`。

## 4. Canonical semantic inventory

设计必须逐项给出 canonical owner、wire decoder、capability requirement、lowering 和测试不变量：

| Domain | 必须表达的语义 |
|---|---|
| Instruction/conversation | instruction source、ordered user/assistant items、role 与 content ordering |
| Content/resource | text、image、audio、file；URL/inline/provider reference 仅是 source variant |
| Tool declaration | function、server-side、Provider-native；schema/config 与 execution owner 分离 |
| Tool lifecycle | declaration、call、arguments、result、error、approval、source/citation 和 identity |
| Reasoning | requested effort、visible text、summary、opaque replay state 与 visibility |
| Output constraint | unconstrained text、JSON object、JSON Schema 与 strictness |
| Generation control | output limit、sampling/stop/seed、parallel-tool policy；保留 absent/value distinction |
| State | continuation reference、Provider session/cache 和 opaque state 分离 |
| Response | ordered output items、finish reason、status、usage、Provider metadata |
| Streaming | lifecycle event、delta identity、terminal、error、usage、EOF 与 materialization |

`RequestRequirements` 中已有的 capability facts 应由 canonical request 纯投影产生，而不是继续与 full request 平行解析；
media size/source aggregate、requested parameter set 和 stream usage request仍是 planning projection，不应塞回每个 IR node。

## 5. IR 边界

### 进入 IR

- downstream 请求表达的 portable model-interaction semantic；
- decoder 能可靠识别的 Provider-native declaration 或 opaque state；
- upstream response 解码后的 ordered semantic items/events；
- identity、visibility、execution owner、state affinity 和 fidelity 所需标记。

### 不进入 IR

- Public Model alias、Route order、Target URL、credential、auth/proxy headers；
- retry/cooldown/health、transport client、HTTP status classification；
- request/response body limit 的执行器状态；
- tracing span、JSONL record 或 Provider raw transcript；
- Agent run/step/subagent、tool implementation 和 arbitrary orchestration state；
- Embeddings、Images Generations 等独立 operation 的现有 DTO。

IR 可以投影 capability 和 observability，但不得反向依赖 Registry、RoutePlan、transport 或 telemetry。

## 6. 设计不变量

1. Chat、Responses 以及后续 Messages/Gemini wire 只存在于 decoder/encoder 边界。
2. 同协议不代表同 capability；Native fast path 只能是可证明保持 IR semantic 的优化。
3. item 与 content 顺序默认具有语义，不在 decoder 中合并 reasoning/text/tool items。
4. Gateway identity 与 Provider wire identity 分离；synthetic ID 不伪装成 Provider replay ID。
5. opaque state 带 namespace 与 affinity；不理解 payload 也不能 arbitrary passthrough。
6. unknown portable semantic fail closed；Provider extension 只有目标 profile 显式接受时才能 encode。
7. capability check 在 lowering 前完成，encoder 只编码已决定的 lowering。
8. loss、normalization、synthesis、omission 和 emulation 可机器观察，不由 adapter 静默执行。
9. Event IR 必须能 materialize 为与 non-stream IR 等价的 terminal response。
10. commit 前可 retry/fallback，commit 后任何第二 Provider 输出都不得拼接。

## 7. 本设计焦点的三个 checkpoint

### D1：边界与库存

本节即 D1：固定现有 owner、semantic inventory、复杂度预算和非目标。

### D2：Static IR 与 lowering

形成最小富语义 algebra、identity/state、Provider extension、capability projection、fidelity report 和纯函数签名；用现有
Chat/Responses fixtures做 paper walkthrough。

### D3：Event IR、server-side tools 与迁移验证

形成 event algebra、materializer、server-tool 注入/剥离/执行边界、IR-native test layers 和 rewrite branch 原子替换门槛。

## 8. 完成与授权边界

本焦点完成条件：D1-D3 形成一份内部一致、能被现有 fixture 反证、明确 alternatives/open questions 的设计基线；所有外部
事实仍链接 `docs/references/`，不复制动态 Provider capability 表。

本焦点不授权：创建 rewrite branch、定义生产 Rust IR types、修改 runtime、公开 API、Registry schema、OpenAPI、canonical
fixtures 或 Provider registration。设计评审通过后，用户需另行批准 rewrite branch 的实现焦点。
