# 当前架构

当前 crate 是 v2 Rust 库：统一语义核心、显式边界 adapters、纯目标 lowering、固定 topology 和 caller-driven execution。**没有网关 listener、凭据池或生产 HTTP client。**旧运行时见 [Git 归档](archive.md)；设计合同见 [v2 架构](architecture-v2/README.md)。

```text
Client request bytes
 → Client Adapter.decode_request
 → adapter::Request
     task semantics + semantic context/delivery + scoped carriers/fidelity
 → public admission / trusted transform / requirements
 → fixed endpoint Adapter.encode_request (lowering + context projection)
 → prepared target/auth/body → caller-owned transport
 → Attempt: Provider Adapter JSON/SSE decode
 → Response/Event IR
 → ResponseDelivery: Client Adapter lowering/encode
 → caller-owned downstream I/O → explicit commit / complete
```

| Owner | 当前责任 |
|---|---|
| `src/semantic/` | Generation ordered items、settings、usage、validation、requirements、reducer；`context.rs` 拥有标准 cache/execution hints、reported context 和 delivery intent，不依赖 protocol/provider/execution |
| `src/adapter/` | 双向边界 facade；`Dialect` 将 Standard/OpenBridge/DeepSeek/Xiaomi 组合为显式 wire rules；统一 request 表示与纯目标 context 投影，不访问网络/凭据/registry |
| `src/protocol/` | 共用 Chat/Responses 语法、完整 envelope、event codecs、strict JSON/SSE adapter；`adaptation.rs` 执行可信规则，`fidelity.rs` 保管有界来源/依赖记录 |
| `src/lowering/` | 对不可变最终语义检查固定目标可表示性，构造 codec 输入；不选择 Provider，不恢复删除值 |
| `src/provider/` | 可信 origin/路径、认证材料边界、HTTP 错误分类；请求 Debug 不打印 auth 或 body |
| `src/topology/` | 固定 Model/Route/Endpoint 编译，绑定协议、适配规则、表示/执行合同与凭据 locator；不持有秘密 |
| `src/execution/` | 固定 plan、请求准备、单帧增量 intake、响应投影和单调交付生命周期；I/O 调用方确认 commit/complete |
| `src/transport/sse.rs` | 有界 SSE framing，无语义判断或 socket 所有权 |

## 适配与保真

厂商差异不形成另一套 Generation/Usage。DeepSeek 有效 usage 缺失/null cache-write 时按 [ADR 0008](architecture-v2/decisions/0008-stable-core-and-vendor-adapters.md)归一为 0，并记录其兼容来源；其他 profiles 不继承此默认。reported 值和非法值不能被默认覆盖。

标准 response context 在 semantic 中，instruction echo 的 wire fidelity 独立保存。classified extras 绑定协议、适配合同、可信来源和响应依赖；仅终态捕获/输出，目标不兼容或语义修改后不恢复旧值。encrypted replay 继续使用其更严格的 owner/origin/finality 合同。

## 流式与执行边界

`Attempt::push` 最多消费一个 frame，返回已验证语义 events；不保存整个 event log。`ResponseDelivery::encode_events` 增量投影，I/O caller 保留未消费后缀并控制背压。Attempt 只暂存终态，严格 EOF 验证成功后，`finish_stream` 才编码成功/非成功终态。输出字节不等于实际提交：调用方在外部可见边界调用 `commit`，最终交付后调用 `complete`。late failure/cancel 不得恢复为成功或 post-commit fallback。

当前没有生产 ingress 或自动 retry/fallback。固定 SDK handler 仍是独立 synthetic 消费者，不是生产链。`tests/transport/chain.rs` 验证 library execution 与 synthetic HTTP，`examples/live_probe.rs` 是另需精确授权的受控诊断入口，不是服务。

## 验证入口

- `tests/semantic.rs`：独立语义/codec/profile/变换反例。
- `tests/transport.rs`：framing、协议终态、增量执行/显式 commit 与 synthetic body I/O。
- `tests/sdk_loopback.rs`：显式 ignored 的固定 SDK gate。

具体命令与外部验收边界见[开发指南](development.md)，当前缺口只由[实施基线](architecture-v2/migration.md)维护。
