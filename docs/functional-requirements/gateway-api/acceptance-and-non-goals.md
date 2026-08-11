# 功能验收要求与非目标

## 状态

本文是[网关 API 域](README.md)的验收与非目标模块。验收项是功能需求文档的行为约束；"必须""不得""只允许"
是验收约束，不代表当前实现已经满足。代码、测试、probe 或真实运行已经证明的内容只写入
`implementation-status/`。

## 1. 功能验收要求

| ID     | 应被保护的用户可观察行为                                                                                                                                                           |
|--------|------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| API-01 | 有效静态 token 可访问标准/扩展模型与业务 endpoint；认证失败、未知 Public Model、不支持 feature 与非 JSON 请求在 egress 前安全失败。                                                |
| API-02 | 标准/扩展 Models 接口满足[模型能力契约](../model-capability/README.md)的身份、逐字段一致性与部署信息隔离要求。                                                      |
| API-03 | Native Chat/Responses 已知且被接口接受的请求字段除统一 instructions/store envelope、固定 Public Model reasoning 输入归一化、受信模型/认证改写、Provider wire mapping 及已验证的普通参数忽略规则外保持 wire 语义；未知请求顶层字段在 egress 前拒绝，上游响应中的未知合法字段/event 不因网关丢失。 |
| API-04 | SSE 分片、终态、EOF、上游 error 和下游 cancel 不会产生伪成功、重复 terminal 或跨 Upstream Target 拼接。                                                                            |
| API-05 | Chat/Responses 普通 function tool 的 call/result identity 与 fragmented arguments 在已声明路径中保持；生成链路不执行这些工具。                                                      |
| API-06 | Codex Native profile 能在受限 allowlist 下保留其已验证的 turn-state 扩展；bridge、route change 或 fallback 不会误复用该状态。                                                      |
| API-07 | 对 Codex、OpenAI SDK 或 Hermes 的兼容声明均有相应 endpoint/feature 的可重复证据，并写入实施现状而非仅引用设计。                                                                    |
| API-08 | 客户端只选择 Public Model 与下游协议；固定能力契约不支持时统一拒绝，普通忽略参数按选中 API 删除，其他支持请求保持配置 Route 顺序，不按请求能力筛选或重排候选。                       |
| API-09 | 无状态请求避开短时 cooldown 的 quota/fault scope；target-bound continuation 不因健康状态切换 issuing target。                                                                      |
| API-10 | reasoning input 只接受 canonical vocabulary 与 Public Model `accepted_levels` 的交集；`strict` 保持精确值，`clamp_positive_floor` 只在正向 effort 中解析到固定接口实际 `levels`，`none` 不参与转换；有效值再按选定 Upstream API 的已校验规则改写，未知值、歧义源或非法目标在 egress 前失败。   |
| API-11 | 无状态 Responses 是核心兼容面、默认使用方式和当前验收基线；`store` 省略或 false 均规范化为每个 Responses egress 的显式 false，true 在 egress 前拒绝；非空 `previous_response_id` 与 `background:true` 仍属次要且不完整的 Native 目标。 |
| API-12 | Embeddings、图片、文件与音频分别满足[扩展共同规则](../extended-capabilities/embedding-and-native-multimodal.md)及其功能页的 wire、能力、资源归属、限制和证据边界。                                             |
| API-13 | token-bearing text/tool/reasoning SSE delta 只触发一次 TTFT/生成窗口，非流式 Chat/Responses 成功 JSON 只在首个非空下游 body chunk 记录一次可直接观测的 gateway TTFT，不得据此伪造 upstream TTFT、生成时长或输出速度；OTLP metrics 不含请求正文、响应正文、Authorization、credential、用户或 request ID。 |
| API-14 | 有效静态 token 可通过 `POST /mcp` 发现唯一静态 `hello(name: string)` 工具并取得 `Hi, {name}!`；Origin、transport metadata、无效参数、未知工具/method 与非 POST method 按固定边界失败，且调用不访问 Provider 或外部系统。 |
| API-15 | `include: []` 作为 no-op 在全部 egress 前移除；非空 `include` 按 Public Model 逐值交集预检，未知或 Bridge 不可保真的投影 zero-egress 失败；`prompt_cache_key` 只在固定候选全部支持时原样转发，且不承诺缓存效果。 |
| API-16 | Chat `stream:true` 下的空 `stream_options` 与 `include_usage:false` 作为 no-op 在能力预检和 egress 前移除；有效 `include_usage:true` 只有在固定 Chat interface 完整保证时接受，Native 原样保留，Chat→Responses Bridge 从合法 terminal usage 生成标准 usage-only 尾块，非法形状、Responses 顶层字段和缺失/非法 terminal usage 均 fail closed。 |
| API-17 | 通用 Generation 只解析一次客户端 instructions 来源并在缺失时使用项目默认值；Native/Bridge/候选/重试/probe 编码一致，首条合格 Chat 指令只提升删除一次，后续 transcript 保序，专用 task 不注入。 |
| API-18 | Responses `reasoning.summary` 接受 `"auto"` 与兼容 `false`：Native 精确保留，Responses→Chat 消费且只返回真实 Chat `reasoning_content` 对应的 Responses reasoning content，不伪造 summary；非法值与 `none+auto` 在 egress 前失败。 |
| OBS-01 | OTLP exporter 默认禁用；只有合法的 startup-only OTLP/HTTP 配置能启用相应 signal，collector host 可由配置所有者选择，非法配置在 listener 和 exporter egress 前失败，业务请求无法覆盖。 |
| OBS-02 | 一个已认证业务请求产生一个脱敏 request root span，每个实际 Provider attempt 产生一个有序 child span；terminal、retry、fallback、失败与取消不重复也不改变实际因果关系。       |
| OBS-03 | OTLP metrics 使用 SDK 原生 counter/histogram 和有界维度；单 attempt output speed 只由明确 output usage 与 generation duration 计算，分位数、平均值、错误率、缓存 token 比例与 Provider + Public Model 排名由外部系统计算，未知值不补零。 |
| OBS-04 | OTLP logs 只导出安全、限频且可通过 trace/span id 关联的运行诊断；不记录逐 chunk/delta，也不复制完整 request/attempt terminal 形成冗余高频日志。                              |
| OBS-05 | export 使用有界异步队列和有界关闭；collector 故障、超时或背压不阻塞请求、不改变 HTTP/SSE/Provider 行为，只允许丢弃 telemetry 并产生限频本地诊断。                          |
| OBS-06 | 所有 signals 都不包含 credential、Authorization、用户身份、业务正文、tool/reasoning 内容、原始错误正文、query 或真实 endpoint URL；metric attributes 不含高基数身份。       |
| OBS-07 | OTLP metrics 覆盖 request/attempt、韧性、timing、usage 与 cache 事实后，旧 metrics HTTP endpoint 和自定义 snapshot 聚合保持删除，不为未发布原型保留兼容垫片。 |
| OBS-08 | 本地下游 HTTP header/body 日志由四个彼此独立的 bootstrap 开关控制；随附开发配置显式全开、缺表/缺字段时回退关闭，只覆盖认证后客户端边界，敏感 header 强制脱敏、body capture 有界且每个方向最多一个终态事件，并保持 OTLP exclusion。 |

## 2. 非目标

- GUI、Web 控制台、客户端安装/注册/配置管理；
- Realtime、Responses WebSocket、Files、Images、Videos、Conversations、管理 API 或"实现全部 OpenAI API"；
- 保存、查询、删除、翻译或跨 Provider/Target 迁移 response 状态，以及未有真实需求前实现 continuation ledger；
- 让 Chat ↔ Responses、任何 tool 或 Provider 私有扩展自动无损互转；
- 代表下游 Agent 执行任意 function tool、shell、computer 或网页操作；
- 在 MCP endpoint 中执行 `hello` 以外的工具、桥接 Provider、产生外部 side effect、兼容旧版 session lifecycle 或提供浏览器 Origin allowlist；
- 用 API token 建立多用户权限、配额、账单或审计系统。

## 关联文档

- [网关 API 域导航](README.md)
- [产品范围](../product-scope/product-scope.md)
- [Public Model 与模型能力契约](../model-capability/README.md)
- [配置与凭证](../configuration-credentials/README.md)
- [路由与 Provider 韧性](../routing-resilience/provider-resilience.md)
- [交付与证据要求](../delivery-evidence/delivery-and-evidence.md)
- [当前实现总览](../../implementation-status/current-implementation.md)
