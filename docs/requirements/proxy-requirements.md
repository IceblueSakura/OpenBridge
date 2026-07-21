# OpenAI-compatible AI Proxy：初版需求

## 状态

**初稿；目标需求，待实施。** 本文定义产品范围、外部契约和验收方向；具体实施顺序与退出条件以[开发计划](../plans/development-plan.md)为准。未确认的上游能力不得因本文存在而被视为可用。

## 1. 目标

构建一个主要服务于 Agent 的 OpenAI-compatible proxy。它聚合受控的多个上游 provider，对下游统一提供 Chat Completions 和 Responses 接口；在能力允许时完成 Chat ↔ Responses 转换；并由 proxy 自己管理上游 API key 或经确认可用的 OAuth2 credential。

proxy 的价值不只是统一 URL：它必须把 provider、上游模型、认证材料、协议能力和路由决策收敛在控制面，使下游 Agent 不接触上游 credential 或任意上游网络目标。

## 2. 范围、参与者与非目标

### 2.1 参与者

| 参与者 | 目标 |
|---|---|
| Agent client | 通过稳定的 public model alias 调用 Chat Completions 或 Responses，执行多轮 function tool loop，并消费流式结果。 |
| Proxy administrator | 管理 provider、credential、deployment、public alias、proxy-issued API key 和审计策略。 |
| Provider adapter | 按固定 provider 契约构造上游请求、认证头、协议转换和错误映射；不接受客户端配置。 |

### 2.2 当前范围

- `POST /v1/chat/completions` 与 `POST /v1/responses` 的流式和非流式请求。
- `/v1/models` 返回当前 principal 已获授权的 public model aliases。
- 多 provider、多 deployment 和稳定 alias；每个 provider 当前仅一个 active credential。
- 标准 API key upstream；经真实契约和条款 preflight 确认后才可接入 OAuth2 upstream。
- OpenAI-compatible function tool definitions、tool calls 与 tool outputs。
- Chat ↔ Responses 的请求、最终响应和 SSE 事件转换；每次有损转换均可检测。
- metadata-first 审计、请求关联、速率/并发限制和控制面管理接口。
- 在 capability、authorization 与 citation 契约明确后，将受控 provider-hosted tool 作为 MCP local tool result 暴露；初始目标为 OpenAI `web_search`，详见[Hosted tool MCP 暴露需求](hosted-tools-mcp.md)。

### 2.3 当前非目标

- 不承诺支持 OpenAI 全部资源 API、Realtime、Files、Conversations 或管理 API。
- 不承诺 Chat ↔ Responses 无损转换，也不静默删除 feature 后伪造成功。
- 不执行模型请求中的 function tool；proxy 仅转发或转换 wire-level tool call/result。
- 不支持同 provider 多 credential、多账号/工作区路由、credential pool 或 credential failover。
- 未实现 proxy-managed resource store 前，不承诺 Responses 的 retrieve、delete、input items、background、cancel 或 `store=true` 资源语义。
- 不允许客户端指定上游 URL、provider、credential、任意认证头或调试配置。

## 3. 产品原则

1. **Native first**：当 deployment 原生支持下游请求协议时直接转发；协议 bridge 仅用于 capability 明确允许的路径。
2. **Capability before call**：在上游调用前判定 endpoint、stream、tools、structured output、continuation、background/resource 等能力；不支持时返回明确错误。
3. **State affinity**：`previous_response_id`、opaque reasoning、encrypted content、tool continuation 和 proxy-managed resource 都绑定 issuing deployment/issuer；不得跨 provider 或不兼容 endpoint 重放。
4. **Route snapshot**：请求开始后固定 public alias、deployment、credential binding、capability decision 和 policy version；控制面更新不得改变进行中的 stream。
5. **No silent downgrade**：任何近似、丢弃、拒绝或 emulation 都必须产生 machine-readable `ConversionNotice` 和审计事实。
6. **Secret isolation**：下游 proxy key 与上游 API key/OAuth token 是独立安全域；数据面、日志和普通数据库不保存或返回明文 secret。

## 4. 功能需求

### 4.1 下游 API 与路由

| ID | 需求 | 验收方向 |
|---|---|---|
| FR-01 | proxy 必须支持 `/v1/chat/completions` 与 `/v1/responses` 的 JSON 和 SSE 响应。 | OpenAI Python/Node SDK 与首批 Agent 在 stream/non-stream fixture 上可消费。 |
| FR-02 | client 的 `model` 只能解析为 public model alias；alias 解析、principal 授权、capability gate 和 candidate selection 必须在上游调用前完成。 | 未知/未授权 alias 或 endpoint 返回明确 4xx，且不产生上游调用。 |
| FR-03 | `/v1/models` 只返回当前 principal 可调用的 aliases，不暴露内部 deployment、provider 或 credential 数量。 | 不同 principal 获得经授权过滤后的结果。 |
| FR-04 | 每个请求必须生成不可变 `RouteSnapshot`，用于实际调用、审计和 ConversionNotice。 | 同一 stream 内的路由不因控制面更新而变化。 |
| FR-05 | fallback 仅可发生在 capability 与 state affinity 仍满足时；含 provider-bound continuation 或已输出业务 SSE 的请求不得透明切换 provider。 | fallback 的 candidate、原因、次数和最终 route 可审计。 |

### 4.2 Provider、认证与控制面

| ID | 需求 | 验收方向 |
|---|---|---|
| FR-06 | provider 只能由编译期 catalog 中的 adapter 实现；运行时配置仅选择已编译 provider、endpoint profile、deployment 和策略。 | 配置不能注入任意 URL、header、认证方案或 transform。 |
| FR-07 | 每个 provider 当前只允许一个 active `ProviderCredential`，其类型为 `api_key` 或 `oauth`。 | 数据层与控制面均不存在 credential selector/pool/failover。 |
| FR-08 | API key 与 OAuth material 必须仅以 vault secret reference + version 方式被控制面引用；account/workspace binding 与所需非 secret auth context 属于不可拆分的 credential route metadata；adapter 在发送上游请求前短时构造认证头。 | 普通 DB、HTTP response、audit、trace、error、queue 和 fixture 中不存在 secret；refresh/fallback 不会跨 account/workspace 复用 header 或 token。 |
| FR-09 | OAuth2 upstream 仅在确认合法适用的 client registration、redirect URI、issuer、scope/resource、token/refresh contract 与条款后启用；Codex path 不得导入 `auth.json`、复用 Codex CLI client registration，或模拟其 loopback login。 | preflight 证据完备；否则保持 mock OAuth adapter 或 API-key upstream。 |
| FR-10 | OAuth credential 必须支持单飞 refresh、secret-version CAS、完整 rotated credential bundle 的原子提交、`NeedsLogin → Active → Refreshing → Active | NeedsReauth | Revoked` 状态机，以及受控的 401 refresh/retry。 | 并发近过期请求只发生一次 refresh；`invalid_grant` 后停止自动 refresh；refresh 后 token 与 account/workspace metadata 一致。 |
| FR-11 | proxy 必须签发可撤销的 opaque downstream API key，并按 principal 执行 alias、endpoint、tools、stream、conversion、resource、RPM/TPM/concurrency 授权。 | 无权访问在上游调用前拒绝；撤销/过期在既定 cache 预算内生效。 |

### 4.3 Protocol conversion 与 tools

| ID | 需求 | 验收方向 |
|---|---|---|
| FR-12 | bridge 必须使用 `wire → Canonical IR → wire`；不得以字段 rename 代替语义转换。 | 文本、multimodal item、function schema、tool call、tool output、usage 和 status 都由 fixture 覆盖。 |
| FR-13 | `call_id` 是 tool invocation 与 tool output 的唯一关联键；`item_id`、`response_id` 与 `output_index` 不得互换。 | 并行 tool calls 可分别匹配 output；无关联键的 tool output 被拒绝。 |
| FR-14 | provider-native/builtin tools、background/resource API、opaque reasoning 和 `previous_response_id` 必须由 capability gate 决定 native、emulated 或 reject。 | 不可等价映射的能力不会静默删除或跨 provider 透传。 |
| FR-15 | 每个降级必须返回或记录 `ConversionNotice`，至少包含 `code`、source/target protocol、deployment、action（`dropped`/`approximated`/`rejected`/`emulated`）及关联 item/call id（如适用）。 | 所有有损 fixture 都能断言 notice 和 audit event。 |
| FR-16 | bridge 必须具备 re-entry guard，避免 Responses fallback 到 Chat 后再次被路由回同一 bridge。 | 对每类 bridge 路径执行无递归调用测试。 |

### 4.4 SSE、stream 与取消

| ID | 需求 | 验收方向 |
|---|---|---|
| FR-17 | SSE parser 必须按 UTF-8、SSE field 和空行 event boundary 解析；不得以 TCP/HTTP chunk 为 JSON 或 event 边界。 | fragmented UTF-8、多 event 同 chunk、注释、空 `data:` 与未知 event fixture 通过。 |
| FR-18 | 每个 request 维护独立的 `StreamAssembly`，追踪 response/item/call/output-index identity、text/reasoning/tool-argument buffer、usage、terminal state 与 provider data。 | 并行 tool call、arguments 分片、text/tool 交错不会串请求或过早结束。 |
| FR-19 | `response.output_item.done` 不得视为 Responses stream terminal；正常 SSE terminal 仅按实际协议 event 判定。 | `item.done` 后的 output 或 terminal event 仍被正确转发。 |
| FR-20 | 下游 client 取消、上游 transport error、协议 terminal、EOF before terminal 和 idle timeout 必须形成可区分 outcome。 | 每个 outcome 只产生一次 terminal API/audit 结果。 |
| FR-21 | terminal 缺失但已有可验证 output 时，仅可走显式 `recovered_terminal_missing` 分支，并保留诊断；没有可用 output 时必须报错。 | 不把不完整 stream 静默伪装为正常完成。 |
| FR-22 | client disconnect 后必须在有限 deadline 内取消/关闭上游请求并释放并发、credential 和 buffer 资源；已输出业务 SSE 后不得拼接重试。 | slow client、disconnect、上游断流与 retry boundary 的集成测试通过。 |

### 4.5 Responses resource 与可观测性

| ID | 需求 | 验收方向 |
|---|---|---|
| FR-23 | 第一阶段 Responses 为 transient response；resource endpoint/background/store capability 默认拒绝，直到定义 proxy-managed store 或验证原生安全透传的权限、TTL、删除和恢复语义。 | 所有未支持 resource 请求在上游调用前返回可识别错误。 |
| FR-24 | 默认只记录 metadata：request ids、principal/key locator、public alias、deployment、credential state/id、outcome、error class、attempt、TTFT、duration、usage 与 conversion notice。 | secret scan 和 redaction 测试证明不含 OAuth material、API key、cookie、完整 prompt/response、tool arguments。 |
| FR-25 | audit/metrics 写入必须经有界异步 outbox；stream 热路径不得逐 token 同步落库。 | audit sink 故障、队列饱和和慢 client 有确定的 fail-open/fail-closed 策略，无无界内存增长。 |

### 4.6 Provider-hosted tool 的 MCP facade

| ID | 需求 | 验收方向 |
|---|---|---|
| FR-26 | OpenBridge 可将经过 capability 和 principal scope 验证的 provider-hosted tool 作为 MCP tool 暴露；初始仅支持 `openai_web_search`。 | route 不具备原生 Responses/web search/citation 能力时，在上游调用前返回可识别错误。 |
| FR-27 | MCP facade 必须使用 OpenBridge 的 RouteSnapshot、provider adapter、credential binding、限流和 metadata audit；不得自行读取上游 credential、接受任意 base URL/header 或复制 OAuth/HTTP adapter。 | MCP 请求不能扩大出站目标、credential 或 route 权限；facade audit 可关联 proxy request。 |
| FR-28 | provider-hosted tool 的 `*_call` 由 provider 执行；facade 只消费 terminal response 并返回 MCP ToolResult，不得伪造 client-side `function_call` 或 `function_call_output`。 | fixture 证明 `web_search_call` 不触发本地 tool executor，也不会生成伪 tool output。 |
| FR-29 | MCP tool 必须声明 input/output schema，并返回同时包含 text content 和 schema-valid `structuredContent` 的稳定结果；OpenAI web search 结果至少含 answer、citation、source list 与安全的 request correlation id。 | MCP reference client 可验证 schema；不支持 structured content 的 client 仍能从 text content 读取等价 JSON。 |
| FR-30 | citation 的 URL、标题与相对 `answer` 的范围必须在 facade 输出中保留；source list 与实际 citation 语义必须区分。 | 真实或脱敏 fixture 的多 citation、重复 URL、无 source list 与 malformed annotation 有明确、可测试结果。 |
| FR-31 | 面向最终用户展示的 citation 由 MCP client/UI 负责；若目标 integration 无法保留可点击来源，必须明确降级或拒绝，而非静默输出无来源文本。 | 至少一个目标 MCP client 完成可点击 citation 的端到端验证，或明确报告 `citation_delivery_unsupported`。 |

## 5. 质量、安全与兼容性要求

### 5.1 安全

- 上游 host、base URL、header allowlist 和 credential binding 必须是控制面配置，不能由 client request 覆盖。
- OAuth authorization code、PKCE verifier、state、access/refresh token、Authorization、cookie 和下游 API key 必须纳入 secret scan/redaction 范围。
- 浏览器 OAuth callback 必须验证 state 的单次消费、短 TTL、发起管理会话绑定、redirect URI、issuer 和 provider binding。
- Phase 1–3 仅运行于 loopback 或受信私网；完成 inbound key、授权、限流和最小审计后才可提供共享网络访问。

### 5.2 兼容性

- 首批目标是 OpenAI Python/Node SDK 和选定 Agent；兼容性按版本化 fixture 和实际请求验证，而不只按字段列表判断。
- 未知上游 SSE event 和新增字段必须安全处理并保留协议前向兼容性；是否透传、忽略或降级由 adapter 明确规定。
- provider adapter 对请求、响应、SSE、错误、限流 header 和认证行为拥有边界清晰的实现；核心路由器不得积累 provider-specific if/else。

### 5.3 性能与容量

- 每个 principal、deployment 和 provider credential 必须支持独立的 requests、tokens、并发 streams 与 stream duration 限额。
- proxy 必须记录 ingress、route、upstream first byte、first emitted event/token、terminal 的分段时间，并能区分认证、转换、上游和下游背压时间。
- request body、SSE event、tool arguments、Canonical IR、reasoning/provider state 和 slow-client buffer 都必须有可配置上限。

## 6. 首批调研与验证 backlog

| 优先级 | 调研项 | 最小产物 | 完成门 |
|---|---|---|---|
| P0 | Provider Capability Matrix | 每个 deployment 的 endpoint、stream、tools、reasoning、resource、auth、limit 与 `native/bridge/reject` 结论 | 每项有官方资料、脱敏实测或明确 `unknown`。 |
| P0 | 原始 SSE fixture corpus | 每个首批 provider 的 text、parallel tools、partial arguments、reasoning、error、EOF、cancel、unknown-event 脱敏 transcript | parser/assembler contract test 可离线运行。 |
| P0 | Agent Compatibility Matrix | Hermes、Codex、OpenAI SDK 和选定 Agent 的端点、严格字段、tool loop、SSE、timeout/retry/cancel 行为 | 首批 client 的 stream/non-stream E2E 通过。 |
| P0 | Codex OAuth preflight | 可用 client registration、redirect URI、scope/resource、token/refresh、account/workspace、条款证据 | 明确 `implementable` 或 `blocked`；blocked 时不接入真实 OAuth。 |
| P1 | Responses resource semantics | `store`、retrieve/delete/input_items/cancel/background 的 native、proxy-store、reject 决策 | resource API 的权限、TTL、删除、重启恢复规则成文。 |
| P1 | 错误、重试与幂等性 | 统一错误分类、retry matrix、已输出 stream 和 tool side effect 边界 | 所有错误类都有 API、SSE、audit 和 retry 预期。 |
| P1 | 容量与背压 | 并发 stream、slow client、credential refresh storm、outbox 饱和的测量与上限 | 负载/soak 基线和拒绝/降级策略通过。 |
| P1 | Hosted tools as MCP | OpenAI `web_search` 的 MCP input/output schema、provider response/citation fixture、scope/route/audit/cancellation 策略 | `stdio` MCP reference client 能消费 schema-valid 结果；无原生能力/权限时 fail closed。 |

## 7. 初始验收集

在进入共享网络部署前，至少满足：

1. 一个 API-key upstream 能原生完成 Chat 和 Responses 的 stream/non-stream 调用。
2. 两个不同 provider/deployment 能通过 alias、授权和 capability gate 被安全选择；不能支持的 feature 在上游调用前拒绝。
3. OpenAI SDK 与首个目标 Agent 可完成并行 function tool call → tool output → 后续调用闭环。
4. SSE parser 与 StreamAssembly 通过 fragment、unknown event、partial JSON arguments、EOF、timeout、cancel、terminal-missing fixture。
5. upstream/downstream credential 均不出现在普通日志、audit、trace、DB、error 或 fixture。
6. OAuth preflight 已明确结论；若未通过，真实 Codex OAuth 功能仍处于 blocked，API-key provider 路径不受影响。
7. 所有 bridge 降级和 resource reject 都返回 machine-readable reason，并写入 metadata-only audit。

## 8. 外部证据与调研限制

- OpenAI Responses API reference 定义了 Responses 的 create/retrieve/delete/cancel/input-items 等资源面；因此 proxy 必须明确 resource 的 native、proxy-managed 或 reject 语义，而不能只实现 create 后伪造 response id。<https://platform.openai.com/docs/api-reference/responses/create>
- OpenAI function calling 将 function call output 关联到特定 `call_id`；tool loop 是多轮协议闭环，不是单次文本生成。<https://platform.openai.com/docs/guides/function-calling>
- OpenAI API reference 明确可能新增 streaming event type；adapter 必须对未知 event 保持前向兼容。<https://platform.openai.com/docs/api-reference/introduction>
- Anthropic 的官方 streaming 文档显示 tool input 可作为 partial JSON delta 传输，unknown event 可能随版本加入；这验证了 `StreamAssembly`、延迟 JSON parse 和 unknown-event policy 的必要性。<https://docs.anthropic.com/en/api/messages-streaming>
- Codex 官方认证文档说明本地 Codex 可使用 ChatGPT 或 API key 登录，并把 API key 作为自动化的推荐默认方式；该文档未提供可直接断言为 proxy OAuth client 的公开 registration/refresh 契约。因此真实 OAuth 仍是 preflight 硬门。<https://developers.openai.com/codex/auth>
- 本地 Codex 源码快照进一步显示 CLI 的 loopback/PKCE、token rotation、account-bound header 和本地工具闭环；这些是设计参考，不是第三方复用其 OAuth client 或私有 credential storage 的授权。[OAuth 与工具调用源码调研](../research/codex/oauth-and-tool-call-analysis.md)
- Hermes 与 LiteLLM 的当前源码均实现 ChatGPT/Codex subscription device-code login、refresh 与 account-bound header；Hermes 还维护本地 credential pool。它们均属本地客户端实现，不能作为第三方 proxy 复用 client identity、CLI credential file 或专用 header 行为的授权依据。[Hermes 与 LiteLLM OAuth 实现调研](../research/chatgpt-oauth/hermes-and-litellm-oauth-analysis.md)
- OAuth 2.0 Security BCP（RFC 9700）是 redirect flow、token replay 和 refresh token protection 的安全基线。<https://datatracker.ietf.org/doc/html/rfc9700>

本轮尚未采集真实 provider 流量，也未选择首批 provider/deployment 或 Agent 版本。所有 provider-specific capability 均应在 P0 Matrix 中由版本化资料和脱敏 fixture 重新确认。

## 9. 关联文档

- [架构与路线](../architecture/architecture-and-roadmap.md)
- [开发计划](../plans/development-plan.md)
- [Chat/Responses 转换设计](../design/chat-responses-conversion.md)
- [Hosted tool MCP 暴露需求](hosted-tools-mcp.md)
- [Codex OAuth 凭证边界](../design/codex-oauth-credential-boundary.md)
- [控制面、模型、密钥与可观测性](../architecture/control-plane-models-keys-and-observability.md)
- [Hermes Agent 协议分析](../research/hermes/chat-responses-analysis.md)
- [LiteLLM 协议分析](../research/litellm/chat-responses-analysis.md)
