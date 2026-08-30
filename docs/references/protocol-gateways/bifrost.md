# Bifrost 多协议 schema、Provider 转换与 streaming 调研

## 文档元数据

| 字段 | 值 |
|---|---|
| Source snapshot | [`maximhq/bifrost` `dev` @ `7e26cffbd47cd295f35b64176bfbb721fdd0924a`](https://github.com/maximhq/bifrost/tree/7e26cffbd47cd295f35b64176bfbb721fdd0924a) |
| Last reverified | 2026-08-30，本地只读源码与测试源码复核 |
| Scope | Chat/Responses schema、Provider interface、OpenAI/Anthropic/Gemini 转换、reasoning、server-side tools、stream terminal/truncation、Responses state |
| Evidence boundary | 未构建或启动 Bifrost，未调用 Provider；测试源码只证明项目意图和 deterministic contract，不证明真实 Provider 当前行为 |
| Recheck trigger | `core/schemas/`、Provider interface、Responses/Chat converter、stream state/terminal、server-side tool 或 license 变化时 |

## 1. Architecture

Bifrost 的 HTTP transport、core dispatch、operation-specific schema 和 Provider implementation 分层清楚：Provider interface 集中在 `core/schemas/provider.go:633-753`，可选 Responses lifecycle 与 WebSocket 能力通过额外 interface 暴露：`core/schemas/provider.go:755-776`。这避免 transport 直接知道每个 Provider 的认证和 wire conversion。

其统一层不是单一 canonical semantic IR。Chat 与 Responses 保留为两套第一等 schema：`BifrostChatRequest` 位于 `core/schemas/chatcompletions.go:13-33`，`BifrostResponsesRequest` 位于 `core/schemas/responses.go:42-55`。通用 stream chunk 是更宽的 operation union，除 text/chat/Responses 外还包含 speech、transcription、image generation、passthrough 和 error；本文关注的 Generation 子集只是其中三类：`core/schemas/bifrost.go:1835-1846`。Provider 分别实现 Chat 和 Responses 转换；部分 Responses 路径通过 Chat response 再构造 Responses。架构复杂度因此更接近“protocol-shaped intermediates + Provider converters”，不是所有 wire 先进入一个中立 semantic core。

## 2. Conversion 与 fidelity

operation-specific schema 让 Responses item、Chat message 和 Provider-native block 保持较高表达力，但无法自动证明跨 schema 等价。源码中存在显式 downcast、normalization 与内部转换；未发现统一的 `Exact / Lossy / Unsupported` disposition owner。Bifrost 的经验更适合作为 conversion edge-case 证据，而不是直接作为 canonical IR 模板。

Responses lifecycle 还具有独立 state/affinity：retrieve/delete/cancel/input-items 通过可选 Provider interface 执行；schema 注释要求多 key 场景把 lifecycle call pin 回产生 response ID 的上游账户：`core/schemas/responses.go:57-63`、`84-120`。WebSocket session 另外维护 last response ID、upstream connection、Provider session ID 和 terminal turn：`transports/bifrost-http/websocket/session.go:14-37`。这些 identity 不能被压缩成普通 message metadata。

## 3. Tools 与 reasoning

Bifrost Responses schema覆盖 function、custom、web search、tool search 等工具形状。`core/schemas/responsestoolunmarshal_test.go:14-111` 展示 dated Anthropic tool-search type 需要 normalization，同时必须保留 regex/bm25 variant 和 caller 显式 name；否则会静默降级为不同搜索语法。`core/schemas/responsestoolunmarshal_test.go:124-183` 又固定 cache control、allowed callers、examples、eager streaming 和 strict 等公共字段。

Gemini server-side tool stream test 保留每轮 search 的独立 call ID、query、source 和顺序，而不是把多轮搜索合并：`core/providers/gemini/serversidetools_stream_test.go:54-94`。GenAI round-trip 还要求 tool call/result 上的 thought signature 保持原节点、只出现一次：同文件 `100-182`。这说明 hosted tool invocation、result、source/citation 和 opaque reasoning affinity 需要关联到具体 item，而不是只记录“使用过 web search”。

Anthropic redacted thinking 被映射为带 encrypted content 的独立 reasoning item，并在 added/done/completed 生命周期保持稳定 ID：`core/providers/anthropic/redactedthinkingresponses_test.go:86-140`。混合 redacted、visible thinking 和 text 时，测试要求三个 item 不互相污染：同文件 `221-323`。可读 reasoning 与 opaque replay state 必须分开。

## 4. Streaming

streaming 有独立状态和转换代码，不是单纯切割最终 JSON。`StreamTerminalDetector` 增量解析 SSE/plain JSON，并识别 `[DONE]` 或 finish reason：`core/providers/utils/streamterminaldetector.go:17-89`。测试覆盖跨 byte chunk、CRLF、多 event、metadata-only、多个 candidate 只有全部结束才 terminal，以及有界 pending buffer：`core/providers/utils/streamterminaldetector_test.go:8-155`。

OpenAI truncation tests明确区分 EOF 与 semantic terminal：缺少 `[DONE]`、finish reason 或 Responses terminal event 的 EOF 必须作为 truncation error，不能合成正常结束：`core/providers/openai/streamtruncation_test.go:16-28`。pre-first-byte failure 可以进入 retry/fallback；mid-stream failure保留已输出内容后发错误，但不得追加伪 terminal：同文件 `152-200`。这类测试比“最终文本相同”更适合协议网关。

需要注意：Bifrost 的通用 stream chunk是多个 protocol response pointer 的 union，Provider converter仍直接产生目标 protocol event；它没有独立、协议中立的 Event IR。

## 5. Capability、routing 与 error

Provider interface 和可选 capability interface 能表达 operation 支持，但 protocol schema 本身不能证明具体 semantic capability。core 在第一 stream chunk检查 HTTP 200 内错误：`core/providers/utils/stream.go:9-32`；core retry 对每次 streaming attempt 重建 plugin/post-hook pipeline：`core/bifrost.go:7072-7077`。这些是有价值的资源生命周期边界，但不等于请求 capability algebra。

## 6. 可吸收测试资产

优先自主重写下列 deterministic 场景，而非复制完整 Go fixture：

1. SSE terminal 跨 bytes、CRLF、多 event、metadata-only 与多 candidate；
2. EOF-before-terminal 在 pre-commit 与 post-commit 的不同结果；
3. visible reasoning、opaque reasoning 和 text 的 item 隔离；
4. server-side search 多轮 call identity、query/source ownership 与顺序；
5. thought signature 只附着于原 tool call/result，round-trip 不重复；
6. dated provider-native tool type normalization 不得改变 variant；
7. absent optional tool fields保持 absent，不物化零值；
8. stored response lifecycle 的 Provider/account affinity。

Bifrost 使用 Apache-2.0；即使只重写场景，也应在对应参考文档或 fixture provenance 中记录本 commit 和原测试路径。

## 7. Lessons

### Adopt

- operation-specific decoder/encoder 与 transport 分离；
- streaming terminal/truncation、opaque reasoning、hosted tool identity 的细粒度 deterministic tests；
- stateful lifecycle 使用独立 capability/affinity contract。

### Adapt

- 保留富语义 item/block，但将 Chat/Responses/Anthropic/Gemini schema 进一步 decode 到 protocol-neutral semantic domains；
- 将 Provider-native tool payload放在 typed namespace 与 capability gate 后，而不是依赖 type string normalization；
- 把 pre-first-visible-event retry 和 post-commit failure结合本地 commit boundary，而不是复制其错误分类。

### Avoid

- 把多套 protocol-shaped schema 当成已经完成的 canonical IR；
- 依靠 pairwise/Provider converter默默完成 normalization 或丢字段；
- 用统一 stream chunk union 替代明确的 Event IR lifecycle。

### Open Questions

- 每个 direct Chat↔Responses helper的实际 loss surface 是否都有对应 negative test；
- server-side tool result/citation在不同 Provider间是否存在可证明的 portable subset；
- Responses lifecycle 和 WebSocket state在 fallback、credential rotation时何时失效；
- Provider-specific stream terminal detector与 canonical Event IR 的责任边界如何划分。
