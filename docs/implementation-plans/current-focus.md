# 当前开发焦点

## 状态

**已规划、待实施：计划 1——Responses streaming timeout policy 与归因。**

本文只授权[有序计划族](../pre-execution-design/implementation-sequence.md)中的计划 1。计划 2 及之后的
`reasoning.encrypted_content`、Generation 字段级错误、precommit/EOF、Images 证明与 legacy 清理仍未获准实施。实现必须从本文规定的
RED 开始；完成后把确认事实写入 implementation status，并将本文恢复为空焦点。

## 可观察行为

一个经过认证、已经通过 Public Model preflight 的 Responses SSE 请求，在 upstream 持续产生符合协议的 event、event 间隔没有超过
受信 idle policy 且最终到达合法 terminal 时，不得仅因 elapsed time 超过当前 Target 的普通 `request_timeout` 而被截断。

完成后：

1. 等待 response headers、等待首个有效 SSE event、event 间 idle、可选 stream total 和非流式 total 使用不同的 typed phase；
2. upstream SSE 在持续取得合法进展时可以超过旧 120 秒 total deadline；
3. upstream 非流式 JSON 继续受原有 total deadline 保护；
4. timeout/body error 以安全低基数分类关联到 request、attempt、phase 和 commit state，不暴露 reqwest、Provider、代理或正文；
5. 本焦点保持现有 commit 语义：headers 后仍可建立 downstream response，commit 后不 retry/fallback、不拼接第二条流、不伪造 terminal。

上游 delivery mode 由受信 Upstream API/Route planning 决定，而不是直接相信下游 `stream` 字段。被固定策略转换为 upstream Responses SSE、
再有界组装为下游 JSON 的请求也必须使用 streaming liveness policy，不能重新落回普通 total deadline。

## 对应需求与设计

- [Native Path 与流式语义](../functional-requirements/gateway-api/native-path-and-streaming.md#2-流式语义)：streaming 与非流式 deadline
  分离、terminal 和 commit 不变量。
- [路由与 Provider 韧性](../functional-requirements/routing-resilience/README.md#3-retryfallback-与取消)：只有下游业务 response
  提交前的 timeout 才可进入既有有限 retry/fallback；commit 后禁止重试。
- [运行期观测](../functional-requirements/observability/README.md)：request/attempt 因果关系、低基数 failure stage 与敏感数据排除。
- [计划 1 详细设计](../pre-execution-design/responses-stream-premature-termination-and-timeouts.md#切片-a修正-total-deadline-与增加归因)。

## 当前基线

- `UpstreamTargetConfig::request_timeout` 当前只有一个 `Duration`，generation Target 普遍为 120 秒。
- `src/transport/upstream.rs::send_request` 把该值传给 reqwest `RequestBuilder::timeout`；锁定的 reqwest 0.13.4 将其应用到完整
  response body。
- transport 在收到 headers 后把 `response.bytes_stream()` 交给 ingress；deadline 到期后的 body error 因而出现在 downstream
  response 已建立之后。
- `src/ingress/streaming.rs::validate_sse_body` 当前丢弃 body error 的 timeout/reset 分类，只保留统一 stream failure。
- 现有 replay tests 已保护 commit 后不 retry/fallback、不注入 synthetic terminal；这些断言本焦点必须保持。
- 约 247–250 秒 Hermes 外层周期如何由 SDK、Agent、OpenBridge attempts 或代理组成仍未证明，本焦点不把该假设当作实现前提。

## 目标 timeout 合同

对 generation 请求，由受信 operation/delivery policy 解析以下闭合阶段：

| 阶段 | 首轮迁移语义 |
|---|---|
| response headers | 继续使用当前 Target timeout 作为 connect/TLS/headers 的上限 |
| first SSE event | 使用当前 Target timeout；超时后按当前已提交状态产生安全失败 |
| inter-event idle | 每个完整、合法 SSE event 后重置当前 Target timeout |
| stream total | generation SSE 设为 `None`，不再以 elapsed wall time 截断仍有进展的合法流 |
| non-stream total | 继续使用当前 Target timeout，覆盖完整非流式 response body |

必须以 typed policy 直接替换含义含混的单一 total timeout 使用方式；不保留 legacy alias、按 URL/Provider 名称猜测或请求可覆盖的兼容路径。
若 live source 证明 policy 应由 Target、Upstream API 或 prepared operation 中的另一层拥有，可以调整存储位置，但上述客户端行为与安全边界不变。

## 先失败的测试

实现前按顺序建立 deterministic RED；测试使用 paused Tokio time 或短测试 policy，不实际等待 120 秒：

1. 一个持续产生合法 Responses event、总时长超过旧 total deadline 并最终 completed 的 loopback stream 在当前实现下以 body timeout 失败；
2. 新 policy 下同一 stream 完成且保留原始 SSE bytes、顺序和唯一 terminal；
3. headers 前 timeout 仍产生 typed timeout，并继续服从既有 precommit retry/fallback budget；
4. upstream 非流式 body 超过 total deadline 仍返回稳定 `504 upstream_timeout`；
5. 首 event 或 event idle timeout 在当前 headers-commit 语义下终止 body，且不 retry/fallback、不追加 terminal；
6. timeout observation 精确记录 phase、attempt 和 commit state，各 request/attempt terminal 只记一次，且不含 URL、credential、prompt 或 event body。

其中第 1 项必须先在旧实现上按预期失败；已有 commit/replay 测试只作为回归保护，不能冒充 RED。

## 实施顺序

1. **测试与需求基线**：建立 active stream 越过旧 deadline 的 RED，固定非流式、commit 后和敏感数据回归。
2. **Typed policy**：为 upstream delivery 引入闭合 timeout policy，直接迁移 generation 注册和 prepared request；不改变客户端可选字段。
3. **Transport deadline**：把 headers 与非流式 total 从 reqwest 的单一 request timeout 中拆开，保留 typed `TransportError::Timeout`。
4. **Streaming liveness**：在受 event size/UTF-8/framing 限制的现有 body lifecycle 中实现 first-event 与 inter-event idle timer，stream total 为空。
5. **安全归因**：保留 body timeout phase 到 request/attempt observation，记录 commit state 和最后进展时间，不保留正文或底层错误字符串。
6. **收口**：运行 focused 与完整 baseline，更新 native generation、resilience 和 telemetry implementation status，然后清空当前焦点。

## 明确非目标

- 不实现计划 4 的单 event precommit gate；首 event 前失败本轮仍可能表现为已建立 HTTP response 后的 body error；
- 不改变 terminal 前 clean EOF 的客户端表现，不把它补造成 `response.failed`、`response.completed` 或其他 synthetic event；
- 不在 commit 后 retry、fallback、切换 credential 或拼接两条 SSE；
- 不修改 Hermes/OpenAI SDK 的 retry，也不读取或修改部署 Nginx 配置；
- 不机械地把 120 秒提高到 300/600 秒，也不把 headers、first-event 和 idle timeout 改成无限；
- 不处理 Images 的 504 mapping、single-attempt coordinator 或其他 operation timeout；
- 不修改 Public Models、下游 request schema、OpenAPI endpoint 或私有 credential/config；
- 不用一次真实 Provider 长流替代 deterministic transport/SSE tests。

## 验证顺序

先运行：

```powershell
cargo test --locked transport::upstream::tests
cargo test --locked --test sse_contract
cargo test --locked --test process_replay_contract
cargo test --locked --test forwarding_contract
cargo test --locked --test observability_contract
```

随后运行完整 Rust 基线：

```powershell
cargo fmt -- --check
cargo test --locked
cargo clippy --locked -- -D warnings
git diff --check
```

文档另检查相对链接、锚点和 requirement/status owner。只有修改 `testdata/` 或 `tools/corpus/` 时才追加 corpus baseline。本焦点不要求
真实 Provider、Hermes、OpenAI SDK、Nginx、负载或长时间运行验收；若另行执行，只能记录对应日期、环境和 payload 的有限证据。

## 完成判定与回滚

完成必须同时满足：active stream 越过旧 deadline 的 RED 转绿；非流式 total、headers timeout、commit 后不重试和 SSE terminal 回归保持；
timeout phase/commit observation 有确定性证据；requirements、实现、测试和 implementation status 一致；当前焦点恢复为空。

回滚单位是 typed timeout policy、transport/streaming lifecycle、observation 与对应文档测试的完整切片。不得只恢复 reqwest total timeout、
留下半迁移 phase，或保留新旧 timeout 路径并存。
