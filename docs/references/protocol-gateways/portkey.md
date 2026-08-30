# Portkey Gateway Provider adapter 与 middleware 调研

## 文档元数据

| 字段 | 值 |
|---|---|
| Source snapshot | [`Portkey-AI/gateway` `main` @ `669825cbe89ee51569918b8f78a9db486fd69dd4`](https://github.com/Portkey-AI/gateway/tree/669825cbe89ee51569918b8f78a9db486fd69dd4) |
| Last reverified | 2026-08-30，本地只读 TypeScript 源码与测试源码复核 |
| Scope | Provider config/API、request/response/stream/error transformation、OpenAI/Anthropic/Google-family adapter、handler/middleware 边界 |
| Evidence boundary | 未构建或启动Gateway，未调用Provider；静态transformer不证明跨协议语义等价或生产错误行为 |
| Recheck trigger | `src/providers/`、request/response handlers、stream parser、Provider options、middleware或license变化时 |

## 1. Architecture

Portkey以Hono handlers组织ingress和middleware，Provider差异主要集中在 `src/providers/`。`ProviderAPIConfig`负责headers、base URL、endpoint和form-data判断；Provider config还可注册operation-specific request handler：`src/providers/types.ts:43-83`、`131-160`。这是较清晰的endpoint/auth/transport adapter边界。

普通JSON请求通过Provider参数表转换；upload/file等特殊operation可走自定义stream/body handler：`src/services/transformToProviderRequest.ts:143-223`。response handler再按Provider和endpoint选择response/stream transformer：`src/handlers/responseHandlers.ts:62-68`。

## 2. Conversion model

统一输入主要是OpenAI-shaped `Params`和message/content types，而不是protocol-neutral semantic IR。每个Provider config将输入字段映射到目标path，或通过custom transform重写。Anthropic adapter例如把assistant tool calls改成`tool_use` block，将tool message改成user-role `tool_result`：`src/providers/anthropic/chatComplete.ts:141-195`。

通用参数转换只遍历Provider config中列出的字段；未列出的输入字段不会进入transformed request：`src/services/transformToProviderRequest.ts:75-127`。数值低于/高于Provider min/max时被直接clamp：同文件 `28-72`。这两类行为没有统一loss report：unknown field可静默丢失，越界值可被改写，适合说明为什么Gateway需要decode后的capability/fidelity gate。

## 3. Provider adapter 边界

Provider interface把URL/header、request shape、response shape、stream shape和error mapping放在同一Provider目录，能避免pipeline散落大量Provider-name match。OpenAI-compatible Provider可复用`open-ai-base`参数与response transformer，差异较大的Anthropic/Bedrock/Google维护专用converter。

边界并不完全严格：共享 `Params`、`Message` 和大量 `any` 允许Provider-specific字段进入公共类型，Provider transform同时承担default、clamp、semantic conversion和字段drop。应借鉴目录/注册结构，而不是配置表的全部责任。

## 4. Tools、reasoning 与 streaming

Anthropic tool call转换对arguments执行`JSON.parse`，空arguments合成空object：`src/providers/anthropic/chatComplete.ts:163-175`。tool result在缺少call ID时使用空string：同文件 `184-195`。这些兼容策略对应用Proxy可能实用，但会掩盖malformed arguments或missing identity；严格Gateway应明确reject或标记synthesized fidelity。

Provider通用response type把reasoning token、cache token、search query和citation/grounding metadata并入OpenAI-shapedresponse：`src/providers/types.ts:162-238`。这种aggregation便于客户端，但会压平source/tool lifecycle和Provider-specific语义。

部分non-stream response可被重新切成SSE：OpenAI helper按500字符拆text/thinking，并生成Chat chunks：`src/providers/openai/chatComplete.ts:150-280`。这是synthetic streaming，不是upstream Event IR；chunk boundary、timing、item start/end和原Provider delta均无法恢复。

## 5. Error 与 middleware

Provider-specific error transformer统一返回OpenAI-styleerror object，方便Proxy consumer。需要注意的是统一shape会丢失Provider retry hints、nested errors或HTTP-200内错误，除非另行保留raw evidence。middleware/auth/routing与Provider transform解耦的方向可借鉴，但semantic validation不能放进通用middleware字符串规则。

## 6. 可吸收测试资产

建议自主重写：

1. 未列入目标Provider contract的字段必须显式unsupported，而非静默drop；
2. value超界必须reject或产生visible normalization disposition，不能无声clamp；
3. malformed tool arguments、空arguments和missing call ID分别处理；
4. assistant text + parallel tool calls + tool results保持order/identity；
5. Provider stream block/delta映射不丢thinking signature；
6. synthetic non-stream→stream明确标记synthesized并满足唯一terminal；
7. HTTP status、Provider error、retryability和raw evidence同时保持；
8. custom base URL/header只来自受信config，不来自business request。

Portkey使用MIT。外部tests多为Provider adapter形状，吸收时应选最小deterministic场景并自主编写fixture。

## 7. Lessons

### Adopt

- Provider目录内聚endpoint、auth、request/response/stream/error adaptation；
- shared OpenAI-compatible base加显式Provider override；
- 特殊binary/stream operation使用operation-specific handler。

### Adapt

- 若用于严格Gateway，参数配置表应只实现已通过前置检查的lowering，不应自行判断capability或静默normalize；
- `Params/Message`说明OpenAI-shaped common DTO的局限，不自动构成protocol-neutral IR；
- error normalization保留retryability、Provider code和sanitized raw evidence。

### Avoid

- 未列字段静默drop、越界值静默clamp；
- missing tool identity合成空string；
- 把non-stream文本切块称为原生stream conversion；
- 让`any`或Provider options穿越不受信Gateway边界。

### Open Questions

- Responses与Anthropic Messages是否共享真正semantic converter，还是继续pairwise/provider-specific；
- Provider stream parser如何处理EOF-before-terminal和post-terminal event；
- hosted tool/source/citation是否有独立生命周期类型；
- middleware retry/fallback是否理解stream commit和state affinity。
