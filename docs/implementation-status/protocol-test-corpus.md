# 协议测试语料与工具

## 状态

**Confirmed。** corpus 与 Python testkit 仍保持 runtime-independent；Rust contract tests 现在只读选定 canonical
artifact，用于 bridge 状态机回放和一个真实 loopback HTTP 429 SUT 回放。它们未接入外部 SDK、Codex、Hermes
或真实 Provider。

日常使用和维护说明见 [Corpus 指南](../../testdata/README.md) 与 [Testkit 指南](../../tools/corpus/README.md)；本文件只记录已执行验证与尚未证明的边界。

## 当前版本

| 项目 | 值 |
|---|---|
| Corpus | `testdata/`，版本 `0.6.0` |
| 工具 | `tools/corpus/`，独立 `uv + Python` project |
| Python | 3.12 |
| Canonical cases | 45 |
| Review 状态 | 20 `accepted`、25 `reviewed` |
| 分类 | 13 `exact`、6 `reject`、26 `native_only` |
| 默认生成结果 | seed `20260726` 下 306 个 SSE wire variants |
| 工具测试 | 36 个 pytest tests |

覆盖内容：

- Chat → Responses 与 Responses → Chat 的 text stream/non-stream；
- 双向单 function call、并行 calls、fragmented arguments 和 tool result；
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
- JSON Schema、provenance、secret scan、重复 JSON key、case 内路径与未声明文件、artifact 组合、terminal count、deterministic generation、coverage report 与 deterministic ZIP。

## Mock Server/Client

同一个 `tools/corpus/` Python 工具还实现了：

- incremental SSE parser；
- canonical case 到 server scenario 和 client plan 的编译；
- 基于 `asyncio + h11` 的 HTTP/1.1 Mock Server 与无自动重试的 Mock Client；
- normal terminal、HTTP error、transport abort、EOF 和 cancellation observation；
- 单 exchange 与有序多-exchange loopback；
- 零次或单次上游 attempt 的 canonical observation 判定，覆盖 identity、上下游 path、JSON/SSE body、
  HTTP status、结束分类、terminal、声明的下游 response headers 与 body hash 自洽性。

这些工具不加载或启动 OpenBridge，不读取 credential，也不调用真实 Provider。`testdata/runtime/` 中的 scenario、
plan 和 observation 均为可重建临时产物，不进入 corpus ZIP。

## 验证命令与结果

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
- Rust 回归：`cargo fmt`、76 个默认测试和 Clippy 零告警通过，1 个需要下载外部 SDK 的测试保持 ignored；
- `git diff --check` 通过；
- `generated/`、`reports/`、`dist/`、`runtime/`、`.venv/` 和 Python caches 均被 Git 忽略。

## 这证明什么

- schema、45 个 canonical cases 和 7 份 provenance 可被当前工具读取与校验；
- 默认 seed 的 wire variant generation 可重复；
- pack 不包含 derived directories，并具有固定 entry metadata 和内容 manifest；
- coverage report 会显式暴露未固定 source ref 和 pending license，而不是把它们隐藏为已完成。
- 进程级 loopback 已验证 scenario/plan 编译、Mock Server/Client 调用和双方 terminal observation。
- 单 case verifier 能确定地接受匹配 observation，并以不回显正文的字段路径拒绝 JSON、SSE、path、transport、
  terminal、header 或摘要不匹配。
- Rust bridge replay 只读复用 canonical SSE，验证双向文本/并行 tool identity、四类 Responses terminal、
  不完整 arguments、event/type 冲突、EOF、terminal 后事件、重复 terminal 与重复 output identity。

## 这不证明什么

- 不证明全部 case 已被 OpenBridge 执行或通过；当前 Rust tests 只读回放了明确列出的 bridge 与 429 fixture；
- 不证明完整 Bridge Plan、wire renderer、production route、continuation 或 hosted tool 已实现；
- 不证明 canonical oracle 等于完整 OpenAI API；
- 不证明外部项目默认分支在未来保持相同行为；
- 不证明真实 SDK、Agent 或 Provider 兼容。
- 不证明 TLS、HTTP/2、并发、背压、负载或真实网络 packet 边界。

## 已知待处理项

- 7 份外部/项目来源当前均未固定 commit；
- 两份 OpenAI protocol 文档来源的许可证状态仍为 `pending`；
- 25 个涉及 OpenBridge 错误、commit point、identity 与 continuation 策略的 case 保持 `reviewed`；
- 已有最小 Rust loopback runner 同时启动 OpenBridge Router 与 mock upstream，通过真实 HTTP socket 回放
  `responses_native.rate_limit.non_stream`，验证两次 attempt、上游 request 和最终安全错误；它尚不是可枚举全部
  canonical cases 的通用 CLI，也未覆盖 streaming cancellation、fallback 序列或 bridge renderer。
- Python 单 case verifier 仍只消费已经生成的 observations，不启动 OpenBridge。

## 关联文档

- [测试集调研](../references/cross-project/chat-responses-sse-tool-test-suite-survey.md)
- [Corpus README](../../testdata/README.md)
- [Testkit README](../../tools/corpus/README.md)
