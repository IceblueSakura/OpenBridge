# Vercel AI SDK Provider-neutral language model types 调研

## 文档元数据

| 字段 | 值 |
|---|---|
| Source snapshot | [`vercel/ai` `main` @ `69428b1f8b037e4d118fb4853428d5c4e620493c`](https://github.com/vercel/ai/tree/69428b1f8b037e4d118fb4853428d5c4e620493c) |
| Last reverified | 2026-08-30，本地只读 `packages/provider` 源码与测试源码复核 |
| Scope | `LanguageModelV4` prompt/content、tools、reasoning/files/sources、Provider extensions、warnings、usage 与 stream parts |
| Evidence boundary | 应用 SDK interface，不是多租户 Gateway trust model；未构建 SDK 或运行 Provider tests，不证明任何 Provider 当前行为 |
| Recheck trigger | Provider interface major version、content/tool/stream types、provider options/metadata、warning 或 license 变化时 |

## 1. Architecture 与 static semantic model

`LanguageModelV4CallOptions` 是 Provider adapter 的标准调用面，包含 normalized prompt、generation settings、response format、function/provider tools、tool choice、reasoning 和 Provider options：`packages/provider/src/language-model/v4/language-model-v4-call-options.ts:8-138`。它明确不是用户直接输入，而是上层 API映射后的内部格式：同文件 `9-17`。

prompt仍以 messages组织，但 assistant/tool content是 ordered parts：text、file、custom、reasoning、reasoning-file、tool call/result和approval response：`packages/provider/src/language-model/v4/language-model-v4-prompt.ts:9-59`。Provider返回的 static content进一步包含 source、tool approval request和custom content：`packages/provider/src/language-model/v4/language-model-v4-content.ts:11-20`。这是“message作为组织容器、semantic part作为一等对象”的折中，不是 Responses wire DTO改名。

multimodal file data使用 data、URL、Provider reference和inline text等 tagged source，media type独立表达：`packages/provider/src/language-model/v4/language-model-v4-prompt.ts:148-188`。这有助于把资源语义与wire编码分开，但资源获取权限、大小预算和Provider reference affinity不由该类型解决。

## 2. Tools

function tool与Provider tool分开。Provider tool使用 `<provider-id>.<unique-tool-name>` ID和Provider定义的 args：`packages/provider/src/language-model/v4/language-model-v4-provider-tool.ts:1-28`。tool call具有全局唯一 call ID、stringified input、`providerExecuted`、dynamic/MCP标志和Provider metadata：`packages/provider/src/language-model/v4/language-model-v4-tool-call.ts:3-41`。

Provider-executed tool result是独立对象，保留call ID、tool name、JSON result、error、preliminary/final和dynamic状态：`packages/provider/src/language-model/v4/language-model-v4-tool-result.ts:4-51`。prompt也能回放 provider-executed call/result，并表达approval response：`packages/provider/src/language-model/v4/language-model-v4-prompt.ts:191-260`。

这些类型比“function name + arguments”更完整，特别适合研究hosted tool execution lifecycle。但 `providerExecuted?: boolean`仍把执行owner压成布尔值；Gateway还需区分客户端、Gateway、Provider和第三方MCP executor，以及requested/injected/observed状态。

## 3. Reasoning、source 与 Provider extensions

reasoning是独立content part和stream lifecycle，reasoning file也与普通file分离。Provider-specific custom part使用 `{provider}.{kind}` namespace：`packages/provider/src/language-model/v4/language-model-v4-prompt.ts:80-145`。几乎每个message/content part还可携带 `providerOptions`，输出携带 `providerMetadata`。

这种 `Core semantic + namespaced extension` 是重要参考，但SDK调用方通常是受信应用。Gateway不能允许不受信 downstream arbitrary `providerOptions`、headers或custom payload直接穿越Route；需要schema registry、Target affinity、大小限制、exposure policy和禁止endpoint/auth/header override。

## 4. Streaming Event Algebra

`LanguageModelV4StreamPart`显式区分：

- text start/delta/end；
- reasoning start/delta/end；
- tool input start/delta/end、tool call/result/approval；
- file、reasoning file、source与custom content；
- stream-start warnings、response metadata、finish、raw和error。

定义见 `packages/provider/src/language-model/v4/language-model-v4-stream-part.ts:14-110`。text/reasoning/tool input都使用稳定ID，finish携带usage和finish reason；error可出现多次。static IR与stream part分开，明显优于把最终response分块。

其边界也值得注意：没有类型级唯一terminal约束，raw chunk与多error允许存在，Event materialization和post-finish rejection需要上层实现。Gateway还需区分semantic terminal、transport EOF、cancel和first-visible-event commit。

## 5. Fidelity、warnings 与 usage

Provider可以在stream-start返回 `unsupported`、`compatibility`、`deprecated` 或其他warning：`packages/provider/src/shared/v4/shared-v4-warning.ts:1-66`。这让兼容降级可观察，但warning不是Gateway的安全策略。无法证明等价的请求不能先降级再仅靠warning通知；需要在encode前由policy产生 exact/equivalent/lossy/unsupported disposition。

usage将input、output、cached/reasoning token等子类分开；这适合作为稳定observability projection，但missing与zero必须保留区别，不能在Provider缺失时合成“真实”计数。

## 6. 可吸收测试资产

V4类型目录本身没有统一的 `*.test.ts` 套件；候选场景应从 `packages/ai/src/model/as-language-model-v4.test.ts`、`packages/provider-utils/src/streaming-tool-call-tracker.test.ts` 以及OpenAI/Anthropic/Google等Provider package的adapter tests中提炼：

1. message content ordering覆盖text/reasoning/tool call/result/file；
2. provider-executed与client-executed tool不可混淆；
3. preliminary tool result必须最终被non-preliminary result结束；
4. approval allow/deny与call ID关联；
5. text/reasoning/tool-input start-delta-end identity；
6. stream warning与finish/usage独立；
7. Provider namespace collision与错误schema；
8. provider reference/file只允许原Target encode；
9. raw event存在时不污染semantic materialization；
10. unsupported/compatibility warning在严格policy下转为reject。

Vercel AI SDK使用Apache-2.0。跨项目采用优先重写最小场景，不复制Provider测试中的API key、snapshot噪声或完整SDK harness。

## 7. Lessons

### Adopt

- ordered semantic parts、独立static content与stream event类型；
- Provider tool、provider-executed call/result、approval、preliminary result；
- namespaced extension和Provider metadata位置；
- start/delta/end与stable identity。

### Adapt

- `providerExecuted`扩展为明确executor/ownership；
- warning扩展为可机器执行的fidelity disposition和route policy；
- provider options/metadata增加schema、Target affinity、exposure/replay和安全边界；
-补充唯一terminal、EOF、cancel、post-terminal rejection和materialization contract。

### Avoid

- 把SDK的arbitrary headers/provider options暴露为Gateway downstream contract；
- 用warning替代fail-closed capability validation；
- 假设message/part模型已覆盖stored response、continuation和全部item identity。

### Open Questions

- source/citation与server-side tool result的portable边界；
- approval是否属于Gateway generation semantic，还是只属于Agent runtime；
- preliminary result在Chat/Anthropic/Gemini lowering时如何表示；
- static content与Event materialization在错误、cancel和partial tool input下的等价条件。
