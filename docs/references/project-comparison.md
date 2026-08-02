# OpenBridge 参考项目比较矩阵

## 状态

**持续更新。** 本矩阵于 2026-07-25 按当前产品目标重整，并于 2026-08-01 更新本地参考目标：OpenBridge 是单配置所有者、单服务、headless 的 Agent 网关，可从私有文件加载下游用户；不提供 GUI、企业控制面或在线客户端管理。各深度调研保留其自己的源码快照日期与 commit，本矩阵不以更新日期覆盖原始逐行证据；当前本地 checkout 与模块级复核见[参考文档索引](README.md#2026-08-01-本地参考目标更新与复核)。

参考项目不是实现模板，更不是功能承诺。每个项目只在预先定义的材料范围内提供证据；没有落入本矩阵的内容，默认不因“项目中已有实现”而进入 OpenBridge 设计。

## 1. 使用规则

### 1.1 证据角色

同一设计问题可以同时参考多个项目，但它们的角色不同：

| 角色 | 含义 | 结论规则 |
|---|---|---|
| 主参考 | 对某个 wire contract、实现形状或失败边界最直接的来源 | 作为该问题的首要研究入口；仍需官方规范或本地 fixture 验证。 |
| 互证参考 | 从不同协议、客户端或 Provider 视角补充主参考 | 用于发现遗漏和限制适用范围，不能以“多数项目这样做”代替契约。 |
| 负面案例 | 收集 translator、state 或路由的已知失败方式 | 产出 failure taxonomy 与回归 fixture，而不是复用其整体方案。 |
| 明确排除 | 项目中存在、但不适合 OpenBridge 的产品/实现 | 不进入实现计划；最多用于说明为什么不采用。 |

所有外部观察都必须落到下列一种可审查产物，才可影响实施计划：

```text
source/commit + files or issue
→ observed fact
→ OpenBridge applicability and non-applicability
→ minimal contract fixture / rejection rule / explicit non-goal
```

### 1.2 OpenBridge 自主决定的边界

以下问题不以任何代理项目为架构模板：

- 代码注册表与受限 secret source 分离、下游 Bearer token 与上游 API key 的安全存放；
- 按稳定下游 user id 和 Public Model 关联的 headless 调用量、usage、TTFT/TTFB 和错误率统计；
- 秘钥生命周期、日志脱敏、配置 reload、监听与 TLS 信任边界；
- 是否声明某个 Agent/Provider/bridge 兼容。

外部项目最多提供字段兼容性、错误分类或失败反例；最终规则由[产品范围](../functional-requirements/product-scope.md)、对应功能需求和当前实施现状定义。OAuth 的合法性与授权边界优先依赖官方资料，不从 Codex、Hermes、CLIProxyAPI 等本地客户端实现推导通用 proxy 资格。

## 2. 项目任务书

| 项目 | 开源协议 | 在 OpenBridge 中的预定角色 | 适合提取的材料 | 预期产物 | 明确不参考 |
|---|---|---|---|---|---|
| **Codex** | [Apache License 2.0](https://github.com/openai/codex/blob/main/LICENSE) | 本地正在使用的 Agent；Responses 下游契约与 Rust 实现主参考 | custom Provider 的 HTTP/SSE profile；Responses SSE bytes 分帧、event 生命周期；function/custom tool 的 `call_id`、item、取消与终态处理 | Codex compatibility fixture；SSE/parser 与 tool-lifecycle 设计约束；实际版本差异记录 | OAuth client registration/auth cache、订阅 backend、CLI 产品、审批/sandbox/hook、Provider catalog |
| **Hermes Agent** | [MIT License](https://github.com/NousResearch/hermes-agent/blob/main/LICENSE) | 本地正在使用的第二 Agent；Chat/Responses 与完整 Agent loop 互证样本 | `api_mode`/endpoint 选择；Chat 与 Responses 的 tool loop、strict endpoint、usage-only final chunk、stream error、Provider 切换后的状态 | Hermes 仅在声明兼容时启用的 E2E corpus；Chat/Responses 模式选择反例 | Agent 内部会话模型、Provider UI/catalog、客户端配置与管理功能；将其内部 adapter 当成网关协议标准 |
| **LiteLLM** | [MIT License（`enterprise/` 目录除外）](https://github.com/BerriAI/litellm/blob/main/LICENSE) | Provider Family/adapter 与协议变换的资料库 | Provider-specific request/response normalization；capability/参数差异；错误分类、有限 retry/fallback；usage 字段兼容；native 与 transform 的边界 | Provider adapter 上界；字段保留/拒绝 fixture；错误分类对照 | Proxy 的 virtual key、用户/团队/预算、DB/Redis 控制面、计费和分布式管理链路 |
| **cc-switch** | [MIT License](https://github.com/farion1231/cc-switch/blob/main/LICENSE) | 面向 Code Agent 的 Protocol Bridge 状态机主参考 | Codex Responses ↔ Chat 请求转换；每请求 tool context；Chat SSE → Responses 的 item/arguments/terminal 重建；tool history 与 continuation 的失败边界 | Chat/Responses bridge fixture；`ToolConversionContext`、tool identity 与 stream assembler 约束 | Tauri/桌面组件、客户端配置接管、usage UX、provider/model 名称猜测、无 issuer/route/TTL 约束的 history fallback |
| **CLIProxyAPI** | [MIT License](https://github.com/router-for-me/CLIProxyAPI/blob/main/LICENSE) | translator/stateful routing 的负面案例库；credential pool 仅作有限重试对照 | `previous_response_id`、tool identity、state affinity、SSE/WebSocket terminal、translator 失败 issue；credential attempt/cooldown 的配置边界 | failure taxonomy；最小 transcript；拒绝或 issuer-bound state 规则；credential pool 的硬预算反例 | subscription/OAuth 账号聚合、非官方 client identity、管理控制面，或直接复制其 4xx/账号轮转策略 |

### 2.1 测试资产补充角色

以下项目只补充[Chat/Responses、SSE 与工具调用测试集调研](cross-project/chat-responses-sse-tool-test-suite-survey.md)，不提升为 OpenBridge 的整体架构参考：

| 测试资产 | 证据角色 | 允许进入 OpenBridge 的产物 | 不得由此推导 |
|---|---|---|---|
| OpenAI gpt-oss compatibility-test | 真实模型、function calling 与 API shape smoke | 可选 external-conformance 任务和脱敏结果 | 完整 OpenAI API、SSE 状态机或双向 Bridge 兼容 |
| Open Responses Compliance | Responses schema、SSE terminal 与 continuation 的外部黑盒互证 | 固定版本的 `/v1/responses` acceptance 子集 | Open Responses 与 OpenAI Responses 完全等价，或 Chat Bridge 正确 |
| OpenAI Codex tests | Responses SSE/tool lifecycle 的确定性场景来源 | `call_id`、并行 tool、item/terminal 的最小 transcript | 复制 Codex runtime，或把客户端可消费子集当完整规范 |
| `CallOrRet/responses-proxy` | Rust Responses → Chat → Responses 实现与 fixture 对照 | 第一批 Responses → Chat 正向 fixture 的互证 | 静默丢弃 unsupported tool，或完整双向/fault 覆盖 |
| `beranekio/openai-compatibility-tester` | 官方 Go SDK endpoint smoke | 可选 Go SDK 黑盒 CI | 内部转换语义、身份或错误策略正确 |

## 3. 问题分工与允许重叠

“主参考”指定先读什么，并不表示其他项目不能提供证据。重叠时必须保留各自的视角，不能把结果混为同一事实。

| 设计问题 | 主参考 | 互证 / 负面案例 | 应产出的 OpenBridge 结论 | 不得由此推导 |
|---|---|---|---|---|
| Codex 下游 Responses HTTP/SSE 契约 | Codex | cc-switch、CLIProxyAPI；官方 Responses 规范作为协议基线 | custom Provider 的最小 HTTP/SSE corpus、事件与 header 的版本化观察 | Codex 所能消费的子集等于完整 Responses API，或可复用其 OAuth |
| 下游 Chat 与完整 Agent tool loop | Hermes | Codex、LiteLLM | 仅在宣称 Hermes/Chat 兼容时所需的 mode、tool、cancel、error E2E | Hermes 内部 Chat history 是网关的通用 IR |
| Rust SSE 解析、事件生命周期与工具关联 | Codex | cc-switch 的跨协议 stream state | 分帧、terminal、`call_id`/item id/stream index 的区分和 fixture | 必须复制 Codex 的客户端架构或把任意事件都视作文本 token |
| Chat ↔ Responses Protocol Bridge | cc-switch | LiteLLM；Codex/Hermes 作为目标客户端验证；CLIProxyAPI 提供失败案例 | 显式 capability gate、每请求转换上下文、双向 item/terminal fixture、loss notice | 无损转换、通用 provider-name heuristic 或全局 history cache |
| tool identity、continuation 与 state affinity | cc-switch | Codex/Hermes tool loop；CLIProxyAPI issues | `call_id` 不可替代；issuer/deployment/route/TTL 绑定的 continuation ledger 或明确拒绝 | 仅按 response id 或全局唯一 `call_id` 跨路由恢复 |
| Provider adapter 粒度、参数与错误恢复 | LiteLLM | CLIProxyAPI 与 cc-switch 的有限重试/故障边界反例 | Family 代码与 Deployment 数据的边界；可证明的 error class/retry/fallback fixture；API-key pool 的最小隔离与硬预算 | 多租户路由、subscription/OAuth 账号聚合、预算或分布式控制面 |
| Native Path 的字段保留与失败策略 | Codex、Hermes | LiteLLM | 对已支持协议最小改写；未知合法字段、SSE 与已输出错误的处理测试 | 只因 bridge 已存在就默认转换，或输出后 retry/fallback |
| 注册表、secret 与单所有者部署 | OpenBridge 产品需求 | Codex/Hermes 配置形状仅作客户端接入样本 | 编译期 Provider 注册、启动时用户表、显式 secret binding 与脱敏要求 | 采用任何项目的账户池、在线 key 管理或 GUI 配置模型 |
| usage、TTFT/TTFB 与错误率 | OpenBridge 产品需求 | LiteLLM 的 usage/error 字段仅作兼容检查 | 低基数聚合、唯一终态、正确分子/分母与无正文记录 | 复制 Proxy 的用户计费、审计或 callback/control-plane 链路 |
| OAuth credential adapter | 官方资料和明确授权 | Codex/Hermes/CLIProxyAPI 仅可说明本地客户端行为和风险 | preflight/拒绝规则或经授权的独立 adapter 契约 | 由观察到的客户端流程推导可复用 client id、refresh 或账号身份 |

## 4. 各项目的研究入口与完成条件

### 4.1 Codex：下游契约与 Rust 细节

**先回答：** 当前本地 Codex 在 custom Provider profile 下实际发送、解析和期待什么？其 Rust SSE/tool lifecycle 中哪些是可证明的 wire 约束？

- 已有材料：[Codex Responses SSE 与工具生命周期](codex/codex-sse-and-tool-lifecycle-analysis.md)、[Codex OAuth 安全边界](codex/codex-oauth-and-tool-call-analysis.md)；Codex [repository](https://github.com/openai/codex)；[配置参考](https://developers.openai.com/codex/config-reference)。
- 固定 commit 后优先阅读：SSE bytes 的分帧/解析和 terminal、event 到 response/tool item 的映射、`call_id` 传递、并行 tool、cancel；OAuth 仅保留为“不得外推”的安全边界。
- 完成条件：指定 Codex 版本的 text/tool/parallel/cancel/error fixture 通过，并记录 `supports_websockets = false` 的实际 HTTP/SSE 观察。

### 4.2 Hermes：可选的完整 Agent 互证

**先回答：** 若声明 Hermes 兼容，Chat/Responses mode 和完整 tool loop 是否在真实 Agent 中保持一致？

- 已有材料：[Hermes Chat/Responses 分析](hermes/hermes-chat-responses-analysis.md)；Hermes [repository](https://github.com/NousResearch/hermes-agent)。
- 优先检查：显式/隐式 `api_mode`、tool result 回填、Provider 切换、HTTP 200 中 stream error、usage-only final chunk。
- 完成条件：记录实际 Hermes 版本，并为所声明的 native path 各完成一个真实 Agent E2E；未声明则不扩展实现范围。

### 4.3 LiteLLM：Provider 差异与变换对照

**先回答：** 哪些差异必须编译进 Provider Family，哪些只是受信 Deployment 数据，哪些变换会丢失语义？

- 已有材料：[Chat/Responses 分析](litellm/litellm-chat-responses-analysis.md)、[Proxy 调用链](litellm/litellm-proxy-call-chain-analysis.md)、[性能观察](litellm/litellm-proxy-performance-bottlenecks.md)、[调用统计与 Prometheus 边界](litellm/litellm-observability-analysis.md)；LiteLLM [repository](https://github.com/BerriAI/litellm)。
- 优先提取：adapter 可证明的参数/response 差异、error class、输出前 retry 与 fallback 条件、usage/finish/terminal 字段。
- 完成条件：每项借鉴都变为 OpenBridge adapter 规则或 fixture，并说明为何不引入其 key/team/budget/DB 管理层。

### 4.4 cc-switch：Code Agent Bridge 状态机

**先回答：** Responses ↔ Chat 何处必须保存每请求上下文，何处必须有状态，何处应拒绝而不能恢复？

- 已有材料：[cc-switch Chat/Responses 与 Agent Tool 转换分析](cc-switch/cc-switch-chat-responses-tool-conversion-analysis.md)；cc-switch [repository](https://github.com/farion1231/cc-switch)。
- 优先提取：`ToolConversionContext`、连续/并行 tool call 组装、fragmented arguments、`output_index`、Responses terminal、tool-result adjacency、同 issuer/deployment 的 continuation recovery。
- 完成条件：每个采用点都有双向 fixture；任何 history/replay 状态都证明具备 issuer、deployment、route snapshot 与 TTL/容量边界。

### 4.5 CLIProxyAPI：把事故变为拒绝规则

**先回答：** 多轮 tool、`previous_response_id`、stateful routing 和 SSE/WebSocket 何时已经不安全，应该如何向下游失败？

- 已有材料：[CLIProxyAPI 状态与 Bridge 负面案例](cliproxyapi/cliproxyapi-stateful-bridge-analysis.md)。一手入口：CLIProxyAPI [repository](https://github.com/router-for-me/CLIProxyAPI)、[Chat → Codex tool output failure](https://github.com/router-for-me/CLIProxyAPI/issues/2132)、[stateful routing affinity](https://github.com/router-for-me/CLIProxyAPI/issues/2594)、[`previous_response_id` continuation failure](https://github.com/router-for-me/CLIProxyAPI/issues/2596)。
- 优先提取：可复现 transcript、触发条件、最终错误/错误 terminal、state 绑定缺失的后果；不要把 issue 中的账号池或 OAuth workaround 当作可行方案。
- 完成条件：每个使用的 issue 产出 failure taxonomy、最小 fixture 与明确的 OpenBridge eligibility/state rule；不能复现的 issue 只保留为待验证线索。

## 5. 调研输出与收敛规则

一次调研只回答矩阵中的一个问题。输出必须同时包含：

```text
研究问题
→ 参考项目及其预定角色
→ 固定 source/commit、文件或 issue
→ 观察到的事实
→ 适用于 OpenBridge 的最小规则
→ 不适用/不能推导的范围
→ 本地 fixture、E2E 或明确不实施的决定
```

- Codex/Hermes 是本地 Agent 兼容样本，不能被代理项目替代，也不形成客户端管理范围。
- cc-switch 是 Code Agent bridge 的状态机参考；LiteLLM 是 Provider/adapter 对照；CLIProxyAPI 是 state/translator 负面案例库。
- 重叠证据出现冲突时，优先采用与问题最近的主参考，并以官方协议、本地 fixture 和目标客户端实际版本裁决。
- 一个核心规则至少要有正向证据、不同实现视角或负面案例，以及与风险相称的本地验证；否则留在实施计划假设中。
- 新参考项目若不能提供新的协议事实、失败类型或可验证的反例，不再扩展样本集合。
