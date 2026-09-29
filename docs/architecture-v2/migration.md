# Generation 实施基线与差距

本页拥有当前完成度、具体缺口与证据入口；[semantic-ir](semantic-ir.md)拥有目标设计，[text profiles](responses-text-profile.md) / [Chat profile](chat-text-profile.md)拥有逐分支准入，[next-goal](../implementation-plans/next-goal.md)拥有优先级。标准依据为[固定上游版本](../references/upstream-sync.md)，不是全部最新 API 或真实 Provider 的能力声明。

## 完成度判断

**纯文本 Generation 已形成可修改、可重编码的离线语义主链，但尚未完成已准入边界的全部正确性验收，也不是可运行网关。** 不宜用一个百分比混合衡量语义表达、codec、消费者和执行四层。

这里的“纯文本”包括文本/refusal、结构化输出、function/custom 的文本调用与结果、reasoning 历史及相关控制/上下文，不仅是一个 prompt 返回一个字符串。它不自动包含所有“结果恰好是文本”的 hosted tools、状态资源或 program 执行。文本是验证[最终网关目标](README.md#产品目标与阶段判据)的阶段性载体：当前最大的产品缺口是尚未把这些机制接成网关整体流程，不是缺少一份更大的字段清单。

| 层级 | 当前判断 | 证据与不能推出的结论 |
|---|---|---|
| Task IR / 变换 / lowering | 已有完整主链，目标合同仍为受限子集 | `src/semantic/task/generation/`、`src/lowering/`；最终 IR 权威、候选独立投影与拒绝已有测试，不代表模型 capability 或 Provider selection 已接通 |
| Responses JSON/SSE | 常用无状态文本分支已实现，必填性已闭合但未审计全部标准分支 | `src/protocol/openai/`、`tests/semantic/`、`tests/transport/responses_sse.rs`；见下方缺口状态，不能称完整标准实现 |
| 单候选 Chat JSON/SSE | 已是同一 IR 的第二协议验证，不是待从零建立的 codec | `tests/transport/chat.rs`；部分标准文本 metadata、usage 与终态仍被拒绝，不等于 Chat 协议无法表达 |
| 固定消费者 / Agent 场景 | 已有 Responses/Chat 三轮 JSON/SSE synthetic gates，尚非网关全链 | `tests/sdk_loopback.rs`、`tests/sdk/` 的 handler 直接构造 fixture 回答，不经真实 Provider adapter；显式 ignored，默认 Rust tests 不执行，覆盖范围见[开发指南](../development.md#固定-openai-sdk-loopback) |
| 缓存亲和性 | 有表示与保序基础，尚无执行亲和策略或命中效果验收 | CacheHints、schema order、origin-bound replay 已存在；缓存 scope 的执行绑定、跨轮/跨目标策略及真实 hit/成本/延迟效果不能由字段往返推出 |
| 生产执行 | 最小执行链已接线（synthetic 全链 + 受控真实探测），仍非网关 | `src/provider/`、`src/topology/`、`src/execution/`、`tests/transport/chain.rs`、`examples/live_probe.rs`；无服务入口、credential 池、重试/自动 fallback 与生产提交链路，见[当前焦点](../implementation-plans/current-focus.md) |

## 已实现的纯文本基线

“已实现”指下列有限语义与独立测试存在，不表示所有标准组合或负例均验收完毕。精确 presence、默认值与拒绝合同仍只在各 profile 维护。

| 域 | 当前闭合范围 | 主要源码 / 独立测试 |
|---|---|---|
| Instructions / messages | 顶层 instructions、有序 system/developer/user/assistant、文本/refusal、instruction 非完成生命周期、assistant `phase` 与 status 分离 | `request.rs`、`responses.rs`、`chat.rs`；[instructions](../../tests/semantic/instructions.rs)、[phase](../../tests/semantic/phase.rs)、[text_profile](../../tests/semantic/text_profile.rs) |
| 输出控制 / Schema | sampling、输出上限、Responses verbosity/logprobs/truncation；双协议 text/json_object/json_schema；schema 属性顺序、结构/strict/default、本地递归引用、独立预算、pattern/format 语法准入 | `settings.rs`、`schema.rs`、`pattern.rs`；[schema](../../tests/semantic/schema.rs)、[text_profile](../../tests/semantic/text_profile.rs) |
| Function / custom | 定义、choice、调用关联、文本/文本数组结果、原始参数权威、调度字段表示、program/program_output 的 opaque 表示 | `tool.rs`、`function_tools.rs`、`responses.rs`；[tools](../../tests/semantic/tools.rs)、[function_events](../../tests/semantic/function_events.rs)；不执行工具或 program |
| Reasoning / replay | effort/context/mode、summary/readable text、owner/origin/finality 约束、有序 `configuration_update.reasoning.effort`、SDK `parsed`/`parsed_arguments` 验证后丢弃 | `reasoning.rs`、`fidelity.rs`；[reasoning](../../tests/semantic/reasoning.rs)、[parsed_replay](../../tests/semantic/parsed_replay.rs)；不是运行时 effective-settings 管理 |
| Response / events | identity、reported settings、annotations/logprobs、已准入 usage、完成/非成功终态；有序 reducer 与 Static/Event 一致性；queued 事件生命周期 | `response.rs`、`event.rs`、`events/`；[response](../../tests/semantic/response.rs)、[text_events](../../tests/semantic/text_events.rs)、[text_profile](../../tests/semantic/text_profile.rs) |
| Context / 扩展 | CacheHints（含 `prewarm`）、Responses execution hints；request body sections 与 Codex headers 的有界 typed/opaque 承载 | `envelope.rs`、`chat_envelope.rs`、`extensions.rs`；[extensions](../../tests/semantic/extensions.rs)、[text_profile](../../tests/semantic/text_profile.rs)；不管理 turn 或凭据 |
| 原始字节 / I/O | strict JSON、重复 key 拒绝、有界 SSE framing、UTF-8 分片、EOF/terminal/poisoning、padding 预算；synthetic body 的取消/背压 | `json.rs`、`sse.rs`、`chat_sse.rs`；[transport](../../tests/transport.rs)；测试 body 不是生产 ingress |

上表的短文件名按所属 semantic/protocol 域解释；实际模块布局由[当前架构](../architecture.md)维护。Schema 和派生 view 已有实质实现，不再列成需要重建的基础设施。

## 当前正确性缺口

真实 Provider 侧的上游 wire 差异已由[双轨](decisions/0008-stable-core-and-vendor-adapters.md)收敛：厂商适配 encode/decode 吸收 usage 缺省三态（两家无 cache-write 计费维度，缺席属预期）、回显缺省、`output_text` 派生 view、`content_filters` 等 classified 扩展、`reasoning_content`/`reasoning_text` 与 replay scope 绑定后，[2026-09-28 真实矩阵](../implementation-status/evidence/2026-09-28-deepseek-xiaomi-provider-live-matrix.md) 32/32 场景（双模型 × 双协议 × 双交付 × text/json_object/tool 含续轮）全部真实消费通过，核心 IR 未改。编码端永不填充缺省或伪造回显。固定 SDK 标准事件的 required/presence 与完整 Response snapshot 必填性已闭合：标准事件缺失 `sequence_number`、完整 snapshot 缺失 `output` 数组（含 queued/created/in_progress 初始 snapshot）在完整字节入口与低层 snapshot 分支都被拒绝，显式空数组仍然合法，拒绝后不能恢复为成功。规则与回归测试入口由 [Responses text profile](responses-text-profile.md#complete-stream-required-fields) 维护。

仍未声称所有事件的 required/null/跨 kind 检查均已审计：更广标准分支的 required/presence 按[尚未映射的文本能力](#尚未映射的文本能力)收敛，新反例按 owner 立项，不以“全部标准分支已审计”为前提。

## 尚未映射的文本能力

这些是当前显式拒绝或尚无合同的分支，与上面的错误接受分开。

| 域 | 具体缺口 | 对下一步的意义 |
|---|---|---|
| Chat usage | `prompt_tokens_details.cache_write_tokens`、text token 细分、prediction token 细分未映射；当前只接受 cached/reasoning 等有限 details | 固定 SDK 已有这些字段，尤其 cache-write 不是 Responses-only；IR 已有 `input_cache_write_tokens`，Chat lowering 尚未接通（`static_response.rs::usage` 编码侧已就绪） |
| Responses 厂商形状适配 | 已由[双轨厂商适配](decisions/0008-stable-core-and-vendor-adapters.md)收敛：回显与 usage 细分缺省按三态忠实保留、`output_text` 派生 view 校验后丢弃、`content_filters`/`frequency_penalty`/`presence_penalty` 有界 classified fidelity 同源保留、流式 `reasoning_text` part 与 encrypted replay 的同源 scope 绑定 | 编码端永不填充缺省或伪造回显；未知键仍拒绝。厂商 wire 再变化时新增观察，不改核心 IR |


| Chat 非成功终态 | `content_filter` 未映射；静态与流式仅准入 stop/tool_calls/length | 现有 `IncompleteReason::ContentFilter` 可作为语义起点；需同时验收 JSON、SSE、partial output、DONE 与跨协议投影，不把过滤伪装成 length/success |
| Chat 标准上下文 | `system_fingerprint` 已按标准 reported fact 准入（presence 保留、流内首值绑定）；`service_tier`、`metadata` 仍无 Chat 投影 | `chat_envelope.rs`、`static_response.rs`、`events/chat.rs` 闭合字段表按 [Chat profile](chat-text-profile.md) 演进；带未准入字段的 envelope 仍确定性拒绝，不能称通用兼容 |
| 其余 Chat 文本投影 | logprobs、其他生成控制、文本 content-array 等未准入；Responses `phase`、reasoning replay/custom/program 等也不能无损投影 | 前者按具体消费需求逐项立项；后者不能靠丢字段强行变成 Chat，也不以 Chat 限制反向缩减 Responses IR |
| Context 扩展 | response body 自定义段、typed observation headers、body/header 跨位置一致性、namespace 版本和 turn 管理模式尚未闭合 | [ADR 0007](decisions/0007-stateless-cache-affinity-and-extensions.md) 已定义 carrier，不等于 scoped runtime 已实现；继续扩张前须有具体来源和生命周期 |
| 更广标准准入 | 尚无固定 union 的完整逐分支验收；部分 required/presence、snapshot/event 组合仍需审查 | 按当前已支持分支及反例收敛，不以“全部标准事件”作为一个实现切片的退出条件 |

## 明确边界与暂缓方向

- **既定 profile 边界**：Chat 单候选（`n` 缺省/null/1）不是待补多候选；deprecated aliases 不是当然的补齐目标。`summary:false`、`response.cancelled` 是本地兼容形式，`text.format` / Chat `response_format` 显式 null 是文档化 local choice，不能宣称标准值。
- **Schema 责任边界**：[当前有限 profile](schema-profile.md)已验证结构/strict/default/refs/order/预算及 pattern/format 准入；不执行 regex/format、不做通用 JSON Schema 求值、不验证模型输出 adherence 或所有模型的限制。没有新的具体反例/消费需求，不以这些非目标制造“Schema 未完成”的无限待办。
- **Configuration / program 表示与执行分开**：固定 SDK 的 configuration update 只声明 reasoning effort，当前有序表示已存在；不虚构“effort 以外配置”作为既定标准缺口。是否计算 effective settings、调度 program、管理 Codex turn 属于独立执行设计。`prewarm` 也仅表示，未建立其 `generate` override 合同。
- **有状态 API 暂缓**：previous response/conversation/store/background、prompt、compaction/reference、retrieve/cancel 及 WS lane/steering 尚未实现；queued 事件已准入不等于静态 queued/in_progress body 或 state service 可用。当前只准入 inactive state 形式，编码显式 `store:false`。
- **其他语义域暂缓**：媒体双向映射、hosted/dynamic tools、独立 Embedding/Images/Speech 任务不是当前纯文本闭合的前置条件；基础 Resource 类型和 program item 表示不证明这些任务或执行已迁移。
- **生产缺口独立存在**：trusted topology、model binding、credential、Provider I/O、retry/fallback、cancel/commit、资源生命周期与 ingress 未接线。离线核心可分阶段验收后另开最小执行设计，不要求先完成未来所有标准分支，也不恢复旧目录当作实现。

## 验收边界

当前判断基于源码、独立 fixtures、默认离线 Rust tests 与定向变异反例。SDK gates 的存在不等于每次审阅都执行；具体一次检查结果在交付时报告，不在此维护测试数量或完成日记。完整 SDK/Agent、真实 Provider/TLS/网络、模型输出质量、负载与长期生产稳定性均需各自证据。

每个获准行为切片先记录[当前焦点](../implementation-plans/current-focus.md)，以独立 decode/encode、IR 修改/删除、Static/Event 与失败/资源反例验收。保持最终 typed 语义权威、候选不可变投影、extension 不覆盖标准/认证/目标、失败不变成功。推进顺序只由[下一步目标](../implementation-plans/next-goal.md)维护。
