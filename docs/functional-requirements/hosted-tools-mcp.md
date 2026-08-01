# Provider-hosted tool facade：后续技术方向

## 状态

**后续方向。** 本能力不属于当前基础目标，也不依赖 Chat/Responses Protocol Bridge 完成。它与 Anthropic Messages 兼容同级；仅在出现明确用户结果后，以单个行为的 TDD 开始。

初始候选是将 OpenAI Responses 原生 `web_search` 结果规范化为本地 MCP tool 或显式 OpenBridge tool endpoint。

## 1. 术语

必须区分三类能力：

| 名称 | 责任 |
|---|---|
| Protocol Bridge | 在 Chat、Responses、Messages 等模型协议之间转换 wire-level message/tool call/result。 |
| Tool Bridge | 将本地函数或 MCP server 的工具提供给 Agent，并由客户端/本地执行器执行。 |
| Hosted Tool Facade | OpenBridge 调用 Provider 原生托管工具，解析 Provider 执行结果，再返回独立工具结果。 |

Provider 返回的 `web_search_call` 等 hosted item 不是普通 client-side `function_call`，不能伪造对应 `function_call_output`。

## 2. 目标

- 复用 OpenBridge 已配置的 Provider、Upstream Target/Offering、credential、HTTP transport 和 cancellation；
- 仅暴露经过显式 capability 验证的 hosted tool；
- 将 Provider-specific output 规范化为稳定、可测试的 ToolResult；
- 保留 answer、citation/source 和安全 request correlation；
- 在单用户本地环境中提供简单超时、最大输出和可选成本上限；
- 不把任意 Provider 请求或任意 URL 变成通用代理工具。

## 3. 初始范围

首个候选：

```text
openai_web_search
  → native OpenAI Responses request with web_search tool
  → wait for terminal response
  → extract answer, citations and sources
  → return MCP ToolResult / structured result
```

第一版：

- 只使用一个明确配置的 native Responses Upstream Target/Offering；
- 只支持单次、无会话的 search request；
- 使用 `stdio` MCP transport 或同进程模块；
- 返回 text content 和 schema-valid structured content；
- 生成轻量 CallRecord 并复用核心调用统计口径；
- 不支持远程多用户 MCP transport。

## 4. 非目标

- 通用 Provider request tunnel；
- 任意 URL/header/credential 输入；
- 将 hosted tool 转换成模型 function tool 的假等价；
- 代表 Agent 自动执行任意网页动作或有副作用操作；
- 多租户 scope、配额、计费或合规审计；
- 在 OpenBridge 核心未收敛前建立复杂 MCP gateway；
- 保证所有 MCP client 都能把 citation 渲染为可点击 UI。

## 5. 前置条件

Hosted Tool Facade 的真实前置条件：

1. 至少一个 Offering 原生支持目标 hosted tool；
2. Provider adapter 能识别请求、输出、terminal、error、cancel 和 usage；
3. capability 明确为 `Native`，不能由 Protocol Bridge 推断；
4. 非 loopback 使用静态下游 token/TLS；
5. 单次调用的 timeout、最大结果、并发上限和可选成本上限已配置；
6. 至少一个目标 MCP client 完成结构化结果和 citation 消费实验。

Chat/Responses bridge 不是硬前置。相反，`web_search` 首版必须走 native Responses route。

## 6. 组件边界

候选部署形态：

| 方案 | 优点 | 缺点 | 当前倾向 |
|---|---|---|---|
| 同进程模块 | 复用 config/transport 最直接，部署简单 | MCP runtime 与 proxy 生命周期耦合 | 首版候选 |
| 独立 sidecar | 故障和依赖隔离 | 需要本地认证与额外进程 | 后续候选 |
| 独立 MCP server 调用 OpenBridge | 边界最清晰 | 需要定义额外内部 API | 待比较 |
| 不提供 MCP，只保留 native API | 范围最小 | 客户端需自行支持 hosted tool | 始终保留的退路 |

无论形态如何，不复制 credential 解析和 Provider HTTP adapter。

## 7. Tool 契约

候选输入：

```json
{
  "query": "string",
  "max_results": 5,
  "search_context_size": "low|medium|high",
  "user_location": {
    "country": "optional",
    "city": "optional",
    "region": "optional",
    "timezone": "optional"
  }
}
```

所有字段都必须有大小/枚举限制。客户端不能提供：

- Provider/model/base URL；
- Authorization/cookie/header；
- redirect/callback；
- arbitrary HTTP method/body；
- credential reference。

候选结构化输出：

```json
{
  "answer": "...",
  "citations": [
    {
      "start": 0,
      "end": 12,
      "url": "https://example.com",
      "title": "Example"
    }
  ],
  "sources": [
    {
      "url": "https://example.com",
      "title": "Example"
    }
  ],
  "provider": "openai",
  "upstream_target": "openai-search",
  "request_id": "obr_...",
  "usage": {
    "input_tokens": 0,
    "output_tokens": 0
  }
}
```

`citations` 表示 answer 中的具体引用范围；`sources` 是来源集合，二者不能混为一谈。

MCP 返回同时包含：

- 人类可读 text content；
- 与 output schema 一致的 `structuredContent`。

## 8. Citation 语义

- citation range 只相对于 facade 返回的 `answer`；
- 外层 Agent 改写 answer 后，原 range 不再自动有效；
- URL、title、annotation 和 source list 都按不可信 Provider/网页输入处理；
- 不把网页标题或 URL 放入日志格式字符串/HTML 而不转义；
- malformed range、重复 URL、缺失 title/source 必须有确定处理；
- 若目标 MCP client 无法保留 citation，可返回结构化 sources，但必须标明 UI delivery 未验证。

## 9. 状态与错误

建议单次状态：

```text
Accepted
→ ProviderRunning
→ Completed | Failed | Cancelled | TimedOut
```

错误分类：

```text
hosted_tool_unsupported
invalid_tool_input
provider_auth_error
provider_rate_limited
provider_error
provider_stream_incomplete
tool_timeout
tool_cancelled
citation_parse_error
result_too_large
```

已收到部分 Provider output 时，不自动切换到另一个 Provider 拼接答案。首版可以完全不做 hosted-tool fallback。

## 10. 最小安全和资源约束

即使是单用户，也必须：

- 不接受任意出站 URL/header/credential；
- 禁用不受控 redirect；
- 限制 query、Provider response、citation/source 数量；
- 设置 call timeout 和最大并发；
- 支持 client cancellation；
- 不记录完整 credential、cookie 或 Provider payload；
- 仅在用户明确配置的 Upstream Target/Offering 上执行；
- 对可能产生明显费用的 context size/结果规模提供本地上限。

这些是单用户服务的资源保护，不是多租户配额系统。

## 11. 调用统计

请求结束后可写入普通 `CallRecord`：

```text
request id
tool name
Provider/Upstream Target/Offering
terminal outcome / error class
gateway latency / first output time
input/output tokens
estimated cost
citation/source counts
```

默认不记录 query/answer 正文。JSONL 等本地 sink 故障不应阻塞已完成的 tool result；准确口径和输出边界遵循[调用统计与可观测性](observability.md)。

## 12. 开始时需要回答的问题

选择一个 hosted-tool 行为进入当前焦点前，按需回答：

- 哪个目标 MCP client 和具体工作流确实需要该能力，output schema 与 citation UI 如何消费；
- 同进程、sidecar、独立 MCP server 或不实施中，哪种边界最小；
- 对应 native Provider 的脱敏 request、terminal response、citation/source、usage、error 和 cancel 样本是什么；
- Provider output 如何规范化为稳定 ToolResult，以及 malformed annotation 和结果上限如何测试；
- 是否需要 `stdio` transport、structured/text content、cancel/timeout 或 CallRecord；
- 是否真的需要 sidecar/独立进程，以及它是否会改变核心 Protocol Bridge 语义。

这些问题不是预定义切片；只为当前选择的单一行为补足所需测试和设计。

## 13. 不可突破的设计边界

- hosted tool 只走 native capability；
- 业务输入不能扩大出站目标或 credential 权限；
- terminal/error/cancel/timeout 行为可重复；
- citation 和 source 结构通过真实 fixture；
- 至少一个目标 MCP client 消费结构化结果；
- 无法支持时可关闭该模块，不影响核心 Chat/Responses proxy；
- 不引入 principal、配额、合规审计或独立控制面作为强依赖。

## 14. 关联文档

- [产品范围](product-scope.md)
- [调用统计与可观测性](observability.md)
- [服务架构](../implementation-plans/service-architecture.md)
- [配置与路由](../implementation-plans/configuration-and-routing.md)
- [Protocol Bridge 设计](../implementation-plans/protocol-bridge.md)
- MCP tools specification：https://modelcontextprotocol.io/specification/2025-06-18/server/tools
