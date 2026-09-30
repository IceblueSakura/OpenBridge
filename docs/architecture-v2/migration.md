# Generation 实施基线与差距

本页拥有当前完成度、具体缺口与证据入口；[semantic-ir](semantic-ir.md)拥有目标设计，[text profiles](responses-text-profile.md) / [Chat profile](chat-text-profile.md)拥有逐分支准入，[next-goal](../implementation-plans/next-goal.md)拥有优先级。标准依据为[固定上游版本](../references/upstream-sync.md)，不是全部最新 API 或真实 Provider 的能力声明。

## 完成度判断

**纯文本 Generation 已形成可修改、可重编码的语义主链，并接通最小认证 HTTP 网关与 synthetic SDK 全链；尚未完成全部标准分支和生产验收。** 不宜用一个百分比混合衡量语义表达、codec、消费者和执行四层。

这里的“纯文本”包括文本/refusal、结构化输出、function/custom 的文本调用与结果、reasoning 历史及相关控制/上下文，不仅是一个 prompt 返回一个字符串。它不自动包含所有“结果恰好是文本”的 hosted tools、状态资源或 program 执行。文本是验证[最终网关目标](README.md#产品目标与阶段判据)的阶段性载体：最小整体流程已接线；当前产品门槛转为新入口的受控外部验收、选定 Agent/缓存场景和必要的运行保障，而不是无限扩大字段清单。

| 层级 | 当前判断 | 证据与不能推出的结论 |
|---|---|---|
| Task IR / 变换 / lowering | 已有完整主链，目标合同仍为受限子集 | `src/semantic/task/generation/`、`src/lowering/`；最终 IR 权威、候选独立投影与拒绝已有测试；最小入口仅使用启动固定候选，不代表动态选择或任意模型 capability |
| Responses JSON/SSE | 常用无状态文本分支已实现，必填性已闭合但未审计全部标准分支 | `src/protocol/openai/`、`tests/semantic/`、`tests/transport/responses_sse.rs`；见下方缺口状态，不能称完整标准实现 |
| 单候选 Chat JSON/SSE | 已是同一 IR 的第二协议验证，不是待从零建立的 codec | `tests/transport/chat.rs`；部分标准文本 metadata、usage 与终态仍被拒绝，不等于 Chat 协议无法表达 |
| 固定消费者 / Agent 场景 | codec fixture gates 与实际 Gateway/HTTP Provider 全链 gate 并存 | `tests/sdk/gateway.rs` 让固定 SDK 经同一 Router 完成双协议 JSON/SSE 工具与 reasoning 续轮；上游为 synthetic，显式 ignored，默认 Rust tests 不执行。范围见[开发指南](../development.md#固定-openai-sdk-loopback) |
| 缓存亲和性 | 有表示与保序基础，尚无执行亲和策略或命中效果验收 | CacheHints、schema order、origin-bound replay 已存在；缓存 scope 的执行绑定、跨轮/跨目标策略及真实 hit/成本/延迟效果不能由字段往返推出 |
| 执行库 / HTTP 接线 | 最小 loopback 服务已接通认证、固定入口、Provider transport 与增量 body；未生产验收 | `src/gateway/`、`src/transport/http.rs`、`tests/gateway.rs`；启动与错误边界见 [HTTP 指南](../http-gateway.md)。[Flash 外部验收](../implementation-status/evidence/2026-09-29-flash-provider-adapter-acceptance.md)仍仅证明先前库级版本/场景，不证明新 binary；没有凭据池、自动 retry/fallback 或动态 registry |

## 已实现的纯文本基线

“已实现”指下列有限语义与独立测试存在，不表示所有标准组合或负例均验收完毕。精确 presence、默认值与拒绝合同仍只在各 profile 维护。

| 域 | 当前闭合范围 | 主要源码 / 独立测试 |
|---|---|---|
| Instructions / messages | 顶层 instructions、有序 system/developer/user/assistant、文本/refusal、instruction 非完成生命周期、assistant `phase` 与 status 分离 | `request.rs`、`responses.rs`、`chat.rs`；[instructions](../../tests/semantic/instructions.rs)、[phase](../../tests/semantic/phase.rs)、[text_profile](../../tests/semantic/text_profile.rs) |
| 输出控制 / Schema | sampling、输出上限、Responses verbosity/logprobs/truncation；双协议 text/json_object/json_schema；schema 属性顺序、结构/strict/default、本地递归引用、独立预算、pattern/format 语法准入 | `settings.rs`、`schema.rs`、`pattern.rs`；[schema](../../tests/semantic/schema.rs)、[text_profile](../../tests/semantic/text_profile.rs) |
| Function / custom | 定义、choice、调用关联、文本/文本数组结果、原始参数权威、调度字段表示、program/program_output 的 opaque 表示 | `tool.rs`、`function_tools.rs`、`responses.rs`；[tools](../../tests/semantic/tools.rs)、[function_events](../../tests/semantic/function_events.rs)；不执行工具或 program |
| Reasoning / replay | effort/context/mode、summary/readable text、owner/origin/finality 约束、有序 `configuration_update.reasoning.effort`、SDK `parsed`/`parsed_arguments` 验证后丢弃 | `reasoning.rs`、`fidelity.rs`；[reasoning](../../tests/semantic/reasoning.rs)、[parsed_replay](../../tests/semantic/parsed_replay.rs)；不是运行时 effective-settings 管理 |
| Response / events | identity、reported settings、annotations/logprobs、已准入 usage、完成/非成功终态；有序 reducer 与 Static/Event 一致性；queued 事件生命周期 | `response.rs`、`event.rs`、`events/`；[response](../../tests/semantic/response.rs)、[text_events](../../tests/semantic/text_events.rs)、[text_profile](../../tests/semantic/text_profile.rs) |
| Context / 扩展 | 统一 CacheHints（含 `prewarm`）、execution/reported context、delivery；request body sections 与 Codex headers 的有界 typed/opaque 承载 | `semantic/context.rs`、`adapter/request.rs`、`extensions.rs`；[extensions](../../tests/semantic/extensions.rs)、[text_profile](../../tests/semantic/text_profile.rs)；不管理 turn 或凭据 |
| 原始字节 / I/O | strict JSON、重复 key 拒绝、有界 SSE framing、UTF-8 分片、EOF/terminal/poisoning、padding 预算；synthetic body 的取消/背压 | `json.rs`、`sse.rs`、`chat_sse.rs`；[transport](../../tests/transport.rs)；测试 body 不是生产 ingress |

上表的短文件名按所属 semantic/protocol 域解释；实际模块布局由[当前架构](../architecture.md)维护。Schema 和派生 view 已有实质实现，不再列成需要重建的基础设施。

## 当前正确性缺口

当前[显式适配合同](decisions/0008-stable-core-and-vendor-adapters.md)把标准语义、厂商形状和兼容默认值分开。DeepSeek 有效 usage 缺失/null cache-write 时归一为 0，并保留 intake 归一化记录；标准/client/Xiaomi 不继承该默认，缺失整个 usage 不补造。extras 绑定来源/协议/方言/语义依赖，仅终态为权威；JSON/SSE 使用同一投影。现有验收见 `tests/semantic/adapters.rs`、`tests/transport/chain.rs`。[历史 MiMo Pro 矩阵](../implementation-status/evidence/2026-09-28-deepseek-xiaomi-provider-live-matrix.md)仅证明其当时实现；[重构后的 Flash 验收](../implementation-status/evidence/2026-09-29-flash-provider-adapter-acceptance.md)覆盖固定双协议文本与工具续轮，并保留 MiMo 单次 JSON 输出格式异常，未据后续复测成功将其宣称为已修复。codec 成功不等于模型输出 adherence，真实样本也不替代完整标准分支或 SDK 验收。固定 SDK 标准事件的 required/presence 与完整 Response snapshot 必填性已闭合：标准事件缺失 `sequence_number`、完整 snapshot 缺失 `output` 数组（含 queued/created/in_progress 初始 snapshot）在完整字节入口与低层 snapshot 分支都被拒绝，显式空数组仍然合法，拒绝后不能恢复为成功。规则与回归测试入口由 [Responses text profile](responses-text-profile.md#complete-stream-required-fields) 维护。

仍未声称所有事件的 required/null/跨 kind 检查均已审计：更广标准分支的 required/presence 按[尚未映射的文本能力](#尚未映射的文本能力)收敛，新反例按 owner 立项，不以“全部标准分支已审计”为前提。

OpenRouter 的固定 `gpt-6-luna` → `openai/gpt-6-luna` 已接入环境变量 bootstrap、双协议目标和 JSON/SSE adapter。根据 live 差异补齐了限定格式的 Chat summary/encrypted reasoning、标准 service-tier 回显、零值媒体计数归一化、Responses 格式/费用字段和尾随 DONE；准入与拒绝边界由 [OpenRouter text adapter](openrouter-text-profile.md)维护，执行范围见[带日期验收](../implementation-status/evidence/2026-09-29-openrouter-luna-acceptance.md)。[加密 reasoning 专项验收](../implementation-status/evidence/2026-09-29-reasoning-continuation-acceptance.md)另行覆盖了实际密文获取与同目标续轮，并保护了 partial/final、owner 与 response 各自生命周期；readable-only reasoning 不要求密文或 opaque origin。这不恢复历史 GLM 绑定，也不表示任意 OpenRouter backend/媒体/工具均兼容。

额外五家 API-key Provider 已有固定 Chat 绑定与具名 adapter 合同，见 [API-key text profiles](api-key-text-profiles.md)。Provider 的 native Responses entry 现可缺省，未准入的协议不能编译成候选或由 bootstrap 自动开放。接入实测见 [onboarding evidence](../implementation-status/evidence/2026-09-29-api-key-provider-onboarding.md)；[定向跟进](../implementation-status/evidence/2026-09-29-provider-followup.md)确认了百炼 `invalid_api_key`、Kimi 余额相关 429，以及 MiMo none 下已存在于交付 wire 的大小写偏差。NVIDIA 的[后续边界探测](../implementation-status/evidence/2026-09-30-nvidia-boundaries.md)保留了新观察到的工具续轮 502，仍未定位原调用的失败层；默认 JSON object 在预算内截断，显式 none 对照通过，不据此静默改变请求或收缩能力。Kimi 真实测试现暂停；测试中继认证已增加真实错误 token 负例。接线完成不等于八家全部 live 验收通过。

## 尚未映射的文本能力

这些是当前显式拒绝或尚无合同的分支，与上面的错误接受分开。

| 域 | 具体缺口 | 对下一步的意义 |
|---|---|---|
| Chat usage | text token 与 prediction token 细分尚未映射 | cached/cache-write/reasoning 的双协议 JSON/SSE 映射已有；新细分按真实语义归属补齐，不另建 Provider Usage |
| Chat 非成功终态 | `content_filter` 未映射；静态与流式仅准入 stop/tool_calls/length | 现有 `IncompleteReason::ContentFilter` 可作为语义起点；需同时验收 JSON、SSE、partial output、DONE 与跨协议投影，不把过滤伪装成 length/success |
| Chat 标准上下文 | `system_fingerprint`、响应 `service_tier` 已按标准 reported fact 准入（presence 保留、Chat 流内首值绑定）；请求 service-tier、metadata 仍无 Chat 投影 | `chat_envelope.rs`、`static_response.rs`、`events/chat.rs` 闭合字段表按 [Chat profile](chat-text-profile.md) 演进；带未准入字段的 envelope 仍确定性拒绝，不能称通用兼容 |
| 其余 Chat 文本投影 | logprobs、其他生成控制等未准入；请求中的非空有序纯文本 content-array 已准入，响应与媒体数组未扩张；Responses `phase`、custom/program 等也不能无损投影；reasoning replay 仅在显式限定格式的 Chat 扩展中映射 | 前者按具体消费需求逐项立项；后者不能靠丢字段强行变成 Chat，也不以 Chat 限制反向缩减 Responses IR |
| Context 扩展 | response body 自定义段、typed observation headers、body/header 跨位置一致性、namespace 版本和 turn 管理模式尚未闭合 | [ADR 0007](decisions/0007-stateless-cache-affinity-and-extensions.md) 已定义 carrier，不等于 scoped runtime 已实现；继续扩张前须有具体来源和生命周期 |
| 更广标准准入 | 尚无固定 union 的完整逐分支验收；部分 required/presence、snapshot/event 组合仍需审查 | 按当前已支持分支及反例收敛，不以“全部标准事件”作为一个实现切片的退出条件 |

## 明确边界与暂缓方向

- **既定 profile 边界**：Chat 单候选（`n` 缺省/null/1）不是待补多候选；deprecated aliases 不是当然的补齐目标。`summary:false`、`response.cancelled` 是本地兼容形式，`text.format` / Chat `response_format` 显式 null 是文档化 local choice，不能宣称标准值。
- **Schema 责任边界**：[当前有限 profile](schema-profile.md)已验证结构/strict/default/refs/order/预算及 pattern/format 准入；不执行 regex/format、不做通用 JSON Schema 求值、不验证模型输出 adherence 或所有模型的限制。没有新的具体反例/消费需求，不以这些非目标制造“Schema 未完成”的无限待办。
- **Configuration / program 表示与执行分开**：固定 SDK 的 configuration update 只声明 reasoning effort，当前有序表示已存在；不虚构“effort 以外配置”作为既定标准缺口。是否计算 effective settings、调度 program、管理 Codex turn 属于独立执行设计。`prewarm` 也仅表示，未建立其 `generate` override 合同。
- **有状态 API 暂缓**：previous response/conversation/store/background、prompt、compaction/reference、retrieve/cancel 及 WS lane/steering 尚未实现；queued 事件已准入不等于静态 queued/in_progress body 或 state service 可用。当前只准入 inactive state 形式，编码显式 `store:false`。
- **其他语义域暂缓**：媒体双向映射、hosted/dynamic tools、独立 Embedding/Images/Speech 任务不是当前纯文本闭合的前置条件；基础 Resource 类型和 program item 表示不证明这些任务或执行已迁移。
- **生产缺口独立存在**：最小认证 ingress、环境变量凭据绑定、实际 HTTP Provider I/O、body handoff/commit、取消与 deadline 已接通并通过 synthetic 验收。尚无多用户凭据池/OAuth、动态 registry、自动 retry/fallback、生产观测与负载/长稳证据；raw client replay token 的源头真实性仍由 issuer 验证，内部 scope 绑定不是来源证明。不能把最小本机服务称为生产就绪。

## 验收边界

当前判断基于源码、独立 fixtures、默认离线 Rust tests、定向变异反例及显式 synthetic SDK/HTTP 全链；外部 live 证据按其固定版本分别解释。SDK gates 的存在不等于每次审阅都执行；具体一次检查结果在交付时报告，不在此维护测试数量或完成日记。完整 SDK/Agent、真实 Provider/TLS/网络、模型输出质量、负载与长期生产稳定性均需各自证据。

每个获准行为切片先记录[当前焦点](../implementation-plans/current-focus.md)，以独立 decode/encode、IR 修改/删除、Static/Event 与失败/资源反例验收。保持最终 typed 语义权威、候选不可变投影、extension 不覆盖标准/认证/目标、失败不变成功。推进顺序只由[下一步目标](../implementation-plans/next-goal.md)维护。
