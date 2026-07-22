# OpenBridge 开发与调研收敛计划

## 状态

**实施中；当前是调研—实验—决策计划，不是已冻结的功能路线图。**

OpenBridge 的最终实现方向尚未完全收敛。当前 Rust 代码用于验证设计假设；阶段完成与否应由证据和决策门判断，而不是由代码量或已有模块名称判断。

## 1. 核心目标

构建一个单用户、单服务的多 Provider Agent API proxy：

1. 为 Codex custom Provider 提供可靠的 Responses HTTP/SSE 原生入口；
2. 为 Hermes Agent 提供可靠的 Chat/Responses 入口；
3. 聚合多个 Provider/deployment，并用稳定 alias 路由；
4. 保留原生协议语义，只在必要时执行受限 Protocol Bridge；
5. 正确处理 SSE、tool identity、continuation、取消和首输出前 fallback；
6. 核心稳定后增加 usage、Hosted Tool Facade、Tool Bridge/MCP 和可选 OAuth。

当前明确不以多租户、principal/ACL、配额、计费、合规审计或独立控制面为目标。

## 2. 设计声明状态

计划和设计文档使用：

| 状态 | 说明 |
|---|---|
| `Invariant` | 预期长期保持，例如 native first、state affinity、secret isolation。 |
| `Working hypothesis` | 当前首选方向，需要实验和反例验证。 |
| `Candidate` | 多个方案之一。 |
| `Accepted` | 已完成比较、实验和决策记录。 |
| `Rejected` | 已有证据表明不适合当前产品边界。 |
| `Deferred` | 核心后再考虑。 |
| `Blocked` | 依赖外部契约或当前不可得证据。 |

“原型已实现”不能自动把对应架构提升为 `Accepted`。

## 3. 当前不变量与工作假设

### Invariants

- Native first；
- capability before call；
- continuation/state 不跨 issuing deployment；
- immutable RoutePlan；
- no silent downgrade；
- no stream stitching；
- 服务所有者配置上游，业务请求不能控制 URL/credential/auth header；
- 非 loopback 至少需要静态高熵 token + TLS/可信反向代理。

### Working hypotheses

- 编译期 Provider Family + 运行时 Deployment 是合适的扩展边界；
- Native Path 应绕过 Bridge IR；
- 四态 capability `Native/Bridged/Unsupported/Unknown` 足够支撑核心路由；
- Responses → Chat 应先于 Chat → Responses 实施；
- Anthropic Messages 足以作为首个异构 Provider archetype；
- continuation ledger 可以延期，首版 bridge 可要求完整历史或拒绝 stateful path；
- Codex 首版可通过 custom Provider 的 `supports_websockets = false` 稳定使用 HTTP/SSE，Responses WebSocket 可延期。

## 4. 研究工作流

每个关键问题使用统一记录：

```text
Research question
Affected decision
Current hypothesis
Competing hypotheses
Primary sources and pinned commits
Observed facts
Negative evidence / failures
Required experiment
Result
What this proves
What this does not prove
Decision status
Reversal trigger
```

外部项目调研优先围绕 OpenBridge 问题组织，而不是只按项目写概览。

## 5. 收敛门

### Gate C0：产品范围与目标客户端

**问题**

- Codex 和 Hermes 实际需要哪些 wire contract？
- 哪些路径必须 native，哪些 path 才需要 bridge？
- 固定哪些客户端版本作为第一轮基线？

**产物**

- [核心需求](../requirements/proxy-requirements.md)；
- [目标客户端契约](../design/target-client-contracts.md)；
- 固定 Codex/Hermes 版本和配置；Codex 基线使用独立 custom Provider id、`wire_api = "responses"` 和 `supports_websockets = false`；
- client fixture 目录和 case 清单。

**退出条件**

- 核心范围不再把企业网关能力列为前置或验收条件；
- Codex Responses-first、首版 HTTP/SSE-first 与 Hermes multi-transport 结论有源码/文档证据；
- 明确 P0 native path 和 P1 bridge path；
- 每个客户端至少录制一组原始请求和成功/错误 SSE；Codex 还需记录诊断，确认 custom Provider 未启用 WebSocket。

### Gate C1：双 Native Path

**路径**

```text
Codex Responses HTTP/SSE → Responses Provider
Hermes Chat → OpenAI-compatible Chat Provider
```

**实验**

- 文本 stream/non-stream；
- 单/并行 function tool calls；
- tool result replay；
- usage；
- cancel；
- provider error；
- EOF/terminal；
- unknown event/field。

**退出条件**

- 两个目标 Agent 各完成真实多轮 tool loop；
- unknown native fields 不因内部 schema 被删除；
- client disconnect 关闭上游；
- fixture 明确记录证明和未证明事项；
- Codex 诊断确认 active custom Provider 的 `supports_websockets` 为 false，实际请求未进入 WebSocket transport。

### Gate C2：Provider 聚合核心

**问题**

- Provider Family 与 Deployment 的边界是否足够？
- 受信 owner 配置应允许哪些 endpoint/header 差异？
- capability 是否需要条件表达式？

**产物**

- `ProviderFamily`、`Deployment`、`PublicModelAlias`、`RoutePlan`；
- 至少两个 Provider Family；
- ordered candidate；
- capability filtering；
- 首输出前 fallback；
- state affinity。

**退出条件**

- Generic OpenAI-compatible endpoint 不需要新编译 variant；
- 业务请求不能改变 URL/header/credential；
- 相同 config + request 产生确定 route；
- `previous_response_id`/tool continuation 不跨 candidate；
- Provider conformance suite 可复用。

### Gate C3：Responses → Chat Bridge

**原因**

Codex 的下游契约优先是 Responses，而大量兼容 Provider 只提供 Chat；这是第一条最直接产生使用价值的 bridge。

**首个范围**

- text；
- function tool schema/call/result；
- usage；
- stream terminal；
- 无 continuation ledger。

**退出条件**

- Codex 通过 Chat-only Provider 完成最小 tool loop；
- 并行 call identity、arguments delta 和 output order fixture 通过；
- hosted tool、opaque continuation/resource 等在调用前拒绝；
- bridge 不向 Codex 注入未知 SSE event；
- 无递归 bridge。

### Gate C4：Chat → Responses Bridge

**目标**

让 Hermes Chat transport 使用 Responses-only Provider，同时检验反向 identity 和 stream renderer。

**退出条件**

- Hermes Chat tool loop 可在 Responses Provider 上完成；
- assistant `tool_calls[]` 与 `function_call_output` 映射稳定；
- usage-only final、item done/response done、error/cancel 正确；
- stateful continuation 默认拒绝或有明确 ledger decision。

### Gate C5：异构 Provider 验证

**候选**

Anthropic Messages，或另一个不是 OpenAI wire dialect 的 Provider。

**目的**

- 反证 Provider Family/trait 粒度；
- 反证 Bridge IR 是否只是 OpenAI 两协议的共同外形；
- 验证 content block、tool use/result、stop reason 和 stream event。

**退出条件**

- adapter 不要求在核心 router 增加 Provider-specific branch；
- Bridge IR/typed extension 边界清晰；
- 无法共同表达的语义被明确拒绝；
- 若需要重构，在继续增加 Provider 前完成。

### Gate C6：核心接受

核心在以下条件全部满足后视为基本收敛：

- C1–C5 通过；
- Codex/Hermes 兼容 corpus 固定；Codex corpus 明确只覆盖 custom Provider HTTP/SSE profile；
- 至少三个 Provider archetype 完成设计验证；
- native/bridge/capability/state/fallback 边界有文档和测试；
- 非 loopback security baseline 明确；
- 原型代码与目标设计差异已列出；
- 剩余问题不再影响 Provider onboarding 和双向最小 tool loop。

## 6. 外部项目调研计划

### 核心参考

| 项目 | 研究职责 |
|---|---|
| Codex | 目标客户端 Responses wire、tool/continuation、custom Provider 配置和 HTTP/SSE/WebSocket transport 决策。 |
| Hermes Agent | 目标客户端 Chat/Responses/Anthropic transport 和完整 Agent loop。 |
| LiteLLM | Provider 参数/错误/转换差异资料库，不复制企业 proxy 全部结构。 |
| cc-switch | 单用户本地接入、Codex bridge、tool context/history recovery 与使用量体验。 |

### 新增优先参考

| 项目 | 研究职责 |
|---|---|
| Bifrost | Provider core、request/response pipeline、model catalog、native/compatibility 边界和真实转换 bug。 |
| CLIProxyAPI | Codex/Chat/Responses translation、continuation/state affinity、tool-call 和 SSE 失败案例。 |

详细问题、状态和链接见[参考项目比较矩阵](../research/project-comparison-matrix.md)。

### 项目选择原则

每个核心决策至少需要：

- 一个支持当前假设的项目/证据；
- 一个不同架构的替代方案；
- 一个 issue、修复历史或负面案例；
- 一个 OpenBridge 本地实验。

新增项目只有在能提供新架构流派、新失败模式或直接目标客户端证据时才进入深度调研。

## 7. 调研停止条件

一个研究问题满足以下条件后进入阶段性决策：

1. 至少比较三个相关实现或规范，其中至少两个架构流派不同；
2. 有支持证据和反例/替代方案；
3. 至少一个本地 fixture/原型实验；
4. 已知剩余不确定性；
5. 写明 reversal trigger；
6. 新增同类项目不再显著改变候选方案或失败分类。

阶段性决策允许未来推翻，但不能因“还能再找项目”无限延期。

## 8. 原型实验记录

每个非平凡原型新增 `docs/experiments/EXP-xxxx-*.md` 或等价记录：

```text
Experiment ID
Hypothesis
Environment/client/provider versions
Fixture
Observed result
What this proves
What this does not prove
Affected decision
Artifacts/tests
```

示例：OpenAI SDK loopback fixture 可证明特定输出形状被 SDK 消费，但不证明真实 Provider、Codex/Hermes tool loop、异构 bridge 或未来 SDK 版本等价。

## 9. 候选实施顺序

在当前证据下，候选顺序是：

```text
C0 target client corpus
→ C1 dual native paths
→ C2 provider aggregation
→ C3 Responses→Chat bridge
→ C4 Chat→Responses bridge
→ C5 heterogeneous provider
→ C6 core acceptance
```

它是当前工作假设，不是不可修改的线性 Phase。某个独立调研（例如 OAuth）被阻塞时，不应阻塞其他核心工作流。

## 10. 核心后的增强

### E1：Usage analysis

- 请求结束生成 `UsageRecord`；
- stdout/JSONL，随后可选 SQLite；
- tokens、TTFT、latency、route、outcome 和估算成本；
- 默认不记录 prompt/response/tool 正文。

### E2：被动健康与更丰富 fallback

- 临时错误 cooldown；
- route reason；
- 不覆盖 state affinity；
- 不引入复杂动态权重作为首个版本。

### E3：Provider-hosted tool facade

- 先验证 native hosted-tool Provider corpus；
- 不依赖 Protocol Bridge；
- 选择同进程/sidecar/独立 MCP server；
- 目标客户端 citation 消费 E2E。

### E4：Tool Bridge/MCP

- 本地/MCP 工具发现与执行；
- 与 Protocol Bridge、Hosted Tool Facade 分开状态机；
- 不让模型请求提供任意出站 URL/credential。

### E5：Optional OAuth

- mock issuer 验证通用 state machine；
- 真实 Codex OAuth 受官方契约和条款 preflight 阻塞；
- API-key core 不等待该结果。

### E6：Simple UI

- 配置状态、route、usage 和错误；
- 不演化成多租户控制面。

## 11. 当前原型证据

当前代码已经提供：

- loopback listener 与静态 Bearer；
- OpenAI API-key upstream；
- Chat/Responses native forwarding；
- immutable config/route snapshot；
- ordered deployment candidates；
- capability gate；
- `/v1/models`；
- shared connection pool；
- SSE framing；
- cancel propagation；
- 首个业务输出前的 fallback boundary；
- OpenAI Python/Node SDK loopback fixture。

当前代码尚未证明：

- 第二 Provider Family；
- Codex/Hermes 真实 Agent tool loop；
- trusted custom endpoint 配置的最终边界；
- Bridge IR 和双向 stream assembler；
- continuation ledger；
- Anthropic Messages 抽象；
- hosted tool、usage、OAuth 或 UI。

## 12. 风险与 reversal trigger

| 风险 | 当前应对 | 推翻/调整条件 |
|---|---|---|
| Provider Family 过于刚性 | 运行时 Deployment 支持同族兼容 endpoint | 第三个 archetype 仍需大量核心 if/else 时重构 trait/registry。 |
| Bridge IR 变成万能协议 | Native Path 绕过 IR，typed extension 有边界 | `provider_data`/optional fields 持续膨胀时拆分协议对 IR。 |
| Codex/Hermes 版本漂移 | 固定版本 corpus + 升级重跑 | 目标客户端采用新 event、字段或 transport 行为时更新 contract。 |
| continuation 过于复杂 | 首版拒绝 stateful bridge | 实际目标工作流必须依赖时再评估 ledger。 |
| 调研无限扩张 | 问题驱动矩阵和停止条件 | 新项目不提供新信息时停止。 |
| Hosted tool 扩大产品边界 | Deferred，独立 facade | 无明确目标客户端价值时不实施。 |
| Codex transport 漂移 | custom Provider 显式 `supports_websockets = false`，记录诊断和实际请求 transport | 固定版本忽略该配置、移除 HTTP/SSE 或核心场景需要 WebSocket 时重开范围。 |
| OAuth 阻塞主线 | API key baseline 独立 | 只有官方 preflight 通过才进入实现。 |

## 13. 质量门

每个合并到核心实现的切片至少执行：

```bash
cargo fmt -- --check
cargo test --locked
cargo clippy --locked -- -D warnings
```

并根据范围增加：

- Provider/client fixture；
- SSE fragmentation/terminal/error；
- cancellation；
- secret scan；
- Native unknown-field preservation；
- Bridge identity/order/property tests；
- non-loopback token boundary；
- memory/backpressure baseline。

## 14. 关联文档

- [核心需求](../requirements/proxy-requirements.md)
- [目标客户端契约](../design/target-client-contracts.md)
- [目标架构与路线](../architecture/architecture-and-roadmap.md)
- [Rust Provider adapter 与数据流](../architecture/rust-provider-adapter-dataflow.md)
- [Protocol Bridge 设计](../design/chat-responses-conversion.md)
- [参考项目比较矩阵](../research/project-comparison-matrix.md)
- [当前实现说明](../implementation/current-implementation.md)
