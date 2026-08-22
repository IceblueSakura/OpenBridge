# Responses 流提前终止与 timeout 边界设计

> **状态：候选执行前设计，不构成实施授权。** 本文是[实现顺序](implementation-sequence.md)中计划 1 与计划 4 的详细设计 owner，记录 Hermes 经 OpenBridge 调用 Responses 时出现 premature termination / incomplete chunked read 的证据、已确认机制、未决根因、目标生命周期和验证矩阵。真正实施前必须重新读取 live source、部署配置、反向代理配置与工作树，只将一个可观察切片提升到 [`implementation-plans/current-focus.md`](../implementation-plans/current-focus.md)。

## 1. 问题摘要

2026-08-22，Hermes 通过 OpenBridge 调用 `gpt-5.6-luna` Responses streaming 时多次报告：

```text
peer closed connection without sending complete message body
(incomplete chunked read)
```

相关日志显示：

- HTTP stream 已建立；
- 错误发生在 response body 尚未完整结束时；
- 一个 session 的连续失败间隔约为 248、247、250 秒；
- 同期存在多个并发调用，部分调用后续恢复，至少一个调用连续失败；
- 错误不同于 capability preflight 400，也不同于请求体 413。

初步分析曾把接近 240 秒的周期视为反向代理或 upstream idle timeout 迹象。进一步检查 OpenBridge 锁定源码后，已有更直接的机制可以导致截断：

- generation Target 普遍配置 `request_timeout = 120s`；
- `src/transport/upstream.rs` 把该值传给 reqwest `RequestBuilder::timeout`；
- 锁定依赖为 `reqwest 0.13.4`；
- 该版本源码明确说明 request timeout 从连接开始持续到 response body 完成为止，是 total deadline；
- upstream body 通过 `response.bytes_stream()` 继续携带该 deadline；
- deadline 到期后的 body error 被 OpenBridge 转换成 downstream body error，客户端看到 incomplete chunked read。

因此：

> 当前 120 秒 total request deadline 足以截断任何超过 120 秒的合法长流，是已确认的设计缺陷；约 247–250 秒的 Hermes 外层周期可能由 SDK/agent 重试叠加两个约 120 秒调用形成，但没有 request ID 关联前不能把“两个周期如何组成”写成已确认根因。

## 2. 目标

本设计的目标是：

1. 长寿命 generation stream 不受普通非流式 total request timeout 截断；
2. 等待 response headers、等待首个有效 SSE event、事件间 idle 和整个流生命周期分别建模；
3. 在任何 downstream output commit 前发生的失败可安全返回 HTTP error，并按既有 policy retry/fallback；
4. 首个 downstream output 后绝不 retry、fallback 或拼接第二条流；
5. terminal 前 EOF、body transport error、Provider terminal failure、客户端取消和合法 terminal 明确区分；
6. 不伪造 `response.completed` 或 `[DONE]`；
7. 日志、metrics 和 trace 足以把 Hermes 错误关联到 OpenBridge request、candidate attempt、最后事件和 timeout 类别；
8. 不用盲目放大所有 timeout 掩盖根因。

## 3. 协议不变量

Responses streaming 是 typed SSE 生命周期。正常终态至少包括：

- `response.completed`；
- `response.incomplete`；
- `response.failed`；
- 当前外部参考还列出 `response.cancelled`，是否被目标 Provider 使用应在实施前重新核验。

必须保持：

- response/item/content delta 只是中间事件；
- partial text 不能等价于完整成功；
- `response.failed` 是 Provider 明确表达的协议失败，不是 transport EOF；
- terminal 前 clean EOF 是语义失败；
- body transport error 是传输失败；
- 已输出部分内容后不能重试并拼接另一条流；
- 网关不能把异常终止补造成 `response.completed`；
- 网关也不应凭空伪造 Provider `response.failed`，除非未来定义并公开 gateway-owned terminal extension；本文不建议这样做。

相关现有参考：

- `docs/references/openai/responses/streaming.md`；
- `docs/references/hermes/hermes-chat-responses-analysis.md`；
- `tests/process_replay_contract.rs`；
- `tests/forwarding_contract/resilience.rs`。

## 4. Hermes 侧观测证据

检查范围为 2026-08-20 至 2026-08-22 的本机 Hermes 日志。核心事件集中在：

```text
~/.hermes/logs/agent.log
~/.hermes/logs/errors.log
```

2026-08-22 的 `gpt-5.6-luna` / Responses 记录中出现六次 incomplete chunked read。一个连续失败序列时间为：

```text
13:47:11
13:51:19  +248s
13:55:26  +247s
13:59:36  +250s
```

该序列证明：

- 失败具有近似固定周期；
- 客户端看到的是 peer/body 未完整结束，而不是一个完整 JSON 504；
- 外层存在重试或重复调用。

它不单独证明：

- Nginx 一定配置 240 秒 timeout；
- 每个 OpenBridge request 实际持续 240 秒；
- OpenBridge 内部一定执行了两个 candidate；
- Provider 一定在 240 秒主动关闭；
- Hermes 没有内部 SDK retry。

必须使用 request ID、attempt 时间线和代理日志才能分解这 247–250 秒。

## 5. 当前源码事实

### 5.1 generation Target 使用 120 秒 request timeout

当前 Provider registration 中，OpenAI、ChatGPT、OpenRouter、DeepSeek、Bailian generation、MiMo、LongCat 等 Target 普遍使用：

```rust
request_timeout: Duration::from_secs(120)
```

Images 等 operation 可有不同值，例如 180 秒；这进一步说明该字段是 Target 级总 request deadline，而不是统一 connect timeout。

### 5.2 request timeout 被应用到完整 reqwest 请求

`src/transport/upstream.rs`：

```rust
client
    .request(...)
    .timeout(request.timeout)
    .send()
    .await
```

锁定依赖：

```text
reqwest v0.13.4
```

锁定版本 `async_impl/request.rs` 对 `RequestBuilder::timeout` 的定义是：

```text
The timeout is applied from when the request starts connecting until the
response body has finished.
```

因此它不是“只等待 response headers 120 秒”。即使 SSE 持续正常输出，只要整个 body 超过 120 秒，deadline 也会到期。

### 5.3 body error 会变成 downstream transport error

transport 将：

```rust
response.bytes_stream()
```

包装为 Axum `Body`。`src/ingress/streaming.rs` 的 `validate_sse_body` 遇到 upstream body error 时：

- 记录 upstream failure；
- 返回新的 `io::Error("upstream SSE stream terminated unexpectedly")`；
- 结束 downstream chunked body；
- 不追加 terminal。

这与 Hermes 的 incomplete chunked read 形状一致。

### 5.4 当前 body error 分类丢失原始原因

`validate_sse_body` 对 `Some(Err(_))` 丢弃原始 error，并统一记录 `InvalidUpstreamResponse`。因此 metrics 不能区分：

- reqwest total deadline；
- upstream connection reset；
- incomplete HTTP body；
- TLS/body decode error；
- proxy close；
- 本地 body wrapper error。

这解释了为什么仅靠当前 Hermes 日志不能确定具体层级。

### 5.5 headers 后立即返回 downstream Response

`src/ingress/forwarding/response.rs` 在识别 SSE 后创建 validation body，并立即构造 HTTP response。body 的第一个 event 尚未读取时，downstream status/header 已可提交。

因此：

- upstream headers 成功后、首个 SSE event 前发生的 body timeout，也可能只能表现为 downstream 200 + broken body；
- runner 的 retry/fallback 只处理 `send()` 返回前的 transport error 或 retryable HTTP status；
- body takeover 后不再进入 attempt loop。

代码注释声称 retry 只允许在 first downstream event 前，但当前实际 commit boundary 更接近“upstream headers 已接收并构造 downstream response”，两者并不完全一致。

## 6. 当前测试事实

### 6.1 transport error after output

`tests/process_replay_contract.rs` 已验证：

- partial SSE 原样到达下游；
- downstream body 以 transport error 结束；
- 不 retry；
- 不 fallback；
- 不追加 synthetic terminal；
- request 和 provider attempt 记为 failed。

这符合严格网关在已 commit 后的基本边界。

### 6.2 clean EOF before terminal

当前测试要求：

- partial SSE 原样到达；
- downstream 以 clean EOF 结束；
- 不追加 terminal；
- metrics 记录 `sse_eof_before_terminal` failure；
- 客户端传输层不报错。

这存在客户端可见语义缺口。Hermes 当前在已经累计 output、但没有 terminal 时可能降级为 completed；因此只在 metrics 中记录失败而向客户端 clean EOF，会让严格失败被客户端容错掩盖。

目标设计应重新评估该合同：terminal 前 EOF 应向 downstream body 表达失败，而不是 clean success boundary。表达方式应是 body error，不是伪造 terminal。

### 6.3 buffered non-streaming conversion

当 OpenBridge 为下游非流式请求缓冲 upstream Responses SSE 时，terminal 前 EOF 已返回 502 `invalid_upstream_response`。这证明在尚未 commit 时，网关可以给出完整 HTTP error；streaming path 的困难来自 commit 时机，而不是无法识别 terminal。

## 7. timeout 领域模型

不得再使用一个 `request_timeout` 同时承担所有阶段。建议至少拆分：

```rust
struct UpstreamTimeoutPolicy {
    response_headers_timeout: Duration,
    first_event_timeout: Duration,
    inter_event_idle_timeout: Option<Duration>,
    total_timeout: Option<Duration>,
}
```

### 7.1 connect timeout

已有 client-level `connect_timeout`，只拥有 DNS/TCP/TLS connect 边界。它不能替代 response headers timeout。

### 7.2 response headers timeout

限制从 request 发出到收到 upstream status/headers。此阶段还未取得 response body，可以安全：

- 返回 504；
- retry credential；
- 按 policy fallback；
- 保持零 downstream output。

### 7.3 first event timeout

限制收到 SSE headers 后到第一个完整、合法、可下发事件。它必须结合 precommit gate；否则 timeout 仍只会造成 200 broken body。

首事件应定义为完整 SSE frame，而不是任意 TCP chunk。可进一步区分：

- 首个合法 SSE event；
- 首个 generation output；
- keepalive/comment。

第一阶段建议以首个完整合法 SSE event 作为 commit gate，避免无限缓冲，同时单独记录 TTFT。

### 7.4 inter-event idle timeout

限制相邻完整 upstream SSE event 之间无进展的持续时间。它不是 total stream lifetime。

是否启用以及具体值必须来自真实 Provider 证据：

- 某些 reasoning model 可能长时间思考且不发送事件；
- 某些 Provider 会发送 keepalive；
- 太短的 idle timeout 会误杀合法请求；
- 无限 idle 会占用连接和并发预算。

第一阶段可以允许 `None`，先消除错误 total deadline，再根据观测数据决定。

### 7.5 total timeout

对 generation SSE 默认应为 `None`，或是远高于普通请求且由 operation 明确拥有的 hard safety budget。不能复用非流式 120 秒。

如果保留 hard total timeout，必须：

- 明确它会终止仍有事件流动的请求；
- 在 Models/配置/运维文档中可发现；
- 高于已验证的 Provider 最大正常持续时间；
- 有独立 metric 和测试；
- 不被描述为 idle timeout。

## 8. precommit gate

### 8.1 目标

在向下游提交 200/SSE headers 前，读取并验证至：

- 第一个完整合法 SSE event；或
- 一个明确 Provider terminal；或
- body error / invalid framing / EOF / timeout。

### 8.2 结果

| precommit 结果 | 行为 |
|---|---|
| 首个合法 event | 提交 200/SSE，先下发已缓冲 event，继续流式读取 |
| Provider failed terminal | 可提交原始 SSE failure，或按既定协议合同处理；不得改写为成功 |
| timeout/body error | 尚未 commit，可进入 retry/fallback，最终返回 502/504 |
| clean EOF before terminal | 尚未 commit，返回 502 `invalid_upstream_response` |
| invalid SSE | 返回 502 `invalid_upstream_response` |
| client cancellation | 取消 upstream，不 retry |

### 8.3 资源边界

- 只缓冲至一个完整 SSE event；
- 继续受 `max_sse_event_bytes` 限制；
- 不缓冲完整 generation；
- 不延迟超过 first-event timeout；
- precommit buffer 不得进入日志；
- cancellation 必须释放 upstream body。

### 8.4 retry/fallback

只有 precommit 尚未输出任何 downstream byte 时，现有 attempt policy 才可重试或 fallback。任何已下发 event 后：

- 不 retry；
- 不 fallback；
- 不重新发请求；
- 不拼接另一 candidate；
- 不追加 synthetic terminal。

## 9. commit 后的终止合同

### 9.1 正常 terminal

收到并下发合法 terminal 后，request/attempt 按 terminal 分类完成。terminal 后的额外合法事件应按协议定义处理；terminal 后发生连接 close 不应把已完成请求重新标成失败。

建议 terminal 一旦完整下发：

- 标记 lifecycle final；
- 停止继续解释业务事件；
- 丢弃或关闭 upstream body；
- downstream clean EOF。

是否允许 terminal 后 usage/metadata 必须按具体 Provider grammar 核验，不能全局假设。

### 9.2 Provider failed/incomplete terminal

- 原样下发合法 terminal；
- downstream transport 可 clean finish；
- request 记为 failed；
- 不 retry/fallback；
- 不把协议失败降级成 transport error。

### 9.3 clean EOF before terminal

目标应改为 downstream body error：

- 保留已下发 partial bytes；
- 不伪造 terminal；
- 客户端明确看到不完整 body；
- metrics 分类为 `sse_eof_before_terminal`；
- request/attempt failed；
- 不 retry/fallback。

这会有意改变当前测试中的 clean EOF 合同，需要需求、fixture、process replay 和 implementation status 原子更新。

### 9.4 upstream body transport error

- partial bytes保持；
- downstream body error；
- 原始安全分类进入 observation；
- 不把 reqwest/TLS/地址等原始字符串发给客户端；
- 不 retry/fallback。

### 9.5 downstream cancellation

- drop upstream source；
- request/attempt cancelled，而非 failed；
- 不 retry/fallback；
- 与 upstream body error 分开计数。

## 10. body error 分类

建议 transport 保留闭合、安全的 body error kind：

```rust
enum UpstreamBodyErrorKind {
    TotalDeadline,
    IdleTimeout,
    ConnectionReset,
    IncompleteBody,
    Decode,
    OtherTransport,
}
```

要求：

- 分类在 reqwest error 仍可检查 `is_timeout()` / source chain 时完成；
- ingress 只接收闭合 kind，不接收包含 URL/正文的错误字符串；
- 对外仍表现为 body failure，不暴露内部细节；
- OTel/metrics 使用低基数枚举；
- clean EOF before terminal 不是 transport kind，而是 protocol lifecycle kind。

## 11. observability 设计

每个 request/attempt 至少需要：

- downstream request ID；
- route ID / provider kind 的既有安全低基数标签；
- candidate attempt ordinal；
- protocol 与 operation；
- timeout phase；
- configured timeout duration；
- headers received elapsed；
- first SSE event elapsed；
- first generation output elapsed；
- last complete SSE event elapsed；
- stream lifetime；
- bytes/chunks/events count；
- terminal kind；
- commit 是否发生；
- end kind：completed / provider_failed / eof_before_terminal / body_timeout / body_transport / client_cancelled；
- retry/fallback next action，仅限 precommit。

禁止记录：

- SSE data 内容；
- prompt、reasoning、tool arguments；
- Authorization/Cookie；
- Provider 原始 error body；
- 完整 URL query；
- credential/member identity。

日志关联必须能回答：

```text
Hermes 的一次 API call
→ OpenBridge request ID
→ candidate attempt 1/2/...
→ 120s deadline 是否触发
→ 是否收到首 event / terminal
→ 是否已 commit
→ 为什么没有 retry/fallback
```

## 12. 对约 248 秒周期的假设矩阵

| 假设 | 当前证据 | 验证方法 | 状态 |
|---|---|---|---|
| 单次 OpenBridge stream 被 120s total deadline 截断 | reqwest 锁定源码 + Target config 足以证明机制存在 | loopback 发送持续超过 120s 的合法 SSE | 高置信、待运行复现 |
| Hermes/OpenAI SDK 对同一调用内部重试一次，形成约 240s | 连续错误间隔接近 2×120s | 对齐 Hermes `api_request_id`、OpenBridge request ID 和 access log | 假设 |
| OpenBridge 两个 candidate 各等待约 120s | Public Model topology 可能有多 candidate，但 body commit 后不 fallback | attempt trace | 假设，stream-opened 后可能性较低 |
| Nginx `proxy_read_timeout≈240s` | 只有周期形状，没有配置证据 | 读取部署 Nginx 配置和 error log | 假设 |
| Provider 主动在固定时限关闭 | 可能，但无 Provider request ID/日志 | OpenBridge upstream attempt trace、直连对照 | 假设 |
| Hermes stale-call watchdog 主动关闭 | 其他日志中存在 stale kill，但本组错误是 peer incomplete body | 对齐 cancellation/connection close 时序 | 较低置信假设 |

不能在获得关联证据前把任何一个“约 240 秒”假设写成最终根因。

## 13. 测试矩阵

### 13.1 transport timeout semantics

- response headers timeout 在 headers 前返回 typed timeout；
- first-event timeout 在 SSE headers 后、首 event 前触发；
- inter-event idle timeout 只看事件间进展；
- total timeout 为 `None` 时，持续输出超过 120 秒的 stream 不被截断；
- 非流式 JSON 仍受明确 total deadline；
- timeout config 非零、范围和 operation compatibility 通过启动校验。

测试不应真的等待 120 秒；使用 paused Tokio time 或短测试 policy。

### 13.2 precommit

- headers 后 body error、尚无 event：返回 502/504，可按 policy retry；
- headers 后 clean EOF、尚无 terminal：返回 502；
- invalid first SSE frame：返回 502；
- first event 后才提交 downstream 200；
- precommit buffer 不超过 `max_sse_event_bytes`；
- cancellation 释放 source。

### 13.3 post-commit

- body transport error：partial bytes + downstream body error，无 retry/fallback/terminal；
- clean EOF before terminal：partial bytes + downstream body error，无 synthetic terminal；
- Provider `response.failed`：原始 terminal + clean transport finish，request failed；
- completed terminal：clean finish，request completed；
- terminal 后 connection close 不把 completed 改成 failed；
- downstream cancellation：cancelled，不计 stream failure。

### 13.4 fallback

- precommit timeout 可 retry credential；
- exhausted credential 后可按固定 Route 顺序 fallback；
- 任意 downstream event 后禁止 fallback；
- fallback candidate body 和 credentials 独立；
- attempts、retry、fallback metrics 恰好一次。

### 13.5 process replay / external client

- 保留现有 transport-error-after-output corpus；
- 修改 EOF-before-terminal oracle，从 clean EOF 改成 client body error；
- 增加 valid stream duration > old 120s deadline 的快时钟测试；
- 用 reqwest/OpenAI SDK/Hermes loopback 验证客户端看到的错误形状；
- 最后才用真实 Provider 长 reasoning 请求验证，不以一次成功替代 deterministic tests。

### 13.6 reverse proxy

独立测试 Nginx：

- `proxy_buffering`；
- `proxy_read_timeout`；
- HTTP/1.1 chunked 与 HTTP/2；
- 下游断开传播；
- 超时 error log；
- 事件间隔小于/大于 proxy timeout。

代理验证不能替代 OpenBridge 自身 timeout 测试。

## 14. 非目标

本文不批准：

- 把所有 timeout 改成无限；
- 只把 120 秒机械提高到 300/600 秒；
- commit 后重试或 fallback；
- 拼接两条 SSE 流；
- 伪造 `response.completed`、`response.failed` 或 `[DONE]`；
- 把 partial output 当成功；
- 向客户端暴露 reqwest/Nginx/Provider 原始错误；
- 修改 Hermes 的容错逻辑来掩盖网关错误；
- 在没有 request ID 证据时宣称 Nginx 是根因。

## 15. 候选实施切片

建议分两次独立进入 current focus。

### 切片 A：修正 total deadline 与增加归因

1. 建立一个持续超过旧 total deadline、但持续产生合法 SSE event 的 RED；
2. 拆分 headers/stream timeout policy；
3. generation SSE 不再使用 120 秒 total request deadline；
4. 保留非流式 deadline；
5. body error 保留安全分类；
6. 增加 attempt/timeout phase/commit/last-event observability；
7. focused transport、streaming 和 process replay tests。

### 切片 B：precommit 与 EOF 可见失败

1. 建立 headers 成功但首 event 前 timeout 的 RED；
2. 引入单事件 precommit gate；
3. precommit failure 接回 retry/fallback policy；
4. post-commit EOF-before-terminal 改为 body error；
5. 更新 canonical corpus、requirements 和 implementation status；
6. 外部 SDK/Hermes loopback 验收。

切片 A 可以独立消除已确认的 120 秒误杀；切片 B 改变 streaming commit/error 行为，风险更高，应单独评审。

## 16. 执行前诊断清单

- [ ] 重新读取锁定 reqwest 源码与 Cargo.lock；
- [ ] 确认所有 generation Target timeout；
- [ ] 导出脱敏 OpenBridge request/attempt timeline；
- [ ] 将 Hermes `api_request_id` 与 OpenBridge request ID 对齐；
- [ ] 读取部署 Nginx timeout/buffering 配置；
- [ ] 区分 SDK retry、Hermes retry、OpenBridge credential retry 和 Route fallback；
- [ ] 记录首 headers、首 event、首 output、last event、terminal 和 body error 时间；
- [ ] 用 loopback 快时钟复现 120 秒 total deadline；
- [ ] 确认 current focus 只包含一个切片；
- [ ] 不读取或保存 Provider credential、prompt 或 SSE 内容。

## 17. 当前验证边界

已验证：

- Hermes 确实观察到多次 incomplete chunked read；
- 失败周期接近 247–250 秒；
- OpenBridge generation Target 普遍设置 120 秒 request timeout；
- 锁定 `reqwest 0.13.4` 将该 timeout 应用于完整 response body；
- upstream body error 会被转换为 downstream body error；
- body error 当前丢失 timeout/reset 等具体安全分类；
- commit 后现有测试禁止 retry/fallback 和 synthetic terminal；
- clean EOF before terminal 当前只在 metrics 中失败、对客户端 clean EOF。

尚未验证：

- 线上每个失败 request 的精确 duration；
- 约 248 秒是否来自 OpenAI SDK 两次 120 秒请求；
- 线上 Nginx timeout 配置；
- Provider 是否同时存在固定 close boundary；
- 修正 total deadline 后真实 Provider 长流是否完全稳定。

真正实施前必须基于稳定 live source 和工作树重新确认 observability owner 与测试基线。
