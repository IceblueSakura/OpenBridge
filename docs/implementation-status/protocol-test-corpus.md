# 协议测试语料与工具

## 状态

**Confirmed，仅限独立 corpus 与管理工具。** 当前数据集未接入 OpenBridge runtime、Rust tests、SDK、Codex、Hermes 或真实 Provider。

## 当前版本

| 项目 | 值 |
|---|---|
| Corpus | `testdata/`，版本 `0.1.0` |
| 工具 | `tools/corpus/`，独立 `uv + Python` project |
| Python | 3.12 |
| Canonical cases | 13 |
| Review 状态 | 10 `accepted`、3 `reviewed` |
| 分类 | 10 `exact`、2 `reject`、1 `native_only` |
| 默认生成结果 | seed `20260726` 下 60 个 SSE fragmentation variants |
| 工具测试 | 9 个 pytest tests |

覆盖内容：

- Chat → Responses 与 Responses → Chat 的 text stream/non-stream；
- 双向单 function call、并行 calls、fragmented arguments 和 tool result；
- 后续 fragment 只有 index 时的 identity 回归样本；
- Responses terminal 前 EOF；
- hosted tool 与无受限 ledger continuation 的 proposed preflight reject；
- JSON Schema、provenance、secret scan、重复 JSON key、case 内路径与未声明文件、artifact 组合、terminal count、deterministic generation、coverage report 与 deterministic ZIP。

## 验证命令与结果

2026-07-26 在 Windows、uv `0.11.32`、Python `3.12.9` 下运行：

```powershell
uv lock --check --project tools/corpus
uv run --project tools/corpus pytest tools/corpus/tests
uv run --project tools/corpus corpus --root testdata lint
uv run --project tools/corpus corpus --root testdata generate --seed 20260726
uv run --project tools/corpus corpus --root testdata report --output testdata/reports/coverage.json
uv run --project tools/corpus corpus --root testdata pack --output testdata/dist/openbridge-protocol-corpus-0.1.0.zip
```

观察结果：

- lock 与 `pyproject.toml` 同步；
- pytest：9 passed；
- corpus lint passed；
- 生成 60 个 variant，chunks 重组 SHA-256 等于 source SHA-256；
- required core feature 无缺口；
- pack 生成 ZIP 和 `.sha256` sidecar；
- `generated/`、`reports/`、`dist/`、`.venv/` 和 Python caches 均被 Git 忽略。

## 这证明什么

- schema、13 个 canonical cases 和 5 份 provenance 可被当前工具读取与校验；
- 默认 seed 的 bytes fragmentation generation 可重复；
- pack 不包含 derived directories，并具有固定 entry metadata 和内容 manifest；
- coverage report 会显式暴露未固定 source ref 和 pending license，而不是把它们隐藏为已完成。

## 这不证明什么

- 不证明任一 case 已被 OpenBridge 执行或通过；
- 不证明 Bridge、continuation、hosted tool 或相关错误策略已实现；
- 不证明 canonical oracle 等于完整 OpenAI API；
- 不证明外部项目默认分支在未来保持相同行为；
- 不证明真实 SDK、Agent 或 Provider 兼容。

## 已知待处理项

- 5 份外部/项目来源当前均未固定 commit；
- OpenAI protocol 文档的许可证状态仍为 `pending`；
- 两个 reject case 和反向并行 tool case 保持 `reviewed`，集成前仍需结合最终产品决策复核；
- 尚无 OpenBridge-specific replay/runner；这是当前刻意排除项。

## 关联文档

- [协议测试语料构建](../implementation-plans/protocol-test-corpus.md)
- [测试集调研](../references/cross-project/chat-responses-sse-tool-test-suite-survey.md)
- [Corpus README](../../testdata/README.md)
