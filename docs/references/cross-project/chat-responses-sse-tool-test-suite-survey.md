# Chat/Responses、SSE 与工具调用测试集调研

## 状态与范围

**外部测试资产调研；不代表 OpenBridge 已实现 Chat ↔ Responses Bridge，也不代表采用任一外部项目的兼容口径。**

| 项目 | 值 |
|---|---|
| 研究问题 | 是否已有可直接用于 Chat/Responses 协议转换、SSE 和复杂工具调用 TDD 的公开测试集 |
| 在线复核日期 | 2026-07-26 |
| 参考分支 | 各仓库默认分支；本次只读在线检查未固定 commit |
| OpenAI 官方基线 | [迁移到 Responses：更新流式消费者](https://developers.openai.com/api/docs/guides/migrate-to-responses#7-update-streaming-consumers)、[gpt-oss 实现验证](https://developers.openai.com/cookbook/articles/gpt-oss/verifying-implementations#quick-verification-of-tool-calling-and-api-shapes) |
| OpenBridge 关联文档 | [网关 API 与客户端兼容需求](../../functional-requirements/gateway-api-compatibility.md)、[协议测试语料与工具现状](../../implementation-status/protocol-test-corpus.md) |
| 不在范围 | 模型回答质量评测、benchmark 排名、完整 OpenAI API 合规认证、真实 Provider 的当前能力声明 |

所有外部仓库都可能继续变化。若后续复制、改写或运行其中的测试，必须先固定 commit，并在 fixture manifest 中记录来源与许可证；本文的日期快照不能替代这一操作。

## 1. 结论

存在多组相关测试资产，但截至本次复核，**没有发现一组公开测试集同时覆盖以下全部边界**：

- Chat Completions → Responses 与 Responses → Chat Completions 双向转换；
- 非流式与 SSE 流式；
- SSE bytes 任意分片、UTF-8/CRLF、多行 `data:` 与 EOF；
- 单个、连续和并行 function calls；
- `call_id`、item id、choice/output index 的身份与顺序；
- arguments 分片、晚到或省略的重复身份字段；
- tool result 回填、continuation 与 route/state affinity；
- terminal、HTTP 200 内错误、partial failure、cancel；
- 不支持字段的 preflight reject，以及首输出后的 no retry/fallback。

因此不能把某个外部套件直接作为 OpenBridge Bridge 的唯一 TDD oracle。较稳妥的分工是：

1. OpenBridge 自有、确定性的 contract corpus 负责转换状态机与失败策略；
2. Open Responses Compliance 负责 Responses 外部形状的黑盒互证；
3. Codex 测试提取 Responses SSE/tool lifecycle 场景；
4. `responses-proxy` 提供 Rust 中 Responses → Chat → Responses 的实现与 fixture 对照；
5. LiteLLM 的测试与 issue 提供跨协议丢字段、错关联等负面回归样本；
6. OpenAI gpt-oss compatibility-test 与 SDK compatibility tester 只作为 SDK/真实模型 smoke，不作为确定性语义 oracle。

## 2. 评估维度

本文用以下维度判断外部测试资产能否进入 OpenBridge：

| 维度 | 对 OpenBridge 的意义 |
|---|---|
| 协议方向 | 必须区分 Native Responses、Native Chat、Responses → Chat 和 Chat → Responses，不能用“支持两种 API”替代“双向转换”。 |
| 确定性 | TDD 的核心 fixture 应由固定请求和固定 wire transcript 驱动，不应依赖模型是否按 prompt 选择正确工具。 |
| 流式粒度 | 必须同时检查 SSE framing、事件序列、增量内容和最终聚合结果。 |
| 工具身份 | `call_id`、item id、choice/output index 和函数名用途不同；必须验证关联而不只是检查“出现了工具调用”。 |
| 终态与失败 | completed、failed、incomplete、`[DONE]`、EOF、cancel 和 transport error 不能混成“stream ended”。 |
| Bridge 策略 | 外部项目的静默丢弃、缓存补全或合成身份不自动适用于 OpenBridge。 |
| 可集成性 | 是否可离线运行、能否固定输入、是否依赖真实模型、SDK 或项目内部类型。 |
| 证据强度 | 区分 schema smoke、客户端可消费、状态机 contract 与真实 Provider E2E。 |

OpenAI 官方迁移资料明确区分两种流式模型：Chat Completions 使用带 `delta` 的增量 chunk，Responses 使用按 `type` 分派的类型化 SSE 事件，function calling 还可产生 `response.function_call_arguments.delta` 与 `.done`。这意味着只比较最终文本或最终 JSON，无法证明流式 Bridge 正确。

## 3. 已发现的测试资产

### 3.1 OpenAI gpt-oss compatibility-test

来源：

- [compatibility-test 目录与 README](https://github.com/openai/gpt-oss/tree/main/compatibility-test)
- [cases.jsonl](https://github.com/openai/gpt-oss/blob/main/compatibility-test/cases.jsonl)
- [runCase.ts](https://github.com/openai/gpt-oss/blob/main/compatibility-test/runCase.ts)
- [官方使用说明](https://developers.openai.com/cookbook/articles/gpt-oss/verifying-implementations#quick-verification-of-tool-calling-and-api-shapes)

观察事实：

- 使用 TypeScript Agents SDK 及其底层 OpenAI client；
- Provider 配置可选择 `chat` 或 `responses`，并可用 `--streaming` 运行；
- `cases.jsonl` 以 prompt、可用工具、预期工具和可选参数构成模型驱动 case；
- 主要判断 API shape、是否调用预期工具及参数，完整运行会重复 case 以观察一致性；
- README 明确写明 Chat API events 当前未被测试；
- 官方文档将其定位为 basic function calling/API shape 的 smoke test，并明确说明不保证完整 OpenAI API 兼容。

适用方式：

- 作为真实模型或兼容 Provider 的后置 smoke；
- 发现 SDK 形状或基本 function calling 的明显回归；
- 作为 OpenBridge `external_conformance` 的可选、非阻塞任务。

不适用：

- 不能作为确定性的 Bridge golden corpus；
- 不能证明任意 SSE 分片、并行 call 交错、terminal 或 cancel；
- `chat` 与 `responses` 分别运行不等于验证二者转换的语义等价。

### 3.2 Open Responses Compliance

来源：

- [Open Responses 项目说明](https://www.openresponses.org/)
- [Acceptance Tests 页面](https://www.openresponses.org/compliance)
- [openresponses/openresponses](https://github.com/openresponses/openresponses)
- [compliance-tests.ts](https://github.com/openresponses/openresponses/blob/main/src/lib/compliance-tests.ts)
- [compliance-test CLI](https://github.com/openresponses/openresponses/blob/main/bin/compliance-test.ts)

观察事实：

- Open Responses 是以 OpenAI Responses API 为基础的独立开放规范和生态，不是“与官方 Responses 完全相同”的声明；
- 当前 `testTemplates` 可见 17 个场景，覆盖 HTTP/SSE 与 WebSocket；
- HTTP 侧包括基本文本、assistant phase、schema fixture、SSE、system prompt、单 function tool、image、multi-turn、compact 与缺少 model；
- WebSocket 侧包括基础响应、同连接连续响应、`store:false` continuation、重连后的 recovery、缺失 previous response、失败 continuation 的缓存处理与 compact 新链；
- 流式校验会解析事件 schema，并要求得到 terminal response；连接在 terminal 前关闭会记录错误；
- function tool 场景只要求输出中存在 `function_call`，没有覆盖复杂并行调用、arguments 任意分片、tool result 往返或 Chat 转换。

适用方式：

- 作为 `/v1/responses` 对外边界的黑盒 schema/terminal acceptance；
- 从 WebSocket continuation 测试提取 state 生命周期问题，但是否支持 WebSocket 仍由 OpenBridge 产品范围决定；
- 对未来 OpenBridge Responses 实现进行独立规范互证。

不适用：

- 其规范与 OpenAI 官方 Responses API 必须分开记录；
- 不覆盖 Chat Completions，也不能验证 Chat ↔ Responses；
- 不足以覆盖复杂 tool identity 与 SSE fault injection。

### 3.3 OpenAI Codex 测试

来源：

- [Responses mock/event helpers](https://github.com/openai/codex/blob/main/codex-rs/core/tests/common/responses.rs)
- [tool_parallelism.rs](https://github.com/openai/codex/blob/main/codex-rs/core/tests/suite/tool_parallelism.rs)
- 本仓库既有调研：[Codex Responses SSE 与工具生命周期](../codex/codex-sse-and-tool-lifecycle-analysis.md)

观察事实：

- `tests/common/responses.rs` 提供 Responses 事件与 SSE 序列构造、request capture、function/custom tool item 和 terminal/error fixture；
- tool parallelism 测试关注多个工具能否并发启动、function calls 与 outputs 的顺序、output 是否按 `call_id` 匹配；
- 部分测试通过控制 `response.completed` 的释放时机，验证工具可在完整响应 terminal 到达前开始；
- 测试目标是 Codex 客户端 runtime，主要是 Responses 侧，不是通用 Bridge 合规套件。

适用方式：

- 提取 `call_id`、output item、terminal、并行执行和时序的最小 transcript；
- 作为 Codex client compatibility fixture 的主要来源；
- 与 OpenBridge 的 stream assembler/renderer 状态机互证。

不适用：

- 不复制 Codex 的工具执行、审批、sandbox 或 session runtime；
- 不把 Codex 当前能消费的事件子集当成完整 Responses 规范；
- 不能替代 Chat 侧和双向转换 fixture。

### 3.4 LiteLLM Responses 与翻译测试

来源：

- [tests/llm_responses_api_testing](https://github.com/BerriAI/litellm/tree/main/tests/llm_responses_api_testing)
- [base_responses_api.py](https://github.com/BerriAI/litellm/blob/main/tests/llm_responses_api_testing/base_responses_api.py)
- [tool result 修复测试](https://github.com/BerriAI/litellm/blob/main/tests/llm_responses_api_testing/test_anthropic_tool_result_fix.py)
- [empty call id 测试](https://github.com/BerriAI/litellm/blob/main/tests/llm_responses_api_testing/test_anthropic_tool_result_empty_call_id.py)
- [Responses arguments delta 丢失 issue #20711](https://github.com/BerriAI/litellm/issues/20711)
- [跨协议 tool input 丢失 issue #25321](https://github.com/BerriAI/litellm/issues/25321)

观察事实：

- 测试目录按 OpenAI、Azure、Anthropic、Google 等 Provider 覆盖 Responses 请求、streaming iterator、hooks 和 tool result；
- 测试深度依赖 LiteLLM 的内部类型、Provider adapter、cache 与兼容策略；
- issue #20711 给出可复现的典型错误：首个 Chat tool-call chunk 带 id，后续 chunk 只带 index；转换器若没有 `index → call_id` 状态，会丢弃后续 arguments delta；
- issue #25321 展示另一种相同类别的错误：切换 content block 时丢弃携带 tool input delta 的触发 chunk，最终下游只看到空参数；
- 某些 LiteLLM 修复策略会跳过空 `call_id` 或从 cache 重建缺失调用，这不是 OpenBridge 的默认策略。

适用方式：

- 把 issue 中的最小复现转为独立、确定性的负面 fixture；
- 用多 Provider 测试发现字段兼容与 adapter 差异；
- 借鉴 VCR/record-replay 的组织方式，但对 credential、脱敏与数据保留单独设限。

不适用：

- 不直接导入 LiteLLM 的内部对象作为 OpenBridge IR；
- 不采用“静默跳过空 id”或全局 cache 补全；
- 不以 Provider 测试通过推导 OpenBridge 双向 Bridge 已正确。

### 3.5 CallOrRet/responses-proxy

来源：

- [responses-proxy](https://github.com/CallOrRet/responses-proxy)
- [verification_tests.rs](https://github.com/CallOrRet/responses-proxy/blob/main/tests/verification_tests.rs)

观察事实：

- Rust/Axum proxy 接收 Responses 请求，转换为 Chat 请求，再把 Chat 响应转换回 Responses；
- 对外支持 HTTP SSE 与 WebSocket，并包含 function call/output、reasoning 和 Codex 相关行为；
- verification tests 使用较真实的请求 payload，直接调用转换函数并维护 streaming conversion state；
- 其主方向与 OpenBridge 第一阶段的 Responses → Chat 上游调用最接近；
- README 说明不在 allowlist 中的 tool type 会被静默丢弃。

适用方式：

- 参考 Rust 类型边界、转换函数拆分和基础 streaming state fixture；
- 提取 Responses request → Chat request、Chat response/SSE → Responses 的正向样本；
- 对照 OpenBridge 自有实现发现遗漏，但不逐行移植。

不适用：

- 不是 Chat 与 Responses 两个公共 endpoint 的双向通用套件；
- 没有覆盖 OpenBridge 所需的完整 fragmentation/fault/cancel 矩阵；
- “静默丢弃 unsupported tool”与 OpenBridge 的 preflight reject 原则冲突，应写成负面案例。

### 3.6 beranekio/openai-compatibility-tester

来源：

- [openai-compatibility-tester](https://github.com/beranekio/openai-compatibility-tester)

观察事实：

- 通过官方 OpenAI Go SDK 对任意 HTTP endpoint 执行黑盒兼容测试；
- 默认覆盖 models、Chat、Chat stream、Responses、Responses stream，扩展套件增加 tools 和 errors 等场景；
- payload 无法被 SDK 解析或基础校验失败时以非零状态退出，适合 CI smoke；
- 提供确定性的 canned mock server，但项目仍较新，测试语义和稳定性需要固定 commit 后再评估。

适用方式：

- 作为 OpenBridge Python/Node SDK 测试之外的 Go SDK 黑盒互证；
- 对发布候选执行 endpoint/SDK shape smoke；
- 可选地使用其 mock server 检查 runner 集成。

不适用：

- 不验证 source protocol 到 target protocol 的内部转换；
- 不替代精确事件序列、身份、terminal 与错误策略断言；
- 在没有固定版本前不作为 required CI gate。

### 3.7 官方 SDK 测试与 stream accumulator

来源：

- [openai-node streaming helpers](https://github.com/openai/openai-node/blob/main/helpers.md)
- [openai-python Chat streaming implementation](https://github.com/openai/openai-python/blob/main/src/openai/lib/streaming/chat/_completions.py)

这些资料可用于理解 SDK 如何聚合 Chat chunks、暴露 tool-call argument delta，或作为下游消费者验证。它们测试的是 SDK 自己的解析/累积逻辑，不是代理的跨协议转换契约。因此 OpenBridge 可以继续用官方 SDK 做“客户端能否消费”的证据，但 golden wire 仍应由本仓库固定。

## 4. 覆盖比较

符号含义：`强` 表示该资产直接覆盖；`部分` 表示可提取场景或做互证；`无` 表示不能从现有测试推导。

| 测试资产 | Chat | Responses | 双向 Bridge | SSE 语义 | 复杂 tools | fault/cancel | 确定性 | 建议角色 |
|---|---:|---:|---:|---:|---:|---:|---:|---|
| gpt-oss compatibility-test | 部分 | 部分 | 无 | 部分 | 部分 | 无 | 弱 | 真实模型/API shape smoke |
| Open Responses Compliance | 无 | 强 | 无 | 部分 | 弱 | 部分 | 中 | Responses 黑盒 acceptance |
| Codex tests | 无 | 强 | 无 | 强 | 强 | 部分 | 强 | 客户端 lifecycle fixture 来源 |
| LiteLLM tests/issues | 部分 | 强 | 部分 | 强 | 强 | 部分 | 中 | Provider 对照与负面回归来源 |
| responses-proxy tests | 部分 | 强 | 单方向强 | 部分 | 部分 | 弱 | 强 | Rust 转换 fixture 对照 |
| openai-compatibility-tester | 强 | 强 | 无 | 部分 | 部分 | 部分 | 中 | SDK endpoint smoke |
| OpenBridge 自有 corpus（目标） | 强 | 强 | 强 | 强 | 强 | 强 | 强 | TDD 主 oracle |

“确定性弱”不是项目质量评价，只表示结果受模型选择、采样或远端 Provider 影响，不适合逐 commit 的转换器红绿测试。

## 5. OpenBridge 已有资产与缺口

当前仓库已经有：

- `tests/sdk_compatibility.rs`：Python/Node SDK 的 Chat/Responses stream/non-stream、单/并行 function tools、arguments 分片和 tool result identity；
- `tests/sse_contract.rs`：fragmented UTF-8、CRLF 与多行 `data:`；
- `tests/forwarding_contract.rs`：EOF、partial stream failure、pending 与 cancellation；
- 已移除的 Rust `tools/upstream-fixture-server` 曾提供确定性基础 JSON/SSE、HTTP 429 和真实上游 proxy；
- [网关 API 与客户端兼容需求](../../functional-requirements/gateway-api-compatibility.md)中的协议、identity、terminal 与失败边界。

截至 corpus/testkit `0.4.0`，当前 corpus cases 与 Python Mock Server 覆盖 Chat/Responses 原生 stream/non-stream、429/`Retry-After`、健康检查、非法 JSON、未知 endpoint 与同进程多请求。当前 testkit 不提供真实上游 proxy、credential 注入、默认模型补全或安全响应 header 白名单。

这些资产目前主要证明 Native Path 与 forwarding contract。它们尚未构成 Chat ↔ Responses Bridge 的可执行语义 corpus，特别缺少：

- 同一逻辑 case 的 source request、upstream request、upstream response/SSE 和 downstream expected 四段 fixture；
- 任意 bytes 分片与逻辑 arguments 分片的组合；
- 多 choice/output item 与并行 call 的 index/id 映射；
- unsupported item/field 的 reject 或 loss notice；
- terminal 只出现一次、EOF-before-terminal、输出后 failure 不 fallback；
- continuation 的 issuer/deployment/route/TTL 绑定。

## 6. 建议的 TDD 测试集结构

建议让 OpenBridge 自有 fixture 只保存协议事实与项目决策，外部来源只作为 provenance：

```text
tests/fixtures/bridge/
  responses_to_chat/
    non_stream/
    stream/
  chat_to_responses/
    non_stream/
    stream/
  state_and_continuation/
  faults/
  external_conformance/
```

每个 case 至少包含：

```text
case.yaml
source-request.json
expected-upstream-request.json
upstream-response.json              # 非流式
upstream-stream.sse                 # 流式
expected-downstream-response.json   # 非流式
expected-downstream-stream.sse      # 流式
```

`case.yaml` 建议记录：

```yaml
id: responses_parallel_tools_fragmented_arguments
source_protocol: responses
target_protocol: chat_completions
stream: true
tags: [parallel_tools, fragmented_arguments, identity, terminal]
origin_project: openai-codex
origin_url: https://github.com/openai/codex/...
origin_ref: <pinned commit>
retrieved_at: 2026-07-26
license: <verified SPDX expression>
adaptation_notes: <只保留了哪些 wire 事实>
openbridge_expected_behavior: exact
```

其中 `openbridge_expected_behavior` 至少区分：

- `exact`：声明共同子集内应稳定往返；
- `approximate`：有显式 loss notice，且测试具体损失；
- `reject`：在调用上游前拒绝；
- `native_only`：不得进入 Bridge；
- `deferred`：保留为研究材料，不进入 required suite。

## 7. 首批最小 case 矩阵

### 7.1 基础映射

- 两个方向的 text-only non-stream；
- 两个方向的 text-only stream；
- system/developer/user/assistant 的允许子集；
- usage 与 finish/terminal 的最小映射；
- model alias 与 upstream model 不泄漏。

### 7.2 工具调用

- 单 function call 与 tool result；
- 两个并行 calls，结果反向到达；
- name/id 只在首 fragment 出现，后续仅有 index；
- arguments 在所有 byte 边界分片，包括 UTF-8 中间位置；
- 空 arguments、合法 `{}` 与不完整 JSON 分开处理；
- 同名函数的两个 call 仍按 identity 区分；
- item done 先于 response completed；
- tool result 引用未知、重复或冲突 `call_id` 时 preflight reject。

### 7.3 SSE 与终态

- CRLF、LF、多行 `data:`、comment/keepalive；
- 一个 JSON event 被拆为任意 bytes chunks；
- completed、failed、incomplete、error、`[DONE]` 的声明子集；
- terminal 至多一次；
- EOF-before-terminal；
- terminal 后多余 event；
- 首输出前 upstream error 与首输出后 stream error；
- downstream cancel 传播且不补 terminal、不 retry/fallback。

### 7.4 不支持与状态

- hosted tool、reasoning、multimodal、structured output 的当前 eligibility；
- unknown 合法字段的 Native preserve 与 Bridge reject/notice；
- `previous_response_id` 缺失、过期或跨 deployment；
- continuation 不跨 issuer、route snapshot 或 fallback candidate；
- bridge re-entry guard。

## 8. 分层执行建议

| 层 | 测试性质 | 是否 required | 失败含义 |
|---|---|---:|---|
| L0 | converter/assembler/renderer 纯函数与 property tests | 是 | Bridge 语义或不变量回归 |
| L1 | loopback HTTP/SSE contract fixtures | 是 | transport、framing、错误或取消回归 |
| L2 | 官方 Python/Node/Go SDK 黑盒消费 | 是，但可按环境拆分 | 对外 API shape 或 SDK compatibility 回归 |
| L3 | Open Responses Compliance 子集 | 初期可选，稳定后 required | Responses 外部规范互证失败 |
| L4 | gpt-oss/真实 Provider/真实模型 smoke | 否，定时或发布前 | 环境或模型兼容风险，不直接证明 converter 错误 |
| L5 | Codex/Hermes 实际 Agent E2E | 仅在声明相应兼容时 | 目标客户端集成回归 |

TDD 的红绿循环应主要运行 L0/L1；L2/L3 用于边界互证；L4/L5 不应因网络、采样或 credential 波动阻塞每次本地开发。

## 9. 外部测试转入规则

外部 case 只有同时满足以下条件才进入 required corpus：

1. 固定 source repository、commit、文件/issue 与检查日期；
2. 记录许可证，确认可以复制或改写；否则只保存链接和自主重写的最小 transcript；
3. 把“观察事实”和“OpenBridge 决策”拆开；
4. 删除真实 credential、用户数据、request id 和不必要的模型输出；
5. 将模型生成内容改成固定 upstream transcript；
6. 明确 case 属于 exact、approximate、reject、native-only 或 deferred；
7. 证明期望行为不依赖外部项目的静默丢弃、全局 cache、合成 id 或 Provider heuristic；
8. 让同一逻辑 case 可对 non-stream/stream 与不同 bytes 分片重放。

外部项目策略与 OpenBridge 冲突时，不修改期望去迎合外部实现。例如：

- `responses-proxy` 静默丢弃 unsupported tools：OpenBridge 应测试 preflight reject；
- LiteLLM 跳过空 `call_id` 或 cache reconstruction：OpenBridge 应测试身份冲突或 state eligibility；
- SDK 最终能拼出完整 arguments：不能掩盖中间 delta 已丢失或顺序错误；
- 模型最终调用正确工具：不能证明转换器保留了每个 wire event。

## 10. P0 补充调研与落地

2026-07-26 在构建 corpus `0.2.0` 前再次复核一手资料：

- [OpenAI Responses streaming events](https://developers.openai.com/api/reference/resources/responses/streaming-events)分别定义 `response.failed`、`response.incomplete` 与 `error`，不能把它们合并为 EOF 或 completed；
- [OpenAI function calling streaming](https://developers.openai.com/api/docs/guides/function-calling#streaming)说明 Chat `tool_calls[].index` 标识增量所属 call，而 `id`、`function.name`、`type` 等字段可以只出现在首个 delta；
- [WHATWG Server-sent events](https://html.spec.whatwg.org/dev/server-sent-events.html)规定 UTF-8、CRLF/LF/CR 换行、多行 `data:` 拼接、comment keepalive，以及流末没有空行时不派发最后 event；
- [LiteLLM issue #20711](https://github.com/BerriAI/litellm/issues/20711)继续作为 index-only 后续 arguments fragment 的回归来源；
- [Open Responses compliance tests](https://github.com/openresponses/openresponses/blob/main/src/lib/compliance-tests.ts)继续作为 terminal-before-close 的外部互证，而不是 OpenAI Responses 的等价规范。

据此把 P0 拆成 18 个新增 canonical cases，而不是用一个组合 fixture 掩盖失败原因：

| P0 分组 | 新增覆盖 |
|---|---|
| terminal | `response.failed`、`response.incomplete`、`error`、Chat DONE 前 EOF、duplicate terminal、terminal 后 event |
| transport | 首输出前 HTTP error、首输出后 transport error、downstream cancel、输出后 no fallback |
| identity | 双向未知 tool result、重复冲突 `call_id`、同名并行 calls、反序 tool results |
| arguments | 空字符串 reject、合法 `{}`、不完整 JSON、转义引号与 UTF-8 跨 fragment |
| SSE framing | comment/keepalive、多行 `data:`、CRLF 生成、all-in-one chunk、每 chunk 多 event、event/type 冲突 |

其中官方 wire 事实标记为 `accepted`；涉及 OpenBridge commit point、preflight reject、terminal 后截断和 event/type 冲突的策略保持 `reviewed`，不能仅因进入 P0 corpus 就声称 runtime 已实现。

## 11. 采用决策

建议先建立 OpenBridge 自有测试集，再按 Slice B1/B2/B3 进行 TDD。外部资产的采用顺序为：

1. 从现有 OpenBridge Native/forwarding fixture 提炼共同测试 harness；
2. 从 Codex 和 LiteLLM issue 导入 tool identity、arguments delta、parallel 与 terminal 负面场景；
3. 用 `responses-proxy` verification tests 对照第一批 Responses → Chat fixture；
4. 接入 Open Responses Compliance 的 HTTP/SSE 子集；
5. 保留当前 Python/Node SDK 测试，并在稳定后可选增加 Go SDK compatibility tester；
6. 最后运行 gpt-oss 与真实 Provider smoke，不把概率性结果写成 Bridge correctness。

若未来重新进入协议转换实施，仍应先固定 corpus、identity、ordering、terminal 与 error invariants，再建立转换器的当前开发焦点。

## 12. 复核触发条件

出现以下任一变化时，应重新检查本文：

- OpenAI 修改 Chat/Responses streaming event 或 tool-call schema；
- Open Responses Compliance 新增 Chat、并行 tool 或 fault 场景；
- gpt-oss compatibility-test 开始验证 Chat streaming events；
- Codex 调整 Responses event/tool lifecycle；
- LiteLLM 或 `responses-proxy` 修改 identity、continuation、unsupported-tool 策略；
- OpenBridge 开始实现 Slice B1/B2/B3，准备把外部样本固定到本地；
- 任一外部 suite 被提升为 required CI gate。
