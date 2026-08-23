# new-api 路由、计费与运维机制

## 文档元数据

| 字段 | 值 |
|---|---|
| Source snapshot | `QuantumNous/new-api` @ `2d8e50bf36e94200b809dfb39e73624ec48b1e23` |
| Last reverified | 2026-08-24，本地只读源码复核 |
| Scope | 渠道索引、priority/weight、retry、affinity、计费、性能指标和后台任务 |
| Evidence boundary | 静态源码；未连接数据库、Redis、支付系统或真实 Provider |
| Recheck trigger | channel cache、retry policy、billing session、quota math、system task 或 metrics 变化时 |

## 1. 渠道索引和选择

进程内缓存按 `group + model` 建立候选渠道索引：`model/channel_cache.go:19-76`。候选先按 priority 降序排列；请求选择时：

1. 查找精确模型，必要时尝试归一化模型名；
2. 收集不同 priority；
3. retry index 选择当前 priority 层；
4. 同一 priority 内按 weight 随机；
5. Advanced Custom 渠道还可按模型和请求路径过滤。

证据：`model/channel_cache.go:114-236`。同层权重为零或过小时会做平滑调整：
`model/channel_cache.go:181-204`。

这套策略面向大量同质 deployment 的流量分配。它不是 capability compiler：候选是否可用主要来自渠道配置、模型能力记录和状态，
不是每次请求对所有候选做保守能力交集。

## 2. Retry 和渠道故障

主请求循环位于 `controller/relay.go:184-244`。`RetryParam` 还保存 token group、模型、路径、自动分组索引和切组状态：
`service/channel_select.go:13-20`、`service/channel_select.go:48-161`。

每次失败会：

- 分类错误并决定是否 retry；
- 记录已使用渠道序列；
- 选择下一 priority 或自动 group；
- 按状态码策略判断是否继续；
- 符合条件时异步自动禁用渠道。

自动禁用入口：`controller/relay.go:365-370`。普通 relay 的可配置 retry 状态码位于
`controller/relay.go:331-360` 和 `setting/operation_setting/status_code_ranges.go:17-85`。

任务型 relay 在 `controller/relay.go:516-661` 另有相似的调用、retry 和结算流程，判断规则与普通请求并非完全共享，因此存在策略漂移风险。

## 3. 渠道 affinity

渠道 affinity 可按以下来源构造 key：

- context integer/string；
- request header；
- JSON path；
- group、model 和 rule name。

规则还定义 TTL、参数覆盖模板以及失败时是否禁止 retry：
`setting/operation_setting/channel_affinity_setting.go:5-36`。

默认规则覆盖 Codex `prompt_cache_key` 和 Claude `metadata.user_id`，并列出需要透传的 CLI/session headers：
`setting/operation_setting/channel_affinity_setting.go:39-148`。

该机制解决 prompt cache、session 或 upstream state 与特定渠道绑定的问题；它不等于通用负载均衡 sticky session，错误配置会降低
fallback 能力或把不可迁移状态送到错误渠道。

## 4. 可重放请求体

请求在进入重试前保存 body，每次 attempt 使用新的 reader：`controller/relay.go:208-229`。文本 handler 支持：

- passthrough：复用原始 body；
- converted：转换 DTO 后重新序列化。

证据：`relay/compatible_handler.go:73-113`。这保证第一次读取 body 后仍可 retry，但大 body、多模态和 Base64 路径仍需要关注
内存上限、临时存储、取消和清理。

## 5. 计费生命周期

请求前先估算 token 和价格，再创建 billing session 预扣额度：`controller/relay.go:148-171`、`service/billing.go:18-42`。
请求结束按实际 usage 结算：

```text
estimated quota
  → pre-consume/reserve
  → upstream execution
  → actual quota
  → settle delta
  → supplement or refund
```

差额处理见 `service/billing.go:49-95`。失败退款入口在 `controller/relay.go:173-182`。计费来源可区分钱包和订阅；任务型接口还支持
提交后、完成后的参数调整：`relay/channel/adapter.go:40-62`。

## 6. 数值安全和审计

额度列以 32-bit integer 为边界。`common/quota_math.go:10-153` 集中处理：

- float/decimal 到 quota 的统一舍入；
- NaN、overflow 和 underflow 饱和；
- strict variant 拒绝超界值；
- checked variant 返回 `QuotaClamp` 供日志审计。

预扣前会拒绝负数和已发生 clamp 的额度：`service/billing.go:20-35`。消费日志可把 saturation marker 放入管理员字段，避免算术溢出
把费用变成负数或静默损坏账本。

## 7. 性能与消费观测

性能样本覆盖：

- request/success count；
- latency；
- TTFT；
- output tokens；
- generation duration 和 token rate。

计算入口：`pkg/perf_metrics/metrics.go:27-55`。样本写入热 bucket 并异步持久化：
`pkg/perf_metrics/metrics.go:57-77`、`pkg/perf_metrics/flush.go:13-76`。

消费日志还记录模型、渠道、token、价格、转换链、retry 渠道序列和管理员审计字段。它是一套业务数据库导向的运营观测，不等同于
低基数 OpenTelemetry 指标模型。

## 8. 渠道巡检和后台任务

统一 system task runner 注册：

- 周期渠道测试；
- 上游模型列表变化检测；
- Midjourney polling；
- 通用异步任务 polling。

入口：`controller/system_task_handlers.go:15-152`。任务通过数据库 lease 在多个 master 间去重，并把每轮 payload、progress、result、
status 和 error 持久化：`model/system_task.go`。

渠道测试支持 bounded concurrency、每轮摘要、响应时间阈值和自动禁用：`controller/channel-test.go:902-960`。模型更新区分周期自动检测和
管理员手动强制检测：`controller/system_task_handlers.go:72-111`。

## 9. 不宜脱离上下文复制的实现

- 渠道 cache 是包级 mutable map，并存在数据库直读与 cache 双路径：`model/channel_cache.go:19-121`；
- `SyncChannelCache` 为长期循环，快照一致性和取消边界需要单独审计；
- 普通与任务 relay 的 retry/error policy 不统一；
- Realtime WebSocket `CheckOrigin` 无条件返回 true：`controller/relay.go:258-263`；
- pprof 可监听 `0.0.0.0:8005`：`main.go:160-166`；
- debug 路径可记录原始或转换后的完整请求 body：`relay/compatible_handler.go:97-105`、`relay/compatible_handler.go:176`。

这些行为依赖部署网络、鉴权、日志访问控制和运营模式，不能作为安全默认值移植。

## 10. 证据结论

可复用价值主要在 priority/weight 渠道模型、可解释 retry、affinity、请求重放、预扣/结算状态机、quota 数值防护和定期巡检。
但控制面动态状态、自动禁用和业务数据库观测都带有多租户运营平台假设；是否采用必须先明确隔离单位、持久化责任、恢复策略和敏感数据边界。
