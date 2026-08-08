# 协议测试语料与工具

## 状态

**Confirmed。** corpus 与 Python testkit 仍保持 runtime-independent；Rust contract tests 现在只读选定 canonical
artifact，用于 bridge 状态机回放、12 个真实 loopback HTTP error SUT 回放，以及 post-output transport-abort、
downstream-cancellation 和 clean EOF-before-terminal production replay。它们未接入外部 SDK、Codex、Hermes 或真实
Provider。本次新增的 Native function-tool wire cases 与 semantic cases 尚未接入 Rust/OpenBridge；它们是下一阶段 runner 的输入
与 oracle，不是实现完成声明。

日常使用和维护说明见 [Corpus 指南](../../testdata/README.md) 与 [Testkit 指南](../../tools/corpus/README.md)
；本文件只记录已执行验证与尚未证明的边界。

## 当前版本

| 项目            | 值                                          |
|-----------------|---------------------------------------------|
| Corpus          | `testdata/`，版本 `0.7.0`                   |
| 工具            | `tools/corpus/`，独立 `uv + Python` project |
| Python          | 3.12                                        |
| Canonical wire cases | 51（26 `accepted`、25 `reviewed`）    |
| Semantic cases  | 9（6 `accepted`、3 `reviewed`）             |
| Wire 分类       | 13 `exact`、6 `reject`、32 `native_only`    |
| 默认生成结果    | seed `20260726` 下 342 个 SSE wire variants |
| 工具测试        | 45 个 pytest tests                          |

覆盖内容：

- Chat → Responses 与 Responses → Chat 的 text stream/non-stream；
- 双向单 function call、并行 calls、fragmented arguments 和 tool result；
- Native Chat/Responses 各自的 strict + forced 单 function call、结构化 tool result 续接和并行流式 arguments；
- 协议无关的无需工具、工具选择、必填参数澄清、none/required/forced tool choice、无序并行 calls 与固定结果事实 oracle；
- 后续 fragment 只有 index 时的 identity 回归样本；
- Responses terminal 前 EOF；
- `response.failed`、`response.incomplete`、`error`、Chat DONE 前 EOF；
- duplicate terminal、terminal 后 event 与 SSE event/payload type 冲突；
- 首输出前 HTTP error、首输出后 transport error、downstream cancel 与 no-fallback commit point；
- 双向未知 tool result、重复冲突 `call_id`、同名并行 calls 与反序 tool results；
- 空字符串/`{}`/不完整/转义 UTF-8 arguments；
- comment keepalive、多行 `data:`、CRLF、all-in-one 和 event-pairs wire variants；
- Chat/Responses 原生非流式成功响应；
- 400、401、403、404、422、429、500、502、503、504 HTTP error matrix；
- delta-seconds/HTTP-date `Retry-After`、纯文本/损坏 JSON body 与错误状态携带 SSE Content-Type；
- hosted tool 与无受限 ledger continuation 的 proposed preflight reject；
- JSON Schema、provenance、secret scan、重复 JSON key、case 内路径与未声明文件、artifact 组合、terminal count、deterministic
  generation、coverage report 与 deterministic ZIP。

## Mock Server/Client

同一个 `tools/corpus/` Python 工具还实现了：

- incremental SSE parser；
- canonical case 到 server scenario 和 client plan 的编译；
- 基于 `asyncio + h11` 的 HTTP/1.1 Mock Server 与无自动重试的 Mock Client；
- normal terminal、HTTP error、transport abort、EOF 和 cancellation observation；
- 单 exchange 与有序多-exchange loopback；
- normalized semantic trace 的 schema、call/result identity、function argument schema、ordered/unordered call set 与固定回答事实判定；
- 零次或单次上游 attempt 的 canonical observation 判定，覆盖 identity、上下游 path、JSON/SSE body、 HTTP
  status、结束分类、terminal、声明的下游 response headers 与 body hash 自洽性。

这些工具不加载或启动 OpenBridge，不读取 credential，也不调用真实 Provider。`testdata/runtime/` 中的 scenario、 plan 和
observation 均为可重建临时产物，不进入 corpus ZIP。

## 验证命令与结果

2026-08-08 在当前 Windows checkout、uv `0.12.1`、Python `3.12.9` 下验证 `0.7.0`：

```powershell
uv lock --check --project tools/corpus
uv run --project tools/corpus pytest tools/corpus/tests
uv run --project tools/corpus corpus --root testdata lint
uv run --project tools/corpus corpus --root testdata generate --seed 20260726
uv run --project tools/corpus corpus --root testdata report --output testdata/reports/coverage.json
uv run --project tools/corpus corpus --root testdata pack --output testdata/dist/openbridge-protocol-corpus-0.7.0.zip
uv run --project tools/corpus corpus --root testdata pack --output testdata/dist/openbridge-protocol-corpus-0.7.0-repeat.zip
cargo fmt -- --check
cargo test --locked
cargo clippy --locked -- -D warnings
git diff --check
```

观察结果：

- lock 与 `pyproject.toml` 同步；pytest 收集并通过 45 项；corpus lint passed；
- seed `20260726` 生成 342 个 variant；现有 generation tests 逐一重建 chunks 并验证 wire SHA-256、CRLF 与 terminal 解析；
- report 得到 51 个 wire case、9 个 semantic case，required wire/semantic feature 与 generation kind 均无缺口；
- 两次 `0.7.0` pack 的 SHA-256 均为 `fbe0d20c4a382c200df1e1a8c035dcf749a38632f5d9139c7e8513b43e7333e5`；
- 最新共享 checkout 的 Rust baseline 为 290 个默认测试通过、0 ignored，Clippy 和 diff whitespace 检查通过；
- `cargo fmt -- --check` 在并发且不属于本次范围的 `src/providers/bailian/registration.rs` 上报告一处 rustfmt 差异；本次语料工作未修改该
  文件，也没有越界代为格式化；
- `generated/`、`reports/`、`dist/`、`runtime/`、`.venv/` 和 Python caches 均为 Git 忽略的派生物。

2026-08-01 在 Windows、uv `0.12.1`、Python `3.12.9` 下运行：

```powershell
uv lock --check --project tools/corpus
uv run --project tools/corpus pytest tools/corpus/tests
uv run --project tools/corpus corpus --root testdata lint
uv run --project tools/corpus corpus --root testdata generate --seed 20260726
uv run --project tools/corpus corpus --root testdata report --output testdata/reports/coverage.json
uv run --project tools/corpus corpus --root testdata pack --output testdata/dist/openbridge-protocol-corpus-0.6.0.zip
uv run --project tools/corpus corpus --root testdata pack --output testdata/dist/openbridge-protocol-corpus-0.6.0-repeat.zip
cargo fmt -- --check
cargo test --locked
cargo clippy --locked -- -D warnings
git diff --check
```

观察结果：

- lock 与 `pyproject.toml` 同步；
- pytest：36 passed；
- corpus lint passed；
- 生成 306 个 variant，chunks 重组 SHA-256 等于 wire SHA-256，CRLF 变体另保留 canonical SHA-256；
- required core feature 与 required generation kind 均无缺口；
- pack 生成 ZIP 和 `.sha256` sidecar；
- 两次 `0.6.0` pack 的 SHA-256 均为 `a0058dfe927398ee078ce31015bbe0aa2ca1c94518fd555fb5d8805e19d0474a`；
- Rust 回归：`cargo fmt`、91 个默认测试和 Clippy 零告警通过；
- `git diff --check` 通过；
- `generated/`、`reports/`、`dist/`、`runtime/`、`.venv/` 和 Python caches 均被 Git 忽略。

2026-08-08 在 `0.7.0` 语料扩展前的同日 Windows checkout 曾追加运行：

```powershell
cargo test --locked --test process_replay_contract
cargo fmt -- --check
cargo test --locked
cargo clippy --locked -- -D warnings
git diff --check
```

当时观察结果：focused process replay 为 8 passed；Rust baseline 为 283 个默认测试通过，当时没有 ignored test target；format、
Clippy 和 diff whitespace 检查通过。该次测试精简删除了独立 Python Embeddings client 和运行时下载 OpenAI Python/Node SDK
的 opt-in loopback 资产；当时没有修改 `testdata/` 或 `tools/corpus/`，因此未重复运行 Python baseline。当前 `0.7.0` 的完整结果以上方
同日验证块为准。

## 这证明什么

- schema、51 个 canonical wire cases、9 个 semantic cases 和 8 份 provenance 可被当前工具读取与校验；
- 默认 seed 的 wire variant generation 可重复；
- pack 不包含 derived directories，并具有固定 entry metadata 和内容 manifest；
- coverage report 会显式暴露未固定 source ref 和 pending license，而不是把它们隐藏为已完成。
- 进程级 loopback 已验证 scenario/plan 编译、Mock Server/Client 调用和双方 terminal observation。
- 单 case verifier 能确定地接受匹配 observation，并以不回显正文的字段路径拒绝 JSON、SSE、path、transport、 terminal、header
  或摘要不匹配。
- semantic verifier 能确定地接受匹配 normalized trace，并以不回显正文的字段路径拒绝错误/缺失/额外/禁止调用、参数 schema 或
  参数值错误、call/result identity 错误、预置工具输出错误和最终回答固定事实错误；并行 oracle 可按调用集合而非发射顺序匹配。
- Rust bridge replay 只读复用 canonical SSE，验证双向文本/并行 tool identity、四类 Responses terminal、 不完整
  arguments、event/type 冲突、EOF、terminal 后事件、重复 terminal 与重复 output identity。
- Rust conversion/forwarding contracts 复用 accepted bridge artifacts，验证双向 request、non-stream response、
  text/function/reasoning SSE renderer、生产 `Bridged` Route 和 canonical preflight rejects。
- Rust loopback 的 HTTP matrix 现覆盖 Chat/Responses `400/401/403/404/422`、三种 `429`、两个 `500`、`502` 和 `504`：
  非 429 `4xx` 只产生一次 attempt，单 member `429` 不重复 credential，`5xx` 只产生两次有界 local attempt；每个 replay
  都匹配 canonical upstream request、status、Content-Type 和 request/Provider 唯一失败终态；OpenAI-compatible JSON
  error case 还匹配 canonical body。
- delta-seconds 与 HTTP-date `Retry-After` 都保留；HTTP-date case 同时保留 allowlist
  `x-ratelimit-remaining-requests`。纯文本 502、损坏 JSON 500 和错误携带 SSE Content-Type 的 500 都先按 HTTP status
  分类，不进入 SSE decoder 或伪造 terminal。
- `responses_native.transport_error.after_output` 通过显式 event barrier 等待下游首字节，再让真实 upstream HTTP body
  abort；production Router 保留已输出 Native bytes、只执行一次 attempt、不 retry/fallback、不补 terminal，并且只记录一次
  request failed 与 Provider stream_failed。
- `responses_native.cancel.after_output` 按两个完整 logical event 分帧；下游收齐声明的边界后 drop body，upstream pending
  stream 的 drop guard 在有界等待内触发，且 gateway/Provider 各只记录一次 cancelled terminal，不 retry/fallback。
- `responses_native.eof_before_terminal` 经过真实 upstream/downstream socket clean EOF；production Router 保留 partial
  Native bytes、不补 terminal、不 retry/fallback，并且只记录一次 request failed 与 Provider stream_failed。
- 静态 case-id 扫描显示 51 个 canonical wire case 中 40 个已被 Rust 测试源码直接引用；原有剩余 5 个均已记录既有 owner 或证据冲突，
  本次新增 6 个 Native tool case 则明确留给下一阶段 SUT runner；
  不能把这个数字解释为 runtime、分支或真实 Provider 覆盖率。

## 这不证明什么

- 不证明全部 51 个 wire case 或 9 个 semantic case 已被 OpenBridge 执行或通过；production loopback 明确覆盖 12 个 HTTP error 与 3 个 streaming
  lifecycle case，其他直接引用还包括局部 parser、Bridge state machine 和 conversion contract；
- 不证明 continuation、hosted/custom tool、opaque 或未建模 reasoning、image 或 Provider 私有扩展可跨协议转换；
- 不证明 canonical oracle 等于完整 OpenAI API；
- 不证明外部项目默认分支在未来保持相同行为；
- 不证明真实 SDK、Agent 或 Provider 兼容。
- 不证明 TLS、HTTP/2、并发、背压、负载或真实网络 packet 边界。
- post-output abort replay 按当前 Native byte-transparent 实现比较 `upstream-stream.sse`；它不证明 canonical
  `expected-client-stream.sse` 中的 Public Model response projection 已实现。
- 纯文本 502 与损坏 JSON 500 replay 只证明 status-first classification、Content-Type、attempt 和终态；canonical 自身明确
  不规定 production Router 必须透传原始 body，因此测试不把当前 raw bytes 固定为对外契约。

## 已知待处理项

- 8 份外部/项目来源当前均未固定 commit；
- 三份 OpenAI protocol/function-calling 文档来源的许可证状态仍为 `pending`；
- 25 个涉及 OpenBridge 错误、commit point、identity 与 continuation 策略的 case 保持 `reviewed`；
- 3 个涉及歧义选择、缺参澄清与固定结果事实的 semantic case 保持 `reviewed`；
- 已有最小 Rust loopback runner 同时启动 OpenBridge Router 与 mock upstream，通过真实 HTTP socket 回放
  12 个固定 Chat/Responses HTTP error；同一 runner 还以无 sleep 的 event barrier 回放
  `responses_native.transport_error.after_output`，以 pending-body drop guard 回放 `responses_native.cancel.after_output`，并以
  clean EOF 回放 `responses_native.eof_before_terminal`。它尚不是可枚举全部 canonical cases 的通用 CLI，也未覆盖完整
  fallback 序列组合。
- 仍未直接引用的 Chat/Responses success 与 SSE framing 四个 case 受 Native Public Model response projection 差异阻塞；
  `responses_native.transport_error.before_output` 的 metadata 实际声明 HTTP 503 `error_response`，不能当作真实 socket transport
  failure 的直接 oracle。对应 owner 与后续焦点记录在[测试补全计划](../implementation-plans/test-coverage-completion-plan.md)。
- Python 单 case verifier 仍只消费已经生成的 observations，不启动 OpenBridge。
- semantic verifier 只消费调用方已经规范化的 trace；它不解析 Native/Bridge envelope，也不运行模型或工具。

## 关联文档

- [测试集调研](../references/cross-project/chat-responses-sse-tool-test-suite-survey.md)
- [Corpus README](../../testdata/README.md)
- [Testkit README](../../tools/corpus/README.md)
