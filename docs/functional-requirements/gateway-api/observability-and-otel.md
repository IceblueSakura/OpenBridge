# 运行期观测与 OpenTelemetry 导出

## 状态

本文是[网关 API 域](README.md)的观测模块：定义 OpenTelemetry traces/metrics/logs 的 Signal 所有权、
配置安全边界与导出运行时约束。其他模块见[网关 API 域](README.md)导航。

OpenTelemetry 是可选的 headless 出站观测通道，不是新的下游管理 API。OpenBridge 只负责在协议生命周期边界产生无法从外部重建的
原始事实；collector/backend 负责持久化、窗口查询、分位数、错误率、缓存 token 比例、Provider + Public Model 比较和
可视化。缺失的 Provider usage、cache 或非流式 upstream TTFT 必须保持"未观测"，不得补零或由 gateway body 到达时间伪造。

## 1. Signal 所有权

- **Traces**：每个已认证业务请求形成一个 `downstream_request` root span；每个实际出站的 Provider attempt 形成一个有序 child
  span。span 只记录稳定 operation、Public Model、编译期 Provider/Target/Route、Native/Bridged、streaming、低基数 outcome、
  已直接观测的 timing 与 Provider 明确返回的 usage。retry、fallback、取消和 terminal 必须保持实际因果关系且每个 span 只结束一次。
- **Metrics**：只提交用于外部计算的原始 counter/histogram，包括 request/attempt outcome、TTFT、response-ready、duration、
  generation duration、input/output/cache token，以及仅在明确 output usage 和 generation duration 同时存在时计算的单 attempt
  output speed。metric attributes 只允许有界的 Provider、Public Model、upstream model、typed operation、Route/Target、Route mode、
  streaming 和 outcome；request id、trace id、user、HTTP status 原值、错误文本或 endpoint URL 不得成为 metric attribute。平均值、
  分位数、cache ratio、error rate 或 Provider 排名由外部系统计算。
- **Logs**：导出启动、关闭、exporter 状态和需要人工诊断的安全结构化事件，并通过 trace/span id 关联业务 trace。不得为每个 SSE
  chunk/delta 产生日志，也不得把已经由 attempt/request span terminal 完整表达的事实再复制为一组高频业务日志。
- **本地开发内容日志**：Bootstrap 的四个独立开关可以分别记录认证后下游 request header/body 与最终 response header/body；
  仓库随附开发配置显式全开，自定义配置缺表或缺字段时对应回退关闭。
  header 值先强制脱敏认证、Cookie 和 secret-like 名称；body 只保留既有 request/JSON-response budget 内的有界 snapshot，长流明确
  标记截断且每个方向最多一个事件。该本地 formatter 事件不进入 span-only OTLP layer，不得被解释为原始 Provider wire dump。

OpenBridge 不执行下游 Agent 的工具，不能从 tool arguments、tool result 文本或下一轮 prompt 猜测工具是否执行成功；实际 tool error
rate 只有在未来存在显式、低基数且不携带业务内容的客户端 outcome 契约时才可统计。本次迁移只保留已有的协议级 tool 生命周期事实，
不为获取工具错误率增加正文解析或日志采集。

## 2. 配置、安全与运行时边界

- exporter 默认禁用，只能由 bootstrap 显式启用并提供受限 URL shape 的 OTLP/HTTP collector；配置所有者可以选择 loopback、非
  loopback IP 或 DNS host，业务请求不能选择 endpoint、protocol、header、resource attribute 或采样策略。无效 scheme、缺失 host、
  URL credential、path、query、fragment 或不支持字段必须在 listener 与 exporter egress 前失败。
- 所有 signals 使用固定 `service.name = "openbridge"` 和本次进程资源身份。traces 可携带 request id 以供关联；任何 signal 都不得包含
  Authorization、credential、用户身份、请求/响应正文、tool arguments/result、reasoning 正文、原始上游错误正文、query 或真实
  endpoint URL。
- 本地开发内容日志不是 OTLP signal；即使显式启用，认证、Cookie、credential 与 secret-like header 值仍不得进入日志。body snapshot
  可能包含受控开发业务内容；随附开发配置显式全开，生产部署必须由 bootstrap 所有者按需关闭。
- request hot path 只写入内存中的有界 signal primitive；网络 export 必须批处理并与请求异步隔离。队列满、collector 不可达或 export
  timeout 只能丢弃观测并产生有界、限频的本地诊断，不能改变下游状态、重试、fallback、取消或 Provider 结果。关闭时 flush 也必须有界。
- metrics 只通过启动时配置的 OTLP/HTTP 出站；不得保留自定义进程内累计查询 API，旧
  `/openbridge/v1/metrics` 与 `/openbridge/v1/metrics/providers` 必须保持未注册。
- OpenBridge 不内置 collector、SQLite、历史数据库、dashboard、Prometheus endpoint 或分布式聚合；这些属于外部部署和分析程序。

## 关联文档

- [网关 API 域导航](README.md)
- [配置与凭证](../configuration-credentials/README.md)
- [当前实现总览](../../implementation-status/current-implementation.md)
- [遥测指标实施现状](../../implementation-status/telemetry-metrics.md)
