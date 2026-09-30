# Generation 当前能力与边界

本页是当前主线 text Generation 完成度与剩余缺口的唯一汇总，不是旧版本迁移清单。当前实现以产品目标和现行合同独立演进，不要求追平旧版；旧版仅作[历史参考](../archive.md)。[semantic-ir](../architecture-v2/semantic-ir.md)拥有目标设计，[Responses profile](../architecture-v2/responses-text-profile.md) / [Chat profile](../architecture-v2/chat-text-profile.md)拥有逐分支准入，[next-goal](../implementation-plans/next-goal.md)拥有优先级。标准依据为[固定上游版本](../references/upstream-sync.md)，不是全部最新 API 或真实 Provider 的能力声明。

## 完成度判断

**受限的无状态 text Generation 主链已闭合：请求、响应、事件经同一 IR，可变换并重编码，且已接通最小认证 HTTP 网关与固定 SDK synthetic 全链。可以推进选定文本场景验收，但不能称完整文本标准实现、全面 Agent 兼容或生产就绪。** 不用一个百分比混合衡量语义表达、codec、消费者和执行层。

这里的“纯文本”包括文本/refusal、结构化输出、function/custom 的文本调用与结果、reasoning 历史及相关控制/上下文，不仅是一个 prompt 返回一个字符串。它不自动包含所有“结果恰好是文本”的 hosted tools、状态资源或 program 执行。文本是验证[最终网关目标](../architecture-v2/README.md#产品目标与阶段判据)的阶段性载体：最小整体流程已接线；当前产品门槛转为新入口的受控外部验收、选定 Agent/缓存场景和必要的运行保障，而不是无限扩大字段清单。

| 层级 | 当前判断 | 证据与不能推出的结论 |
|---|---|---|
| Task IR / 变换 / lowering | 已准入文本子集的主链闭合，不是完整标准 IR | `src/semantic/task/generation/`、`src/lowering/`；最终 IR 权威、候选独立投影与拒绝已有测试；最小入口仅使用启动固定候选，不代表动态选择或任意模型 capability |
| Responses JSON/SSE | 已准入无状态文本分支有双向与 Static/Event 验证；完整 union 尚未逐分支审计 | `src/protocol/openai/`、`tests/semantic/`、`tests/transport/responses_sse.rs`；见下方缺口状态，不能称完整标准实现 |
| 单候选 Chat JSON/SSE | 已是同一 IR 的第二协议验证；文本 context 与 content/refusal probabilities 已有双向映射 | `tests/semantic/chat_logprobs.rs`、`tests/transport/chat.rs`；请求/静态/SSE、IR 编辑与目标拒绝分别验证；其他标准文本控制仍有限，不等于 Chat 协议无法表达 |
| 固定消费者 / Agent 场景 | 有固定 SDK 工具/reasoning 续轮 gates，未证明一般 Agent 兼容 | `tests/sdk/gateway.rs` 让固定 SDK 经同一 Router 完成双协议 JSON/SSE 工具与 reasoning 续轮；上游为 synthetic，显式 ignored，默认 Rust tests 不执行。范围见[开发指南](../development.md#固定-openai-sdk-loopback) |
| 缓存亲和性 | 有表示与保序基础，尚无执行亲和策略或命中效果验收 | CacheHints、schema order、origin-bound replay 已存在；缓存 scope 的执行绑定、跨轮/跨目标策略及真实 hit/成本/延迟效果不能由字段往返推出 |
| 执行库 / HTTP 接线 | 最小 loopback 服务已接通认证、固定入口、Provider transport 与增量 body；未生产验收 | `src/gateway/`、`src/transport/http.rs`、`tests/gateway.rs`；启动与错误边界见 [HTTP 指南](../http-gateway.md)。没有凭据池、自动 retry/fallback 或动态 registry |

### 使用判断

- 普通文本/refusal、结构化输出、function 工具续轮及已准入 reasoning 历史已有独立语义、字节和 synthetic HTTP/SDK 验证入口；具体请求仍须满足 Public Model、Endpoint 和所选 adapter 的合同。
- 同一 wire family 也走 IR；双协议兼容指可表示子集的映射，不是任意 Responses ↔ Chat 转换。Schema strict 默认、reasoning scope、reported facts 与响应投影必须同时成立，仅请求可编码不足以证明一次 exchange 可交付。
- 扩展类型存在、codec 可往返、实际请求主链接线和外部接受分别判断；未接线的 carrier 不算可用网关能力。实例启用与真实 Provider 可用性仍须现场核查，不由本页或 synthetic fixtures 推断。

## 已实现的纯文本基线

“已实现”指下列有限语义与独立测试存在，不表示所有标准组合或负例均验收完毕。精确 presence、默认值与拒绝合同仍只在各 profile 维护。

| 域 | 当前闭合范围 | 主要源码 / 独立测试 |
|---|---|---|
| Instructions / messages | 顶层 instructions、有序 system/developer/user/assistant、文本/refusal、instruction 非完成生命周期、assistant `phase` 与 status 分离 | `request.rs`、`responses.rs`、`chat.rs`；[instructions](../../tests/semantic/instructions.rs)、[phase](../../tests/semantic/phase.rs)、[text_profile](../../tests/semantic/text_profile.rs) |
| 输出控制 / Schema | sampling、输出上限、Responses verbosity/truncation、双协议 logprobs 控制；双协议 text/json_object/json_schema；schema 属性顺序、结构/strict/default、本地递归引用、独立预算、pattern/format 语法准入 | `settings.rs`、`schema.rs`、`pattern.rs`；[schema](../../tests/semantic/schema.rs)、[text_profile](../../tests/semantic/text_profile.rs) |
| Function / custom | 定义、choice（含 Chat function-only allowed_tools）、调用关联、双协议文本/文本数组结果、原始参数权威、调度字段表示、program/program_output 的 opaque 表示 | `tool.rs`、`function_tools.rs`、`responses.rs`；[tools](../../tests/semantic/tools.rs)、[function_events](../../tests/semantic/function_events.rs)；不执行工具或 program |
| Reasoning / replay | effort/context/mode、summary/readable text、owner/origin/finality 约束、有序 `configuration_update.reasoning.effort`、SDK `parsed`/`parsed_arguments` 验证后丢弃 | `reasoning.rs`、`fidelity.rs`；[reasoning](../../tests/semantic/reasoning.rs)、[parsed_replay](../../tests/semantic/parsed_replay.rs)；不是运行时 effective-settings 管理 |
| Response / events | identity、reported settings、annotations、content/refusal 概率的独立 owner、typed Usage 文本/prediction 细分、Chat content_filter 与其他已准入完成/非成功终态；有序 reducer 与 Static/Event 一致性；queued 事件生命周期 | `response.rs`、`event.rs`、`events/`；[response](../../tests/semantic/response.rs)、[usage](../../tests/semantic/usage.rs)、[text_events](../../tests/semantic/text_events.rs)、[text_profile](../../tests/semantic/text_profile.rs) |
| 标准 Context / delivery | 统一 CacheHints、双协议 metadata/service-tier request context、静态 reported context 与交付意图 | [context.rs](../../src/semantic/context.rs)、[request.rs](../../src/adapter/request.rs)；[text_profile](../../tests/semantic/text_profile.rs)、[chat_logprobs](../../tests/semantic/chat_logprobs.rs)；hint 不冒充 reported fact，缓存效果另验收 |
| 扩展 carrier（仅 codec 层） | request body sections 与独立 Codex headers 的有界 typed/opaque 表示及读写 | [extensions.rs](../../src/protocol/extensions.rs)、[extensions tests](../../tests/semantic/extensions.rs)；非空 body sections 被 adapter 请求准入拒绝，Codex headers 未接入 HTTP 请求主链，不管理 turn 或凭据 |
| 原始字节 / I/O | strict JSON、重复 key 拒绝、有界 SSE framing、UTF-8 分片、EOF/terminal/poisoning、padding 预算；synthetic body 的取消/背压 | `json.rs`、`sse.rs`、`chat_sse.rs`；[transport](../../tests/transport.rs)；测试 body 不是生产 ingress |

上表的短文件名按所属 semantic/protocol 域解释；实际模块布局由[当前架构](../architecture.md)维护。Schema 和派生 view 已有实质实现，不再列成需要重建的基础设施。

## 已闭合的正确性边界

以下是已准入分支的独立回归边界，**不是尚未修复的问题清单，也不是完整标准审计证明**。精确 wire/presence 规则留在 owning codec 与 profiles，不在本页重复字段表。

- **最终 IR 权威**：插入、替换、删除、重排驱动双向编码；失效的 annotations、probabilities、replay 和 classified extras 不恢复旧语义。入口见 [tools](../../tests/semantic/tools.rs)、[schema](../../tests/semantic/schema.rs)、[reasoning](../../tests/semantic/reasoning.rs) 和 [adapters](../../tests/semantic/adapters.rs)。
- **完整边界必填性**：snapshot、文本事件和完整 SSE 的 required 字段不由低层简写或默认空值补齐；规则及反例见 [Responses profile](../architecture-v2/responses-text-profile.md#complete-stream-required-fields)、[text admission](../../tests/semantic/text_admission.rs) 和 [Responses SSE](../../tests/transport/responses_sse.rs)。
- **角色与身份归属**：assistant-only history 字段不在其他角色上接受后丢失；nullable continuation 不替换已绑定 call identity；概率和静态/事件事实保持对应 owner。见 [text admission](../../tests/semantic/text_admission.rs) 与 [Chat probabilities](../../tests/semantic/chat_logprobs.rs)。
- **失败与交付**：解析拒绝后不可恢复成功，terminal 须等待严格 EOF；编码不算 commit，late failure 不补成功、不拼接另一 attempt。见 [execution chain](../../tests/transport/chain.rs) 与 [gateway body tests](../../src/gateway/body_tests.rs)。

[显式适配合同](../architecture-v2/decisions/0008-stable-core-and-vendor-adapters.md)把标准语义、厂商形状和兼容默认值分开；具名规则与拒绝理由归代码和 independent regressions，不维护厂商 wire 清单或实测结果。更广 union 的 required/null/跨 kind 与组合验收仍是以下审计缺口；新发现的错误接受应单独列出具体反例，不能用“字段未支持”或“测试通过”代替。

## 尚未映射的文本能力

区分未准入字段、缺少目标 wire 位置、未接线扩展和验收覆盖不足；它们不是同一种缺陷，也不意味着已有回归仍然失败。

| 域 | 具体缺口 | 对下一步的意义 |
|---|---|---|
| Chat 文本 API 覆盖 | `stop`、`seed`、presence/frequency penalties、`logit_bias`、`prediction` 等 create 控制及 history `name` 尚未准入；Responses verbosity/truncation 也无当前 Chat 投影 | 固定 SDK 字段与 `chat.rs::FIELDS` / role shells 的范围不同；这些是受限实现，不是标准不存在。按具体消费需求选定 owner、双向映射与独立反例，不无限追平字段清单 |
| 跨协议 / 静态事件投影 | Chat fingerprint、choice 概率-bearing history、Responses refusal 概率、`phase`、custom/program 和部分 reasoning 形状没有无损对端位置；Chat 静态 reported metadata 没有 chunk 槽位，响应多 part grouping 仍有限 | [lowering](../../src/lowering/generation.rs) 与 [Chat profile](../architecture-v2/chat-text-profile.md)拒绝不可表示目标，不靠丢字段强行兼容。Schema strict 缺省语义不同也可能拒绝，即便两个 shell 都能表达 Schema |
| Context 扩展接线 | `CustomSections` 能在低层 Responses envelope 读写，但 [adapter request](../../src/adapter/request.rs) 的准入和目标编码拒绝任何非空 sections；[CodexHeaders](../../src/protocol/extensions.rs) 是独立 carrier，未接入 Gateway/Provider HTTP 主链。response body 自定义段、typed observation headers、body/header 一致性、版本与 turn 管理仍未闭合 | [ADR 0007](../architecture-v2/decisions/0007-stateless-cache-affinity-and-extensions.md) 的设计和 carrier tests 不等于 scoped runtime；接线前须选定来源、生命周期、安全与目标合同 |
| 更广标准准入与验收 | 尚无固定 union 的完整逐分支 required/null/跨 kind 审计；并非全部 request/response/event 组合都有独立验收，更广 SDK/Agent 消费未验证 | 按已支持分支、具体反例与使用场景收敛；默认 Rust tests 不执行 SDK gates，SDK synthetic gate 也不能替代外部实际执行 |

## 明确边界与暂缓方向

- **Usage 投影边界**：文本/prediction 细分已由共享 Usage 持有并在 Chat JSON/SSE 原义读写；固定 Responses schema 没有相应槽位，存在这些事实（含零）时必须拒绝投影。既有 cached/cache-write/reasoning 跨协议映射不受影响；这不是建立 Provider Usage 或丢弃细分的理由。精确合同见 [Chat profile](../architecture-v2/chat-text-profile.md#reported-token-details)。
- **既定 profile 边界**：Chat 单候选（`n` 缺省/null/1）不是待补多候选；deprecated aliases 不是当然的补齐目标。`summary:false`、`response.cancelled` 是本地兼容形式，`text.format` / Chat `response_format` 显式 null 是文档化 local choice，不能宣称标准值。
- **Schema 责任边界**：[当前有限 profile](../architecture-v2/schema-profile.md)已验证结构/strict/default/refs/order/预算及 pattern/format 准入；不执行 regex/format、不做通用 JSON Schema 求值、不验证模型输出 adherence 或所有模型的限制。没有新的具体反例/消费需求，不以这些非目标制造“Schema 未完成”的无限待办。
- **Configuration / program 表示与执行分开**：固定 SDK 的 configuration update 只声明 reasoning effort，当前有序表示已存在；不虚构“effort 以外配置”作为既定标准缺口。是否计算 effective settings、调度 program、管理 Codex turn 属于独立执行设计。`prewarm` 也仅表示，未建立其 `generate` override 合同。
- **有状态 API 暂缓**：previous response/conversation/store/background、prompt、compaction/reference、retrieve/cancel 及 WS lane/steering 尚未实现；queued 事件已准入不等于静态 queued/in_progress body 或 state service 可用。当前只准入 inactive state 形式，编码显式 `store:false`。
- **其他语义域暂缓**：媒体双向映射、hosted/dynamic tools、独立 Embedding/Images/Speech 任务不是当前纯文本闭合的前置条件；基础 Resource 类型和 program item 表示不证明这些任务或执行已实现。
- **生产缺口独立存在**：最小认证 ingress、环境变量凭据绑定、实际 HTTP Provider I/O、body handoff/commit、取消与 deadline 已接通并通过 synthetic 验收。尚无多用户凭据池/OAuth、动态 registry、自动 retry/fallback、生产观测与负载/长稳证据；raw client replay token 的源头真实性仍由 issuer 验证，内部 scope 绑定不是来源证明。不能把最小本机服务称为生产就绪。

## 验收边界

当前判断应基于源码、独立 fixtures 及实际执行的相应检查。SDK gates 的存在不等于每次审阅都执行；具体一次检查结果在交付时报告，授权 probe 产物留在 ignored run，不在文档维护 Provider 结果或完成日记。完整 SDK/Agent、真实 Provider/TLS/网络、模型输出质量、负载与长期生产稳定性均需各自证据。

每个获准行为切片先记录[当前焦点](../implementation-plans/current-focus.md)，以独立 decode/encode、IR 修改/删除、Static/Event 与失败/资源反例验收。保持最终 typed 语义权威、候选不可变投影、extension 不覆盖标准/认证/目标、失败不变成功。推进顺序只由[下一步目标](../implementation-plans/next-goal.md)维护。
