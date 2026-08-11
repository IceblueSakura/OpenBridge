# 功能验收要求与非目标

下列 ID 是网关 API 的稳定行为约束。实施证据由[实施现状](../../implementation-status/README.md)单独记录。

## 1. 功能验收要求

| ID | 应被保护的用户可观察行为 |
|---|---|
| API-01 | 有效静态 token 可访问标准/扩展 Models 与业务 endpoint；认证失败、未知 Public Model、不支持 feature 与非法请求在 egress 前安全失败。 |
| API-02 | 标准/扩展 Models 接口满足[模型能力契约](../model-capability/README.md)的身份、逐字段一致性与部署信息隔离。 |
| API-03 | Native Chat/Responses 中已知且被接口接受的字段，除统一 instructions/store envelope、固定 reasoning 归一化、受信 model/auth 改写、Provider wire mapping 与闭合普通参数忽略规则外保持 wire 语义；未知请求顶层字段拒绝，上游响应的未知合法字段/event 不丢失。 |
| API-04 | SSE 分片、terminal、EOF、上游 error 与下游 cancel 不产生伪成功、重复 terminal 或跨 Target 拼接。 |
| API-05 | Chat/Responses 普通 function tool 的 call/result identity 与 fragmented arguments 在声明路径中保持；generation 链路不执行 tool。 |
| API-06 | Codex Native profile 只在受限 allowlist 内保留 turn-state 扩展；Bridge、Route change 或 fallback 不复用该状态。 |
| API-07 | Codex、OpenAI SDK 或 Hermes 等客户端专属承诺必须限定 endpoint、feature、transport 与版本，不得把专属 profile 扩大为通用兼容契约。 |
| API-08 | 客户端只选择 Public Model 与下游协议；固定契约不支持时拒绝，普通忽略参数只按选中 API 删除，其他请求保持固定 Route 顺序。 |
| API-09 | 无状态请求避开短时 cooldown 的 quota/fault scope；target-bound state 不因健康状态切换 issuing Target。 |
| API-10 | reasoning input 只接受 canonical vocabulary 与 Public Model `accepted_levels`；`strict` 保持精确值，`clamp_positive_floor` 只处理正向 effort，`none` 不参与转换；非法值在 egress 前失败。 |
| API-11 | 无状态 Responses 是默认契约：`store` 省略或 false 规范化为 Native egress 的显式 false，其他显式值拒绝；`background:false`/省略与 `previous_response_id:null`/省略可用，`background:true` 与非 null `previous_response_id` 在当前固定接口中 zero-egress 拒绝。 |
| API-12 | Embeddings、图片、文件与音频满足[扩展共同规则](../extended-capabilities/README.md)及各功能页的 wire、能力、资源归属和限制。 |
| API-13 | token-bearing text/tool/reasoning SSE delta 只触发一次 TTFT/generation window；非流式成功 JSON 的 gateway-visible body timing 不伪造 upstream TTFT、generation duration 或 output speed；telemetry 不含正文或身份 secret。 |
| API-14 | 有效 token 可通过 `/mcp` 使用 `2026-07-28` stateless discovery 或 legacy initialize/session lifecycle 发现并调用唯一 `hello(name)`；两种 lifecycle 都执行相同认证、Origin 与无 Provider egress 边界，非法 metadata/session/tool/method 在执行前失败。 |
| API-15 | `include: []` 作为 no-op 在 candidate 展开前移除；非空 `include` 按逐值交集预检，未知或 Bridge 不可保真的投影 zero-egress；`prompt_cache_key` 只在全部固定候选原样支持时转发且不承诺缓存效果。 |
| API-16 | Chat `stream:true` 下空 `stream_options` 与 `include_usage:false` 作为 no-op 移除；`include_usage:true` 只有固定 interface 完整保证时接受，Native 原样保留，Chat-to-Responses Bridge 只从合法 terminal usage 生成标准 usage-only 尾块。 |
| API-17 | 通用 Generation 只解析一次客户端 instructions 并在缺失时使用项目默认值；Native/Bridge/candidate/retry/probe 编码一致，首条合格 Chat 指令只提升删除一次，专用 task 不注入。 |
| API-18 | Responses `reasoning.summary` 接受 `"auto"` 与兼容 `false`：Native 精确保留，Responses-to-Chat 消费且只返回真实 Chat reasoning content，不伪造 summary；非法值与 `none+auto` 在 egress 前失败。 |

## 2. 非目标

- GUI、Web 控制台或客户端安装/注册/配置管理；
- Realtime、Responses WebSocket、Files、Images、Videos、Conversations、response resource 或管理 API；
- response storage、background job、查询、删除、翻译、跨 Provider/Target state migration 或 continuation ledger；
- 让 Chat/Responses、任意 tool 或 Provider 私有扩展自动无损互转；
- 代表下游 Agent 执行 function tool、shell、computer 或网页操作；
- 在 MCP endpoint 执行 `hello` 之外的 tool、桥接 Provider、产生外部 side effect 或提供 browser Origin allowlist；
- 用 API token 建立多用户权限、配额、账单或审计系统。

## 关联文档

- [网关 API 域](README.md)
- [产品范围](../product-scope/README.md)
- [Public Model 与模型能力契约](../model-capability/README.md)
- [配置与凭证](../configuration-credentials/README.md)
- [路由与 Provider 韧性](../routing-resilience/README.md)
- [运行期观测验收](../observability/README.md)
