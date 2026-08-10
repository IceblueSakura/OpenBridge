# 功能：Models 接口、Public Model 契约与能力预检

## 状态

**已完成（当前 checkout）。** 标准 Models、扩展 Models 和请求预检共享同一启动期编译的 Public Model execution interface；客户端看到的是
固定公共契约，而不是某条 Route 的能力上限。

## 已完成内容

- `GET /v1/models` 与 `GET /v1/models/{model}` 提供 OpenAI 标准四字段模型对象；扩展 Models 提供 operation、输入/输出 modality、reasoning、
  state、独立的 `streaming`/`non_streaming` 支持状态、typed `multimodal_input` 和 `supported_parameters` 等下游安全事实。
- Canonical Model 使用必填 `Generation | Embedding | SpeechRecognition | SpeechSynthesis | VoiceDesign | VoiceClone` task union；
  `ModelInfo` 保存同构 owned union，context/modalities/parameters/reasoning 不再有独立 shadow fields 或第二个 task tag。
  扩展 Models 的 task 总映射固定为 Generation → `chat,text_generation`、Embedding → `embedding`，其余四个专用 task 分别投影
  `speech_recognition`、`speech_synthesis`、`voice_design`、`voice_clone`。
- Generation reasoning 由 `ReasoningProfile` 单源保存，普通 canonical parameters 不再包含 `reasoning`/`reasoning_effort`；
  interface compiler 按目标 downstream protocol 派生对应 reasoning wire parameter。
- Public Model 另以 `Strict | ClampPositiveFloor` 保存 reasoning input policy。当前通用文本 generation 注册使用
  `ClampPositiveFloor`，在 `minimal < low < medium < high < xhigh < max` 中 floor 到不高于请求值的最高实际档位，并把低于最小档的
  输入 clamp 到最小档；四个 MiMo 音频专用模型与 Embeddings 保持 `Strict`。`none` 独立于正向序列，只有实际接口包含它时才接受。
- 扩展 Models 的 interface reasoning 同时投影实际可执行 `levels`、下游 `accepted_levels` 与 `input_policy`；标准 Models 四字段对象
  未改变。Registry 拒绝在非 generation Public Model 上配置正向档位归一化。
- 每个 Public Model 的 Chat、Responses、Embeddings interface 在启动期按所有可执行 candidate 的保守交集编译；未知事实保持未知，不被
  猜测为支持。
- Chat/Responses analyzer 先用同一份协议级类型化顶层字段目录分类请求。目录外字段（包括值为 `null` 的字段）在 Native/Bridge
  规划前返回 `unknown_parameter` 与精确 `param`；目录内但不属于固定 interface 的字段返回
  `unsupported_model_capability`，两类拒绝都不会调用 Provider。
- generation `supported_parameters` 表示 OpenBridge 接受对应字段。每条 Route 只能以“当前 API 转发”或“当前 API 明确忽略”贡献
  参数；Bridge 还要求该转换方向可完整表示字段。固定 interface 继续按全部候选相交，不按请求参数筛选或重排 Route。
- Responses `include` 由 `ResponseInclude` 精确 wire 枚举解析为请求集合；Route contribution 保存逐值集合，Public Model 对全部固定
  candidate 求交集并以 `response_includes` 公开。公开值表示接口能够接受并安全处理该条件性请求，不保证对应 output item 存在或
  reasoning 形态发生变化。`reasoning.encrypted_content` 当前在 `glm-5.2` Responses、`deepseek-v4-flash` Responses、
  `mimo-v2.5` Responses 和仅由 ChatGPT 提供的 Responses Public Model 上进入交集；Native 原样转发，GLM 的 Responses→Chat Bridge
  显式消费且不伪造 opaque item。`include: []` 在 candidate 展开前移除，已知但不在交集中的值和未知 wire 值都在 egress 前失败关闭。
- 旧的 `prompt_caching: SupportState` 已删除。`prompt_cache_key` 现在是独立 request option：Provider/Target 声明只表示 exact
  forwarding，编译后仅通过 `supported_parameters` 公开，不表示 cache hit、延迟或成本效果。Bridge 只有在目标 Upstream API 明确支持且
  converter 原样复制时才贡献该参数；options、retention 与 breakpoint 仍保持未实现。
- 每 Upstream API 的闭合普通参数忽略集合不从该 interface 删除字段，但启动时校验 canonical 声明、唯一性、与 disabled 参数互斥及
  generation-only 边界。当前只允许 `frequency_penalty`、`presence_penalty`、`temperature`、`top_p`、`seed`；输出数量、结构或
  reasoning 可见性字段仍按固定契约 fail closed。generation canonical parameter 还必须存在于同一类型化目录，防止配置任意字符串。
- generation interface 使用 typed function-tool profile，以及
  `JsonObject | JsonSchema(JsonSchemaSupport) | JsonObjectAndJsonSchema(JsonSchemaSupport)` 闭合 Structured Output profile。
  Provider/Target、Route contribution、Public execution interface 与 preflight 直接共享这一 profile；候选没有共同 mode 时结果为 `None`，
  同时删除 `response_format`、`text` 和 `structured_outputs` 参数。Models 的 `support/modes/strict_schema` 只在序列化时投影，
  不保存三份 shadow state。
- 请求 analyzer 冻结精确的 function `tool_choice`，并用
  `Unconstrained | JsonObject | JsonSchema(NonStrict | Strict) | Unknown` 保存 Structured Output request facts；同 mode 合并且 strict
  取并，冲突/未知 fail closed。preflight 直接对 request union 与同一 fixed profile 做双 enum match。function tool 未显式指定
  `tool_choice` 时按协议默认的 `auto` 进行预检。
- Native image profile 使用 checked envelope：正数 `max_parts` 加
  `RemoteUrl | DataUrl | RemoteUrlAndDataUrl` source-payload union。Remote payload 只保存正数 URL limit；Data payload 只保存非空唯一
  media type set 和完整 inline encoded/decoded 单项及累计预算，不再保存可独立漂移的 source slice、media slice 与六个 flat limit。
  Remote limit 至少能容纳 9-byte `https://a`，inline per-item limit 至少能容纳 4-byte Base64 quantum 与 1-byte decoded payload；累计预算还在
  outer constructor 内校验可由 `max_parts` 达到。
- Image detail 使用 `OmittedOnly { default } | Explicit(profile)` 判别联合；省略行为与显式 allowed domain 独立，explicit default 不要求是
  allowed member。Public Model 编译要求共同 default，逐 source 相交；Data MIME 空交集只移除 Data source，Both 可降为 Remote-only。
  `max_parts`、单项和累计预算取保守最小后，以 checked `u64` 将累计 encoded/decoded clamp 到 `per-item × max_parts`，再通过同一 owned
  constructor 重验。
- Models 的 image 对象继续输出原 flat key/shape，但不适用字段的 `0` 只在只读 DTO projection 生成：Remote-only 投影空 media type 和
  四个 inline `0`，Data-only 投影 URL `0`。preflight 只读取 source-specific owned union，不读取 DTO。`file_id` 只保留为 Responses
  analyzer fact，在缺少 resource ownership/affinity profile 时 fail closed；Bridge 固定不贡献 image source。
- Chat executable capability 只保存零或一个 concrete audio profile；输入、输出和 voice conditioning 由该 variant 派生。Provider
  audio ceiling 是非空、task 不重复的完整 profile 集合，与单个 executable profile 使用不同静态类型；presence 和完整 payload
  都只能由这两个闭合类型表达。
- Registry 在 snapshot 构造前先做 Provider ceiling containment，再校验 operation/concrete audio profile 与 canonical task：
  专用 task 只接受同名 profile，Embedding 只接受 Embeddings，Responses 只接受 Generation；Generation AudioUnderstanding 还要求
  canonical Audio input 与 Text output 都有明确证据。Public Model 跨 operation 混合 task，或同 task/same audio variant 的 payload
  交集为空，也会以 typed error 拒绝启动。
- MiMo 四个音频专用 target 将 Provider-wide function-tool ceiling 收窄为 `None`；扩展 Models 公开 tools `unsupported`，并在 egress
  前拒绝带 function tool 的合法音频 task。通用 `mimo-v2.5` 与 Pro 的工具契约不受影响。
- 请求先解析 operation-specific requirements，再对选定 Public Model 做一次能力、限制和 private continuation contract preflight；
  不支持的请求在任何 Provider egress 前以稳定本地错误拒绝，preflight 不回查 Target capability。
- preflight 只返回需要变化的有效 reasoning level；planning 在静态 candidate 展开前改写一次 canonical body，随后 Native、Bridge 与
  全部 fallback candidate 共享同一结果。Provider-specific wire mapping 仍只在 candidate egress 阶段执行。
- Chat audio analyzer 只冻结 `RequestedAudio::Input | Generated` 任务无关结构，以及有界 source/format/size 与
  `InputAudioMessageShape`/`GeneratedAudioMessageShape`，不猜 ASR/TTS/VoiceDesign/VoiceClone。preflight 取得已编译 audio interface 后才
  解释 task：ASR 只接受 `SingleUserAudioOnly`，VoiceClone 只接受 `AssistantTextOnly`，TTS 接受 `AssistantTextOnly` 或
  `UserTextThenAssistantText`，VoiceDesign 只接受 `UserTextThenAssistantText`；`Other` 以及 extra/empty/role mismatch fail closed。
  VoiceClone reference audio 只匹配独立
  `voice_conditioning`，不投影成 content-understanding input。TTS preset voice 可显式为 `mimo_default`，也保留 downstream
  `voice` 省略的既有 wire。
- 当前 generated-audio executable profile 同时要求完整 JSON 与 SSE delivery；两个 format set 非空、budget 为正且 framing 固定，
  不用 `Option`、空集合或零值表达 JSON-only/SSE-only/disabled 状态。
- 预检通过后仍按注册表的 Route 资格和顺序规划，不会因单条 Route 的额外能力跳过前序 Route、扩大公共契约或自动更换模型。
- streaming-only Upstream API 的非流式转换开关也参与全部候选的保守交集；首选候选关闭转换时，后续候选即使支持 JSON 也不会使
  `non_streaming` 升级或触发 capability routing。
- 细粒度能力只用于一次公共契约预检，不参与候选筛选；同一 Public Model/operation 的全部静态候选以原配置顺序进入 RoutePlan，fallback
  仍只处理既有的首输出前可重试可用性失败。
- Responses Provider ceiling 与 executable Target state 使用不同静态类型。`ExecutableResponsesState` 将独立 storage payload 与
  `Unbound | TargetBound | TargetBoundContinuation` affinity union 组合；只有最后一个 variant 派生 `previous_response_id`、issuer
  和 single-member credential 约束，不再保存独立 bool 或全局 `StateAffinity`。
- Route contribution 和 Public execution interface 分别保存携带 issuer 的 private continuation union；Bridge 固定 unsupported。
  `previous_response_id` 只在所有可执行 Responses candidate 绑定同一且唯一的 issuing Target/API 时公开；潜在签发者不唯一时，
  在上游调用前拒绝，避免把 opaque continuation 盲投到错误 Provider。Public JSON 的 state/parameter 仅为该 private union 的投影。
- credential startup gate 扫描全部启用 Target，不依赖 Public Model/Route 可见性；`TargetBoundContinuation` 使用多 member pool 时拒绝，
  普通 `TargetBound` 仍允许 credential rotation。只有请求实际携带 `previous_response_id` 才关闭跨 Target fallback；无状态请求不因
  候选具备 continuation 而改变现有 fallback。
- 当前 Responses 核心只接受省略或显式 `store:false`，并在每个 Responses candidate 上编码 `false`；`store:true` 在 route 执行前
  统一拒绝，Public Model 的 state 投影固定为 storage unsupported。`previous_response_id` 与 `background` 仍使用既有受限状态能力 gate；
  当前不提供通用 response storage、retrieve/cancel、conversation lifecycle 或 continuation ledger，客户端应每次携带完整历史。

## 实现边界

- Public Model projection 位于 [`src/registry/public_model.rs`](../../../src/registry/public_model.rs)，编译逻辑位于
  [`src/registry/public_model/compiler.rs`](../../../src/registry/public_model/compiler.rs)。
- generation 与 Embeddings analyzer 分开；analyzer 只提取请求事实，不解析 registry entity，也不选择 Route。
- 当前不包含动态目录、通用 capability negotiation、continuation ledger 或请求级 Route 选择 API。

## 验证证据

- [`tests/native_routing_contract.rs`](../../../tests/native_routing_contract.rs) 覆盖 typed mode 交集、未知字段分类、能力预检、Route 顺序、
  candidate 独立请求体和 continuation issuer 安全；图片用例还覆盖 Remote/Data/Both 精确 Models projection、source/MIME/detail 交集降级、
  cross-minima encoded/decoded clamp、最小 `https://a`/`AA==` preflight 正例，以及单项/累计上限拒绝。
- 同一 `native_routing_contract` 以只支持 `medium/high` 的双候选接口覆盖 `minimal/low → medium`、`xhigh/max → high`、`none`/未知拒绝、
  Responses 空 reasoning object 保留和 Models 三字段一致性；`tests/example_config/providers.rs` 另覆盖 Spark Chat 输入先归一化再进入
  Responses Bridge，`tests/embedding_definition_contract.rs` 覆盖非 generation 策略启动拒绝。
- [`tests/capability_definition_contract.rs`](../../../tests/capability_definition_contract.rs) 覆盖 capability subset、三种 image source payload、
  Provider containment、`ResponseInclude` wire round-trip、当前 15 个已探测 Target/API forwarding 声明与 payload elevation；core generation 单测覆盖 9-byte URL、4/1-byte inline 下界、空/重复 set、累计可达性和完整
  source/detail subset lattice。
- [`tests/provider_boundary_contract.rs`](../../../tests/provider_boundary_contract.rs) 固定 MiMo/OpenAI Chat/Responses Provider ceiling 的
  source、media、detail、`max_parts`、URL limit 和四项 inline limit，并确认只有 `mimo-v2.5` executable Chat/Responses Target 开放图片，
  MiMo Pro/audio 与全部 checked-in OpenAI Target 保持 `None`；同一测试还逐 operation 固定 9 个 Provider family ceiling 与 45 个
  checked-in generation Target 的 Structured Output profile。
- [`tests/forwarding_contract/mimo.rs`](../../../tests/forwarding_contract/mimo.rs) 对 Chat/Responses 分别覆盖 data-URL JSON 与 remote-URL SSE
  的 exact upstream body、mixed part 顺序、response bytes 和唯一 successful terminal；同一入口还覆盖非法 role、URL/Base64/MIME/detail、
  `file_id` 与超限输入 zero-egress。[`tests/bridge_forwarding_contract.rs`](../../../tests/bridge_forwarding_contract.rs) 固定图片请求不能经过
  Chat ↔ Responses Bridge 且不会触发 transport。
- [`tests/credential_store_contract.rs`](../../../tests/credential_store_contract.rs) 覆盖 continuation Target 即使没有 Public Model 仍要求
  单 member，以及普通 Target-bound pool 允许多 member；[`tests/forwarding_contract/resilience.rs`](../../../tests/forwarding_contract/resilience.rs)
  覆盖 continuation 请求禁跨 Target fallback，而无状态请求在 continuation-capable candidates 上仍保留 fallback。
- `cargo test --locked --lib core::capability::generation::tests` 验证 typed generation subset 与 audio profile presence 推导；
  `cargo test --locked --test native_routing_contract` 验证交集外 mode 和未知参数在 egress 前拒绝、候选顺序不变，以及一个候选的参数删除
  不污染支持该字段的 fallback candidate。
- Structured Output 首个测试在旧平铺结构上按预期得到 `support: supported, modes: []` 并失败；实现后 core 5×5
  subset/intersection truth table、analyzer merge/conflict、Public 空交集/strict 降级/稳定 mode 顺序、Provider ceiling elevation 与
  HTTP 四类请求 `unsupported_model_capability`/zero-egress 全部通过。MiMo JSON Object、DeepSeek Native/Bridge 与双向 JSON Schema
  Bridge 回归保持通过。
- [`tests/embedding_definition_contract.rs`](../../../tests/embedding_definition_contract.rs) 和 [`tests/embedding_registry_contract.rs`](../../../tests/embedding_registry_contract.rs)
  覆盖 Embeddings interface 的独立编译和公开契约。
- [`tests/example_config/configuration.rs`](../../../tests/example_config/configuration.rs) 覆盖 31 个 canonical leaf 的 task 表、
  canonical/audio mismatch、Provider ceiling miss、Generation AudioUnderstanding modality matrix、Public Model 跨 operation task gate 和
  same-variant 空 payload 交集；[`tests/forwarding_contract/models.rs`](../../../tests/forwarding_contract/models.rs) 覆盖标准/扩展
  Models 的 Generation/Embedding task 投影和 list/retrieve equality，
  [`tests/forwarding_contract/mimo.rs`](../../../tests/forwarding_contract/mimo.rs) 覆盖四个专用 task 投影与 private audio union tag 不泄漏。

2026-08-10 reasoning input policy 迁移验证：失败测试先因 `ReasoningLevelPolicy` 与 Public Model 字段尚不存在而按预期编译失败；实现后
`native_routing_contract` 34 项、`embedding_definition_contract` 7 项、`config_contract` 20 项、Models HTTP 2 项，以及 Spark Bridge
聚焦测试通过。`cargo fmt -- --check`、`cargo clippy --locked -- -D warnings` 与 `git diff --check` 通过。完整
`cargo test --locked` 只在既有 example-config 全等断言失败：当前本地 `config/bootstrap.toml` 启用了 OTLP，而
`config/bootstrap.example.toml` 未启用；未修改该本地配置。使用
`cargo test --locked -- --skip checked_in_examples_compile_into_a_closed_runtime_registry` 后其余全部测试通过。以上检查只证明本地
registry、analysis、planning、静态 Provider 定义与确定性 Bridge，不证明真实 Provider、当前外部 SDK、目标 Agent runtime、负载或长期运行。

2026-08-10 `include`/`prompt_cache_key` 根因修复先以三个旧实现失败用例确认整体 reserved gate：空 `include` 无法通过、未知投影错误分类、
未声明缓存键未进入固定参数 gate。实现后 `capability_definition_contract`、`native_routing_contract`、`forwarding_contract` 与
`bridge_forwarding_contract` 聚焦套件通过；Models 不再输出 `prompt_caching`，改为逐值 `response_includes` 与
`supported_parameters` 中的 `prompt_cache_key`。确定性测试只证明静态声明、交集、预检和 exact egress；真实上游证据边界见 Native/Bridge
专题；该阶段当时未据此承诺缓存命中或开放任何非空 include。

最终验证中，`capability_definition_contract` 14 项、`native_routing_contract` 35 项、`forwarding_contract` 64 项和
`bridge_forwarding_contract` 12 项全部通过；include 交集的 library 聚焦测试与 example-config Route 聚焦测试也通过。
`cargo fmt -- --check`、`cargo clippy --locked -- -D warnings` 与 `git diff --check` 通过。完整 `cargo test --locked` 仍只在未修改的本地
`config/bootstrap.toml` 与示例文件 OTLP 全等断言失败；跳过 `checked_in_examples_compile_into_a_closed_runtime_registry` 后其余测试全部通过。
本轮实现后未重新执行真实 Provider、外部 OpenAI SDK、Hermes、负载或长期运行验收。

2026-08-10 Hermes M1/M2 能力扩展使用失败优先测试锁定了两个旧行为：目标 Responses interface 拒绝
`include:["reasoning.encrypted_content"]`，目标 Chat/Responses interface 拒绝 `parallel_tool_calls:true`。实现后：

- DeepSeek Flash、OpenRouter DeepSeek Flash、MiMo V2.5 与 ChatGPT Codex Responses Target 接受并原样转发该 include；MiMo Pro、
  OpenRouter MiniMax 和未验证 Target 保持空集。GLM 5.2 的 Responses Bridge 接受后在转换器中移除，Chat egress 不携带 `include`，
  response converter 不合成 `encrypted_content`。
- `glm-5.2`、`deepseek-v4-flash` 与 `mimo-v2.5` 的目标 Chat/Responses interface 公开
  `parallel_tool_calls`；DeepSeek Flash 的三个 Chat candidate、两个 Responses candidate，以及 GLM Bridge 和 MiMo Native candidate
  均保留 `true`。DeepSeek Pro 因 Bailian fallback 未验证、MiMo Pro 与 OpenRouter MiniMax 因目标证据不足继续公开 unsupported。
- `tests/example_config.rs` 的完整候选规划、`tests/provider_boundary_contract.rs` 的 Provider/Target 矩阵、
  `tests/forwarding_contract.rs` 的 ChatGPT include 与 MiMo parallel egress，以及 `tests/bridge_forwarding_contract.rs` 的 include 消费
  用例均通过。确定性测试证明本地契约、规划和 wire 行为，不证明上游必定返回 reasoning item、多个 tool call 或内部并行执行。

2026-08-10 Hermes M3 以失败优先测试锁定了三个旧行为：Responses interface 仍错误识别 `stream_options`，目标 Chat interface 尚未公开
该参数，DeepSeek Flash 的流式请求在 Provider egress 前被拒绝。实现后：

- `glm-5.2` 的 1 个、`deepseek-v4-flash` 的 3 个和 `mimo-v2.5` 的 1 个完整固定 Native Chat candidate 共同公开
  `stream_options`；对应 Responses interface、全部 Bridge 和未验证相邻模型继续保持 unsupported。
- 参数分析只接受 Chat `stream:true` 且 `stream_options` 恰为 `{"include_usage":true}`；非对象、空对象、`false`、额外子字段及
  非流式组合在 egress 前以稳定无效请求失败。
- `tests/native_routing_contract.rs` 覆盖 Chat-only 参数目录、精确形状、Responses/Bridge fail-closed；
  `tests/example_config/providers.rs` 覆盖三个 Public Model 的完整候选交集和相邻模型收窄；`tests/forwarding_contract.rs` 使用编译后的
  DeepSeek 首选 candidate 验证 post-adapter 请求与带 Provider 私有 usage details 的 SSE 尾块逐字节保持。

M4 仍未实现且未获准：DeepSeek Flash 与 MiMo V2.5 的 Chat interface 只公开 `json_object`。MiMo 直连探测虽接受非 strict 和
`strict:true` 的 `json_schema`，但 enum/字段名约束出现违背并伴随 `finish=abort`，不能证明 strict 语义可靠；DeepSeek、Bailian、
OpenRouter 也尚未完成同等验证。因此当前完整候选交集继续 fail closed。

M3 最终验证中，`native_routing_contract` 36 项、`example_config` 30 项和 `forwarding_contract` 66 项全部通过；隔离 target 目录下的完整
`cargo test --locked`、`cargo fmt -- --check`、`cargo clippy --locked -- -D warnings` 与 `git diff --check` 均通过。本轮没有重新执行真实
Provider、Hermes、外部 SDK、强制 fallback、负载或长期运行验收。

2026-08-11 M5 使用失败优先测试锁定了两个旧行为：Chat→Responses 没有提升首条合格 system/developer，且 planning 尚无统一
instructions/store 错误与规范化入口。实现后：

- Bootstrap `default_instructions` 直接替换旧字段；只要保留可执行通用 Generation interface 就要求非空值，仅 Embeddings/专用音频
  task 不要求。Responses 显式非空 string 和 Chat 首条纯文本 system/developer 优先，缺失时使用项目默认值；后续 transcript 不扫描、
  拼接或删除，instruction-only Chat→Responses 产生 `input:[]`。
- planning 在 Public Model 预检后、candidate 展开前生成一个 canonical body；Native、Bridge、retry/fallback 与 probe 使用同一有效
  文本。所有 Responses candidate 显式携带 `store:false`，`store:true` 或其他显式形状在 route 执行前返回 typed 400。
  `instructions` 与 false-only store 不进入 model parameter/state 投影，Embeddings 与专用音频 task 跳过该策略。
- ChatGPT 专属 request context 与覆盖 hook 已删除；adapter 只保留固定 Responses stream/input/store envelope、header/OAuth 和输出
  token limit 拒绝。双向 Bridge 分别负责单次提升/删除或 prepend system，不读取 Bootstrap 或 Provider。
- 聚焦验证通过：`bridge_conversion_contract` 20 项、`bridge_forwarding_contract` 12 项、`config_contract` 22 项、
  `embedding_definition_contract` 8 项、`forwarding_contract` 67 项、`ingress_contract` 6 项、`native_routing_contract` 41 项、
  `qwen36_registry_contract` 1 项、`startup_contract` 3 项、probe 单元测试 11 项和 `process_replay_contract` 7 项。
  `example_config` 的 M5 全通用模型/全固定候选审计通过；完整 28 项中其余 27 项通过，唯一失败来自工作区并行 Google/Gemma 变更把
  canonical 数量增加为 33，但其范围内的既有断言仍期望 32。
- `cargo test --locked -- --skip canonical_catalog_assigns_every_model_to_one_expected_task`、
  `cargo clippy --locked -- -D warnings`、`git diff --check`、`uv lock --check --project tools/corpus`、corpus Python 45 项和 corpus lint
  通过。`cargo fmt -- --check` 仍只报告两个未由 M5 修改的已提交文件 `src/providers/kimi_cn/definition.rs` 与
  `src/providers/openrouter/definition.rs` 的既有格式差异；M5 Rust 文件已用同版 rustfmt 单独检查。

以上确定性证据证明本地配置、analysis/planning、Models 投影、Native/Bridge wire、retry body 复用和 canonical fixture，不证明真实
Provider、Hermes、外部 SDK、强制多 Provider fallback、负载或长期运行。M5 未实现或验证 `previous_response_id`；
`reasoning.summary` 仍是独立后续边界。

## 相关文档

- [功能需求：Public Model 与模型能力契约](../../functional-requirements/model-information-and-capability-contract.md)
- [Provider 注册表与模型目录](provider-registry-and-model-catalog.md)
- [MiMo Provider 多模态与工具调用状态](../providers/mimo.md)
- [`mimo-v2.5` Native 图片输入](native-image-input.md)
- [HTTP 网关接口与下游认证](gateway-http-api-and-auth.md)
