# cc-switch request retry 与 failover 调研

## 证据范围

- 固定快照：`farion1231/cc-switch` commit `ebbf141fc71547a99f669df1be8e345130d1d890`，2026-08-02
- 阅读入口：`src-tauri/src/proxy/forwarder.rs` 与 failover 文档
- 本文只记录 cc-switch 的请求级 attempt、错误分类与 circuit-breaker 行为。

## 观察事实

- `RequestForwarder` 把 `max_retries` 转换为 `max_attempts = max_retries + 1`。
- 实际循环还受 provider 数量限制，因此扩大配置条目不会无界放大一次请求的调用次数。
- forwarder 区分 Provider/transport failure 与经归一后仍属客户端请求无效的错误；后者不会继续 failover。
- failover 单位是 Provider，并结合 UI、持久化 circuit breaker 和客户端配置接管。

## 适用边界

- `retry` 的配置语义必须明确是额外次数还是总 attempt 数。
- cc-switch 的 Provider failover 单位不能外推为 credential、route 或 deployment。
- UI 与持久化 circuit breaker 是产品控制面，不是协议事实。

## 一手源码

- [
  `forwarder.rs`](https://github.com/farion1231/cc-switch/blob/ebbf141fc71547a99f669df1be8e345130d1d890/src-tauri/src/proxy/forwarder.rs)
