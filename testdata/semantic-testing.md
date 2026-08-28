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

- capability acceptance/enforcement：使用 admin probe、差分值和非法值，不由 semantic oracle 推断；
- Chat/Responses wire 与 streaming：Rust contract tests + wire corpus；
- function/structured/context 结果：normalized semantic trace；
- web search、hosted/custom tools、stateful Agent loop：只有当前公共合同实现后才新增 required case；
- live web 或真实 Provider 漂移：带日期 evidence，不进入默认 CI。

## 6. Evidence 与存储

`reference-trace.json` 只证明 oracle 自洽；`reviewed` case 只证明设计经过人工审查。实际 run 的 plan、trace 和临时结果留在 ignored `testdata/runtime/`。只有经过脱敏、明确记录 checkout、时间、配置形状、范围和“不证明什么”的结果，才可进入 `docs/implementation-status/evidence/`。

## 7. 当前未覆盖

本流程没有实现通用 network/live runner、完整 capability parameter differential/enforcement matrix、Chat↔Responses canonical IR round-trip 报告、模型生成失败时的 strict retry、live web-search dataset、effective-length 自动曲线/85% threshold、排行榜或生产指标聚合。需要这些能力时必须建立新的获准切片、固定来源与运行边界；不能从 0.8.0 的 reference traces 或 synthetic pass/fail 推断。

## 8. 新增 case

1. 先确定它证明的 OpenBridge 语义和不证明的 model/Provider 事实；
2. 使用自主编写的 synthetic task，或先完成外部数据 license/provenance 审核；
3. 新增 `case.json`、通过自身 oracle 的 `reference-trace.json` 和 verifier 负例；
4. 更新 catalog required feature；
5. 运行 lint、完整 Python tests、coverage report 与 deterministic pack；
6. 只有 task、oracle、provenance、license 与负例审查完成时才把 `reviewed` 提升为 `accepted`；`accepted` 仍不证明 SUT 或 Provider 已通过。
