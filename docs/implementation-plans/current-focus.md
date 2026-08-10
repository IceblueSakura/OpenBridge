# 当前开发焦点：统一 generation `instructions` 策略

## 状态

**规划已更新，尚未获准实施。** M1-M3 已完成并转入
[实施现状](../implementation-status/features/models-api-and-capability-preflight.md)：

- M1 `include: reasoning.encrypted_content`：提交 `64b15b5`；
- M2 `parallel_tool_calls: true`：提交 `64b15b5`；
- M3 `stream_options.include_usage`：提交 `121e39d`，2026-08-10 实机回归确认 Hermes `obc` 三个模型全部通过。

M4（Chat `json_schema`）已决定不纳入。MiMo 上游实测显示 `strict:true` 语义执行不可靠，Hermes 内部
session-title 请求也已由 Hermes 侧绕开。

本轮只重新定义 M5：它不再是给三个 Hermes 目标模型逐个开放 Responses `instructions`，而是建立一个适用于所有通用
generation Chat/Responses interface 的网关级指令策略。M6 `reasoning.summary` 仍是独立边界，不与 M5 混合实施或提交。
M5 的状态契约固定为无状态：`store` 省略时按 `false` 处理，显式值只支持 `false`；本焦点不设计或实现
`previous_response_id`。

## 背景与当前代码事实

Hermes 通过 `obr`（Responses）调用 `glm-5.2`、`deepseek-v4-flash`、`mimo-v2.5` 时仍返回 HTTP 400。当前剩余的
两个拒绝点互相独立：

| 阶段 | 请求字段 | 当前结果 | 已确认原因 |
|---|---|---|---|
| M5 | 顶层 `instructions` | `unsupported_model_capability` | 字段当前被建模为 model/interface parameter，但所有 canonical generation Model（包括 ChatGPT Model）都未声明它 |
| M6 | `reasoning: {effort: "medium", summary: "auto"}` | `unsupported_model_capability`（GLM Bridge） | GLM 的下游 Responses → 上游 Bailian Chat 转换只接受 `reasoning.effort`，拒绝未建模的 `summary` |

当前实现还存在以下不一致：

- Bootstrap 的 `chatgpt_instructions` 只在存在 active ChatGPT Target 时要求非空；
- ChatGPT adapter 在 Native 或 Chat→Responses Bridge 之后无条件覆盖上游 `instructions`，因此它是 Provider 专属强制值，
  不是“客户端优先、默认回落”；
- Chat→Responses converter 当前把所有 `system`/`developer` message 按原顺序保留为 Responses input message item，
  不生成顶层 `instructions`；
- Responses→Chat Bridge 当前因 `instructions.bridge_sources = NEITHER` 在转换前拒绝该字段，converter 本身也没有消费逻辑；
- OpenAPI 当前把 `instructions` 描述为 string 或 array，但运行时既没有完整形状校验，也没有实现 array item 的无损转换。

2026-08-10 的既有直连记录显示 Bailian、DeepSeek、OpenRouter、MiMo、LongCat、ChatGPT 的被测 `/responses` 请求接受
字符串 `instructions`。这只覆盖当时的精确模型、账户、端点、网络和请求形状；M5 扩大到统一策略后，必须重新审计每个实际固定候选，
不能从 Provider family 或 HTTP 200 外推所有 Model。

## 外部协议与参考实现结论

- OpenAI 官方 [Responses 迁移指南](https://developers.openai.com/api/docs/guides/migrate-to-responses) 将 system/developer guidance
  映射为顶层 `instructions`，也允许在需要保留既有 transcript 时继续使用兼容 message item；官方没有要求把任意第一条消息
  提升为 `instructions`。本项目的字段快照见 [Responses request 参考](../references/openai/responses/request.md)。
- 2026-08-11 直接复核 `F:/codespace/hermes-agent` 提交 `b3aa561faffd64f05436e429a6415d175e534ec9`：Hermes 的优先级是
  显式非空 `instructions` → 第一条 `system` message → Hermes 默认 identity；提升首条 `system` 后只删除该条，
  system-only 请求因此产生 `input: []`。
  它总是构造 `store:false`，preflight 接受空 input array、强制有效 `instructions` 非空，并且不把
  `previous_response_id` 列入可发送字段。历史项目快照见
  [Hermes Chat/Responses 分析](../references/hermes/hermes-chat-responses-analysis.md)。
- 当前 LiteLLM 会扫描并拼接所有纯文本 `system` message；这种做法会改变消息位置和边界，OpenBridge 不采用。项目快照见
  [LiteLLM Chat/Responses 分析](../references/litellm/litellm-chat-responses-analysis.md)。

因此，Chat→Responses Bridge **应当只把第一条满足条件的指令消息作为顶层 `instructions`**，而不是把“第一条消息”
无条件提升，也不扫描或拼接后续 system/developer message。

Hermes 是受信任的 agent 消费端，不是公共协议 validator。M5 只借鉴它的来源优先级、单次删除、非空默认值、system-only
空 input 和无状态请求形状，并明确保留以下 OpenBridge 差异：

| Hermes 当前处理 | OpenBridge M5 决定 |
|---|---|
| 缺失、空值或错误类型的 `instructions` 可被 truthiness/`str(...)` 归一化 | Responses 只接受非空 string；显式 null、空白或错误类型稳定 400 |
| 只提升首条 `system` | 同时支持首条 `system` 与 `developer`，但都必须可无损提升 |
| input converter 跳过其余 `system` message | 后续 system/developer 属于 transcript，保持原顺序和内容 |
| `request_overrides` 最后可覆盖已生成字段 | 不提供绕过 canonical request、状态边界或候选一致性的任意 body override |

## M5 需求与可观察行为

### 1. 指令来源与优先级

每个通用 generation 请求只解析一次有效指令来源，优先级固定为：

| 优先级 | 下游来源 | 目标行为 |
|---:|---|---|
| 1 | Responses 顶层 `instructions` 为非空、非纯空白 string | 使用客户端值，不追加、拼接或覆盖 |
| 2 | Chat `messages[0]` 是无损可提升的 `system` 或 `developer`，且 `content` 为非空、非纯空白 string | 将其视为客户端指令；Native Chat 保留原 role，Chat→Responses 映射为顶层 `instructions` |
| 3 | 客户端没有上述来源 | 使用 Bootstrap 的项目级 `default_instructions` |

边界规则：

- Responses 只有“字段省略”才触发默认回落；显式 `null`、空白 string、object、number 或 array 在 egress 前返回稳定 400，
  不能静默回落到默认值；
- M5 首期只实现 string `instructions`。OpenAPI 必须同步收窄为 string，array item 语义以后另立焦点；
- Responses `input` 中的 system/developer item 属于 transcript，不作为顶层客户端 override 来源，不被扫描、合并或删除；
- Chat 只有索引 0 的纯文本 `system`/`developer` 才可提升。第一条 `user`、`assistant`、`tool`，以及不能无损提升的
  复合内容都不视为 `instructions`；原消息必须保持顺序和内容；
- 后续 system/developer message 始终留在 transcript，不能与首条或默认值拼接；
- Chat→Responses 一旦提升首条合格指令，就从该 candidate 的 `input` 删除且只删除一次；同一文本不得同时出现在顶层
  `instructions` 与 input message 中；
- 如果提升唯一一条指令消息会产生空 `input`，仍必须把请求视为需要正确转发的 system-only/developer-only 请求：发送顶层
  `instructions` 与 `input: []`，不注入默认值、不保留重复指令，也不制造 user 内容；
- 空 `messages`、错误 role/content 形状继续按既有严格请求边界失败，不由默认指令掩盖。

### 2. 无状态请求边界

- M5 只承诺无状态请求。Responses 下游省略 `store` 等价于 `store:false`，显式 `store:false` 被接受；`store:true` 在任何
  Provider egress 前稳定失败，不能因某个 Native Target 当前接受而扩大统一契约；
- 每个上游 Responses candidate 都显式编码 `store:false`；Responses→Chat Bridge 消费该无状态事实但不向 Chat wire 伪造
  `store` 字段；
- M5 请求及其确定性/真实验收省略 `previous_response_id`。本焦点不新增该字段的继承、重发、issuer 解析、状态亲和、fallback、
  transcript 恢复或测试规则，也不借 instruction 改动扩大现有 continuation 能力；
- 客户端每次请求携带完整所需历史；有效指令也在该请求内独立解析，不能从未建模的服务端 response state 推断。

### 3. 项目级默认值

- 用 Bootstrap `default_instructions` 替换 `chatgpt_instructions`；未发布原型直接替换，不保留旧字段 alias、双写或弃用期；
- 只要启动编译结果包含至少一个通用 generation Chat/Responses interface，缺失、空或纯空白默认值就阻止启动；
  仅启用 Embeddings 或专用 speech/audio task 时不制造无关要求；
- `config/bootstrap.toml` 与 `config/bootstrap.example.toml` 都显式设置同一开发默认值，并在赋值前保留简洁英文运行效果注释；
- 默认指令会发送到所有适用的上游 generation Provider，配置、文档和日志示例不得包含 credential、用户数据或生产敏感内容。

### 4. 候选无关的统一编码

有效指令必须在固定候选展开前解析一次；同一请求的 Native、Bridge、retry 和 fallback candidate 使用同一段有效文本，
只按目标 wire protocol 编码：

| 上游协议 | 编码 |
|---|---|
| Responses | 顶层 `instructions` string 与显式 `store:false`；system-only/developer-only 边界使用空 `input` array |
| Chat Completions | `messages[0]` 的 `system` message；若客户端 Chat 首条本来是 `developer`/`system`，Native Chat 保留原 role |

具体转换规则：

- Responses→Responses Native：客户端值原样保留；缺失时写入默认值；
- Responses→Chat Bridge：把已解析的客户端值或默认值写成首条 `system` message，随后保持原 input item 顺序；
- Chat→Chat Native：首条合格客户端指令保持原样；否则在原 messages 前插入一条默认 `system` message；
- Chat→Responses Bridge：首条合格客户端指令或刚插入的默认 `system` 映射到顶层 `instructions` 并从 input 删除一次，
  其余 input 顺序不变；仅有该条消息时生成 `input: []`；
- candidate body 必须从同一不可变 canonical request 独立生成，不能让第一个候选的删除/插入影响后续 fallback；
- 同一请求解析出的有效指令文本在 Native、Bridge、retry 与 fallback attempt 间保持字节一致，不按 Provider 重新 trim、拼接或改写；
- 不得因指令来源或形状过滤、重排、跳过 candidate，也不得把转换失败改成隐式选择另一路由。

### 5. 能力与所有权

- 将 `instructions` 从 canonical Model 的 `InterfaceParameter` 重分类为网关拥有的 Responses envelope/指令策略；它不应要求
  每个 Model 重复声明，也不应作为 per-model `supported_parameters` 交集项；
- `/openbridge/v1/models` 继续只投影模型完整固定候选的 model capability；统一 `instructions` 行为由 OpenAPI、功能需求和
  generation endpoint 契约说明；
- generation analyzer 只校验并提取客户端指令事实，不读取注册表、不选择 Route；
- planning 在 Public Model 预检后、candidate 展开前使用启动校验过的默认值解析 canonical request；
- 双向 Bridge 只负责协议编码和 transcript 保序，不拥有默认配置或 Provider 判断；
- 移除 `ProviderRequestContext.chatgpt_instructions`、ChatGPT contextual body hook 及其 registry accessor。ChatGPT adapter 只保留
  OAuth/header、固定 Responses endpoint、input array、`stream:true`、`store:false` 等真实后端 envelope 规则；
- ingress 与 `openbridge-probe` 必须复用同一个受信的通用指令规范化入口，不能在 probe 或 ChatGPT 中重新复制 fallback 逻辑。

### 6. 适用范围

M5 覆盖当前和实施时实际暴露的所有通用 `GenerationModelProfile` Chat/Responses interface 及其完整固定候选，不能只修改
`glm-5.2`、`deepseek-v4-flash`、`mimo-v2.5`。实施前至少审计 ChatGPT、OpenAI、LongCat、DeepSeek、Bailian、OpenRouter、
NVIDIA、Kimi/Moonshot 与 MiMo 的精确 Target/Model/API 组合。

以下不属于 instruction-bearing 通用 generation surface：Embeddings、专用 ASR、TTS、voice design、voice clone、Realtime、
图片生成以及当前未公开的 canonical Model。M5 不向这些 task 注入默认 system message，也不改变它们的严格消息形状。

## M5 失败优先测试

M5 获准后先增加失败测试，再实现最小行为：

| 验证层 | 必须先失败的场景 |
|---|---|
| Bootstrap/config contract | 新字段替换旧字段；通用 generation active 时 missing/blank 失败；仅非通用 task active 时不误报；旧字段因 strict schema 被拒绝 |
| Request analysis | Responses 非空 string、字段省略、显式 null/blank/array/object、`store` 省略/false/true；Chat 首条 system/developer/user、后续 system、复合 content 与空 messages |
| Stateless contract | `store` 省略和 false 都规范化为上游 Responses `store:false`；true 在 egress 前失败；M5 fixture 均省略 `previous_response_id` |
| Chat→Responses conversion | 只提升首条合格 system/developer；从 input 删除一次；后续消息保序；首条 user 不提升；system-only/developer-only 生成 `input: []`，不重复指令或制造 user message |
| Responses→Chat conversion | 显式值和默认值各只产生一条首位 system message；原 input message/tool identity 顺序不变 |
| Planning/candidate contract | Native 与 Bridge、多个固定 candidate、retry/fallback 都携带同一有效文本；candidate 间无 body mutation 泄漏；Route 顺序不变 |
| Models/capability contract | `instructions` 不再依赖 canonical Model `supported_parameters`，不被完整候选交集误删，也不作为 per-model 参数投影 |
| Forwarding/provider contract | 普通 OpenAI-compatible Native JSON/SSE 的客户端优先和默认回落；ChatGPT 不再覆盖客户端值且仍保持其其他固定 envelope |
| Probe/startup contract | 通用 probe 使用相同默认策略；ChatGPT active 不再触发 Provider 专属 instructions 校验或上下文传递 |

聚焦测试应落在最接近所有者的现有测试中，包括但不限于：

- `tests/config_contract.rs`、`tests/example_config.rs`、`tests/startup_contract.rs`；
- `tests/native_routing_contract.rs`；
- `tests/bridge_conversion_contract.rs`、`tests/bridge_forwarding_contract.rs`；
- `tests/forwarding_contract.rs`、`tests/forwarding_contract/native.rs`、`tests/forwarding_contract/chatgpt.rs`；
- `tests/forwarding_contract/models.rs`、`tests/provider_boundary_contract.rs`。

## M5 获准后的实施顺序

1. 先增加上述失败测试，固定 string-only、优先级、system-only、`store:false`、候选一致性和专用 task 排除边界；
2. 替换 Bootstrap 字段及其 conditional startup validation，不保留旧字段兼容；
3. 增加 registry-independent 指令事实提取，并在 planning 的 canonical request 阶段统一解析默认值；
4. 完成 Responses→Chat 与 Chat→Responses 的双向编码，固定 `store:false`，并保持 tool/reasoning identity 和消息顺序；
5. 删除 ChatGPT 专属注入上下文与 hook，让 probe 复用通用规范化路径；
6. 审计全部通用 generation 固定候选，并更新 OpenAPI、README、功能需求、实现状态、配置示例和测试 fixture；
7. 运行聚焦测试后执行 Rust 基线；M5 完成并记录实证后清空当前焦点，M6 另行确认。

未来实施的基线命令：

```powershell
cargo fmt -- --check
cargo test --locked
cargo clippy --locked -- -D warnings
git diff --check
```

## M5 实现后的真实验收

确定性测试通过后，仍需经用户批准执行以下外部验收：

1. 用独立 curl 对每个 Native Responses 精确 Target/Model 分别发送显式 `instructions` 与省略字段请求，检查实际生效内容；
   所有请求显式使用 `store:false` 并省略 `previous_response_id`；
2. 对 Responses→Chat 和 Chat→Responses 的代表性真实候选复测客户端值、默认值、stream 与工具调用；
3. 对至少一个含多个固定候选的 Public Model 做受控 fallback，确认各 attempt 使用同一有效指令且 Route 顺序不变；
4. 对 ChatGPT Native 与 Chat Bridge 分别确认客户端值不再被覆盖，同时 `store:false`、stream、input array 和 OAuth header 不回归；
5. M5 先用不携带 `reasoning.summary` 的等价 Responses 请求复测 GLM 5.2、DeepSeek V4 Flash、MiMo V2.5；携带
   Hermes 原始 `summary:auto` 的普通对话、stream 与工具调用验收属于 M6；
6. 单独复测 Chat system-only 与 developer-only 的 `input: []` 边界；若真实 Responses Provider 不接受空 input，不得以重复
   instructions 或伪造 user input 绕过，应记录为该精确 Target/API 的 Bridge 不兼容证据并重新审视固定候选契约；
7. 查询 `/openbridge/v1/models`，确认 model capability 投影没有因统一 envelope 行为被虚假扩大。

## M6：`reasoning.summary` 后续独立边界

M6 不属于 M5。当前 GLM 路径是下游 Responses → 上游 Bailian Chat，`responses_to_chat` 只接受 `reasoning.effort`，
因对象中出现 `summary` 而 fail closed。Bailian 当前只给部分 Qwen Target 注册 Native Responses；GLM、DeepSeek 等非 Qwen
Bailian Target 仅有 Chat，不能把外部文档中其他地域、业务空间或端点的 Responses 能力外推到本项目固定实例。

2026-08-11 的精确路径实测补充了三类可观察行为：

- DeepSeek V4 Flash 与 MiMo V2.5 的被测 Native Responses 路径都接受 `summary:auto`，字段存在与否未改变被测响应；
- ChatGPT Native Responses 在携带 `summary:auto` 时返回 reasoning item 的非空 `summary`，stream 还增加
  `response.reasoning_summary_part.added`、`response.reasoning_summary_text.delta`、
  `response.reasoning_summary_text.done`、`response.reasoning_summary_part.done`；
- Native 接受但没有 summary 输出，不证明 Responses→Chat Bridge 可以生成 summary，也不要求把 Chat 明文 reasoning 改名为 summary。

OpenAI 官方 [Reasoning 指南](https://developers.openai.com/api/docs/guides/reasoning) 说明不同模型支持不同 summary 设置，
`auto` 选择该模型可用的最详细 summarizer，且只有显式请求才返回 summary。
OpenBridge 在此基础上定义一个更窄的 Hermes 兼容契约：`summary:auto` 是 best-effort 输出偏好；没有 summary 通道的 Chat
上游允许得到空 summary 结果。该降级是明确的 Bridge 行为，不是对任意未知字段的静默忽略。

若 M6 以后单独获准，应先用失败测试固定：

1. Native Responses 保留客户端 `summary:auto`，不得由 analyzer、planning、adapter 或 transport 删除或改写；
2. Responses→Chat Bridge 只消费精确字符串值 `summary:auto`，继续转发同一 `reasoning.effort`，生成的 Chat 请求中不出现
   `summary` 或伪造的替代字段；
3. Chat 的 `reasoning_content` 仍投影为 Responses reasoning item 的 `content[].reasoning_text` 与空 `summary`；stream 仍只生成
   `response.reasoning_text.delta/done`，不得伪造 reasoning summary item 或 summary SSE 事件；
4. `concise`、`detailed`、未知字符串、错误类型及其他未建模 reasoning 子字段继续 fail closed，不因支持 `auto` 而扩大；
5. Native ChatGPT 的 summary item 和上述四类 summary SSE 事件原样通过既有 Responses 校验与转发；完整事件生命周期必须包含
   `response.reasoning_summary_part.done` 的回归覆盖；
6. 保持明文 reasoning 投影、`reasoning.encrypted_content`、Route 顺序、retry/fallback 与固定 candidate 集的既有边界。

## 非目标

- 不把任意第一条 Chat message 当作 instructions，不扫描或合并后续 system/developer message；
- 不支持 Responses `instructions` array，不把多个 instruction item 扁平化为 string；
- 不建立 per-user、per-model、per-provider 或运行时热重载的默认指令；
- 不保留 ChatGPT 专属 instruction override、legacy Bootstrap alias 或兼容 shim；
- 不在 M5 中支持 `store:true`，也不设计或实现 `previous_response_id`、response storage/retrieval、conversation lifecycle 或
  其他服务端状态延续；
- 不向 Embeddings 或专用 audio/speech task 注入默认 system message；
- 不修改 Hermes 默认参数、retry 或 fallback 策略；
- 不改变 Route 排序、请求时 capability routing、candidate 过滤、Provider retry/cooldown 或 credential 体系；
- 不重新修改 M1-M4，也不在 M5 中顺带实现 M6；
- 不把默认指令写入 credential、私有用户文件或日志脱敏例外；
- 不作负载、长期运行、计费、质量或生产可用性承诺。

## 验证边界

- OpenAI 官方协议证明允许顶层 instructions 或兼容 message item，不证明每个第三方 Provider/Model 的当前实现；
- Hermes 与 LiteLLM 只提供设计对照，不自动成为 OpenBridge 契约；
- 2026-08-10 的直连结果是历史精确请求证据，实施后必须重跑目标 Target/Model；
- 2026-08-11 的 `reasoning.summary` 结果只覆盖上述精确 Native 路径与请求形状，不证明 Provider family、其他枚举、Bailian
  Qwen Responses 或 Responses→Chat Bridge 的相同行为；
- Rust 确定性测试只证明配置、分析、预检、候选生成、Bridge 和 wire 行为，不能替代真实 Provider、当前 SDK 或 Hermes runtime；
- fake/loopback fallback 可以证明 attempt 请求一致性，不能证明真实异构 Provider 的指令遵循质量；
- M5 的全部结论只适用于 `store:false` 且不携带 `previous_response_id` 的无状态请求；
- 未运行的真实 Provider、Hermes、外部 SDK、强制 fallback、负载和长期测试不得写成已验收。

## 待用户确认

1. 是否批准按上述 string-only、`store:false`、不涉及 `previous_response_id` 的通用 generation 全候选范围实施 M5？
2. M5 完成并关闭后，是否再为 M6 建立独立短周期焦点？
