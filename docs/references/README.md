# 参考文档

本目录保存外部协议与参考项目的快照、观察事实和适用边界。它们为需求或实施计划提供依据，但不自动构成 OpenBridge 的功能承诺。OpenBridge 是 headless 网关，因此 GUI、企业控制面和客户端管理只作为明确排除项，不是调研或复用目标。

## 目录规则

| 位置 | 内容 |
|---|---|
| 当前目录 | [总索引](README.md)与[项目比较矩阵](project-comparison.md)；只放跨项目导航与分工，不放单一项目的深度调研。 |
| `openai/` | 官方 OpenAI 协议与规范材料。 |
| `codex/`、`hermes/`、`litellm/`、`cc-switch/`、`cliproxyapi/` | 对应单一参考项目的源码、issue、测试与适用边界调研。 |
| `cross-project/` | 确实需要同时比较多个项目、且不能合理归属单一项目的材料，例如 OAuth 风险对比。 |

新增或移动参考文档时先由比较矩阵确定项目角色；能归属单一项目的文档不得留在根目录。每份文档仍需记录来源、快照时间/commit、观察事实、推论、适用边界与复核条件。

## OpenAI 协议

- [API 规范目录](openai/api-specification-catalog.md)
- [Chat Completions 协议](openai/chat-completions-protocol.md)
- [Responses 协议](openai/responses-protocol.md)

## 参考项目

先查[项目比较矩阵](project-comparison.md)，确认某项目在当前问题中是主参考、互证还是负面案例；不要因项目功能更广而扩大 OpenBridge 范围。

### 本地 Agent 契约

- [Codex Responses SSE 与工具生命周期](codex/codex-sse-and-tool-lifecycle-analysis.md)：Rust SSE 解析、终态、`call_id` 与客户端 TTFT 语义。
- [Hermes Chat/Responses](hermes/hermes-chat-responses-analysis.md)：`api_mode`、Chat/Responses agent loop 与 tool result 归一。

### Provider 与 Protocol Bridge

- [LiteLLM Chat/Responses](litellm/litellm-chat-responses-analysis.md)
- [LiteLLM Proxy 调用链](litellm/litellm-proxy-call-chain-analysis.md)
- [LiteLLM 调用统计与 Prometheus](litellm/litellm-observability-analysis.md)：TTFT/延迟/失败指标的边界，不复用其多租户标签和计费。
- [cc-switch Chat/Responses 与 Agent Tool](cc-switch/cc-switch-chat-responses-tool-conversion-analysis.md)：Code Agent bridge 状态机。
- [CLIProxyAPI 状态与 Bridge 负面案例](cliproxyapi/cliproxyapi-stateful-bridge-analysis.md)：`previous_response_id`、ID 映射与 stream terminal 的风险材料。
- [Chat/Responses、SSE 与工具调用测试集调研](cross-project/chat-responses-sse-tool-test-suite-survey.md)：公开测试资产的覆盖比较、适用边界与 OpenBridge 自有 TDD corpus 建议。

### OAuth 安全边界（非当前接入目标）

- [Codex OAuth 与工具调用](codex/codex-oauth-and-tool-call-analysis.md)
- [Hermes 与 LiteLLM OAuth](cross-project/hermes-litellm-oauth-analysis.md)

这两篇只说明既有本地客户端的 OAuth 风险与不可外推范围；OAuth 是否可作为 OpenBridge 上游 credential 必须另依官方资料与明确授权判断。

新增参考需记录来源、快照时间或提交、检查范围、观察事实、推论与适用边界；升级实现前仍需复核官方资料和本地验证。
