# OpenBridge semantic testing

本文是项目 semantic case、execution plan、normalized trace 和结果解释的流程 owner。外部方法依据见[semantic evaluation methods](../docs/references/semantic-testing-methods.md)；canonical 数据模型和 release 规则仍由 [README](README.md) 维护。

## 1. 目标与非目标

语义测试回答两个不同问题：

1. OpenBridge 是否在 Native/Bridge、Chat/Responses 和 stream/non-stream 路径中保持当前公共语义；
2. 显式选择的真实 model/Provider 在固定任务、长度和运行条件下是否给出满足 oracle 的输出。

默认 corpus/testkit 只负责第一个问题的可重复合同部件和第二个问题的任务/oracle。它不读取 credential、不选择 model、不启动 OpenBridge、不调用 Provider，也不把 model quality 写成 capability。

## 2. Case 类型

| 类型 | 当前覆盖 | 判定 |
|---|---|---|
| `function` | 无工具、单/并行工具、选择控制、参数、澄清、结果 grounding | call/result identity、JSON arguments、集合/顺序与固定回答事实 |
| `context` | literal retrieval、latent association、multi-fact integration、stale/current conflict | 跨 byte 长度与 start/middle/end 位置的固定答案和禁答事实 |
| `structured` | 一个自主编写的 nested strict JSON Schema | assistant text 必须可解析并满足 case response schema |

所有 case 都声明四个适用方向：`chat_native`、`responses_native`、`chat_to_responses`、`responses_to_chat`。这表示 runner 可以复用同一 oracle，不表示四条 production path 已执行。

## 3. 确定性流程

### 3.1 校验 canonical corpus

```powershell
uv lock --check --project tools/corpus
uv run --project tools/corpus corpus --root testdata lint
uv run --project tools/corpus pytest tools/corpus/tests
```

### 3.2 编译 execution plan

普通 function/structured case 直接编译 task；context case 还必须选择 case 已声明的 byte 和位置轴：

```powershell
uv run --project tools/corpus corpus --root testdata build-semantic-plan `
  --case context.literal_retrieval `
  --target-bytes 16384 `
  --placement middle
```

输出只能写入 `testdata/runtime/`，并通过 `semantic-plan.schema.json`。context prompt 的 UTF-8 byte 长度精确等于目标值；distractor 由 case seed 确定。byte 不是 token，live runner 必须单独记录实际 input token usage。

### 3.3 执行与规范化

protocol adapter/runner 应：

1. 从 plan 构造目标 Chat 或 Responses 请求；
2. 明确记录 Native/Bridge、stream、Provider target、Public Model 和 reasoning 设置；
3. 执行工具时只使用 case 定义的 synthetic tool，保持 `call_id`；
4. 把输出规范化为 `assistant_tool_call`、`tool_result`、`assistant_message`；
5. 不把协议 envelope 或 Provider 私有字段塞入 semantic trace；这些属于 wire evidence。

默认 testkit 不实现该网络 runner。loopback、外部 SDK、Agent runtime 和 live Provider 分属更高证据层，必须由对应 owner 显式执行。

### 3.4 输入与资源上限

Corpus/testkit 把 canonical 与 runtime JSON 视为受验证输入，而不是无限可信数据：单文件最多 16 MiB，单字符串最多 8 MiB，JSON 深度最多 128、节点最多 200,000；SSE 最多 8,192 个 blocks/4,096 个 data events，且每个 `data:` JSON 使用同一 strict loader。Semantic trace 最多 4,096 个 events。Context case 的目标 prompt 最多 8 MiB，distractor template 最多 1 KiB，长度轴最多 16 个值。超限输入在 schema、strict JSON loader 或 corpus lint 阶段拒绝，不能进入 plan generation、semantic matching 或 pack。

### 3.5 判定 trace

```powershell
uv run --project tools/corpus corpus --root testdata verify-semantic-trace `
  --case structured.strict_nested_json `
  --trace testdata/semantic-cases/structured/structured.strict_nested_json/reference-trace.json
```

verifier 失败只报告字段路径和错误类别，不回显 prompt、arguments、tool output 或 assistant text。

## 4. Context sweep

同一比较内必须固定：checkout、case、seed、route、endpoint、model、reasoning effort、stream 和输出限制。对每个声明长度分别运行 start/middle/end，并记录：

- generated UTF-8 bytes；
- 实际 input/output tokens（若 runtime 提供）；
- pass/fail 与失败类别；
- TTFT、总时长、重试和实际 Provider attempt；
- 是否发生 compaction、fallback、omission 或 protocol conversion。

literal retrieval 是 addressability control，不能单独代表有效推理长度。association、multi-fact 和 conflict 应分开报告；不得把四个 synthetic cases 汇成未经校准的“模型总分”。

## 5. Gateway semantic matrix

### 测试目标分层

`semantic-cases/` 的 task/oracle 面向模型任务结果，不直接构成网关不变量。例如 `tool_choice:required` 的转换测试应证明选择约束到达上游，不应要求网关强制模型生成 tool call；context 检索与澄清措辞也不是转换算法的验收。

网关默认回归按独立转换机制选择 synthetic 场景，只复用任务中的固定数据，不运行模型评测或全部 case × direction × stream 的笛卡尔矩阵。请求投影与响应解析不得调用 production codec 生成 expected，也不得把当前错误输出改成正确 oracle。

### 精简与消融规则

- 用共享缺陷操作描述身份、参数、结果关联、输出内容、工具声明、结构化约束和终态丢失，不为每个 case 人造专属 mutant。
- 对实际观察结果执行适用扰动，并记录检出矩阵与 leave-one-out 损失。方向可作为独立维度，因为两套转换编码器不共享全部故障路径；不得按 Model/Provider 复制同一机制。
- 对有界缺陷集合求最小覆盖只证明该实验内的冗余，不证明完整产品覆盖或生产源码 mutation coverage。未观察到损失的场景应删除、合并，或提出可执行的额外独立见证。
- 等价表示必须有通过控制，例如并行调用顺序变化不应失败；重复调用不能被集合去重掩盖。
- catalog、schema 与资源限制由 corpus lint/tooling tests 保护；不在测试中再次写死完整 case 清单和数量。

### 执行精简 Router 回归

```powershell
cargo test --locked --test semantic_router_contract -- --nocapture
```

该入口复用 production Router 的 loopback harness，只读取 canonical case 的固定工具、历史和结构化数据。覆盖双向 tool-result history JSON、双向 parallel arguments SSE 和 Chat→Responses structured JSON；独立投影比较调用身份/参数、结果关联/值、工具声明、完整 structured format 与下游输出。它不执行工具，也不运行完整 semantic task 或 Python semantic verifier。

测试同时输出共享观察扰动的检出矩阵、逐项消融损失及有界方向最小覆盖；SSE 额外从实际 wire 删除终态，核验 parser 不接受缺失终态。并行调用换序是正控制。归一化后扰动不验证 parser 对所有 wire 变异的敏感性，也不等于修改生产源码后的 mutation testing。

该集合是机制核心，不是全面语义验收：没有新增 Native、reasoning、工具选择控制、普通用户/instruction 文本、usage 或媒体矩阵。既有 Rust contract 继续拥有这些边界，不能因该入口通过而宣布全部分支语义稳定。

### 证据 owner

- capability acceptance/enforcement：使用 admin probe、差分值和非法值，不由 semantic oracle 推断；
- Chat/Responses wire 与 streaming：Rust contract tests + wire corpus；
- function/structured/context 结果：normalized semantic trace；
- web search、hosted/custom tools、stateful Agent loop：只有当前公共合同实现后才新增 required case；
- live web 或真实 Provider 漂移：带日期 evidence，不进入默认 CI。

## 6. Evidence 与存储

`reference-trace.json` 只证明 oracle 自洽；`reviewed` case 只证明设计经过人工审查。实际 run 的 plan、trace 和临时结果留在 ignored `testdata/runtime/`。只有经过脱敏、明确记录 checkout、时间、配置形状、范围和“不证明什么”的结果，才可进入 `docs/implementation-status/evidence/`。

## 7. 当前未覆盖

本流程没有实现通用 network/live runner、完整 capability parameter differential/enforcement matrix、Chat↔Responses canonical IR round-trip 报告、模型生成失败时的 strict retry、live web-search dataset、effective-length 自动曲线/85% threshold、排行榜或生产指标聚合。需要这些能力时必须建立新的获准切片、固定来源与运行边界；不能从 0.9.0 的 reference traces 或 synthetic pass/fail 推断。多任务 IR 的设计准入见 [ADR-0002](../docs/decisions/0002-task-ir-and-semantic-ownership.md)，推进顺序见[下一步目标](../docs/implementation-plans/next-goal.md)。下节之后的 codec 方法是待按切片落实的验收约束，不表示新 runner 或任务 case 已存在。

## 8. 新增 case

1. 先确定它证明的 OpenBridge 语义和不证明的 model/Provider 事实；
2. 使用自主编写的 synthetic task，或先完成外部数据 license/provenance 审核；
3. 新增 `case.json`、通过自身 oracle 的 `reference-trace.json` 和 verifier 负例；
4. 更新 catalog required feature；
5. 运行 lint、完整 Python tests、coverage report 与 deterministic pack；
6. 只有 task、oracle、provenance、license 与负例审查完成时才把 `reviewed` 提升为 `accepted`；`accepted` 仍不证明 SUT 或 Provider 已通过。

## 9. Provider 无关的任务 IR/codec 验收

### 准入与职责

先按 [ADR-0002 的设计准入](../docs/decisions/0002-task-ir-and-semantic-ownership.md#6-设计准入先于语料扩张)明确任务语义、所有权与失败边界，再将语料纳入正式验收。设计阶段可用小型反例检查类型是否折叠有意义的差异；既有测试通过、来源已选或 fixture 已下载都不代表设计准入通过。

Provider 无关意味着测试不需要真实模型、账号、凭据或上游网络；仍必须标明任务、wire 协议、profile 与预期可表达性。优先直接调用纯 codec/IR；只有增加独立价值时才补 synthetic Router/loopback。不要为 codec 正确性先搭建通用 live runner。

### 切片验收映射与完成判据

每个待迁移语义域使用最小映射，不新增一套平行 schema 或逐模型库存。规则依据分别为 [ADR-0002](../docs/decisions/0002-task-ir-and-semantic-ownership.md)、[阶段契约](../docs/decisions/0003-ir-pipeline-and-target-compilation.md)、[来源与保真](../docs/decisions/0004-source-records-and-fidelity.md)及[Event 交付](../docs/decisions/0005-event-ir-and-delivery-lifecycle.md)。

| 记录项 | 必须回答 |
|---|---|
| 支持范围 | 哪个任务、协议/profile、方向和交付模式；是否属于当前公共合同 |
| 语义 owner | 必需的值、presence、身份、资源用途与合法关联由哪个 IR 域拥有 |
| codec 与保留边界 | decode/encode 的独立预期；哪些来源记录可用、何时失效、如何拒绝或按既定策略转换 |
| 生产接线 | 实际调用方是否使用该 IR 及最终 requirements；候选是否独立投影；后置映射是否保持语义 |
| 测试见证 | 最低 owning layer 的独立 expected、正/负控制；必要的 Router 见证及本次实际执行范围 |

类型能表达、codec 已映射、生产已接线与测试已执行是不同结论；测试函数存在不是运行结果，helper 接受域不是 Public Model 支持域。具体当前缺口只在[实施状态](../docs/implementation-status/current-boundaries.md)维护，常规执行结果在交付或既有 CI 报告，不另建完成日志。

适用切片至少保护以下独立机制：

- 不变时同协议语义保真；新增、替换、删除与重排真正影响 wire；来源字段和 annotation 不复活、不误绑，依赖旧内容的记录拒绝复用或执行明确授权策略。
- 从变换后的最终 IR 导出 requirements；各候选的合法省略互不污染，不可表达不筛选 Route；Provider 目标映射不能恢复被移除语义。
- 对 Event 给出独立的增量 encode 预期，不只验证 reducer 或最终聚合值；适用 materialize 与独立静态预期一致，合法分片变化为正控制，EOF、身份冲突、超限及提交前后失败为负控制。

空 changes 不证明完整无损，round-trip 不证明独立 decode/encode 正确，减少解析次数不证明语义迁移完成。适用语义、表示依赖、生产调用和失败边界全部闭合后，才能宣称该切片达标；没有受影响的任务或交付分支不为凑矩阵新增测试。

### 独立 oracle 与最小覆盖

| 检查 | 应保护的结果 |
|---|---|
| wire → IR | 独立预期的语义值、presence、身份与资源用途；不同输入不是无解释地折叠为相同 IR |
| IR → wire | 人工审定或外部协议依据的 expected，不调用同一个生产 encoder 生成答案 |
| 同协议往返 | 等价规范化、来源限定保留与不可表达拒绝分别声明；不把 byte equality 当作唯一保真标准 |
| IR 变换 | 新增、替换、删除真实影响 wire；删除 item 后其元数据不复活，重排不误绑旧下标 |
| 请求/响应闭合 | 候选数量、工具 call/result、输出模式、部分/失败结果在两侧一致；不只测请求可编码 |
| 任务隔离 | 相同 Chat 形状的对话、ASR、TTS 或声音条件按固定任务解释；任务与 endpoint 不匹配失败 |
| 媒体与向量 | bytes 和必要格式/用途不变；Embedding 维度、index、有限值、精度与编码转换符合显式策略 |
| 流式等价 | 不同合法字节分片与 UTF-8/SSE 边界产生相同语义事件；错误终态、EOF、取消及预算不伪装成功 |
| 负例 | 删除字段/终态、错配身份、重复 key、非法扩展、超限等扰动能被对应 oracle 检出 |

round-trip 仅是补充：decoder 和 encoder 可能以相同方式丢掉字段而互相自证。源码 mutation、wire 扰动和 normalized-trace 扰动各自说明作用层，不相互替代。

### 语料选择与接入

- 优先复用已有 canonical wire cases；新样本只补独立语义或失败边界，不按 Provider/model 复制矩阵。
- 协议实现的 JSON/SSE fixtures 可提供候选 wire；JSON Schema 样本可测试 schema 保留，SSE 规范样本可测试 framing。外部实现的输出不自动成为 OpenBridge oracle，schema 保真也不证明模型执行 schema。
- 媒体优先使用小型、确定生成的有效格式 payload；只测元数据时明确合成 bytes 不是媒体解码验证。向量使用已知数值、精度/维度边界，不用检索或生成质量分数判断 codec。
- 固定源版本并审核许可证、敏感信息、大小与必要的语义投影；仅纳入最小合法样本。来源元数据归既有 `sources/` 契约，不在架构/ADR 中复制数据目录。
- 现有 function/context/structured case 模型不因此变成所有任务的万能容器。任务 codec fixtures 与 Rust expected 放在最低 owning layer；确需改变 canonical 格式时，按获准切片同步 schema、fixture、tooling 与测试。

数据准备可预筛，正式验收须在设计准入和独立 oracle 具备后进行。默认测试不下载媒体、不调用 Provider，不引入付费 probe、质量 benchmark 或生产能力声明。
