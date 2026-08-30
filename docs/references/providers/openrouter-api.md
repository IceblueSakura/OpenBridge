# OpenRouter API 调研

- Last reverified：2026-08-30；本次只复核 OpenRouter 官方 Responses、routing、server tools、plugins 和 router metadata 页面；未发送 API 请求。2026-08-27 的模型接入实测边界保持不变。
- Recheck trigger：Responses beta、Provider routing、server tool/plugin contract、router metadata、Models/endpoint schema 或数据政策变化。

## 来源与范围

- [Chat Completions API](https://openrouter.ai/docs/api/api-reference/chat/send-chat-completion-request)
- [Responses API](https://openrouter.ai/docs/api/reference/responses/overview)
- [Models API](https://openrouter.ai/docs/api/api-reference/models/get-models)
- [Model endpoints API](https://openrouter.ai/docs/api/api-reference/models/get-endpoints-for-a-model)
- [Provider Routing](https://openrouter.ai/docs/guides/routing/provider-selection)
- [Server Tools](https://openrouter.ai/docs/guides/features/server-tools)
- [Web Search Server Tool](https://openrouter.ai/docs/guides/features/server-tools/web-search)
- [Plugins](https://openrouter.ai/docs/guides/features/plugins)
- [Router Metadata](https://openrouter.ai/docs/guides/features/router-metadata)

本文只记录入口、认证、Provider routing、工具执行边界与固定 wire 观察，不保存动态工具目录、模型字段、能力表、价格或 Models 快照。

## API 与认证

- API base 为 `https://openrouter.ai/api/v1`。
- Chat Completions、Responses 和 Models 相对 path 分别为 `/chat/completions`、`/responses`、`/models`。
- API key 使用 `Authorization: Bearer ***`。
- `HTTP-Referer` 与 `X-Title` 是可选 attribution/routing header，不是认证字段。
- Responses surface 无状态；官方资料将 `store: true` 与非空 `previous_response_id` 列为不支持。

## Provider routing

Models 目录、用户过滤视图和单模型 endpoint 列表是不同资源；目录可见不证明某个 endpoint、账户或参数组合当前可用。
`provider.require_parameters` 默认为 `false`；设为 `true` 才要求候选 Provider 声明支持请求中的全部参数。具体模型与 endpoint 能力应在采用时重新读取 OpenRouter 官方资源，不在本文复制。

## Server tools、plugins 与 function tools

官方资料区分三种执行语义：

- server tool 由模型决定是否调用、由 OpenRouter 执行，一次请求可调用零到多次，通过 `tools` 中的 `openrouter:*` type 声明；
- user-defined function tool 同样由模型决定是否调用，但由客户端执行，通过 `tools` 中的 `function` type 声明；
- plugin 由 OpenRouter 执行，启用后每次请求运行一次，通过 `plugins` 声明，也可能由账户或组织默认设置注入。

因此，wire request 没有显式 plugin 不足以证明执行链未发生请求或响应变换。官方资料还允许管理员阻止请求覆盖默认 plugin；这类账户策略是外部控制面事实，不是客户端声明的 tool semantic。

`openrouter:web_search` 的官方页面同时描述两条执行路径：OpenRouter 自有搜索引擎和 Provider-native search。参数支持随 engine/Provider 变化，某些不支持参数会被忽略，另一些组合会失败；统一 tool type 不能推出所有 Target 具有同一 capability 或 fidelity。usage 可另外报告 server-tool 调用次数，citation 作为输出 annotation 出现。

## 可观察的路由与变换

`X-OpenRouter-Metadata: enabled` 可请求返回 `openrouter_metadata`。官方页面称其可记录候选、尝试、fallback 和实际运行的 pipeline stage，包括 plugin、server tools、response healing 与 context compression；streaming 时出现在终端事件或 `[DONE]` 前的最终 chunk。该 metadata 是 opt-in、可追加字段的调试面，cache replay 会剥离它，部分 edge/500 错误也不会携带，因此不能作为业务语义或完整审计日志。

## 固定 wire 观察

2026-08-02 的一次 Responses streaming 请求得到 data-only SSE：终态 JSON 顶层 `type` 为 `response.completed`，嵌套 `response.status` 为 `completed`，随后另有 `[DONE]`。该结果只证明当时账户、模型、网络和 payload，不证明其他 endpoint、模型或未来版本。

2026-08-27 对 `z-ai/glm-5.3-flash` 的 Chat/Responses、image、tool、structured output 与 Hermes 接入观察见[带日期证据记录](../../implementation-status/evidence/2026-08-27-openrouter-glm-5-3-flash-integration.md)；本文不复制模型级结果或当前代码结论。

## 后续黑盒验证矩阵

需要真实验证时，至少拆成下列相互独立的场景，而不是一次“web search 成功”请求：

1. 固定 model、Provider 和 payload，分别声明、不声明 `openrouter:web_search`，记录零次、一次和多次调用；
2. 分别选择 OpenRouter engine 与 Provider-native engine，核对请求参数、输出 item/citation、usage 和错误，不假设二者等价；
3. 显式 plugin、账户默认 plugin、请求禁用 plugin 和管理员禁止覆盖四种策略，确认 wire 声明与实际 pipeline 的差异；
4. `provider.require_parameters=false/true` 下固定候选集合，确认参数支持如何影响选择和 fallback；
5. non-stream 与 stream 分别核对 tool lifecycle、terminal、usage、`openrouter_metadata` 所在位置和 EOF-before-terminal；
6. 搜索无结果、engine 拒绝参数、tool execution 失败、Provider 失败与 fallback 后成功，区分协议错误、工具错误和路由错误；
7. 记录 tool-call 次数、token usage、费用、延迟、数据政策与实际 Provider，但不保存 key 或敏感 query/result。

这些场景在执行前还需要独立批准账户、预算、模型、Provider、数据内容和输出保存边界。

## 证据边界

统一 API surface 不表示所有模型共享相同能力、数据政策、配额或 SLA。模型能力以 OpenRouter 当前官方页面和 endpoint 详情为准；OpenBridge 当前映射见[Model 与 Provider 映射](../../implementation-status/model-provider-mapping.md)。

2026-08-30 的新增内容是官方文档声明，不证明账户默认配置、实际 tool invocation、engine 选择、Provider-native lowering、费用、延迟、stream event 序列或 fallback 行为。升级为兼容性结论前，需要固定账户/模型/Provider/payload，并分别测试显式声明、账户注入、禁用、调用零次、多次、错误和 streaming terminal；不得保存 credential 或未脱敏 transcript。
