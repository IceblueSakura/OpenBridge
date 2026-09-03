# 2026-09-02 Generation IR 协议转换语义覆盖静态审计

## 记录类型

本记录是对当前 checkout 的**静态源码审计**（证据层 1）：梳理 Generation Static/Event IR 的协议转换现状、已存在的语义映射挂钩点，以及 IR 对 stateless OpenAI Responses 语义的覆盖与缺口。它不是确定性测试执行、真实 Provider 探测或行为变更授权；实现细节与收窄理由由源码 `//!`/`///` 注释和测试拥有，本记录只保留审计结论与源码指针。

本记录严格区分两层能力：**生产 Bridge 路径**（`pipeline/generation/planning.rs` 实际调用的 `BridgePlan::prepare_with_request_facts` 及其下游）与**静态 codec/helper 能力**（`StaticBridgePlan`/`ToolPlan` 公开 API 中仅被测试或未来路径使用的部分）。凡标注"仅测试路径"的结论不表示生产可用。

## 审计边界

- 日期：2026-09-02，Asia/Shanghai。
- Checkout：commit `29d0b5e`（分支 `feature/generation-ir-rewrite`）。审计开始时工作树干净；本记录与 evidence 索引登记是其后新增的文档变更，不属于被审计源码。
- 工具：`git 2.55.0`；纯源码阅读，未编译、未运行测试、未发送请求。
- 配置形状：未读取任何私有配置；本审计不依赖也不描述运行时配置。
- 源码范围：`src/core/generation_parameter.rs`、`src/core/capability/generation.rs`、`src/bridge/static_codec*.rs`、`src/bridge/event_codec*.rs`、`src/ir/generation/`、`src/pipeline/generation/{planning,preflight}.rs`、`src/providers/openai_compatible/request.rs`、`src/registry/definition.rs`。
- "未覆盖"表示当前生产 codec 路径拒绝或忽略该语义；不代表已知不可实现，也不构成后续实施的获准范围。

## 请求路径关键事实

1. **reasoning level 归一发生在 IR 之前**：`preflight_public_model` 解析出 `normalized_reasoning_level`（`pipeline/generation/planning.rs`），`ReasoningLevelPolicy::resolve`（`registry/definition.rs`）按 `Strict`（必须在可执行档位集合内）或 `ClampPositiveFloor`（向下取整、低于集合时钳到最小档；`None` 独立处理不参与正向归一）解析；`normalize_reasoning_level` 改写 canonical body 的 `reasoning_effort` / `reasoning.effort`，所有候选共享同一份归一后的 body。
2. **每个固定候选的请求构建**：候选先经自身能力事实过滤（`discard_candidate_ignored_parameters`、`filter_candidate_response_includes`、`filter_candidate_prompt_cache_key`、`filter_candidate_serial_tool_control`），再通过生产路径 `BridgePlan::prepare_with_request_facts` 进入 `StaticBridgePlan::prepare_with_reasoning_output`：`validate_source` → `validate_bridge_source`（跨协议时的嵌套字段闭合白名单）→ `decode_request` → Native 只替换 `model` 绑定后重序列化（`request_changes` 为空）；跨协议走 `lower_request` 并产生根级 `SemanticChange{Normalized, ProtocolNormalized}`。Chat 跨协议方向在解码前先剥离 `stream_options`。
3. **准入开关是字段目录**：`GENERATION_REQUEST_FIELDS`（`core/generation_parameter.rs`）以 `bridge_sources` 标记每个顶层字段的 Bridge 方向准入；不可代表字段是否豁免由 `bridge_inactive` 按 `FieldRole` 决定（详见覆盖层 C）。`validate_source` 另有无条件拒绝项：Chat 的 `functions` 键、以及任何 `type` 非 `function` 的工具。
4. **`Unsupported` reasoning 门控不在生产请求路径**：`BridgePlan::prepare_with_reasoning_output` 中"候选 `ReasoningOutput::Unsupported` + 请求携带活跃 `reasoning`/`reasoning_effort` 即拒绝"的守卫只存在于该 helper（当前由合同测试使用）；生产调用的 `prepare_with_request_facts` 绕过它——`Unsupported` 候选同样会解码并把 effort 降级转发到目标协议。请求侧对不可读 reasoning 候选的边界由 preflight 的 interface reasoning support 承担（`route_reasoning_support`：bridged 路由仅当上游协议+`reasoning_output` 为可读组合时才保持 `Supported`），而非 codec 守卫。

## 响应路径关键事实

1. **静态路径**：`classify_generation_response` 选模式后，`render_non_stream` 解码为 `GenerationResponse`；同协议且 `preserves_source()` 时验证后原样回传上游字节；跨协议走 `lower_response` + `encode_response`。`lower_response` 硬性要求恰好 1 个 candidate 且 `Completed`；目标为 Responses 时 `Length`/`ContentFilter` finish 拒绝。
2. **Event IR 路径**：上游 SSE 帧经协议解码器映射为 `Vec<EventEnvelope>`（带严格递增 `Sequence`），纯 `reduce()` 逐事件验证序列、身份、生命周期、资源边界与 usage 单调性；编码器产出下游协议帧（同协议 `preserve_source` 时只验证不重编码）。`finish()` 要求协议解码器与 `reduce(Eof)` 双重 terminal，随后 `materialize()` 产出与非流式路径同型的 `GenerationResponse`（仅 `Completed` terminal 可物化）。
3. **候选级 `reasoning_output` 门控在响应侧**：`reasoning_output` 非 readable 时，上游响应中的 `reasoning_content`/reasoning item（含 `encrypted_content`）拒绝进入跨协议通道；readable 候选的 `encrypted_content` 解码为 `ReasoningPart::Opaque`（`OpaqueExposure::InternalOnly`）。`Summary` 候选在请求降级时被强制 `reasoning.summary="auto"`（effort≠none）。
4. **Opaque reasoning 存在一处方向不对称**：请求侧降级遇到 `ReasoningPart::Opaque` 一律拒绝（`encode_chat_input`/`encode_responses_input`）；但响应侧静态 `encode_chat_response` 对 `ReasoningPart::Opaque` 静默跳过、Chat 流式编码器对 Opaque delta 输出空字节。因此 Responses→Chat 响应转换中，readable 候选携带的 `encrypted_content` 会被静默丢弃，而非显式拒绝。

## 语义映射挂钩点现状

| 挂钩 | 位置 | 现状 | 审计机制 |
|---|---|---|---|
| reasoning level 档位归一 | `preflight` + `normalize_reasoning_level` | 生产运行中；请求级全局归一，IR 不表达同一请求对不同候选用不同档位 | 无（视为归一事实） |
| Provider wire 档位方言 | `registry::ReasoningLevelMapping` + `src/providers/openai_compatible/request.rs::apply_reasoning_level_mapping` | 生产运行中：IR 降级输出标准档位标签后，Provider adapter 按 Upstream API 注册的映射改写为目标 wire 值 | 无 |
| 候选级 reasoning 输出门控 | `prepare_with_reasoning_output`（请求侧守卫仅测试路径）/ `decode_response`（响应侧） | 响应侧生产运行中；请求侧守卫见请求事实 4 | 无 |
| 候选级字段剔除 | `planning.rs` 过滤函数 | 生产运行中 | 无（能力事实） |
| 工具域指令 | `ir/generation/tool_plan.rs` `Inject/Strip` | 机制存在，**生产 planner 未调用**（与 `current-boundaries.md` 一致） | `enforce_loss_policy(_, LossPolicy::Reject)` |
| Provider 原生工具降级 | `lower_provider_server_tool` + `ProviderToolProfile` | 仅经 `prepare_with_tool_plan`（测试/未来路径）；生产 `prepare_with_request_facts` 传 `None` profile，`web_search` 等 server 工具不可降级 | 同上 |
| 通用有损映射 | `ir/generation/fidelity.rs`（`SemanticChange`、`ChangeKind::{Normalized,Synthesized,OpaquePreserved,Emulated,Lossy}`、`LossPolicy::{Reject,Allow}`） | 机制存在，**仅 `prepare_with_tool_plan` 调用且固定 `Reject`；生产 codec 主路径未启用** | 机制本身 |

结构性缺口：`LossPolicy::Allow`/授权式有损映射未接入生产路径。现有**两处**不经显式拒绝、也不记 `Lossy` 变更的语义丢失点：①响应侧 Responses→Chat 转换对 `ReasoningPart::Opaque` 静默丢弃（响应事实 4）；②请求侧显式 `summary:false`（`Disabled`，值域与 `Omitted` 明确区分）降级时与 `Omitted` 同样序列化为省略（A 层），显式禁用语义跨协议后不再可辨。任何新的有损映射在接入生产前必须先决定其审计位置。

## IR 对 stateless Responses 语义的覆盖（按层）

### A. 生产 Bridge 方向完整 round-trip

- 输入：字符串简写；`message`（system/developer/user/assistant，**纯文本** `input_text` parts）；顶层 `instructions`；`reasoning` replay 的 readable parts（`reasoning_text` + `summary_text`；Opaque part 拒绝）；`function_call`/`function_call_output`（call_id 顺序与身份校验，Chat 方向合成确定性 `ItemId`）。
- 工具：function 工具（name/description/parameters/strict）、`tool_choice`（none/auto/required/specific）、`parallel_tool_calls`。
- 控制与约束：`max_output_tokens`、`temperature`、`top_p`；`text.format` 的 text / json_object / json_schema（name/description/schema/strict）。
- reasoning：`effort`（`Omitted/None/Minimal/Low/Medium/High/XHigh/Max`，OpenAI 词表超集）；`summary:"auto"`。**显式 `summary:false` 解码为 `Disabled`，但降级时与 `Omitted` 同样序列化为省略，跨协议后不再保留显式 false。**
- 状态：`prompt_cache_key`。
- 交付元数据（不进语义 IR）：`stream`、`service_tier`；Chat 的 `stream_options` 在跨协议方向被剥离，仅以 `chat_stream_usage` 事实影响 Chat 下游流式 usage 渲染。

### B. IR 值域已预留、生产 codec 未接线或拒绝

- `GenerationControls`：`candidate_count`（n）、`top_k`、`stop`、`seed`、`frequency_penalty`、`presence_penalty`。
- `RequestState`：`CacheDirective.retention`（`InMemory`/`Hours24`）未解码；`continuation`/`background` 固定 `None`/`false`。
- `OutputProjection.includes`：`include` 不进 IR，由候选级过滤处理。
- 多模态输入：`input_image`/`input_file` 可解码为 `ContentPart::Resource` 进入 IR，但跨协议降级经 `flatten_text`/`flatten_tool_output` 遇 Resource 即 `UnsupportedSemantics`——**可解码、不可跨协议降级**（与 `current-boundaries.md` "Bridge 不支持媒体转换"一致）；仅 Native 方向保留原始字节。
- `OpaqueState`：`OpaqueKind::{EncryptedContent, ThoughtSignature}` 值域已存在；**请求**回放解码遇非空 `encrypted_content` 直接拒绝；**响应**解码在 `reasoning_output` readable 时接受为 `Opaque(InternalOnly)`（见响应事实 4）。
- `InputItem::Extension(ProviderExtension)`：codec 拒绝。
- `web_search` 等 server 工具：仅经 `prepare_with_tool_plan` + `ProviderToolProfile` 可降级，生产路径不可达（见挂钩表）。

### C. 目录认识、Bridge 方向拒绝

`bridge_sources: NEITHER` 全集按豁免规则分两组（`bridge_inactive` 按 `FieldRole` 决定）：

- **任何值出现即拒绝**（`InterfaceParameter`/`Envelope` 角色无不活跃豁免，含 null）：`frequency_penalty`、`presence_penalty`、`seed`、`n`、`logprobs`、`top_logprobs`、`include_reasoning`、`logit_bias`、`min_p`、`top_k`、`top_a`、`repetition_penalty`、`stop`、`structured_outputs`、`metadata`、`user`、`safety_identifier`、`prompt_cache_options`、`moderation`、`modalities`、`audio`、`asr_options`（Chat）、`optimize_text_preview`（Chat）、`prediction`（Chat）、`web_search_options`（Chat）、`functions`（Chat，另被 `validate_source` 无条件拒绝）、`function_call`（Chat）、`verbosity`（Chat）、`conversation`（Responses）、`prompt`（Responses）、`context_management`（Responses）、`truncation`（Responses）、`max_tool_calls`（Responses）。
- **仅类型化不活跃值豁免**：`store=false`；`background=null|false`；`previous_response_id=null`；`include=null|[]`（Responses）；`prompt_cache_retention=null`；`stream_options={}`或仅`include_usage:false`（Chat）。

独立值域缺口：`reasoning.summary` 只接受 `"auto"` 与 `false`（`decode_reasoning`），`"concise"`/`"detailed"` 连 `ReasoningSummary` 枚举位置都没有。

### D. 有状态语义（按设计排除，列出仅为完整）

`store=true`、`previous_response_id≠null`、`background=true`、`conversation`、`prompt`（模板引用）——前两者之外均为任何值拒绝（见层 C）。

### E. 响应侧缺口

- `decode_responses_response` 仅接受 `message`（仅 `output_text`，annotations 非空即拒，故 `url_citation`/file citation 不可桥接）、`reasoning`（readable parts + readable 候选的 `encrypted_content`）、`function_call`，加 `status`/`usage`（usage 三项必填且算术一致）。
- 未覆盖输出 item：`web_search_call`、`file_search_call`、`code_interpreter_call`、`image_generation_call`、`local_shell_call`、`custom_tool_call`、`mcp_call`、`refusal` content part、`logprobs`。
- 跨协议降级要求恰好 1 个 candidate 且 `Completed`：`n>1` 与 `incomplete` 响应不可桥接。
- Responses→Chat 转换中 `ReasoningPart::Opaque` 静默丢弃（响应事实 4）。

## 汇总结论

1. 设计原则为 **Native 全字节保真、Bridge closed-set fail-closed**：绝大多数未覆盖语义显式拒绝而非静默丢弃；**已知两处例外**（均不拒绝、不记 `Lossy`）是响应侧 Responses→Chat 的 Opaque reasoning part 静默丢弃，与请求侧显式 `summary:false` 降级为省略。
2. 额外映射行为的基础设施已存在并在生产运行：请求级 effort 归一、Provider adapter 层 `ReasoningLevelMapping` 方言、候选级字段剔除、响应侧 `reasoning_output` 门控。实质缺口是 `LossPolicy::Allow`/授权式有损映射未接入生产路径，以及上述两处不记 `Lossy` 的静默丢失点。
3. "IR 覆盖全部 stateless Responses 语义"不成立；缺口按值域预留 > 接线 > 准入受控分布。最实质的几处：`summary: concise/detailed`（无值域位置）、`encrypted_content`（请求拒绝/响应有条件接受且降级不对称）、一组纯采样控制字段（有值域、双向未接线）、多模态输入（可解码、不可跨协议降级）。

## 不证明什么

- 本记录是静态源码审计，不替代确定性测试、corpus 验证、真实 Provider 或 SDK 兼容验收；字段准入的实际行为以源码与对应测试为准。
- 审计基于指定 commit；后续实现变化更新 `current-state.md` / `current-boundaries.md`，不改写本记录。
- 覆盖清单针对 stateless 语义；有状态域（store/continuation/background）的边界归产品非目标与 `current-boundaries.md` 拥有。
- "生产路径"的判定基于本次阅读到的调用链；若后续 `prepare_with_tool_plan` 或请求侧 `Unsupported` 守卫被接入生产 planner，本记录的"仅测试路径"结论即失效。
