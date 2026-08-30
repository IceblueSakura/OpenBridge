# Protocol gateway 生态与富语义 IR 调研综合

## 文档元数据

| 字段 | 值 |
|---|---|
| Source snapshot | 本页链接的Bifrost、LiteLLM、TensorZero、Vercel AI SDK、Portkey、Helicone和OpenRouter项目级前置；本页不另行拥有源码快照 |
| Last reverified | 2026-08-30，综合已固定的本地源码结论与OpenRouter官方公开资料 |
| Scope | canonical representation、protocol/Provider boundaries、fidelity、streaming、tools/reasoning/state、routing和测试吸收 |
| Evidence boundary | 未构建或运行外部项目，未调用Provider/OpenRouter；这是架构和测试资产调研，不是任何本地产品合同或实施方案 |
| Recheck trigger | 任一项目级快照、协议、server-tool contract、采用范围或IR设计决策变化时 |

## 1. 项目级前置

- [Bifrost](../protocol-gateways/bifrost.md)
- [LiteLLM增量调研](../litellm/litellm-ir-server-tool-regressions-analysis.md)及其[既有索引](../litellm/README.md)
- [TensorZero](../protocol-gateways/tensorzero.md)
- [Vercel AI SDK](../protocol-gateways/vercel-ai-sdk.md)
- [Portkey Gateway](../protocol-gateways/portkey.md)
- [Helicone AI Gateway](../protocol-gateways/helicone.md)
- [OpenRouter公开行为](../providers/openrouter-api.md)

## 2. 架构比较

| 项目 | 内部表示 | Protocol/Provider转换 | Streaming | 主要证据价值 | 主要局限 |
|---|---|---|---|---|---|
| Bifrost | Chat、Responses等operation-specific schema | Provider分别实现operation converter；部分协议间再转换 | 独立Responses events与Provider stream state | 多协议真实edge cases、server tools、reasoning、terminal tests | 没有单一protocol-neutral IR，新增协议/Provider仍易形成乘积复杂度 |
| LiteLLM | OpenAI/Responses SDK types、Chat messages、Provider config与wrapper混合 | pairwise bridge、Provider transform、hook/interception | iterator/wrapper、Provider chunks和terminal state | 最大量兼容回归、state ownership和agentic server-tool loop | loss/drop/compatibility分散，hook可改变stream和执行副作用 |
| TensorZero | semantic content blocks + inference request/response | Provider adapters消费semantic types；Provider tools单独scope | `ContentBlockChunk`等semantic chunks | core semantic与Provider-private capability分离 | Provider tool payload仍是`Value`，`Unknown`需要更严格转发策略 |
| Vercel AI SDK | 最完整的provider-neutral static content和stream part union | Provider package负责encode/decode；通过options/metadata扩展 | text/reasoning/tool start-delta-end + finish/error | static IR/Event IR、provider-executed tool与preliminary result | SDK信任模型允许options/headers/warnings，不等同Gateway fail-closed |
| Portkey | OpenAI-shaped Params/Message + Provider config | 参数表、custom transformer、response/error transformer | Provider stream transforms；也可synthetic切分final text | Adapter目录边界、宽Provider经验 | 未列字段静默drop、数值clamp、`any`和synthetic stream |
| Helicone | endpoint/protocol types，重点不在semantic IR | endpoint mapper + Provider/runtime layers | body relay和runtime metrics | Rust routing、retry、cache、latency、integration tests | candidate集合按endpoint配置，不证明semantic capability等价 |
| OpenRouter | 后端闭源；只观察公开surface | 聚合router、plugins、server tools和Provider-native execution | 官方声明metadata位于terminal/final chunk | 外部行为和policy injection边界 | 公开文档不证明实际账户/模型/Provider行为 |

没有一个项目同时给出“Gateway-grade富语义IR、严格fidelity algebra、安全Provider extension、完整Event IR和capability-safe routing”。合理做法是组合吸收，而不是选定一个模板复制。

## 3. Canonical representation 观察

### 3.1 Static semantic model

Vercel和TensorZero最接近可参考的semantic core：text、reasoning、file/media、tool call/result和source分别建模；Bifrost证明Responses-shaped item model能够承载更丰富语义，但也证明直接使用wire schema会把OpenAI lifecycle和字段命名带入核心。

建议研究方向是item/content-oriented semantic superset，而不是Chat message最小交集。Message可以作为conversation role/order容器，但reasoning、tool invocation/result、source和opaque state需要独立identity与lifecycle。

### 3.2 Provider extension

TensorZero的`ProviderTool { scope, tool }`和Vercel的`provider.<tool>` namespace都保留Provider-native能力。共同缺口是payload仍可较动态。Gateway采用时至少需要：

- namespace与schema registry；
- Target/API/Provider scope；
- trusted configuration origin；
- capability declaration；
- replay/exposure/fallback policy；
- 禁止endpoint/header/auth覆盖。

Opaque/unknown仅表示核心不解释，不表示可以任意跨Route转发。

### 3.3 Fidelity

各项目展示了需要区分的真实类别：

- exact：portable semantic未改变；
- normalized：wire差异被规范化但语义可证明等价；
- synthesized：生成item ID或由final object生成stream lifecycle；
- opaque-preserved：保留Provider-minted reasoning/state但限制affinity；
- emulated：Gateway function loop模拟Provider hosted tool；
- lossy：stream降成buffered、reasoning/terminal被压平、参数clamp/drop；
- unsupported：目标无法承载或identity/state不完整。

Vercel warnings可作为分类输入，但Gateway不能把unsupported warning等同于允许继续；Portkey/LiteLLM的silent drop或compatibility hook则是应避免的反例。

## 4. Streaming/Event IR

Bifrost、Vercel和LiteLLM共同说明stream不是final JSON的分块：

- text、reasoning和tool arguments各有start/delta/end；
- item/call identity跨delta稳定；
- usage可晚于可见content；
- completed、incomplete、failed/error具有不同terminal semantic；
- EOF只是transport结束，不是semantic terminal；
- terminal可能触发opaque resource/ownership commit；
- pre-first-output error与post-output error的retry边界不同。

Event IR需要能materialize成static response，并用同一fixture验证stream/non-stream语义等价。`raw`/unknown event只能留作受限evidence，不能替代typed semantic event。

## 5. Server-side tool 生命周期

调研中至少出现四种不同tool：

1. client-executed function tool；
2. Provider-executed hosted tool；
3. Gateway-executed emulation/interception；
4. always-run middleware/plugin transformation。

它们不能只靠`name + JSON arguments`统一。至少需要区分：

- declaration origin：client、trusted Route policy、Provider profile、account policy；
- executor：client、Gateway、Provider、external MCP/backend；
- request disposition：absent、preserved、injected、stripped、replaced/emulated；
- invocation：zero/one/multiple、approval、call identity、raw/parsed arguments；
- result：text/JSON/media/source、preliminary/final、success/error/denied；
- side effects：network、cost、data exposure、sandbox/session；
- lifecycle：tool execution、model rerun、usage aggregation、cancel、terminal；
- affinity：Provider/Target/account/session和fallback policy。

LiteLLM interception最能展示完整emulation loop；Bifrost/Gemini最能展示Provider-native tool call/result/signature/source关联；Vercel最适合参考portable/provider-executed type shape；TensorZero最适合参考scope与capability gate；OpenRouter说明account/plugin policy可能使wire request不是全部执行事实。

## 6. Capability 与 routing

Protocol support不能替代semantic capability。Helicone按endpoint构造候选和routing policy，说明runtime latency/weight/health应在候选集合之后工作；TensorZero的Provider tool support穷举判断说明Provider-private capability需要显式gate；OpenRouter的`require_parameters`默认值说明“Provider可能接受”与“候选必须声明支持”是不同策略。

静态contract、value-sensitive preflight、lowering fidelity和runtime routing需要分层：routing不应通过动态删除不兼容候选来扩大公共能力，也不应在encoder里猜测参数是否可drop。

## 7. Identity、state 与 reasoning

### Identity/state

- internal correlation ID、wire item ID、tool call ID、response ID、container/session ID不应共用一个字段；
- Provider-minted ID可能要求Target/account affinity；
- terminal可能是资源ownership提交点；
- fallback后旧attempt的opaque ID必须失效或保持隔离；
- continuation、Provider session、cache和conversation semantic state不是同一概念。

### Reasoning

至少分为：visible thought/reasoning text、summary、encrypted/redacted content、signature和usage。Bifrost的redacted-thinking tests与LiteLLM stored reasoning ID均证明opaque replay state必须附着正确item，不能压成`reasoning_content`字符串；Vercel的visible reasoning lifecycle不足以替代opaque namespace。

## 8. 测试吸收清单

### 8.1 Static decode/encode

1. Chat/Responses/Anthropic/Gemini分别decode到相同portable text/tool语义；
2. reasoning summary与encrypted/signature分离；
3. tool declaration unknown/date-versioned variant不静默降为custom function；
4. malformed arguments、empty object、missing call ID分别判定；
5. structured-output schema normalization与unsupported keyword分类；
6. Provider extension只在scope匹配时encode；
7. unknown extension跨Provider fail closed；
8.同协议round-trip保持semantic，不要求byte equality，另设Native wire-preservation测试。

### 8.2 Event IR

1. text/reasoning/tool arguments任意byte fragmentation；
2. parallel tool calls的index、item ID、call ID和name独立分片；
3. usage-only、metadata-onlyevent不误判terminal；
4. `[DONE]`、finish reason、completed/incomplete/failed分别处理；
5. EOF-before-terminal、duplicate terminal、post-terminal event拒绝；
6. stream materialization等于non-stream semantic response；
7. terminal state穿过wrapper/fallback后仍可供resource owner读取；
8. precommit失败允许有限retry，postcommit失败不得拼接新attempt。

### 8.3 Server tools

1. absent/preserved/injected/stripped/replaced五种request disposition；
2. Provider tool调用零次、一次、多轮、多次并行；
3. thought signature与call/result保持在同一native item；
4. source/citation关联正确调用轮次；
5. Gateway emulation保留call ID，result注入后有界rerun；
6. loop budget、费用、cancel、timeout和tool error；
7. stream被buffered/synthesized时产生visible fidelity；
8. internal control字段不得泄漏到Provider；
9. fallback不能把opaque state或tool result发送到不兼容Target；
10. account/plugin policy injection作为observed execution fact，不能冒充client request。

### 8.4 Runtime policy

1. static capability-safe候选集合内的weighted/latency routing；
2. retryable/non-retryable error与Retry-After；
3. cache key包含全部semantic requirement及extension policy；
4.缓存不得伪造live attempt metadata；
5. health/latency更新不改变公共capability；
6. observability从semantic/request lifecycle投影，不反向解析Provider JSON。

## 9. License 与 provenance

- Bifrost、TensorZero、Vercel：Apache-2.0；
- LiteLLM、Portkey：MIT，LiteLLM `enterprise/`另有条款；
- Helicone：GPL-3.0；
- OpenRouter：仅引用官方网页行为。

默认只自主编写synthetic fixture并记录“inspired by”项目、commit、源测试路径和抽象语义，不复制整段test payload。尤其不要复制LiteLLM enterprise内容或Helicone GPL实现进入不同license边界而不做法律/许可证审查。

## 10. 仍待验证

- 外部项目自身tests在固定commit上是否通过；
- Anthropic/Gemini/OpenAI当前官方wire与这些项目实现是否一致；
- OpenRouter账户默认plugin和server-tool真实执行；
- Provider-native tool跨协议是否存在可证明的portable subset；
- exact/equivalent/normalized边界和route policy；
- Native wire-preservation与“所有路径Decode→IR→Encode”的最终关系；
- Event IR terminal/commit与fallback的所有权；
- Gateway tool executor是否属于Generation pipeline还是独立orchestrator。

这些问题需要后续设计评审或明确授权的执行验证；本调研不直接生成Rust schema或迁移计划。
