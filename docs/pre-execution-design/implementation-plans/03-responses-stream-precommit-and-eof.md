# 阶段 3：Responses stream precommit 与 EOF 可见失败

> **状态：候选实施计划，不构成实施授权。** 本阶段有意改变客户端可见 streaming commit/error 语义，必须在阶段 1–2 完成后单独提升。详细设计见
> [Responses 流提前终止与 timeout 边界](../responses-stream-premature-termination-and-timeouts.md#15-唯一候选实施切片precommit-与-eof-可见失败)。

## 1. 可观察结果

- 成功 headers 后、第一个完整合法 SSE event 前仍未 commit：timeout/body transport failure 可按既有有限 policy retry/fallback；invalid frame 或 terminal 前 EOF 直接返回稳定 502，不自动伪装成可重放 transport failure。
- 第一个合法 event 到达后才提交 downstream 200/SSE，并先下发该 event。
- commit 后 transport error 或 terminal 前 EOF 以 downstream body error 结束，不 retry/fallback、不拼接第二条流、不伪造 terminal。
- Provider 明确 failed terminal 仍是协议失败；completed terminal 仍 clean finish；客户端取消立即释放 upstream。

## 2. 已验证基线与 owner

- 已实现的 typed timeout policy 与 `enforce_sse_liveness` 独立拥有 headers/first-event/idle/total 分类，本阶段不重做 timer。
- `src/ingress/forwarding/response.rs::upstream_response` 当前收到 headers 后立即构造 downstream response。
- `src/ingress/streaming.rs` 的 Native/Bridge validators 已识别 framed events、terminal、body error 与 EOF，但 streaming EOF 当前对客户端仍是 clean EOF。
- `src/ingress/forwarding/execution/runner.rs` 当前只在 transport send 返回 headers 前运行 retry/fallback；headers 后 body ownership 已移交 response path。
- `tests/process_replay_contract.rs` 与 `tests/support/process_replay/streaming.rs` 当前固定 EOF/transport-error oracle，必须原子更新。

## 3. RED

1. headers 成功、首 event 前 first-event timeout：旧实现已返回 200 broken body；目标是 precommit retry，最终 504。
2. headers 后首 frame invalid 或 clean EOF：目标 precommit 502，零 downstream event。
3. candidate A precommit timeout/body transport failure 后 candidate B 成功：只下发 B，attempt/recovery accounting 唯一；protocol EOF/invalid frame 不自动触发该路径。
4. 第一个 event 后 transport error：保留 partial bytes并产生 body error，无 retry/fallback/terminal。
5. 第一个 event 后 terminal 前 clean EOF：旧 oracle clean EOF；目标 body error且无 synthetic `response.failed`/`completed`/`[DONE]`。
6. Bridge、Native、buffered non-stream conversion、Provider failed terminal、cancel 各自保持正确终态。

测试使用短 policy/paused time，不等待真实 120 秒。

## 4. 实施步骤

1. 定义有界 precommit outcome：first valid event、Provider failed terminal、timeout/body transport failure、invalid framing、EOF、cancel；buffer 上限不超过 `max_sse_event_bytes`。
2. 把 selected candidate 的 SSE 首事件读取移动到仍由 attempt runner 拥有的 precommit 边界，或建立等价的 operation-owned handoff；在任何 downstream byte 前把失败转回现有 `AttemptCoordinator`。
3. precommit 成功后构造 response，先下发已缓冲 frame，再无缝继续同一 source；不得重新连接或复制 event。
4. 将 precommit timeout 映射到 typed 504；body transport failure按现有安全分类；invalid/EOF映射到安全502且不自动retry。保留request/attempt phase、实际next action与commit state。
5. 修改 post-commit Native/Bridge body lifecycle：terminal 前 EOF 与 transport error都向 downstream 产生 body error；保留 partial bytes，不注入 gateway terminal。
6. 更新 canonical replay/corpus oracle、requirements、status 和 observability assertions；保持每个方向有界，不记录 frame 内容。
7. 删除被 precommit state machine 取代的 headers-immediate 双路径；不留 feature flag 或 legacy behavior。

## 5. 非目标

- 不修改已完成的 timeout phase 时长或 Provider target policy。
- 不缓冲完整 stream，不以完整响应换取 precommit。
- 不改变正常 event bytes、顺序、media type 或 Provider terminal grammar。
- 不在 commit 后 retry、credential rotation 或 fallback。
- 不处理 Images lifecycle。
- 不宣称修复 Nginx、SDK 或 Provider 固定 close boundary。

## 6. 验证

Focused：

```text
cargo test --locked --test sse_contract
cargo test --locked --test process_replay_contract
cargo test --locked --test forwarding_contract
cargo test --locked --test bridge_forwarding_contract
cargo test --locked --test observability_contract
```

若 canonical corpus 改动，追加 `uv lock --check --project tools/corpus`、Python tests 与 corpus lint。最后执行完整 Rust baseline、Markdown/OpenAPI links 和 `git diff --check`。外部 loopback client 是完成门；真实 Provider/Hermes 作为后续独立证据。

## 7. 退出与回滚

完成门：所有 precommit failures 在 HTTP commit 前正确分类并服从预算；所有 post-commit failures 保持 no-retry 且客户端可见；取消释放 source；replay/observability 唯一终态；current focus 清空。回滚必须恢复 state machine、runner handoff、fixtures、requirements 和 status 的完整阶段，不允许混合两种 EOF 合同。
