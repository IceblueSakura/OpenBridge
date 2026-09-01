# 设计关注点导航矩阵

## 文档元数据

| 字段 | 值 |
|---|---|
| Source snapshot | 本目录已有的叶文档与综合文档；本页不另行拥有外部快照 |
| Last reverified | 2026-09-01，仅本地导航整理；没有刷新任何外部来源 |
| Scope | 按网关设计关注点索引外部协议、转换、运行时、凭证与测试事实 |
| Evidence boundary | 导航页不拥有任何事实，不构成采用承诺或实施计划；每条入口的事实、版本与边界由被链接文档维护 |
| Recheck trigger | 相关叶文档/综合文档新增、替换或标题锚点调整；出现表结构未覆盖的新关注点 |

## 使用规则

- 本页只是"按关注点查阅"的指针矩阵：每行链接到拥有事实的叶文档或综合文档，不复述结论。
- 跨项目比较性结论只存在于 [cross-project](../cross-project/README.md) 综合文档；本页需要比较时只链接已有综合。
- 某关注点缺少项目级前置文档时，先按来源目录规则建叶文档并补齐元数据，再登记本页。

## 1. 协议与转换

| 关注点 | 导航入口 | 该组文档覆盖什么 |
|---|---|---|
| Canonical representation / IR 形态 | [Protocol IR 生态综合](../cross-project/protocol-ir-ecosystem-analysis.md)、[Bifrost](../protocol-gateways/bifrost.md)、[TensorZero](../protocol-gateways/tensorzero.md)、[Vercel AI SDK](../protocol-gateways/vercel-ai-sdk.md)、[LiteLLM IR 增量](../litellm/litellm-ir-server-tool-regressions-analysis.md)、[new-api 请求转换](../new-api/new-api-request-conversion-analysis.md) | decode/encode 的 schema 形态（canonical semantic / protocol-shaped / pairwise）、fidelity 与 loss 处理 |
| Chat↔Responses 与多协议转换 | [LiteLLM Chat/Responses](../litellm/litellm-chat-responses-analysis.md)、[new-api 请求转换](../new-api/new-api-request-conversion-analysis.md)、[cc-switch 转换](../cc-switch/cc-switch-chat-responses-tool-conversion-analysis.md)、[Hermes 上游请求合同](../hermes/hermes-chat-responses-analysis.md)、[CLIProxyAPI stateful bridge（负面案例）](../cliproxyapi/cliproxyapi-stateful-bridge-analysis.md) | 入口选择、双向 bridge、history 编译、转换链与负面案例 |
| Streaming 事件与终态 | [Chat SSE](../openai/chat-completions-streaming.md)、[Responses typed SSE](../openai/responses-streaming.md)、[Responses WebSocket](../openai/responses-websocket.md)、[Realtime transport（与 SSE 的差异）](../openai/realtime-transport.md)、[Codex SSE 消费](../codex/codex-sse-and-tool-lifecycle-analysis.md)、[SDK streaming consumer](../openai/openai-sdk-stream-test-assets-analysis.md)、[Vercel Event Algebra](../protocol-gateways/vercel-ai-sdk.md#4-streaming-event-algebra)、[Bifrost streaming](../protocol-gateways/bifrost.md)、[Helicone streaming 与 body lifecycle](../protocol-gateways/helicone.md) | chunk/event shape、accumulator、terminal/EOF/error、任意 bytes 分片 |
| Function tools 与 tool lifecycle | [Chat Function tools](../openai/chat-completions-function-tools.md)、[Responses Function tools](../openai/responses-function-tools.md)、[Codex OAuth 与工具调用](../codex/codex-oauth-and-tool-call-analysis.md)、[Codex SSE tool identity](../codex/codex-sse-and-tool-lifecycle-analysis.md)、[cc-switch tool context](../cc-switch/cc-switch-chat-responses-tool-conversion-analysis.md)、[测试资产覆盖比较](../cross-project/chat-responses-sse-tool-test-suite-survey.md#3-覆盖比较) | call/result round trip、call_id/index 身份、并行与时序 |
| Server-side / hosted tools 与 interception | [LiteLLM server-tool interception](../litellm/litellm-ir-server-tool-regressions-analysis.md#3-server-side-tool-interception)、[TensorZero Provider tools](../protocol-gateways/tensorzero.md)、[OpenRouter server tools](../providers/openrouter-api.md)、[Hosted image generation](../openai/images-responses-hosted-generation.md)、[File Search](../openai/files-responses-file-search.md)、[综合 §5](../cross-project/protocol-ir-ecosystem-analysis.md#5-server-side-tool-生命周期) | hosted tool 生命周期、gateway interception、call identity 归属 |
| Continuation、state 与 identity | [Responses state ownership](../openai/responses-state.md)、[Responses resource lifecycle](../openai/responses-resource-lifecycle.md)、[Stored Chat resources](../openai/chat-completions-stored-resources.md)、[Responses WebSocket](../openai/responses-websocket.md)、[CLIProxyAPI previous_response_id 绑定（负面案例）](../cliproxyapi/cliproxyapi-stateful-bridge-analysis.md)、[综合 §7](../cross-project/protocol-ir-ecosystem-analysis.md#7-identitystate-与-reasoning) | previous_response_id、conversation、replay、connection-local state |
| Structured output | [Chat structured output](../openai/chat-completions-structured-output.md)、[Responses structured output](../openai/responses-structured-output.md) | request wire、result 判定、两协议边界；schema normalization 场景见综合 §8.1 |
| Reasoning 与 opaque data | [Responses reasoning wire 位置](../openai/responses-request.md)、[cc-switch opaque reasoning](../cc-switch/cc-switch-chat-responses-tool-conversion-analysis.md)、[TensorZero reasoning 与 state](../protocol-gateways/tensorzero.md)、[综合 §7](../cross-project/protocol-ir-ecosystem-analysis.md#7-identitystate-与-reasoning) | reasoning summary/signature/encrypted 分离、replay scope |
| 媒体与文件 wire | [OpenAI 索引 §4–§8](../openai/README.md)、[Xiaomi 图片 wire](../providers/xiaomi-image.md)、[Xiaomi 音频 wire](../providers/xiaomi-audio.md)、[Provider 索引](../providers/README.md) | 图片/文件/音频/视频各 operation 的 encoding 与生命周期 |

## 2. 运行时与韧性

| 关注点 | 导航入口 | 该组文档覆盖什么 |
|---|---|---|
| 路由与 capability gating | [Bifrost capability/routing](../protocol-gateways/bifrost.md)、[TensorZero capability 与 extensions](../protocol-gateways/tensorzero.md)、[Helicone routing 与 health](../protocol-gateways/helicone.md)、[new-api 渠道路由](../new-api/new-api-routing-billing-operations-analysis.md)、[OpenRouter provider routing](../providers/openrouter-api.md)、[综合 §6](../cross-project/protocol-ir-ecosystem-analysis.md#6-capability-与-routing) | 候选集合、capability-safe 选择、affinity、健康状态 |
| Retry / cooldown / credential pool | [Credential pool 综合](../cross-project/credential-pool-retry-analysis.md)、[LiteLLM retry](../litellm/litellm-credential-pool-retry-analysis.md)、[CLIProxyAPI cooldown](../cliproxyapi/cliproxyapi-credential-pool-retry-analysis.md)、[cc-switch failover](../cc-switch/cc-switch-retry-failover-analysis.md)、[new-api retry 与渠道故障](../new-api/new-api-routing-billing-operations-analysis.md)、[Helicone retry 与 fallback](../protocol-gateways/helicone.md) | 资源单位、有限重试、冷却、失败分类 |
| Unsupported 与 fail-closed 边界 | [Vercel fidelity 与 warnings](../protocol-gateways/vercel-ai-sdk.md#5-fidelitywarnings-与-usage)、[Portkey adapter 边界](../protocol-gateways/portkey.md)、[API family 与 fake 证据边界](../openai/endpoint-adoption-and-fake-testing.md) | 静默 drop、clamp、warning→reject、兼容档位 |
| 观测与终态统计 | [LiteLLM 调用统计与 Prometheus](../litellm/litellm-observability-analysis.md)、[Helicone cache 与 observability](../protocol-gateways/helicone.md)、[new-api 消费观测](../new-api/new-api-routing-billing-operations-analysis.md)、[综合 §6（运行时策略）](../cross-project/protocol-ir-ecosystem-analysis.md#6-capability-与-routing) | 统计分层、TTFT 口径、标签基数、终态计数 |
| 性能、body 与资源边界 | [LiteLLM 性能瓶颈](../litellm/litellm-proxy-performance-bottlenecks.md)、[LiteLLM 调用链](../litellm/litellm-proxy-call-chain-analysis.md)、[Helicone streaming 与 body lifecycle](../protocol-gateways/helicone.md)、[new-api 可重放请求体](../new-api/new-api-routing-billing-operations-analysis.md) | 瓶颈定位、body 捕获与重放、连接复用 |

## 3. 凭证与消费方

| 关注点 | 导航入口 | 该组文档覆盖什么 |
|---|---|---|
| 上游 OAuth device / refresh | [OAuth 综合](../cross-project/upstream-oauth-device-code-token-refresh-analysis.md)、[Codex 设备登录与刷新](../codex/codex-device-auth-token-refresh-analysis.md)、[Codex 浏览器 OAuth](../codex/codex-oauth-and-tool-call-analysis.md)、[Hermes credential lifecycle](../hermes/hermes-codex-oauth-refresh-analysis.md)、[LiteLLM ChatGPT authenticator](../litellm/litellm-chatgpt-oauth-refresh-analysis.md)、[CLIProxyAPI OAuth scheduler](../cliproxyapi/cliproxyapi-codex-oauth-refresh-analysis.md) | 设备码流程、refresh grant、rotation 风险、401 recovery |
| Consumer 消费合同与插件面 | [Hermes 上游请求合同](../hermes/hermes-chat-responses-analysis.md)、[Hermes provider 插件](../hermes/hermes-provider-plugin-capabilities.md)、[Hermes 网关插件面](../hermes/hermes-gateway-plugin-capabilities.md)、[Codex SSE 消费](../codex/codex-sse-and-tool-lifecycle-analysis.md)、[SDK streaming consumer](../openai/openai-sdk-stream-test-assets-analysis.md) | 宽松 fallback、字段消费、插件默认值、accumulator 需求 |
| MCP transport 与远程认证 | [MCP Rust 生态索引](../mcp/README.md)、[远程访问模式](../mcp/remote-access-modes.md)、[rmcp 官方 SDK](../mcp/rmcp-official-sdk.md) | Streamable HTTP、OAuth 2.1、无状态化、部署模式 |

## 4. 测试与评测

| 关注点 | 导航入口 | 该组文档覆盖什么 |
|---|---|---|
| Fake 合同测试与证据分层 | [API family 与 fake 合同测试分层](../openai/endpoint-adoption-and-fake-testing.md#3-fake-合同测试分层)、[测试资产综合评估维度](../cross-project/chat-responses-sse-tool-test-suite-survey.md#2-评估维度) | 兼容档位、fake 证据能证明与不能证明什么 |
| 外部测试资产吸收 | [测试资产登记表](test-assets-registry.md)、[测试资产综合](../cross-project/chat-responses-sse-tool-test-suite-survey.md)、[综合 §8 测试吸收清单](../cross-project/protocol-ir-ecosystem-analysis.md#8-测试吸收清单) | 资产登记、覆盖缺口、吸收场景清单与采用义务 |
| 语义评测方法 | [Semantic evaluation methods](../semantic-testing-methods.md) | 长上下文、function-tool、structured-output 的任务设计与采用边界 |
