# 功能：Retry、fallback、credential rotation、cooldown 与取消

## 状态

**已完成（当前 checkout）。** 网关在首个下游业务输出提交前，按固定候选顺序执行有界 retry/fallback，并把取消传播到当前上游工作；它不根据
业务请求动态创建或重排 Route。

## 已完成内容

- 请求级与 candidate 级 retry 使用固定硬预算和 capped exponential backoff；只对明确可重试的 HTTP/transport failure 重试。
- 429 可以在同一 credential pool 内按有序 member 轮换；单进程共享 member cooldown 与 target fault cooldown，避免短时间重复击穿。
- candidate 的 retry 耗尽后只沿同一 Public Model 的已注册 Route 顺序 fallback；首个下游业务 body byte 提交后不再切换上游或拼接响应。
- 下游取消会终止当前 send、退避、response body 和后续 attempt；SSE terminal、EOF-before-terminal、body error 和 cancel 各自只收口一次。
- 错误向下游输出稳定且脱敏的本地结果；credential、真实 endpoint、原始错误正文和内部拓扑不会随 failure 泄露。

## 实现边界

- 候选编排位于 [`src/ingress/forwarding.rs`](../../../src/ingress/forwarding.rs)，stream lifecycle 位于
  [`src/ingress/streaming.rs`](../../../src/ingress/streaming.rs)，失败分类与 cooldown 位于 [`src/provider/`](../../../src/provider/)。
- cooldown 只在当前进程内生效，不持久化、不跨进程、不动态探测，也不提供 health/weight 控制面。
- 这些规则同样约束 Embeddings，但当前 Embeddings 仍是单 Route、无 cross-Route fallback 的独立 surface。

## 验证证据

- [`tests/forwarding_contract.rs`](../../../tests/forwarding_contract.rs) 覆盖 retry、fallback、429、错误和首个输出提交点。
- [`tests/sse_contract.rs`](../../../tests/sse_contract.rs) 覆盖 SSE terminal、EOF 和取消收口。
- [`tests/ingress_contract.rs`](../../../tests/ingress_contract.rs) 覆盖 handler、body 和取消边界。
- [`tests/embedding_forwarding_contract.rs`](../../../tests/embedding_forwarding_contract.rs) 覆盖 Embeddings 的有界 retry、backoff 和 cancel。

确定性 transport/fake upstream 测试不证明真实网络重试效果、吞吐、公平性、SLA 或长期运行恢复能力。

## 相关文档

- [功能需求：路由与 Provider 韧性](../../functional-requirements/provider-resilience.md)
- [Native Chat/Responses 转发](native-generation-forwarding.md)
- [运行时指标与遥测](../telemetry-metrics.md)
