# 功能：Chat Completions 与 Responses Native 转发

## 状态

**已完成（当前 checkout）。** 网关可以对已注册且通过 Public Model 预检的 Chat/Responses 请求执行 Native JSON 或 SSE 转发，并保留目标协议
允许的原生语义。

## 已完成内容

- `POST /v1/chat/completions` 和 `POST /v1/responses` 支持当前声明范围内的非流式 JSON 与 streaming SSE。
- Upstream API 使用类型化 streaming policy：普通 API 保留下游 mode；ChatGPT Responses 声明 `stream: true` required，并启用
  bounded Responses SSE buffering。下游非流式 Responses 在合法 terminal 后返回完整 response object，非流式 Chat 再经既有
  Responses→Chat JSON Bridge 返回。
- Native Route 对已知且被接口接受的字段保留下游 canonical wire 语义；Provider adapter 在 egress 阶段绑定固定 upstream model、
  相对 path、普通固定 header 和 purpose-bound authentication。未知顶层字段不再属于 Native 透明透传范围。
- `prompt_cache_key` 只在真实探测过的具体 Target/API 上声明 exact forwarding；Native candidate 与 Provider adapter 保留原值，不附带
  cache-hit 或效果保证。Responses `include: []` 在 candidate 展开前移除；`reasoning.encrypted_content` 当前只在 DeepSeek Flash、
  OpenRouter DeepSeek Flash、MiMo V2.5 与 ChatGPT Codex 的固定 Responses Target 上声明并原样转发。该参数不控制 reasoning 的存在性或
  明文/opaque 形态，`response_includes` 也不构成输出 item 保证；未声明 Target 与其他 include 值继续在 Native egress 前失败关闭。
- `parallel_tool_calls` 只在完整固定候选集均有直接接受证据时进入 Public Model interface，并在 Native candidate 保留原布尔值。
  当前新增范围是 DeepSeek V4 Flash、MiMo V2.5，以及支撑 GLM 5.2 Bridge 的 Bailian Chat Target；DeepSeek V4 Pro 因 Bailian fallback
  未验证、MiMo Pro 与 OpenRouter MiniMax 因对应 Target 未验证而保持 unsupported。接受字段不保证单次响应产生多个 tool call，也不证明
  Provider 内部并发执行。
- Chat `stream_options` 当前只建模 `include_usage` 且只与 `stream:true` 组合。有效 `true` 只有在完整固定 Chat candidate 交集都能履行时
  才公开：Native candidate 要求 canonical model 与 Chat API 的 typed `stream_usage` 保证，并原样保留对象和 Provider usage details；
  Chat→Responses Bridge 则由独立 terminal-usage 投影保证贡献同一公共能力。空对象和 `false` 是不要求该能力的 no-op，会在 Native/Bridge
  candidate egress 前统一移除；非法/额外成员、非对象与非流式组合继续 zero egress 失败。
- Upstream API 可以用闭合 `IgnorableGenerationParameter` 集合接受但不向上游发送已确认不兼容的普通生成字段；这些字段仍保留在
  Public Model `supported_parameters`。当前 Kimi K3 Chat 只删除 `frequency_penalty`、`presence_penalty`、`temperature`、`top_p`；
  ChatGPT GPT-5.5/5.6 Responses 只删除 `seed`。Kimi 的 `n/logprobs/top_logprobs`、MiMo V2.5/Pro Responses 的
  `top_logprobs` 和 ChatGPT 的 `include_reasoning` 改为禁用并从固定 interface 收窄，在 egress 前明确拒绝。
  stream、reasoning level/开关、tools、structured output、state、媒体和输出 token 上限同样不在忽略闭合集合内。
- 参数忽略在每个 candidate 从原始 body 独立构造之后、进入第一个 Bridge/Provider shape 转换之前执行；Native 无忽略规则时继续保留
  原始 bytes。Provider adapter 保留同一删除规则作为最终 egress 防线，前一 candidate 的删除不会改变 fallback body。
- Reasoning level 由 Canonical Model 统一定义并在同一模型的 Chat/Responses interface 中保持一致；Native Responses 保留具体
  effort，只有 thinking 开关的 Chat Provider 将 `none` 映射为关闭、其余已声明 level 映射为开启。未知 level 在 egress 前拒绝。
- 当前 Native surface 包括 OpenAI `gpt-5.6-sol`、LongCat `LongCat-2.0`、DeepSeek Chat 与 V4 Flash 无状态 Responses、
  OpenRouter 的 `deepseek-v4-flash` 与 `minimax-m3` Chat/无状态 Responses、Bailian Qwen3.7 Max/Plus，以及 Xiaomi MiMo 的
  Chat/Responses。
- Bailian Qwen3.7 Native Responses 的 reasoning output 使用官方 `reasoning.summary[]`，与 Chat 的 `reasoning_content`
  plain-text wire 分开建模；两协议仍共享同一七档 Model 能力。
- `deepseek-v4-pro` 与 `deepseek-v4-flash` 的 Chat/Responses 固定 interface 公开非 strict 的 `json_object`。Chat 保留
  `response_format`，V4 Flash Native Responses 保留 `text.format`，V4 Pro Responses-via-Chat 将后者转换为前者；固定候选中的
  OpenRouter/Bailian 只对相应 DeepSeek target 启用该能力，不扩大 MiniMax、Qwen 或 GLM。
- `mimo-v2.5` 的两个同协议 Native surface 还支持固定 typed contract 内的 URL/Base64 图片输入；Chat surface 另支持单个 WAV
  data URL 的通用音频理解，Responses audio 保持关闭。具体边界和证据分别由 [Native 图片专题](native-image-input.md)与
  [Native MiMo 音频专题](native-mimo-audio.md)记录。
- 所有 Responses 请求只接受省略或显式 `store:false`，并对每个 Responses candidate 显式编码 `false`；`store:true` 在 route
  执行前统一拒绝。非空 `previous_response_id` 和 `background:true` 仍按既有固定状态契约预检；DeepSeek V4 Pro 仍只注册 Chat Native API。
- 上游 safe response headers、SSE framing、terminal、EOF-before-terminal 和 body failure 在统一 ingress/transport 边界处理。
- streaming-to-JSON takeover 只接受 Responses SSE，并同时受 JSON response body 与单 SSE event 上限约束；它校验标准 text lifecycle，
  从 response snapshots 与有序 `response.output_item.done` 补齐稀疏 terminal；非法 framing/UTF-8、
  非 SSE success、超限 body 或缺失 terminal 在下游 body commit 前返回安全 502。当前不实现通用 Chat SSE 聚合。
- 成功 streaming response 通过静态 Provider media profile 分类：普通 Provider 必须显式返回唯一的 `text/event-stream`；当前 ChatGPT
  Responses backend 的真实成功响应允许缺失 `Content-Type`，网关仍执行完整 SSE 校验并向下游规范化为 `text/event-stream`。已出现但错误、
  前缀相似或重复的媒体类型不会进入该特例；原生 stream 与 streaming-to-JSON takeover 均在 body commit 前 fail closed。

## 实现边界

- 请求入口位于 [`src/ingress/`](../../../src/ingress/)，请求分析/规划位于 [`src/pipeline/`](../../../src/pipeline/)，Provider adapter 位于
  [`src/provider/adapter.rs`](../../../src/provider/adapter.rs)，共享发送边界位于 [`src/transport/upstream.rs`](../../../src/transport/upstream.rs)。
- Native Route 的额外能力不会扩大 Public Model；它必须先通过公共 interface preflight。
- 这不是外部 OpenAI SDK、Codex/Hermes Agent、真实 Provider、负载或长期运行兼容性声明。

## 验证证据

- [`tests/forwarding_contract.rs`](../../../tests/forwarding_contract.rs) 覆盖 Native JSON/SSE、错误、header 和响应收口。
- [`tests/sse_contract.rs`](../../../tests/sse_contract.rs) 覆盖 SSE framing、terminal、EOF 和错误边界。
- [`tests/provider_contract.rs`](../../../tests/provider_contract.rs) 与 [`tests/provider_boundary_contract.rs`](../../../tests/provider_boundary_contract.rs)
  覆盖 Provider wire、认证和安全出站。
- [`tests/config_contract.rs`](../../../tests/config_contract.rs) 覆盖 streaming policy 与 operation/capability 的启动校验；
  [`tests/forwarding_contract.rs`](../../../tests/forwarding_contract.rs) 覆盖 ChatGPT JSON/SSE、强制上游 `stream: true`、terminal takeover
  以及非法/超限流的安全失败。
- [`tests/forwarding_contract.rs`](../../../tests/forwarding_contract.rs) 覆盖 DeepSeek V4 Flash 的固定 `/responses` egress、typed SSE
  terminal，以及 MiniMax/相邻 Provider 的客户端可见转发行为；不再断言内部候选顺序。
- `forwarding_contract::native::longcat_responses_native_forwards_prompt_cache_key_and_removes_empty_include` 检查 post-adapter LongCat
  Responses egress与空 include 移除。

2026-08-10 使用私有配置中已启用的非 GPT API-key pool 完成 63 次脱敏固定探测；未使用 `OPENAI_API_KEY` 或 ChatGPT OAuth。9 个 Native
Responses Target 的 baseline、空 include、`reasoning.encrypted_content`、缓存键与组合请求均为 HTTP 200/完整 terminal，但没有一次返回
`encrypted_content`。随后对带/不带 include 做同形复测：Bailian Qwen3.8、DeepSeek Flash、OpenRouter GLM/DeepSeek、MiMo V2.5 与
LongCat 均保持各自明文 `summary_text`/`reasoning_text` 形态；OpenRouter DeepSeek 的偶发无 reasoning item 同时出现在两组。ChatGPT
Codex backend 的 GPT-5.6 Luna 在 `store:false` 下两组都返回 opaque `encrypted_content`。因此 include 被建模为上游接受的条件性兼容
请求值，而不是输出开关；当前只对完成固定候选证明的 Public Model 开放。Bailian GLM 没有 Responses endpoint，仍由 Chat Bridge 处理。
该证据只适用于当时 Target、账号、网络和请求形状，不证明其他 include、其他账号或未来 Provider 行为。

同日 `parallel_tool_calls:true` 直连 Chat 探测中，Bailian GLM 5.2/DeepSeek Flash、DeepSeek V4 Flash/Pro、MiMo V2.5、NVIDIA
MiniMax M3、Kimi K3、OpenRouter GLM 5.2/DeepSeek Flash 均返回 HTTP 200；OpenRouter 与 NVIDIA 的单次响应观察到两个 tool call，其他
单次结果没有形成多调用。当前实现只开放 Hermes 目标 Public Model 的完整固定候选集，不把 HTTP 200 外推到缺少工具能力或未完整验证的
fallback。ChatGPT parallel capability 沿用既有 Provider 契约；本轮没有通过标准 endpoint 重新直连探测。

同日 M3 实施前的直连流式 Chat 探测记录目标候选所用的 Bailian、DeepSeek、OpenRouter 与 MiMo 接受
`stream_options:{"include_usage":true}` 并出现 usage 尾块；NVIDIA、Kimi 与 LongCat 也接受，ChatGPT 由用户确认，OpenAI 未探测。
OpenRouter、NVIDIA、Kimi 与 MiMo 当轮只保留请求接受和尾块存在性的结论，没有留存 usage 明细。已留存的原始 Chat 样本为：

- DeepSeek Flash：`prompt_tokens=89`、`completion_tokens=17`、`total_tokens=106`，另有 cached 0、reasoning 14 和 prompt cache
  hit/miss 明细；
- Bailian GLM 5.2：18/116/134，另有 cached 0、reasoning 112；
- LongCat 流：13/20/33，prompt details 各分类均为 0，usage chunk 另带 `lastOne:true`。

用于核对遥测兼容性的 Responses 观测为：Bailian GLM 经 OpenBridge Chat→Responses Bridge 得到 18/125/143；DeepSeek Flash 经
OpenBridge Native Responses 得到 89/16/105，另有 cached 0、reasoning 13；ChatGPT Codex backend GPT-5.6 Luna 直连上游得到
35/6/41，另有 cached/cache-write 0、reasoning 0。前者是 Bridge 输出而非 Bailian 原生 Responses wire。observability parser 同时识别
`input_tokens|prompt_tokens`、`output_tokens|completion_tokens`、显式 total 或两者求和、常见顶层 cache read/write 别名，以及 details
内的 cached/creation 变体；这些解析只生成遥测，不改写下游 body。上述结果不证明 token 数值、缓存明细或计费准确，也未在实现后重新执行。

M3 的失败测试最初分别观察到 Responses 参数目录错误包含 `stream_options`、目标 Chat Models 未公开该参数，以及 DeepSeek 流式请求
返回 400；实现后 forwarding HTTP/wire 聚焦测试通过。确定性证据证明客户端错误、post-adapter 请求与 response bytes，不替代真实
Provider 或 Hermes 复测。

2026-08-11 M7 保留了 Native `include_usage:true` 的 exact-forwarding 行为，并把 `{}`/`false` 统一归一化为省略；同一 typed Route
contribution 现在还能让具备完整 Responses terminal usage 的 Chat Bridge 参与固定 interface 交集。确定性 Native 测试覆盖 true 原样
egress 和不支持该能力时 no-op 仍可执行且不出现在 upstream body；Bridge 生成的 usage 尾块不属于 Native response 改写。由于用户明确
没有有效 OpenAI API key，本轮未重新执行 OpenAI、ChatGPT、Hermes 或其他真实 Provider 验收，既有历史探测也不作为本轮回归结果。

2026-08-09 ChatGPT streaming response media 修复的实际验证：

- 最小脱敏诊断确认上游 HTTP 200 Responses body 有 9 个合法事件及 `response.completed`，但没有 `Content-Type`；诊断不保存正文、ID、
  request ID 或 credential；
- 5 个 GPT 模型最终重跑 Chat/Responses × `stream:false/true` × omitted/high 共 40 个真实单元，全部得到合法 200 JSON/SSE 终态；
  0 个 HTTP、协议或传输错误，0 个单元触发 429/503 重试。

2026-08-09 严格参数处置的最终验证：

- `tests/config_contract.rs` 验证 canonical 参数必须进入类型化目录，以及 ignore rule 的声明、重复/冲突边界；
  `tests/forwarding_contract.rs` 覆盖 Native/Bridge 未知参数、参数删除、fallback 隔离和 zero egress 拒绝；
- 使用真实下游 key 对 Kimi `temperature` 执行 Chat/Responses × JSON/SSE，4/4 为 HTTP 200 且终态合法；同一运行中的未知字段 2/2
  返回 `unknown_parameter`，Kimi `n/logprobs/top_logprobs` 两协议 6/6 返回带精确 `param` 的
  `unsupported_model_capability`；
- 最后使用 GPT-5.6 Luna 对照：Chat/Responses 的 `seed` 2/2 为 HTTP 200，`include_reasoning` 2/2 在 egress 前返回
  `unsupported_model_capability`。全部真实单元一次完成，没有最终 429/503 或传输错误；结果未保存 credential、请求/响应正文、
  reasoning、logprobs 或 Provider request ID。

2026-08-08 DeepSeek V4 Flash Responses Native 变更的实际验证：

- 首条 `cargo test --locked --test example_config deepseek_pro_stays_chat_only_while_flash_prefers_deepseek_responses` 在实现前按预期失败，
  原因为 Flash target 尚无 Responses API；
- `cargo test --locked --test provider_contract`、`cargo test --locked --test provider_boundary_contract`、
  `cargo test --locked --test example_config` 与 `cargo test --locked --test forwarding_contract deepseek_v4_flash`：通过；
- `cargo fmt -- --check`、`cargo test --locked`、`cargo clippy --locked -- -D warnings` 与 `git diff --check`：通过。

2026-08-09 DeepSeek V4 Flash tool-choice 收窄验证：

- DeepSeek Responses Upstream API 只保留真实确认的 `none/auto`，因此 Public Model 的 Responses interface 通过固定 DeepSeek/OpenRouter
  候选交集公开同一集合；`required/named` 不会因为后备 OpenRouter 更强而成为公共保证；
- `example_config::providers::deepseek_flash_responses_exposes_only_proven_tool_choice_modes` 覆盖 Models 投影、正向计划与
  `required/named` 计划阶段拒绝；最终 `cargo test --locked --test example_config` 通过（13 项）；
- 真实 DeepSeek 首选路径的 `none/auto/required/named` × JSON/SSE 共 8/8 符合固定契约：前四项 HTTP 200 且终态合法，后四项在
  egress 前返回 HTTP 400 `unsupported_model_capability`。当前通用能力错误不携带 `param`。

2026-08-09 DeepSeek JSON object 聚焦验证：

- 新增 Models/规划契约在实现前按预期失败：`deepseek-v4-pro` Chat 的 structured output 仍为 `unsupported`；补充后
  `example_config::providers::deepseek_public_interfaces_expose_json_object_across_fixed_candidates`、
  `provider_boundary_contract::provider_capability_ceilings_preserve_verified_feature_differences` 与
  `forwarding_contract::native::deepseek_json_object_is_preserved_by_native_and_bridge_egress` 均通过；
- 使用真实下游 key 运行两个 Public Model 的 Chat/Responses × JSON/SSE，8/8 为 HTTP 200、终态完整、输出可解析且字段符合 prompt；
- 对固定上游候选做脱敏定向验证：DeepSeek 官方 endpoint 6/6、OpenRouter Flash 4/4、Bailian Pro/Flash 4/4，全部为 HTTP 200、
  终态完整且返回预期 JSON；请求均按官方前提包含 `json` 与字段示例，未保存 credential、正文或 request ID；
- 本轮只验证 `json_object`，未验证或公开 `json_schema`/strict schema；没有运行完整真实 E2E、外部 Agent、负载或长时间测试，也没有
  改写当时的 E2E 结果文档。

真实检查不证明外部 OpenAI SDK、Codex/Hermes runtime、其他账户、负载或长期运行兼容性。

## 相关文档

- [功能需求：网关 API 与客户端兼容](../../functional-requirements/gateway-api/README.md)
- [协议 Bridge](protocol-bridge.md)
- [`mimo-v2.5` Native 图片输入](native-image-input.md)
- [DeepSeek API 协议入口快照](../../references/providers/deepseek/api.md)
- [重试、fallback、cooldown 与取消](resilience-retry-fallback-and-cancellation.md)
- [当前代码架构](../current-architecture.md)
