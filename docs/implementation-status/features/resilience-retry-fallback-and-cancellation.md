# 功能：Retry、fallback、credential rotation、cooldown 与取消

## 当前行为

- 首个下游业务输出前，网关按固定候选顺序执行有界 retry/fallback；请求不能动态创建或重排 Route。
- 429 可在同一 pool 轮换有序 member；member/generation cooldown 与 target fault-domain cooldown 在单进程内共享。
- candidate retry 耗尽后只沿同一 Public Model 的注册 Route fallback；首个业务 body byte 提交后不得切换或拼接响应。
- 下游取消终止 send、backoff、response body 和后续 attempt；SSE terminal、EOF-before-terminal、body error 与 cancel 各收口一次。
- response headers timeout 仍在 response 建立前进入既有 retry/fallback；headers 后 first-event、event-idle、stream-total 或非流式 body
  timeout 按当前 commit state 终止 body，不触发第二条 stream。计划中的单-event precommit gate 与 EOF 客户端可见失败仍未实现。
- 错误向下游稳定脱敏；credential、真实 endpoint、原始错误正文和内部拓扑不泄漏。
- Embeddings 复用 attempt/cancel 边界，但当前只有单 Route，不做 cross-Route fallback。

## 所有权

候选循环位于 `src/ingress/forwarding.rs`，stream lifecycle 位于 `src/ingress/streaming.rs`，attempt/credential/target health
位于 `src/ingress/`，Provider failure 分类位于 `src/provider/`。

## 确定性证据

`tests/forwarding_contract.rs`、`tests/sse_contract.rs`、`tests/process_replay_contract.rs` 与
`tests/embedding_forwarding_contract.rs` 覆盖 retry/fallback、429、commit point、EOF、取消和唯一终态；transport/streaming 单元测试另覆盖
headers、first-event、event-idle 与 non-stream total timeout。

## 未证明范围

确定性 transport/loopback 不证明真实网络重试效果、吞吐、公平性、SLA、多进程恢复、负载或长期运行。

## 相关文档

- [路由与韧性需求](../../functional-requirements/routing-resilience/README.md)
- [Native generation](native-generation-forwarding.md)
- [OpenTelemetry 遥测](../telemetry-metrics.md)
