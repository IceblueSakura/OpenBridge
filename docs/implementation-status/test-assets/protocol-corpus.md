# 协议测试语料与工具

## 状态与所有权

**已确认。** `testdata/` corpus 与 `tools/corpus/` Python testkit 保持 runtime-independent；它们不隐式启动
OpenBridge、不读取 credential，也不调用真实 Provider。本页是 corpus 版本、case、variant 与 Python testkit 数量的唯一状态来源。

日常维护说明见 [Corpus 指南](../../../testdata/README.md)与 [Testkit 指南](../../../tools/corpus/README.md)。

## 当前版本

| 项目 | 值 |
|---|---|
| Corpus | `testdata/`，版本 `0.7.0` |
| 工具 | `tools/corpus/`，独立 `uv + Python` project |
| Python 基线 | 3.12 |
| Canonical wire cases | 51（26 `accepted`、25 `reviewed`） |
| Semantic cases | 9（6 `accepted`、3 `reviewed`） |
| Wire 分类 | 13 `exact`、6 `reject`、32 `native_only` |
| 默认生成结果 | seed `20260726` 下 342 个 SSE wire variants |
| Python testkit | 45 个 pytest tests |

覆盖范围包括：

- Chat ↔ Responses 的 text、function tool、tool result、reasoning 与 JSON/SSE 转换；
- Native Chat/Responses 的非流式成功、SSE framing、strict/forced function call 和并行 arguments；
- Responses/Chat terminal、EOF、失败/incomplete/error、重复或冲突 terminal；
- 首输出前 HTTP error、首输出后 transport error、下游取消和 commit point；
- 400/401/403/404/422/429/500/502/503/504、`Retry-After`、纯文本/损坏 JSON error body；
- schema、provenance、secret scan、重复 JSON key、确定性 generation/report/pack 与 ZIP 内容检查。

## 工具边界

Python testkit 提供 incremental SSE parser、canonical case 到 scenario/plan 的编译、基于 `asyncio + h11` 的 HTTP/1.1
Mock Server/Client、observation verifier 和 normalized semantic trace verifier。`testdata/runtime/`、`generated/`、
`reports/` 与 `dist/` 都是可重建派生产物，不进入 canonical corpus。

Rust tests 只读选定 canonical artifact，覆盖 Bridge state machine/conversion、production Router 的 HTTP fault replay，以及
post-output abort、downstream cancellation 和 EOF-before-terminal lifecycle。直接引用 fixture 不等于全部 case 已经经过生产
Router；Python verifier 也不负责实现 OpenBridge 的 Route、retry、fallback 或 credential 策略。

## 记录的验证

2026-08-08 在 Windows、uv `0.12.1`、Python `3.12.9` 上对 `0.7.0` 执行：

```powershell
uv lock --check --project tools/corpus
uv run --project tools/corpus pytest tools/corpus/tests
uv run --project tools/corpus corpus --root testdata lint
uv run --project tools/corpus corpus --root testdata generate --seed 20260726
uv run --project tools/corpus corpus --root testdata report --output testdata/reports/coverage.json
uv run --project tools/corpus corpus --root testdata pack --output testdata/dist/openbridge-protocol-corpus-0.7.0.zip
uv run --project tools/corpus corpus --root testdata pack --output testdata/dist/openbridge-protocol-corpus-0.7.0-repeat.zip
```

结果：lock 与 project 同步，pytest 与 lint 通过；默认 seed 重建 342 个 variant；coverage report 对声明的 required
wire/semantic feature 与 generation kind 无缺口；两次 pack 的 SHA-256 都为
`fbe0d20c4a382c200df1e1a8c035dcf749a38632f5d9139c7e8513b43e7333e5`。派生目录均保持 Git 忽略。

## 已证明与未证明

已证明：

- schema、canonical wire/semantic case 与 provenance 可由当前工具读取、lint 和报告；
- 默认 seed 的 fragmentation generation 与 pack 可重复；
- Rust 的选定 replay 能保护相应 Bridge、HTTP status-first classification、首输出提交、取消和 EOF 生命周期。

未证明：

- 全部 canonical case 已经经过 OpenBridge production Router；
- canonical oracle 等于完整 OpenAI API，或 hosted/custom tool、continuation、图片和 Provider 私有扩展可转换；
- 真实 SDK、Agent、Provider、TLS/HTTP2、并发背压、负载或真实 packet boundary 兼容；
- 外部来源未来仍保持相同行为。

当前 corpus 仍明确保留未固定 source ref、pending license 与 `reviewed` case；coverage report 应继续暴露这些状态，不能把它们
改写为已完成。Python verifier 只消费调用方提供的 observation/normalized trace，不启动模型或工具。

## 相关文档

- [测试资产与保留标准](inventory.md)
- [测试集调研](../../references/cross-project/chat-responses-sse-tool-test-suite-survey.md)
- [Corpus README](../../../testdata/README.md)
- [Testkit README](../../../tools/corpus/README.md)
