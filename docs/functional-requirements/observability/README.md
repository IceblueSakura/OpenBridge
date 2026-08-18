# 运行期观测与 OpenTelemetry

本文定义 request/attempt lifecycle、OpenTelemetry signal、安全边界与本地下游 HTTP snapshot 行为。
Bootstrap schema、默认值和严格解析仍由[配置与凭证](../configuration-credentials/README.md)拥有；实现与验证事实只见
[实施现状](../../implementation-status/README.md)。

OpenTelemetry 是可选的 headless 出站通道，不是下游管理 API。OpenBridge 只产生协议生命周期中无法从外部
重建的原始事实；collector/backend 负责持久化、窗口查询、分位数、错误率、cache-token ratio、Provider/Public
Model 比较和可视化。缺失的 usage、cache 或 upstream timing 保持“未观测”，不得补零或从 gateway body 时间伪造。

## 1. Signal 所有权

- **Traces**：每个已认证业务请求形成一个 `downstream_request` root span；每个实际 Provider attempt 形成一个
  有序 child span。span 记录 request kind、稳定 operation、Public Model、编译期 Provider/Target/Route、Native/Bridged、
  streaming、低基数 outcome、直接观测的 timing/status 与 Provider 明确返回的 usage；`request_id` 只用于 trace
  correlation，不进入 metrics。失败 terminal 使用固定
  `error.type`、failure stage、retryable 与实际 next action；retry、credential rotation、fallback、cooldown skip 只以
  固定名称和字段的 allowlisted routing event 进入 OTLP，普通 tracing event 继续排除。
- **Metrics**：request/attempt started 使用 Counter；每个 terminal 恰好进入对应 duration Histogram，terminal count、
  failure count 与 active 数分别由 histogram count/outcome 和 `started - terminal_count` 得到，不重复设置 completed、
  failure 或 active instrument。独立 histograms 覆盖 downstream response-ready/TTFO、Provider response-ready/first-byte/
  TTFT/generation/attempt duration、input/output/cache/reasoning token 与成功 attempt 的 output speed。metric attributes
  只允许有界 Provider、经 registry planning 验证的 Public Model、upstream model、typed operation、Route/Target、mode、
  request kind、streaming、outcome、status class、规范化 error/stage/action/recovery；request id、trace id、user、
  未验证的请求 `model`、原始 status/error 或 endpoint URL 不得成为 attribute。
- **Logs**：普通进程日志只记录启动、关闭、exporter 状态与需要人工诊断的安全结构化事件；当前不配置 OTLP logs
  exporter。不为每个 SSE chunk/delta 记录日志，也不重复复制 request/attempt terminal。
- **本地开发内容日志**：四个独立开关分别控制认证后下游 request header/body 与最终 response header/body，并写入按 UTC 日期滚动的独立 JSONL 文件。
  header 强制脱敏；body 只保留既有 request/JSON-response budget 内的有界 snapshot，一个方向最多一个 terminal
  event。该事件不进入 stdout 或 reviewed OTLP trace layer，也不是原始 Provider wire dump。

OpenBridge 不执行下游 Agent 的 tool，不能从 arguments、result 文本或下一轮 prompt 推断 tool 是否执行成功；
没有显式低基数客户端 outcome contract 时，不统计业务 tool error rate。

## 2. 配置与运行时安全

- schema 省略对应 signal table 时 exporter 禁用，只能由启动 Bootstrap 显式启用并提供受限 OTLP/HTTP collector URL；随附开发
  profile 可以明确启用 loopback collector。业务请求不能选择
  endpoint、protocol、header、resource attribute 或 sampling policy。
- 配置所有者可以选择 loopback、非 loopback IP 或 DNS host；无效 scheme、缺失 host、URL credential、
  自定义 path/query/fragment 或未知字段必须在 listener 与 exporter egress 前阻止启动。
- signals 使用固定 `service.name = "openbridge"` 与本次进程资源身份。任何 signal 都不得包含 Authorization、
  credential、用户身份、业务正文、tool/reasoning 内容、原始上游 error body、query 或真实 endpoint URL。
- 本地开发内容日志不是 OTLP signal；即使显式启用，authentication、Cookie、credential 与 secret-like header
  值仍强制脱敏。body snapshot 可能包含受控开发业务内容，生产所有者必须按需要关闭。
- request hot path 只写入有界内存 primitive；网络 export 批处理并与请求异步隔离。队列满、collector 不可达、
  timeout 或背压只能丢弃 telemetry 并产生有界、限频本地诊断，不能改变 HTTP/SSE、retry、fallback、取消或
  Provider 结果。关闭 flush 也必须有界。
- metrics 只通过 OTLP/HTTP 出站；`/openbridge/v1/metrics`、`/openbridge/v1/metrics/providers` 及其他自定义
  snapshot 查询/重置 API 必须保持未注册。
- OpenBridge 不内置 collector、SQLite、历史数据库、dashboard、Prometheus endpoint 或分布式聚合。

## 3. Timing 与 usage

- downstream response headers ready、request-relative 首输出、Provider response headers ready、首 body byte、
  首个 token-bearing text/tool/reasoning delta 与 terminal 分别计时，不能互相冒充。
- TTFT 与 generation window 只由首个 token-bearing SSE delta 触发一次；metadata、tool item start、空 delta
  或仅有 response snapshot 不构成生成输出。
- 非流式 Chat/Responses 可以记录首个非空下游 JSON body chunk 的 gateway-visible timing，但不能据此制造
  upstream TTFT、generation duration 或 output speed。
- output speed 只在 attempt 成功且明确 output token usage 与 generation duration 同时存在时计算；平均值、分位数、
  error rate、cache ratio 与排名由外部系统计算。
- usage 只来自通过 endpoint 成功响应校验的 Provider 事实；不能估算缺失 token，也不能把 audio bytes、embedding
  vector 或 chunk count 当作 generation token。
- reasoning output token 只从 Provider 明确 usage 字段单独记录；它是 output token 的子集。total token 不设置独立
  metric，trace 可以保留 Provider 明确 total，外部系统只在口径允许时由 input/output 聚合。

## 4. 功能验收要求

| ID | 行为 |
|---|---|
| OBS-01 | Bootstrap 省略对应 table 时 OTLP exporter 禁用；只有合法的 startup-only OTLP/HTTP 配置能启用 signal，业务请求不能覆盖 collector 或安全策略。 |
| OBS-02 | 一个已认证业务请求产生一个脱敏 root span，每个实际 Provider attempt 产生一个有序 child span；失败使用固定 taxonomy，routing timeline 只允许固定名称/字段的 event，terminal、retry、fallback、失败与取消不重复且保持因果关系。 |
| OBS-03 | metrics 使用 SDK counter/histogram 与有界维度；terminal/failure/active 不设置可推导的重复 instrument；output speed 只由成功 attempt 的明确 usage 和 generation duration 计算，未知值不补零，聚合指标由外部系统计算。 |
| OBS-04 | 普通进程日志只保留安全、限频的运行诊断；当前不启用 OTLP logs，不记录逐 chunk/delta，也不重复 request/attempt terminal。 |
| OBS-05 | export 队列、timeout 与关闭有界；collector 故障或背压不阻塞请求、不改变协议或 Provider 行为。 |
| OBS-06 | signals 不包含 credential、Authorization、用户身份、业务正文、tool/reasoning 内容、原始 error body、query 或真实 endpoint URL；metric attributes 不含高基数身份。 |
| OBS-07 | metrics snapshot HTTP endpoint 和自定义进程内聚合保持删除，不为未发布原型保留兼容垫片。 |
| OBS-08 | 四个本地下游 HTTP 内容开关彼此独立；配置默认由配置域拥有，运行时只覆盖认证后客户端边界，敏感 header 强制脱敏、body 有界且每方向最多一个 terminal event，并保持 OTLP exclusion。 |

## 关联文档

- [配置与凭证](../configuration-credentials/README.md)
- [Native Path 与流式语义](../gateway-api/native-path-and-streaming.md)
- [路由与 Provider 韧性](../routing-resilience/README.md)
- [实施现状](../../implementation-status/README.md)
