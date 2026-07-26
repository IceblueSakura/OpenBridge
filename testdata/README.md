# OpenBridge Protocol Corpus

这是独立于 OpenBridge runtime 和 Rust 测试的协议测试语料。当前版本提供 canonical cases、外部 provenance、deterministic generation recipes、增量 SSE parser，以及独立的 Python Mock Server/Client。

## 使用

从仓库根目录运行：

```powershell
uv run --project tools/corpus corpus --root testdata lint
uv run --project tools/corpus corpus --root testdata generate --seed 20260726
uv run --project tools/corpus corpus --root testdata report
uv run --project tools/corpus corpus --root testdata pack
```

构建并运行一个不经过 OpenBridge 的本地 transport loopback：

```powershell
uv run --project tools/corpus corpus --root testdata build-server-scenario --case responses_native.sse_framing --variant event_pairs
uv run --project tools/corpus corpus --root testdata mock-server --scenario testdata/runtime/responses_native.sse_framing.server-scenario.json --ready-file testdata/runtime/server-ready.json

# 另一个终端读取 server-ready.json 中的 base_url：
uv run --project tools/corpus corpus --root testdata build-client-plan --case responses_native.sse_framing --base-url http://127.0.0.1:<port>
uv run --project tools/corpus corpus --root testdata mock-client --plan testdata/runtime/responses_native.sse_framing.client-plan.json
```

需要同一进程连续处理多个上游 exchange 时，按顺序构建 suite：

```powershell
uv run --project tools/corpus obtest --root testdata build-server-suite `
  --case chat_native.text.non_stream `
  --case responses_native.rate_limit.non_stream
```

Mock Server 支持单 scenario 或有序 suite；`/health` 与 `/healthz` 不消耗 exchange。Mock Client 不使用 OpenAI SDK，也不自动重试。`runtime/` 中的 plan、scenario 和 observation 均为可重建产物。

工具测试：

```powershell
uv run --project tools/corpus pytest tools/corpus/tests
```

## 数据解释

- `cases/` 是人工审查的 canonical oracle；
- `sources/` 记录来源事实，不自动构成功能承诺；
- `recipes/` 只描述 transport bytes 分片；
- `generated/`、`reports/`、`dist/` 和 `runtime/` 是可重建产物，不进入 Git；
- 三类命令的 `--output` 只能写入各自的上述派生目录；
- case artifact 必须留在所属 case 目录内并由 `case.json` 显式声明；
- canonical JSON 不允许重复 object key；
- case 通过数据 lint 不代表 OpenBridge 已实现或通过该 case。

详细构建与集成边界见[协议测试语料构建](../docs/implementation-plans/protocol-test-corpus.md)和 [Mock Server/Client 设计](../docs/implementation-plans/protocol-testkit.md)。

## 0.4.0 内容

- 35 个 canonical cases：20 个 `accepted`、15 个 `reviewed`；
- 双向 text stream/non-stream；
- 双向 single function call、parallel calls 与 tool result；
- 后续仅带 index 的 arguments fragments；
- Responses terminal 前 EOF；
- failed、incomplete、error、Chat DONE 前 EOF、duplicate/late terminal；
- 首输出前后 transport error 与 downstream cancellation；
- 未知/重复 call identity、反序 results、空/不完整/转义 arguments；
- comment、multiline data、CRLF、all-in-one 与 event-pairs transport；
- hosted tool 与无受限 ledger continuation 的 proposed preflight reject；
- 默认 seed 下生成 306 个 SSE wire variants；
- 新增 server scenario、client plan 与 observation JSON Schema；
- 新增增量 SSE parser、单场景 HTTP/1.1 Mock Server 和 Mock Client；
- 支持正常结束、逻辑 EOF、HTTP error、异常断连、事件后取消与敏感 header 脱敏。
- 吸收 Rust mock 的 Chat/Responses 原生非流式成功响应、双协议 429 与 `Retry-After`；
- Mock Server 支持健康检查、非法 JSON 400、未知 endpoint 404 和同进程有序多 exchange。

当前外部来源记录均保存了 URL 和获取日期，但尚未固定 commit；OpenAI protocol 文档的许可证状态保留为 `pending`。这些状态会出现在 `corpus report`，不影响自主改写 canonical payload 的结构校验，但必须在复制外部文件或把数据集提升为发布合规证据前复核。
