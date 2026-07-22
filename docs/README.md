# 文档索引

本目录按文档用途组织；根目录 [`README.md`](../README.md) 是项目入口。

| 目录 | 内容 | 主要文档 |
|---|---|---|
| [`requirements/`](requirements/) | 单用户核心范围与后续增强需求 | [核心需求](requirements/proxy-requirements.md)、[Hosted tool 增强](requirements/hosted-tools-mcp.md) |
| [`architecture/`](architecture/) | 单服务目标架构、Provider adapter、配置和使用量边界 | [架构与路线](architecture/architecture-and-roadmap.md)、[Rust adapter](architecture/rust-provider-adapter-dataflow.md)、[本地配置与使用量](architecture/local-configuration-routing-and-usage.md) |
| [`design/`](design/) | Codex HTTP/SSE 与 Hermes 客户端契约、协议 bridge 与可选 credential 专项设计 | [目标客户端契约](design/target-client-contracts.md)、[Chat/Responses bridge](design/chat-responses-conversion.md)、[Codex OAuth 边界](design/codex-oauth-credential-boundary.md) |
| [`implementation/`](implementation/) | 当前代码、API、配置、路由、SSE 语义与验证证据 | [当前实现说明](implementation/current-implementation.md) |
| [`plans/`](plans/) | 调研、实验、决策门与候选实施顺序 | [开发与调研收敛计划](plans/development-plan.md) |
| [`experiments/`](experiments/) | 原型实验模板、wire evidence 与证明边界 | [实验记录说明](experiments/README.md) |
| [`research/`](research/) | 外部项目源码调研与横向比较 | [参考项目比较矩阵](research/project-comparison-matrix.md) |
| [`research/hermes/`](research/hermes/) | Hermes Agent 源码调研 | [Chat/Responses 分析](research/hermes/chat-responses-analysis.md) |
| [`research/litellm/`](research/litellm/) | LiteLLM 源码调研、调用链与性能观察 | [协议分析](research/litellm/chat-responses-analysis.md)、[调用链](research/litellm/proxy-call-chain-analysis.md)、[性能分析](research/litellm/proxy-performance-bottlenecks.md) |
| [`research/cc-switch/`](research/cc-switch/) | cc-switch 的 Codex Responses ↔ Chat bridge、tool context 与 SSE 状态机 | [协议与工具转换分析](research/cc-switch/chat-responses-tool-conversion-analysis.md) |
| [`research/codex/`](research/codex/) | Codex OAuth 与 Responses tool lifecycle 源码调研 | [OAuth 与工具调用](research/codex/oauth-and-tool-call-analysis.md) |
| [`research/chatgpt-oauth/`](research/chatgpt-oauth/) | Hermes 与 LiteLLM 的 subscription OAuth 实现调研 | [OAuth 实现对比](research/chatgpt-oauth/hermes-and-litellm-oauth-analysis.md) |
| [`specifications/openai/`](specifications/openai/) | OpenAI 官方 API 的协议与规范快照 | [规范目录](specifications/openai/api-specification-catalog.md)、[Chat Completions](specifications/openai/chat-completions-protocol.md)、[Responses](specifications/openai/responses-protocol.md) |

## 文档声明等级

重要设计声明应使用以下状态之一，避免把探索性方案误读为最终决定：

| 状态 | 含义 |
|---|---|
| `Invariant` | 与具体实现无关、预期长期保持的原则。 |
| `Working hypothesis` | 当前首选方向，但仍需外部反例和实验验证。 |
| `Candidate` | 多个可比较方案之一。 |
| `Accepted` | 已完成比较、实验与决策记录。 |
| `Deferred` | 有价值但不影响当前核心收敛。 |
| `Blocked` | 依赖外部契约或当前不可获得的证据。 |

## 文档使用原则

- `requirements/` 定义产品边界，不将企业级网关能力默认纳入单用户核心。
- `architecture/` 与 `design/` 记录不变量、工作假设和候选方案，不表示已有实现。
- `plans/` 以“调研问题 → 实验 → 决策门 → 实施切片”组织，不以原型代码量判断设计完成。
- `implementation/` 只陈述当前代码已经验证的行为和证据边界。
- `research/` 必须区分源码事实、OpenBridge 推论、负面证据和待验证问题。
- `specifications/` 是带采集日期的学习快照；实现或升级前应复核当前官方资料。
