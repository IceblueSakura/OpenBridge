# Responses：当前无状态 Generation profile

本页描述当前库级合同，不是完整标准符合性声明。标准目标归 [Semantic Model](semantic-ir.md#3-客户端-api-目标与扩展边界)；本地兼容形式仍需单独识别；[客户端边界](client-generation-profile.md)不提供独立私有 attachment。精确字段/预算/拒绝分支归 [OpenAI codecs](../../src/protocol/openai/mod.rs)、[lowering](../../src/lowering/generation.rs)和独立测试，HTTP 接线归[网关指南](../http-gateway.md)。

当前范围是有序文本/选定图片 history、文本/工具/reasoning 输出及适用事件。单个 profile 能解析不等于 Public Model/Endpoint 准入；未实现媒体、资源或 task 不由标准名称激活。标准来源和本地选择分开，见[固定基线](../references/responses-standard.md)。

## Response outcome and continuation

`GenerationResponse::outcome()` 表达一次 response 的 completed/incomplete/failed/cancelled，不表达逻辑 turn 结束或工具执行成功。合法 incomplete 保留部分产物及原因，不作为 fallback 理由。完成 response 仍可有待回应 call。

当前 `continuation()` 是从最终 items 借用派生的 pending-result view：call ID 与种类必须匹配后续 result，位置/名称不能替代关联；部分结果只解决其对应 call。历史编辑后重新计算，不保存另一份 pending 权威。无已知 pending 返回 `Unreported`，不是 end-of-turn 或发送许可；partial call/ongoing result 也不能证明就绪。

本地 `ResponseContinuation` 可检查调用方声明的后继关系、缺失/进行中结果与有限依赖；不验证真实 upstream turn identity、issuer、目标可表示性或执行权限。Program history 按当前 profile 要求配套 reported output 后才可 replay，不能由通用 pending view 放宽。

标准 codec 不从 finish label 或 call 数量补 `InteractionProgress`。显式 progress 保留在 typed IR，但当前公开目标没有 carrier，投影拒绝；不推断报告或授权自动 Agent loop。Owners：[continuation](../../src/semantic/task/generation/continuation.rs)、[turn](../../src/semantic/task/generation/turn.rs)、[progress](../../src/semantic/task/generation/progress.rs)。

## Message owners and cross-protocol grouping

当前标准 Responses 没有 `ToolCall.message` 的 carrier。Request/static lowering 拒绝显式 message-call membership，event 投影在 attached call 的 opening 拒绝；不能仅保留两个独立 item 就声称关系仍在。空的 tool-only message owner 也适用。当前 OpenBridge adapter 也不提供私有 wire-ID 关联。

独立 assistant message（包括空 owner）与独立 function call 按其身份/状态保留，不从邻接推断归属。Standalone-call run 的具名 Chat 投影不使反向 membership 自动获得标准位置。重排依赖 final typed identity，删除 owner 不能附到相同位置的新项。

有损 Chat 是[设计方向](protocol-and-lowering.md#semantic-loss)，不是此处现行拒绝的自动撤销；标准 Responses 也不继承 Chat 的损失许可。Owning checks：[group projection tests](../../tests/semantic/group_projection.rs)、[group view](../../src/semantic/task/generation/group.rs)。

## Reasoning replay authority

Reasoning 控制、可读内容与格式绑定的 opaque 值分别拥有权威。Opaque-only 内容合法性与回放条件分开；item-start 值为 partial，item-done 更新/移除最终值。Value 和 owner 必须满足该格式 finality，且 scope/依赖与目标兼容，才能作为 history 回放。

删除 typed 值不能从 fidelity 恢复，单删证明也不授权丢值继续。可读内容不是必要 signature 的替代品，某个 item 已完成不代表整个 response 成功。来源记录与编辑失效归 [source records](protocol-and-lowering.md#source-records)及 [ADR 0006](decisions/0006-reasoning-ownership.md)，不复制另一份 token。

## Tool image results

Responses function/custom result history 的标准 carrier 接受有序 `input_text` 与 URL/inline `input_image` parts。Text string、parts、空数组及单 part 不隐式互换。Call identity、kind、结果状态独立校验；structured result authority 与 execution report 仍无公开 carrier，目标投影拒绝，不能 stringify 后声称保留。

`tool_result_images` 准入独立于 user `image_input`；共用目标图片预算，不因此启用 Chat 工具图片、file ID、cache breakpoint 或输出图片事件。Owners：[tool results](../../src/semantic/task/generation/tool_result.rs)、[Responses codec](../../src/protocol/openai/responses.rs)、[independent oracles](../../tests/semantic/tool_results.rs)。固定 union 来源：[function result](https://github.com/openai/openai-python/blob/be9d66628ad7377bd36fe5a76ae6d735843f0e76/src/openai/types/responses/response_function_call_output_item_list_param.py)、[custom result](https://github.com/openai/openai-python/blob/be9d66628ad7377bd36fe5a76ae6d735843f0e76/src/openai/types/responses/response_custom_tool_call_output_param.py)。

## User image input

当前 user text/image 输入由 typed `Resource` 拥有顺序、URL/inline source 和 detail；编辑不恢复旧媒体。仅投影 HTTP(S) URL 与 Base64 image data URL。File ID 需要 issuer/resource 生命周期，instruction/assistant images 与生成图片不因此准入。Missing detail 与 explicit auto 区分，null 拒绝，目标不支持的 detail 不暗改。

Codec 不下载、重定向、OCR、解析像素、转码或上传；URL 语法合法不证明 Provider 获取安全或可用。库级、HTTP body 与 Endpoint 预算独立。Owners：[resource](../../src/semantic/task/generation/resource.rs)、[image codec](../../src/protocol/openai/image.rs)、[image tests](../../tests/semantic/images.rs)。

Reported image/text/audio token counts 不是从正文或图像大小估计的值。OpenBridge 对 image/text usage 的具名 carrier 属于非标准位置，普通 Responses 目标无对应位置时当前拒绝，包括显式零。计量别名/视图从最终 typed 报告再投影，不保存第二个 total；来源不足不补猜。精确规则归 [adaptation](../../src/protocol/adaptation.rs)、[image usage](../../tests/semantic/image_usage.rs)和[billing modality tests](../../tests/semantic/billing_modal_usage.rs)。

## Raw JSON admission

原始 JSON/SSE 字节入口使用共享严格 parser，拒绝重复键（包括 escape-equivalent keys）、非法 UTF-8/JSON 和尾随非空白，并在分配前限制 bytes、depth/nodes。Value API 不能证明原字节的这些性质；HTTP 收集另行有界。Raw tool argument string 不递归当 envelope 解析。

Unknown、错类型、跨 kind 字段与不合法 presence 不能静默省略。错误不回显输入；stream failure poisons state。所有精确配额以 [JSON parser](../../src/semantic/value/json.rs)、[protocol JSON boundary](../../src/protocol/openai/json.rs)和对应 tests 为准，而不是文档副本。

## Complete-stream required fields

完整 envelope 与 events 使用各自固定 schema，不能套用 request 简写或低层 snapshot 的宽松形状。身份、timestamp、required output/annotations/probability arrays、sequence 与 owner 必须逐分支满足；显式空值不等于缺字段。Named reported-fact normalization 与结构 requiredness 分开，不能补造整份 usage、身份或终态。

Added/delta/done 与最终 snapshot 必须一致；终态不能补缺失的必要事件或改写已经交付的值。Selected product profile 可有具名事件权威规则，但不能改变标准 grammar 或把 EOF 当作成功。Applicable padding 有独立预算，不能因耗尽预算静默关闭所请求的保护。

Framing、event 数/bytes、aggregate state、strict EOF 和真实 terminal 分别验证。终态前或终态后非法数据、缺分隔符、截断和取消均失败，late projection error 只能中止，不伪造成功或前移。Owners：[event codec](../../src/protocol/openai/events/mod.rs)、[SSE](../../src/protocol/openai/sse.rs)、[byte tests](../../tests/transport/responses_sse.rs)。

## Derived replay views

SDK `parsed`、`parsed_arguments` 和 `output_text` 是派生视图，不是正文权威。当前 history 仅按已声明规则验证 parsed views 与原 raw string 的一致性后丢弃；SDK 的 coercion、alias 或 filled defaults 不能冒充相同原值。替换正文不复活旧 parsed view。

上游 response/events 不因 SDK 有便利属性就允许同名未知字段；具名 Provider 规则仍独立。精确 replay 规则及 Chat 对应位置归 [parsed replay tests](../../tests/semantic/parsed_replay.rs)和 owning codec。

## Assistant phase labels

Assistant `phase` 是标准标签，独立于 item lifecycle；missing/null 表示未报告标签，不补默认。History/static/event 对同一 owner 保持一致，不能从 phase 推断终态或语义执行。当前 Chat 无 phase 投影，commentary 与 SDK parsed view 的组合按 owning validation 拒绝。Owners：[phase tests](../../tests/semantic/phase.rs)、[message type](../../src/semantic/task/generation/request.rs)。

## Schema property order

Schema 与工具参数/输出约束保留 authoritative JSON 的属性顺序，变换不能排序、填 required、删除约束或恢复 source。Schema 的结构、strict 默认、局部引用与 quota 归 [Schema profile](schema-profile.md)，request constraint 不补成 response fact。

Value 调用方负责入站前未丢顺序；普通 Value equality/round trip 不证明顺序保持。Shared text-format 意图与 Chat shell 可映射，但不是字节同构；function strict 默认不等价时当前拒绝，而不是归一到同一值。独立证据归 [schema tests](../../tests/semantic/schema.rs)。

## Control, message and annotation admission details

当前有几个不能与标准目标混同的边界：

- `reasoning.summary:false` 与 `response.cancelled` 是本地兼容形式，不是固定 SDK 的标准 summary 值或 SSE 事件名。
- `session_id` 是 OpenBridge body 扩展；identity/cache hints 有各自 owner，不从 key/user/token 派生 session，不透传 session headers，也不提供服务端会话。
- 仅 inactive state forms 准入，request `store` 投影为 false；活动 conversation、previous response、background、模板、moderation 和 compaction 尚未形成主链。
- Configuration/program/custom 分支按 owning codecs 明确准入，只表示有限语义；不是任意设置 patch、工具执行或脚本授权。Program history 需要匹配的 reported output，Chat 不自动支持它。
- `CustomSections`/`CodexHeaders` 是低层 carrier；非空 body sections 在 adapter 主链拒绝，headers 未因此接入 HTTP。未知字段不能通过它们旁路。
- 逐字段 required/null/empty、annotation/probability owner、引用范围与数量、non-complete instruction、reported context 的接受/拒绝由 owning codec 与[独立 admission tests](../../tests/semantic/text_admission.rs)维护，不把低层 convenience 放宽到完整 wire。

这些有效约束仍需在实现新切片时同步迁移。尚未完成的 union 审计、标准路径隔离、资源与媒体主链只归[实现缺口](../implementation-status/generation.md)，不在 profile 保存运行结果或完成度表。
