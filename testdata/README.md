# OpenBridge Protocol Corpus

`testdata/` 是一个可独立发布、可复现的协议测试语料。它固定 Chat Completions、Responses、SSE、function tool 和 HTTP/transport
失败的输入、上游 wire 与预期输出；它不启动 OpenBridge，也不依赖 Rust crate、服务配置、API key 或真实 Provider。

当前 release 为 **0.8.0**：51 个人工审查的 canonical wire cases（26 `accepted`、25 `reviewed`）、14 个协议无关 semantic
cases（6 `accepted`、8 `reviewed`），以及默认 seed 下 342 个可重建的 SSE 分片变体。该版本向后兼容保留 function-tool
case，新增 synthetic context length/position、strict structured output、`semantic-plan` schema 和零网络 plan compiler；既有 wire case
和 runtime document 的 `schema_version` 仍为 `0.1`。项目语义测试流程见 [semantic-testing.md](semantic-testing.md)。

配套的校验、生成、打包和 HTTP/SSE mock 工具位于 [../tools/corpus/README.md](../tools/corpus/README.md)
。当前已验证状态和集成边界见[当前实现](../docs/implementation-status/current-state.md)和
[当前状态边界](../docs/implementation-status/current-boundaries.md#6-测试资产边界)。

## 何时使用

使用 corpus 来：

- 设计或审查 Chat/Responses、SSE、tool-call identity 的可观察契约；
- 用协议无关 oracle 判断工具选择、参数、调用集合与固定最终回答事实；
- 编译确定的 function、context 或 structured execution plan，并统一判定 normalized trace；
- 为后续 SUT runner 编译确定的上游 scenario 与下游 client plan；
- 回归测试 bytes fragmentation、SSE terminal、HTTP error、EOF、abort 和 cancellation 的区分；
- 向其他项目交付不含凭证、可校验的协议 fixture ZIP。

它 **不**证明 OpenBridge 已实现或通过任一 case，也不直接测试 routing、retry、fallback、转换或真实 SDK/Provider。那些工作必须由后续
runner 或集成测试显式完成。

## 快速开始

仓库根目录下，先确认 Python 工具环境和 corpus：

```powershell
uv lock --check --project tools/corpus
uv run --project tools/corpus corpus --root testdata lint
uv run --project tools/corpus pytest tools/corpus/tests
```

生成所有 SSE transport 变体、查看覆盖统计、生成可发布 ZIP：

```powershell
uv run --project tools/corpus corpus --root testdata generate --seed 20260726
uv run --project tools/corpus corpus --root testdata report --output testdata/reports/coverage.json
uv run --project tools/corpus corpus --root testdata pack --output testdata/dist/openbridge-protocol-corpus-0.8.0.zip
```

`lint` 与测试不要求网络、服务端或 credential。`generate`、`report`、`pack` 的输出只能位于对应的派生目录，避免覆盖 canonical
data。

## 数据层和目录

```text
testdata/
  VERSION                 # corpus release，例如 0.8.0
  catalog.json            # wire/semantic case id、required feature、默认 seed
  schemas/                # JSON Schema，schema_version 目前为 0.1
  cases/                  # 人工审查的 canonical request/wire/oracle
  semantic-cases/         # 协议无关 function/context/structured task、oracle 与 trace
  sources/                # 外部或项目来源的事实与许可证状态
  recipes/                # 仅描述 SSE bytes 分片方式
  generated/              # 生成的 wire 变体，忽略且可重建
  reports/                # 覆盖报告，忽略且可重建
  dist/                   # deterministic ZIP 与 sha256，忽略且可重建
  runtime/                # scenario、plan、observation，忽略且可重建
```

| 层                              | 作用                                          | 是否可手改                     |
|---------------------------------|-----------------------------------------------|--------------------------------|
| `sources/`                      | 记录 URL、取得日期、ref、license 与已观察事实 | 可以；不得复制受限外部 payload |
| `cases/`                        | canonical 输入、wire 和 expected output       | 可以；必须经审查并通过 lint    |
| `semantic-cases/`               | task、machine-readable oracle 与 reference trace | 可以；必须保持协议无关       |
| `recipes/`                      | 只定义分片种类和 seeded 变体数                | 可以；不表达协议语义           |
| `generated/`                    | 从 canonical SSE 生成的 Base64 chunks         | 不应手改                       |
| `reports/`、`dist/`、`runtime/` | 运行产物                                      | 不应手改                       |

`catalog.json` 是 wire 与 semantic case 集合的清单。`lint` 会拒绝 catalog 与实际目录不一致、case 内有未声明文件、artifact
路径逃逸目录、重复 JSON key、疑似 secret、不自洽的 stream/non-stream artifact 组合、无效 function JSON Schema、违反
`strict=true` 的对象 schema，或不能通过自身 oracle 的 reference trace。Strict JSON loader 还拒绝 `NaN`/Infinity、超过 16 MiB
的 canonical/runtime JSON、超过 128 层或 200,000 nodes 的结构，以及超过 8 MiB 的单字符串；raw canonical artifact 同样受
16 MiB 文件上限约束。

## Canonical case 的结构

一个 case 目录的最小形态如下：

```text
cases/<category>/<case-id>/
  case.json
  client-request.json
  expected-upstream-request.json       # 有上游 attempt 时
  upstream-response.json|txt           # non-stream 或首输出前 HTTP error
  expected-client-response.json|txt
```

成功或失败的流式 case 则使用 `upstream-stream.sse` 与 `expected-client-stream.sse`。preflight reject 没有 upstream
artifact，且 `expectation.upstream_attempts` 必须为 `0`。

`case.json` 的关键字段：

| 字段                                         | 含义                                                                         |
|----------------------------------------------|------------------------------------------------------------------------------|
| `id`、`title`、`category`、`direction`       | 稳定标识、用途、类别与协议方向                                               |
| `status`                                     | `draft`、`reviewed`、`accepted` 或 `deprecated`                              |
| `stream`                                     | 业务请求是否要求 stream；首输出前 HTTP error 可使用 non-stream artifact      |
| `artifacts`                                  | 该 case 唯一允许存在的 request/wire/expected 文件                            |
| `expectation`                                | outcome、terminal 数量、upstream attempts、fallback 是否允许及不变量         |
| `transport`                                  | 上下游 HTTP status、Content-Type、headers、结束方式、失败阶段和 cancel point |
| `features`                                   | 覆盖报告用的能力标签                                                         |
| `provenance_ref`、`proves`、`does_not_prove` | 证据来源和明确边界                                                           |

`status` 只表示 canonical case/oracle 的审查成熟度，不表示 OpenBridge、SDK、Agent runtime 或真实 Provider 已执行或通过。

`classification` 的含义是：

- `exact`：共同子集应保持声明的结构和 identity；
- `approximate`：允许转换损失，但必须有明确 notice；
- `reject`：上游调用前拒绝；
- `native_only`：只适用于同协议原生路径；
- `research_only`：只记录观察，不是后续 required oracle。

`fallback_allowed` 只表达 case 所在的输出 commit point 是否允许后续决策；它不要求 Mock Client 或 corpus 选择、执行或验证
fallback。

## Semantic case 与规范化 trace

`semantic-cases/<kind>/<case-id>/case.json` 不保存 Chat 或 Responses envelope，而是声明一份可被四个方向共同消费的 task。
`function` 固定工具定义与选择控制，`context` 固定 synthetic needle/distractor、byte 和 position 轴，`structured` 固定 response
schema。每个目录只再包含一个 `reference-trace.json`，它必须通过该 case 的 oracle；完整运行流程见
[semantic-testing.md](semantic-testing.md)。

规范化 trace 由按时间排列的三类 event 构成：

- `assistant_tool_call`：`turn`、`call_id`、function `name` 与已经解析为 JSON object 的 `arguments`；
- `tool_result`：`turn`、关联 `call_id` 与任意 JSON `output`；
- `assistant_message`：`turn` 与最终 `text`。

verifier 会检查 event 顺序、call/result identity、工具参数 JSON Schema、精确或包含式参数匹配、有序或无序调用集合、额外/禁止调用、
预置 tool result、最终回答必含/禁含事实，以及 structured assistant text 的 JSON parse/schema。失败诊断只包含字段路径和错误类别，
不回显 prompt、arguments、tool output 或回答正文。

```powershell
uv run --project tools/corpus corpus --root testdata verify-semantic-trace `
  --case function.result_grounding `
  --trace testdata/semantic-cases/function/function.result_grounding/reference-trace.json
```

该层只定义协议无关判定契约；后续 OpenBridge runner 仍需把 Chat/Responses Native 或 Bridge 输出明确规范化为 trace。reference trace
通过不等价于真实模型、SDK、Provider 或 OpenBridge 已通过该语义 case。

## 当前覆盖

语料包含：

- Chat ↔ Responses 双向 text 的 stream/non-stream；
- 双向单/并行 function call、tool result、交错和只带 index 的 arguments fragment；
- Native Chat 与 Native Responses 对称的 strict/forced 单 function call、结构化 tool result 与 parallel stream；
- 14 个 semantic cases：9 个 function-tool、4 个 context retrieval/integration/conflict 和 1 个 strict nested JSON；
- Responses `completed`/`failed`/`incomplete`/`error`、Chat `[DONE]`、EOF、duplicate terminal、terminal 后事件；
- SSE comment、多行 `data:`、CRLF、UTF-8 跨分片、all-in-one、event-pairs 和 seeded chunking；
- caller cancellation、首输出前/后 transport error；
- unknown/duplicate `call_id`、反序 tool result、空/不完整/转义 arguments；
- hosted tool 与无受限 ledger continuation 的 proposed preflight reject；
- HTTP error matrix：`400`、`401`、`403`、`404`、`422`、`429`、`500`、`502`、`503`、`504`。

HTTP 错误还覆盖 delta-seconds 与 HTTP-date 两种 `Retry-After`，OpenAI 风格 JSON、纯文本与损坏 JSON body，以及 `4xx/5xx`
却错误标为 `text/event-stream` 的分类边界。

用下列命令获取机器可读的当前统计，而不要在脚本中硬编码本文数字：

```powershell
uv run --project tools/corpus corpus --root testdata report
```

## 生成的 SSE 变体

recipe 只改变 bytes 到 chunks 的划分，不改变逻辑事件、payload、terminal、arguments 或 expected oracle。默认包含：

| kind              | 目的                                 |
|-------------------|--------------------------------------|
| `one_byte`        | 每个 byte 单独写入，覆盖最细粒度边界 |
| `line_boundaries` | 在 SSE 行边界拆分                    |
| `utf8_split`      | 强制跨 UTF-8 code point 边界         |
| `all_in_one`      | 所有 wire 一次写入                   |
| `event_pairs`     | event/data 对附近拆分                |
| `crlf`            | 逻辑等价的 CRLF wire                 |
| `seeded`          | 由 seed 决定的额外拆分               |

生成 manifest 保存 canonical/wire SHA-256、transformation 与 Base64 chunks。相同 corpus、工具版本和 seed 必须产生相同内容；TCP
实际 read 边界仍可能不同，因此黑盒断言应比较逻辑事件、顺序、bytes hash 和结束方式，而不是 socket read 次数。

## 新增或修改 case 的流程

1. 先判断它是 wire 协议/HTTP/transport 边界、协议无关工具语义还是仅仅新的分片；分别进入 `cases/`、`semantic-cases/` 或
   `recipes/`，不得为分片差异复制 case。
2. 在 `sources/` 中记录可复核的来源事实，或使用项目设计来源并明确 `does_not_prove`。不要保存 credential、cookie、私人 prompt
   或未脱敏 request id。
3. 新建 `cases/<category>/<case-id>/`，写入 `case.json` 和所有声明 artifact；case id 必须与目录名一致。
4. 将 id 加入 `catalog.case_ids` 或 `catalog.semantic_case_ids`，并按所有权更新 `required_core_features` 或
   `required_semantic_features`；只在确认需要时调整默认 seed 或 recipe。
5. 更新 `VERSION` 与 `catalog.corpus_version`。只改 case 内容或向后兼容地扩展 schema 可升级 corpus release， 并记录兼容边界；破坏性
   schema 改动必须升级 `schema_version` 并提供兼容说明。
6. 更新本文与相关设计/现状文档，尤其是覆盖范围和不证明事项。
7. 运行 lint、工具测试、generate、report 和至少两次 pack；两次 ZIP hash 应相同。

推荐的完整验证命令：

```powershell
uv lock --check --project tools/corpus
uv run --project tools/corpus corpus --root testdata lint
uv run --project tools/corpus pytest tools/corpus/tests
uv run --project tools/corpus corpus --root testdata generate --seed 20260726
uv run --project tools/corpus corpus --root testdata report --output testdata/reports/coverage.json
uv run --project tools/corpus corpus --root testdata pack --output testdata/dist/openbridge-protocol-corpus-<version>.zip
```

## 发布、复现与安全

`pack` 只包含 canonical corpus、schema、recipe 和 provenance；不会包含 `generated/`、`reports/`、`dist/`、`runtime/`
、虚拟环境或工具缓存。ZIP entry 顺序与时间戳固定，并生成 `.sha256` sidecar。

发布前仍需查看 `report` 中的 `pending_license_sources` 和 `unpinned_sources`。通过 lint 只说明结构、路径、secret scan
与内部不变量正确；它不自动解决外部许可证、来源 ref 或语义正确性。

## 与后续 OpenBridge 测试的关系

未来黑盒 runner 的职责是把两端连接到 SUT，并比较：

1. Mock Server 收到的上游请求与 `expected_upstream_request`；
2. Mock Client 收到的 envelope、raw body、SSE event 与 `expected_client_*` artifact；
3. identity、顺序、terminal、attempt、commit point 和明确的不变量；
4. SUT 自身的 retry、fallback、cooldown、转换 notice 与最终错误策略。

工具语义 runner 还需要把任一 Native/Bridge 输出转换为 `semantic-trace.schema.json` 的规范化 event，再调用 semantic verifier；该
规范化步骤本身属于后续接入测试，不能由 reference trace 自证。

本目录和 `tools/corpus/` 刻意不加载 OpenBridge 配置、不启动二进制，也不声明这些产品行为已经实现。
