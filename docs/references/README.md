# 参考文档

本目录保存外部协议与参考项目的快照、观察事实和适用边界。它们为需求或实施计划提供依据，但不自动构成 OpenBridge 的功能承诺。OpenBridge 是 headless 网关，因此 GUI、企业控制面和客户端管理只作为明确排除项，不是调研或复用目标。

参考项目的开源协议以其仓库根目录的 `LICENSE` 为准；下表仅记录 2026-08-02 对默认分支的复核结果，不构成依赖引入、代码复用或法律意见。LiteLLM 的 `enterprise/` 目录适用其单独的 `enterprise/LICENSE`，其余仓库内容适用 MIT License。

## 目录规则

| 位置 | 内容 |
|---|---|
| 当前目录 | [总索引](README.md)与[项目比较矩阵](project-comparison.md)；只放跨项目导航与分工，不放单一项目的深度调研。 |
| `openai/` | 官方 OpenAI 协议与规范材料。 |
| `openrouter/` | OpenRouter 官方模型目录、统一参数与 reasoning 元数据快照。 |
| `providers/` | 具体上游 Provider 的官方协议、认证与 endpoint 事实。 |
| `codex/`、`hermes/`、`litellm/`、`cc-switch/`、`cliproxyapi/` | 对应单一参考项目的源码、issue、测试与适用边界调研。 |
| `cross-project/` | 确实需要同时比较多个项目、且不能合理归属单一项目的材料，例如 OAuth 风险对比。 |

新增或移动参考文档时先由比较矩阵确定项目角色；能归属单一项目的文档不得留在根目录。每份文档仍需记录来源、快照时间/commit、观察事实、推论、适用边界与复核条件。

## OpenAI 协议

- [API 规范目录](openai/api-specification-catalog.md)
- [Chat Completions 协议](openai/chat-completions-protocol.md)
- [Responses 协议](openai/responses-protocol.md)
- [Embedding 与多模态 API 转发参考](openai/embedding-and-multimodal-forwarding.md)
- [扩展协议实现细节索引](openai/implementation-details/README.md)：Embeddings、Chat/Responses 多模态、Audio、Images、Files、检索资源、Videos 与 Realtime 各自的 wire、状态、路由和测试边界。

## OpenRouter 模型目录

- [2026-08-02 模型目录快照](openrouter/model-catalog-2026-08-02.md)：当前 canonical 模型的精确匹配、
  context、最大输出、参数和 reasoning effort 证据。
- [2026-08-02 Provider API 快照](openrouter/provider-api-2026-08-02.md)：Chat endpoint、Bearer 认证、
  Models API、无状态 Responses 和 Nemotron `:free` 变体边界。

## Provider 官方协议

- [DeepSeek 与 Xiaomi MiMo 协议入口](providers/deepseek-mimo-protocols-2026-08-02.md)：Chat/Responses、
  endpoint 与认证方式的官方资料边界。

## 参考项目

先查[项目比较矩阵](project-comparison.md)，确认某项目在当前问题中是主参考、互证还是负面案例；不要因项目功能更广而扩大 OpenBridge 范围。

| 参考项目 | 开源协议 | 许可证来源 |
|---|---|---|
| Codex | Apache License 2.0 | [`LICENSE`](https://github.com/openai/codex/blob/main/LICENSE) |
| Hermes Agent | MIT License | [`LICENSE`](https://github.com/NousResearch/hermes-agent/blob/main/LICENSE) |
| LiteLLM | MIT License（`enterprise/` 目录除外） | [`LICENSE`](https://github.com/BerriAI/litellm/blob/main/LICENSE) |
| cc-switch | MIT License | [`LICENSE`](https://github.com/farion1231/cc-switch/blob/main/LICENSE) |
| CLIProxyAPI | MIT License | [`LICENSE`](https://github.com/router-for-me/CLIProxyAPI/blob/main/LICENSE) |

### 跨项目韧性对照

- [Credential Pool、冷却与有限重试对照](cross-project/credential-pool-retry-analysis.md)：固定
  CLIProxyAPI、LiteLLM 与 cc-switch 快照，只提取 API-key pool、最小健康隔离、错误分类与硬 attempt
  预算；不引入其账号/OAuth 聚合或控制面。

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

## 2026-08-01 本地参考目标更新与复核

以下本地 worktree 均已对其跟踪的 `origin` 分支执行 fast-forward 更新；更新后工作区干净，且 `HEAD...@{u}` 的 ahead/behind 均为 `0/0`。这张表记录的是当前 checkout，不会改写各深度调研中用于逐行证据的固定历史提交。

| 参考项目 | 当前分支与提交 | 本次静态复核 |
|---|---|---|
| Codex | `main` @ `ee0247f95a6fe2b094ba2253d82cae2a2b4c2dff` | `process_responses_event`、`ResponseEvent::Completed`、`ToolCallInputDelta`、`x-codex-turn-state`、`supports_websockets`，以及 OAuth/`call_id` 相关模块仍在职责路径中。 |
| Hermes Agent | `main` @ `470cf66b039c73bdd2c21d43094ce41a4db74eae` | `agent/agent_init.py` 仍由显式 `api_mode` 优先选择 `codex_responses`，升级后仍使 transport cache 失效；`ResponsesApiTransport` 仍登记该 mode。 |
| LiteLLM | `litellm_internal_staging` @ `23de7a15d9d40006ee596e617475ba101d60c5e9` | `/responses` 路径、`base_process_llm_request()`、`route_request()`、Responses resource route types、Prometheus 和 ChatGPT `Authenticator` 仍存在，但调用链文件与行号已演进。 |
| cc-switch | `main` @ `ebbf141fc71547a99f669df1be8e345130d1d890` | `CodexToolContext`、`CodexChatHistoryStore`、`ChatToResponsesState` 与 `create_responses_sse_stream_from_chat_with_context` 仍位于 Codex Chat/Responses bridge 路径。 |
| CLIProxyAPI | `main` @ `bc71c77f5cc42f3fbe1bf040cf14d4f166894835` | `previous_response_not_found` 的保留错误测试、`previous_response_id`、`output_item.done` 与 `response.completed` 的 translator/executor 测试仍可定位；executor 已拆分，旧行号不应视为当前定位。 |

各深度调研中的固定 commit 和行号仍是其原始观察证据；当本表显示模块或行号已经演进时，文档会明确区分“固定证据快照”和“当前模块级复核”。任何新的实现决策仍须在当前提交上重新固定文件/行号并建立 OpenBridge 自有 fixture。
