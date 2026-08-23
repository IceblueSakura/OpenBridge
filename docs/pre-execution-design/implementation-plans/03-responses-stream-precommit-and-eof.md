# 阶段 3：Responses stream precommit 与 EOF 可见失败

> **状态：候选实施计划，不构成实施授权。** 本阶段有意改变客户端可见 streaming commit/error 语义，必须在阶段 1–2 完成后单独提升。本文是该未完成切片的唯一详细设计 owner；已实现 timeout/streaming 事实仍由 implementation status 与 live source 拥有。

## 1. 可观察结果

- 成功 headers 后、第一个完整合法 SSE event 前仍未 commit：timeout/body transport failure 可按既有有限 policy retry/fallback；invalid frame 或 terminal 前 EOF 直接返回稳定 502，不自动伪装成可重放 transport failure。
- 第一个合法 event 到达后才提交 downstream 200/SSE，并先下发该 event。
- commit 后 transport error 或 terminal 前 EOF 以 downstream body error 结束，不 retry/fallback、不拼接第二条流、不伪造 terminal。
- Provider 明确 failed/incomplete terminal 仍是协议失败；completed terminal 仍 clean finish；客户端取消立即释放 upstream。

## 2. 权威事实与执行前基线

当前实现事实只从下列 owner 和执行时 live source 重新确认，本文不复制状态快照：

- [Native generation](../../implementation-status/features/native-generation-forwarding.md)
- [重试、fallback 与取消](../../implementation-status/features/resilience-retry-fallback-and-cancellation.md)
- [遥测与指标](../../implementation-status/telemetry-metrics.md)
- [Native Path 与流式需求](../../functional-requirements/gateway-api/native-path-and-streaming.md)

候选触点包括：

- `src/ingress/forwarding/response.rs::upstream_response` 的 headers→downstream response handoff；
- `src/ingress/forwarding/execution/runner.rs` 的 attempt/retry/fallback ownership；
- `src/ingress/streaming.rs` 的 Native/Bridge framing、terminal、body error 与 EOF lifecycle；
- `tests/process_replay_contract.rs`、`tests/support/process_replay/streaming.rs` 及 canonical corpus oracle。

实施前必须确认这些 symbol、status 与测试仍有效；路径或行为变化时更新计划，不把旧名称当作事实。

## 3. RED

1. headers 成功、首 event 前 first-event timeout：旧实现已返回 200 broken body；目标是 precommit retry，预算耗尽后返回 504。
2. headers 后首 frame invalid 或 clean EOF：目标为 precommit 502，零 downstream event，且不自动进入 retry/fallback。
3. candidate A 在 headers 后、首 event 前发生可重放 timeout/body transport failure，candidate B 成功：只下发 B，attempt/recovery accounting 唯一。
4. 第一个 event 后 transport error：保留 partial bytes 并产生 body error，无 retry/fallback/terminal。
5. 第一个 event 后 terminal 前 clean EOF：旧 oracle clean EOF；目标为 body error，且无 synthetic `response.failed`/`completed`/`[DONE]`。
6. completed、failed/incomplete、terminal 后 close、downstream cancel、Native、Bridge 与 buffered non-stream conversion 各自保持唯一正确终态。

测试使用短 typed policy、paused time 或 loopback fault injection，不等待真实生产 deadline。

## 4. 候选状态机与实施步骤

1. 定义有界 precommit outcome：first valid event、Provider failed/incomplete terminal、timeout/body transport failure、invalid framing、EOF、cancel；单事件 buffer 不超过 `max_sse_event_bytes`。
2. 把选中 candidate 的 SSE 首事件读取移动到仍由 attempt runner 拥有的 precommit 边界，或建立等价 operation-owned handoff；在任何 downstream byte 前把可重放失败交回现有 `AttemptCoordinator`。
3. precommit 成功后构造 response，先下发已缓冲 frame，再无缝继续同一 source；不得重新连接、复制 event 或重新解析成另一条 stream。
4. precommit timeout 使用现有 typed timeout 分类映射 504；可重放 body transport failure 服从既有安全 policy；invalid/EOF 映射安全 502 且不自动 retry。记录实际 phase、next action 与 commit state。
5. post-commit Native/Bridge lifecycle 将 terminal 前 EOF 与 body transport error 变为 downstream body error；保留 partial bytes，不注入 gateway terminal。
6. 复用现有闭合、安全的 transport/timeout kind；clean EOF before terminal 是 protocol lifecycle kind，不伪装成底层 transport error。底层 URL、TLS、正文或原始错误字符串不得进入客户端或低基数属性。
7. 原子更新 requirements、canonical replay/corpus oracle、implementation status 与 observability assertions；删除被新 handoff/state machine 取代的 headers-immediate 双路径，不留 feature flag 或 legacy behavior。

## 5. Post-commit lifecycle 合同

| 终止方式 | Downstream | Request/attempt | Retry/fallback |
|---|---|---|---|
| 合法 completed terminal | 原样 terminal 后 clean EOF；terminal 后普通 close 不反转成功 | completed | 禁止 |
| 合法 failed/incomplete terminal | 原样 terminal，transport 可 clean finish | failed | 禁止 |
| terminal 前 clean EOF | 保留 partial bytes，以 body error 结束；不伪造 terminal | `eof_before_terminal` failed | 禁止 |
| upstream body transport error | 保留 partial bytes，以 body error 结束；不泄漏原始错误 | typed body failure | 禁止 |
| downstream cancellation | 立即 drop upstream source | cancelled，不计作 stream failure | 禁止 |

terminal 后是否允许 usage/metadata 必须服从具体协议/Provider grammar，不能全局假设；一旦合法 terminal lifecycle 完成，不再解释普通业务 event。

## 6. Observability 与资源边界

每个 request/attempt 只记录已有安全身份和低基数状态；本阶段需要确认至少能表达：

- downstream request ID、attempt ordinal、protocol、operation 及既有安全 Route/Provider 标签；
- timeout phase 与 configured duration；
- headers、first complete event、first generation output、last complete event 和 stream end 的 request-relative timing；
- bytes/chunks/events count、terminal kind、commit state；
- `completed | provider_failed | eof_before_terminal | body_timeout | body_transport | client_cancelled` 等闭合 end kind；
- 仅在 precommit 记录实际 retry/fallback next action，且每个 recovery 只计一次。

禁止记录 SSE data、prompt、reasoning、tool arguments、Authorization/Cookie、Provider 原始 error body、底层 URL 或未经归类的错误字符串。precommit 只缓冲一个有界完整 event；不得演变为完整 stream buffering。

## 7. 非目标

- 不修改当前 timeout phase 时长或 Provider target policy，不把 timeout 改成无限，也不机械提高某个历史 deadline。
- 不缓冲完整 stream，不以完整响应换取 precommit。
- 不改变正常 event bytes、顺序、media type 或 Provider terminal grammar。
- 不在 commit 后 retry、credential rotation 或 fallback，不拼接两条 SSE stream。
- 不伪造 `response.completed`、`response.failed` 或 `[DONE]`，不把 partial output 当作成功。
- 不向客户端暴露 reqwest、TLS、Nginx 或 Provider 原始错误。
- 不处理 Images lifecycle，不修改 Hermes/SDK 容错逻辑来掩盖网关错误，也不在缺少 request-ID 证据时归因反向代理。

## 8. 验证矩阵

| 层 | 必测合同 |
|---|---|
| Focused Rust | precommit timeout/body error、invalid first frame、EOF、首 event commit、buffer 上限、cancel |
| Post-commit Rust | body transport、EOF-before-terminal、completed/failed terminal、terminal 后 close、cancel、zero retry/fallback |
| Fallback | credential retry、固定 Route fallback、candidate body/credential 隔离、attempt/recovery 计数唯一 |
| Replay/corpus | 保留 transport-error-after-output；把 EOF-before-terminal oracle 从 clean EOF 改成 body error；同步 JSON/SSE terminal |
| External loopback | reqwest/OpenAI SDK/Hermes 观察到正确 precommit HTTP error 或 post-commit body failure |
| Deployment proxy | 在实际使用 Nginx 时独立核验 buffering、read timeout、HTTP/1.1/2、断连传播和 event interval；不能替代 OpenBridge 测试 |

Focused：

```text
cargo test --locked --test sse_contract
cargo test --locked --test process_replay_contract
cargo test --locked --test forwarding_contract
cargo test --locked --test bridge_forwarding_contract
cargo test --locked --test observability_contract
```

若 canonical corpus 改动，追加：

```text
uv lock --check --project tools/corpus
uv run --project tools/corpus pytest tools/corpus/tests
uv run --project tools/corpus corpus --root testdata lint
```

最后执行完整 Rust baseline、Markdown/OpenAPI links 和 `git diff --check`。外部 loopback client 是完成门；真实 Provider 长 reasoning、目标 Agent runtime、Nginx 和长期稳定性分别记录为更高层有限证据，一次成功不能替代 deterministic tests。

## 9. 执行前诊断清单

- [ ] 重新读取 typed timeout policy、transport、attempt coordinator、SSE liveness wrapper 和当前工作树；
- [ ] 确认 current focus 只包含本切片，阶段 1–2 已完成并清空；
- [ ] 用 loopback 快时钟复现 headers 后首 event 前 timeout、invalid frame 与 terminal 前 EOF；
- [ ] 对齐脱敏 downstream/OpenBridge request ID，区分 SDK retry、credential retry 与 Route fallback；
- [ ] 记录 headers、首 event、首 output、last event、terminal 与 body error 的相对时间，不保存正文；
- [ ] 若部署经过 Nginx，单独读取 timeout/buffering 配置并与网关证据分层；
- [ ] 不读取或保存 Provider credential、prompt、SSE data 或原始私有 error body。

## 10. 退出与回滚

完成门：所有 precommit failures 在 HTTP commit 前正确分类并服从既有预算；所有 post-commit failures 保持 no-retry 且客户端可见；取消释放 source；terminal 后 close 不反转成功；replay/observability 唯一终态；external loopback 满足新合同；requirements/status 与 live source 一致；current focus 清空。

回滚必须恢复 state machine、runner handoff、fixtures/corpus、requirements、status 与 observability 的完整阶段，不允许混合两种 EOF 合同。