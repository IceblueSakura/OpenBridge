# CallOrRet/responses-proxy 测试资产调研

## 状态与来源

- 在线复核日期：2026-07-26
- 本次在线检查未固定 commit。
- 来源：[responses-proxy](https://github.com/CallOrRet/responses-proxy)、[`verification_tests.rs`](https://github.com/CallOrRet/responses-proxy/blob/main/tests/verification_tests.rs)

## 观察事实

- 该 Rust/Axum proxy 接收 Responses 请求，转换为 Chat 请求，再把 Chat response 转回 Responses。
- 对外包含 HTTP SSE、WebSocket、function call/output、reasoning 与 Codex 相关行为。
- verification tests 使用较真实 payload，直接调用转换函数并维护 streaming conversion state。
- README 说明不在 allowlist 中的 tool type 会被静默丢弃。

## 覆盖与边界

它对 Responses → Chat → Responses 单方向转换和 Rust streaming state fixture 有直接参考价值，但不是两个公共 endpoint 的双向通用套件。现有 tests 未形成完整 bytes fragmentation、fault、cancel 和 unsupported-field 矩阵；静默丢弃 tool 是项目策略。

