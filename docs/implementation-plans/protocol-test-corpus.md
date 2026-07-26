# 协议测试语料构建

## 状态

**现行独立构建模式。** 当前只构建 Chat Completions、Responses、SSE、工具调用和 Bridge 失败边界的版本化测试数据，以及用于校验、生成、统计和打包数据的独立工具。

日常使用、case 维护、release 规则和命令示例以仓库内的 [Corpus 指南](../../testdata/README.md) 为准；本文件只保留设计边界与集成前条件。

在 corpus schema、canonical cases 和工具达到可复现状态前，不接入 OpenBridge Rust 测试，也不以数据集存在声明 Bridge 已实现。原 Rust `upstream-fixture-server` 的离线 mock 行为已经吸收到 Python testkit；真实上游 proxy 不属于本阶段测试工具边界。

## 1. 目标与非目标

目标：

- 用稳定 schema 描述 source request、预期 upstream request、upstream response/SSE 和预期 client response/SSE；
- 区分外部观察事实、项目 oracle 和生成变体；
- 固定 case id、状态、方向、分类、不变量、证明范围与 provenance；
- 用确定性 recipe 生成 bytes fragmentation 变体；
- 提供 `lint`、`generate`、`report`、`pack`；
- 生成可由其他项目消费、但不依赖 OpenBridge 内部类型的版本化 artifact。

当前非目标：

- 不运行 OpenBridge converter、Router 或 transport；
- 不修改 `tests/*.rs` 或接入 `cargo test`；
- 不运行 SDK、Codex、Hermes 或真实 Provider；
- 不从概率性模型输出生成 golden；
- 不用数据集 schema 预先固化生产 Bridge IR；
- 不实现 OpenBridge 专用 replay adapter。

## 2. 三层数据

### 2.1 来源层

`testdata/sources/` 记录外部 repository、文件或 issue 的 URL、固定 ref 状态、获取日期、许可证状态、观察事实和改写说明。默认只保存链接与自主改写的最小 wire 事实；复制外部文件前必须确认许可证并固定 commit。

### 2.2 Canonical case 层

`testdata/cases/` 保存人工审查的稳定输入与 oracle。case 不引用 Rust module、error enum 或实现内部状态，只记录：

- `client_request`；
- `expected_upstream_request`；
- `upstream_response` 或 `upstream_stream`；
- `expected_client_response` 或 `expected_client_stream`；
- outcome、terminal、attempt/fallback 约束和协议不变量；
- 可选 HTTP status、content type、结束方式、首输出前后 failure phase 与 cancellation point；
- `proves` 与 `does_not_prove`；
- provenance 和可选 generation recipes。

非流式 case 不要求伪造 stream terminal；拒绝类 case 不要求构造 upstream artifact。

### 2.3 生成变体层

`testdata/generated/` 由工具按 canonical SSE 与 recipe 生成，不提交版本控制。每个变体以 Base64 chunk 数组记录 wire bytes；相同 corpus、seed 和工具版本必须产生相同 manifest 与内容 hash。除保持原 bytes 的分片外，允许生成逻辑等价的 CRLF wire 变体，并分别记录 canonical 与 wire hash。

生成变体只改变 transport fragmentation，不改变 logical event、arguments 或 oracle。identity 缺失、事件交错和 terminal 冲突属于语义差异，必须保存为独立 canonical case。

## 3. Case 分类

Review 状态：

| 状态 | 含义 |
|---|---|
| `draft` | 已收集，wire 或 oracle 尚未完成审查。 |
| `reviewed` | wire、来源和 oracle 已核对，可用于后续 runner 设计。 |
| `accepted` | 属于版本化核心 corpus。 |
| `deprecated` | 保留历史，但不进入默认 profile。 |

转换分类：

| 分类 | 含义 |
|---|---|
| `exact` | 声明的共同子集应保持结构和身份语义。 |
| `approximate` | 必须记录明确的损失或 notice。 |
| `reject` | 在 upstream 调用前拒绝。 |
| `native_only` | 只能走原生协议，不进入 Bridge。 |
| `research_only` | 只保存观察，不进入后续 required suite。 |

## 4. 目录与版本

```text
testdata/
  README.md
  VERSION
  catalog.json
  schemas/
  cases/
  sources/
  recipes/
  generated/   # ignored
  reports/     # ignored
  dist/        # ignored

tools/corpus/
  pyproject.toml
  uv.lock
  src/openbridge_corpus/
  tests/
```

`schema_version` 表示数据结构版本，`VERSION` 与 `catalog.corpus_version` 表示 corpus release。schema breaking change 必须增加 schema version；case 内容变化必须反映在 pack manifest 的 SHA-256 中。

## 5. 工具契约

从仓库根目录运行：

```powershell
uv run --project tools/corpus corpus --root testdata lint
uv run --project tools/corpus corpus --root testdata generate --seed 20260726
uv run --project tools/corpus corpus --root testdata report --output testdata/reports/coverage.json
uv run --project tools/corpus corpus --root testdata pack --output testdata/dist/openbridge-protocol-corpus-0.5.0.zip
uv run --project tools/corpus pytest tools/corpus/tests
```

### 5.1 `lint`

- JSON Schema 与引用完整性；
- catalog case id 与实际目录一致；
- artifact 路径不能逃逸 corpus；
- artifact 路径不能逃逸所属 case，且 case 目录不得包含未声明文件；
- JSON object 不允许重复 key；
- JSON/SSE 基本可解析；
- stream、non-stream、reject 与 upstream attempt 的 artifact 组合必须自洽；
- expected client stream 的 terminal 数量与 manifest 一致；
- provenance、许可证状态和证明边界非空；
- 检查疑似 credential、Bearer token、cookie 和私有 key。

### 5.2 `generate`

- 只读取声明了 recipe 的 SSE artifact；
- 生成 one-byte、line-boundary、UTF-8 split、all-in-one、event-pairs、CRLF 和 seeded chunking；
- 输出包含 seed、canonical/wire SHA-256、transformation、chunks 和重组 SHA-256；
- 生成后必须验证 chunks 重组为声明的 wire bytes；无 transformation 时 wire bytes 必须等于 canonical bytes。
- `--output` 只能位于 `testdata/generated/` 内，避免清理 canonical 目录。

### 5.3 `report`

按 direction、stream、status、classification 和 feature 输出覆盖统计，同时列出未固定 ref、许可证待审、缺失的 required feature 和缺失的 generation kind。

`--output` 只能位于 `testdata/reports/` 内。

### 5.4 `pack`

只打包 canonical corpus、schema、recipe 和 provenance；不包含 `generated/`、`reports/`、`dist/`、`runtime/` 或工具虚拟环境。ZIP entry、时间戳和 manifest 顺序固定，从而支持相同输入产生相同 hash；ZIP 旁生成 `.sha256` 校验文件。

`--output` 只能位于 `testdata/dist/` 内。

## 6. 数据集质量边界

- canonical JSON 使用 UTF-8；SSE 以原始 bytes 读取，不从平台默认编码推断；
- 不保存真实 credential、cookie、私人 prompt 或未脱敏 request id；
- 外部行为与 OpenBridge 决策分开；`research_only` 不能成为后续 required oracle；
- SDK 最终可聚合不能掩盖中间 delta 丢失；
- generator 不替代 canonical semantic negative cases；
- HTTP error response、SSE terminal、EOF、transport error 与 cancellation 必须分开记录；
- `fallback_allowed` 必须结合首个 downstream output commit point 解释；
- coverage matrix 只说明数据覆盖，不说明 OpenBridge 功能或代码覆盖。

### 6.1 HTTP error matrix

HTTP 错误按语义类别选择代表状态，而不是机械枚举全部 `4xx/5xx`：

- 请求错误：400 与 422；
- 身份错误：401 与 403；
- 资源错误：404；
- 限流：429，分别覆盖 delta-seconds 与 HTTP-date `Retry-After`；
- 服务与网关错误：500、502、503、504；
- body 形态：OpenAI 风格 JSON、纯文本和损坏 JSON；
- envelope 边界：流式请求在首输出前收到 JSON HTTP error，以及错误状态错误地携带 SSE Content-Type。

Canonical case 只固定单次 exchange 的 wire 和分类。重试次数、backoff、candidate
fallback、cooldown 与最终错误选择仍属于后续 SUT runner，不由 Mock Client 自动执行。

## 7. 进入 OpenBridge 集成前的条件

只有同时满足以下条件，才重新评估 runner 与现有测试迁移：

1. schema、目录与 ID 规则发布为稳定 corpus 版本；
2. 核心 case 全部为 `reviewed` 或 `accepted`；
3. `lint`、工具自身测试和 deterministic generation 通过；
4. `report` 能解释 required feature 的覆盖与缺口；
5. `pack` 生成无 secret、自包含、可校验的 artifact；
6. corpus 不依赖 OpenBridge 内部类型；
7. 任何仍有争议的产品策略保持 `research_only` 或明确标记为 proposed；
8. 集成工作另建当前焦点，不把 corpus 完成自动解释为迁移授权。

## 8. 关联材料

- [Chat/Responses、SSE 与工具调用测试集调研](../references/cross-project/chat-responses-sse-tool-test-suite-survey.md)
- [协议桥](protocol-bridge.md)
- [Agent Loop Bridge](agent-loop-bridge.md)
- [TDD 开发方式与证据记录](../functional-requirements/delivery-and-evidence.md)
