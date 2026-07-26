# OpenBridge Protocol Corpus

这是独立于 OpenBridge runtime 和 Rust 测试的协议测试语料。当前版本只提供 canonical cases、外部 provenance、deterministic generation recipes，以及独立的 `uv + Python` 管理工具。

## 使用

从仓库根目录运行：

```powershell
uv run --project tools/corpus corpus --root testdata lint
uv run --project tools/corpus corpus --root testdata generate --seed 20260726
uv run --project tools/corpus corpus --root testdata report
uv run --project tools/corpus corpus --root testdata pack
```

工具测试：

```powershell
uv run --project tools/corpus pytest tools/corpus/tests
```

## 数据解释

- `cases/` 是人工审查的 canonical oracle；
- `sources/` 记录来源事实，不自动构成功能承诺；
- `recipes/` 只描述 transport bytes 分片；
- `generated/`、`reports/` 和 `dist/` 是可重建产物，不进入 Git；
- 三类命令的 `--output` 只能写入各自的上述派生目录；
- case artifact 必须留在所属 case 目录内并由 `case.json` 显式声明；
- canonical JSON 不允许重复 object key；
- case 通过数据 lint 不代表 OpenBridge 已实现或通过该 case。

详细构建与集成边界见[协议测试语料构建](../docs/implementation-plans/protocol-test-corpus.md)。

## 0.1.0 内容

- 13 个 canonical cases：10 个 `accepted`、3 个 `reviewed`；
- 双向 text stream/non-stream；
- 双向 single function call、parallel calls 与 tool result；
- 后续仅带 index 的 arguments fragments；
- Responses terminal 前 EOF；
- hosted tool 与无受限 ledger continuation 的 proposed preflight reject；
- 默认 seed 下生成 60 个 SSE bytes fragmentation variant。

当前外部来源记录均保存了 URL 和获取日期，但尚未固定 commit；OpenAI protocol 文档的许可证状态保留为 `pending`。这些状态会出现在 `corpus report`，不影响自主改写 canonical payload 的结构校验，但必须在复制外部文件或把数据集提升为发布合规证据前复核。
