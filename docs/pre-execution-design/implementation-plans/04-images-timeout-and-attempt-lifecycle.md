# 阶段 4：Images timeout 与单 attempt 生命周期

> **状态：候选实施计划，不构成实施授权。** 本阶段只建立 Images 单次不可重放 attempt 与 timeout 合同；response body lifecycle 属于阶段 5。详细缺口见
> [Images 剩余执行证明](../capability-operation-refactor/images-proof-and-legacy-cleanup.md#images-剩余执行证明)。

## 1. 可观察结果

- Images connect/TLS/response-headers timeout 稳定返回 `504 upstream_timeout`，不再泛化为 502 `upstream_error`。
- 每个已发送 Images 请求精确记录一个 physical Provider attempt；HTTP、transport、timeout、cancel 和成功 headers 均有唯一 accounting。
- Images 保持单 candidate、单 credential、无 retry、无 rotation、无 fallback；不确定是否已被接受/计费的请求绝不重放。

## 2. 已验证基线与 owner

- `src/ingress/forwarding/images.rs::forward_images_request` 当前拥有 analysis、planning、credential、`send_single_attempt` 和状态归一化。
- `send_single_attempt` 直接调用共享 transport；所有 `TransportError` 当前被压缩为 502 `upstream_error`。
- 注释和实现明确禁止 Images retry/fallback，但尚未通过 `AttemptCoordinator` 与 `RequestObservation::record_attempt*` 形成统一 attempt lifecycle。
- `tests/images_forwarding_contract.rs` 是 operation-specific contract owner。

## 3. RED

1. synthetic transport 返回 `TransportError::Timeout`：旧实现 502 `upstream_error`；目标 504 `upstream_timeout`、typed request/attempt timeout phase。
2. transport reset/build/invalid-target 与 timeout 使用不同安全分类，均只发送一次。
3. non-success HTTP、success headers、client cancellation各记录一个 started/terminal attempt；不产生 retry/fallback routing event。
4. 一个 credential pool 含多个 member 时仍只绑定既定单 member，不因 401/429/timeout 自动轮换。

## 4. 实施步骤

1. 为 Images 创建 `AttemptCoordinator`，开始一个 candidate 并只调用一次 `start_attempt`；本阶段禁止调用 `next_step`、backoff 或第二次 send。
2. 在 send 前用 compiler-bound Route/target/operation/Provider facts 调用统一 attempt observation；不把 prompt、endpoint 或 credential写入属性。
3. 对 `TransportError` 做 typed mapping：`Timeout` → 504 `upstream_timeout`；其余 transport/config错误保持各自安全 5xx，不解析底层字符串。
4. 对 HTTP status 记录 response-ready 与 closed failure；成功 headers 将 active attempt 交给阶段 5 的 body owner完成，不提前伪造 success。
5. 明确 cancellation：drop send/body future，terminal 只记一次，不重新发送。
6. 增加 operation-specific tests，确认 total attempt count 恒为 1、routing recovery 恒为 none。
7. 更新 Images requirement/status；不抽取通用 framework，除非 Generation/Embeddings/Images 已有完全相同且可证明的 owner。

## 5. 非目标

- 不为 Images 增加自动 retry、fallback、credential rotation 或 cooldown recovery。
- 不处理 response body 超限、损坏 JSON、EOF 与 image telemetry；它们属于阶段 5。
- 不改变 Images request schema、Provider wire、Models profile 或 DashScope扩展。
- 不修改 Generation/Embeddings attempt policy。

## 6. 验证

Focused：

```text
cargo test --locked --test images_forwarding_contract
cargo test --locked --test observability_contract
cargo test --locked transport::upstream::tests
```

随后执行完整 Rust baseline 和 `git diff --check`。真实 Provider计费/幂等性、负载和长期运行不由 deterministic tests 证明。

## 7. 退出与回滚

完成门：timeout 504；每条 send 唯一 attempt；所有失败 no-replay；成功 headers 的 attempt ownership 可由阶段 5继续；无敏感属性；current focus 清空。回滚覆盖 coordinator/observation、typed mapping、tests、requirements/status 的完整阶段。
