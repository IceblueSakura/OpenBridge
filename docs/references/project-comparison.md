# OpenBridge 参考项目比较矩阵

## 状态

**持续更新。** 快照日期：2026-07-22。

本文按 OpenBridge 的设计问题组织参考项目，避免把“研究了更多项目”本身当作收敛。已有深度调研继续使用固定 commit；新增候选项目在形成结论前必须补充 exact commit、文件范围、issue/release 证据和本地实验。

## 1. 项目选择原则

OpenBridge 核心是单用户、单服务的多 Provider Agent proxy。参考项目的优先级取决于它能否回答以下问题：

1. Codex/Hermes 实际 wire contract 是什么？
2. Provider Family 与 runtime Deployment 应如何分界？
3. Native Path 如何保留前向兼容字段和 SSE 语义？
4. Chat/Responses/Messages bridge 在 identity、state、terminal 上如何失败？
5. 单用户本地配置、客户端接入和使用量体验如何保持简单？

多租户、预算、合规和企业控制面不是主要研究目标，除非其中某个实现能提供可复用的底层 Provider/stream/bridge 证据。

## 2. 总览

| 项目 | OpenBridge 中的角色 | 当前状态 | 优先研究 | 不直接复制 |
|---|---|---|---|---|
| Codex | 首要目标客户端、Responses 契约来源 | 已有源码调研；待补充新一轮滚动 corpus | Responses request/SSE、tools、continuation、custom Provider config、HTTP/SSE/WebSocket transport | 本地 auth cache、未公开 subscription backend |
| Hermes Agent | 首要目标客户端、多 transport Agent loop | 已有源码调研；仅在兼容声明时补 E2E | Chat/Responses/Anthropic modes、tool loop、strict endpoint | 完整 Agent 产品和 Provider catalog UI |
| LiteLLM | Provider 行为/参数/错误/转换资料库 | 已有深度调研 | Provider-specific transforms、error/retry、bridge edge cases | 多租户 proxy、virtual keys、预算/DB 控制面 |
| cc-switch | 单用户本地接入与 Codex bridge 参考 | 已有深度调研 | 客户端配置接管、history/tool context、SSE state、usage UX | 桌面 GUI 整体架构、未经独立验证的恢复策略 |
| Bifrost | Provider core 与高性能 pipeline 的对照样本 | 新增研究候选 | Provider interface、request flow、model catalog、native/compatibility split、cancellation | 企业治理、插件生态、性能宣传数字 |
| CLIProxyAPI | Codex/Chat/Responses bridge 负面案例库 | 新增研究候选 | translator、tool identity、`previous_response_id`、stateful routing、SSE failures | 多账号/订阅 credential pooling、非官方 OAuth 路径 |

## 3. Codex

### 研究职责

- 研究 OpenBridge `/v1/responses` 下游契约；
- 观察 function tool、reasoning、usage、cancel 和 continuation；
- 验证 custom model Provider 的 `base_url`、认证、wire mode 和 `supports_websockets`；
- 识别版本升级带来的 HTTP/SSE/WebSocket 行为变化。

### 已有材料

- [本仓库 Codex OAuth 与 tool lifecycle 调研](codex-oauth-and-tool-call-analysis.md)
- Codex repository：https://github.com/openai/codex
- Codex configuration：https://developers.openai.com/codex/config-advanced
- Codex configuration reference：https://developers.openai.com/codex/config-reference
- Provider model source：https://github.com/openai/codex/blob/main/codex-rs/model-provider-info/src/lib.rs
- Responses-only custom Provider direction：https://github.com/openai/codex/discussions/7782

### 当前 transport 工作假设

- Codex custom Provider 的公开配置模型包含 `supports_websockets`；当前源码中该字段使用默认 false。
- OpenBridge 初期使用独立 custom Provider id 并显式写入 `supports_websockets = false`，只承诺 HTTP/SSE。
- 旧版本曾出现内置 `openai` Provider 无法被同名配置覆盖的报告；OpenBridge 不依赖覆盖保留 Provider id，而是验证自己的 custom Provider profile。
- WebSocket 是否进入初期范围由记录实际 Codex 版本的实验决定，不能从字段存在直接推导。

### 待补实验

- 记录一次 Codex release/commit；
- native Responses text/tool/parallel/cancel/error corpus；
- custom Provider headers、base URL 与显式 `supports_websockets = false`；
- Codex 诊断和抓包/代理日志确认实际使用 HTTP/SSE；
- continuation/stateful tool loop；
- 每次版本升级的 compatibility diff。

### 不能推导

- Codex CLI 内部 OAuth client identity 可供 OpenBridge 使用；
- Codex 可消费的 Responses subset 等于完整 OpenAI Responses API；
- 某一版本的 event/header 永久稳定；
- 当前可关闭 WebSocket 意味着未来版本仍会保留 HTTP/SSE。

## 4. Hermes Agent

### 研究职责

- 研究 Chat 与 Responses 客户端契约；
- 验证 transport/api mode 切换；
- 验证完整 Agent tool loop，而不只 SDK parse；
- 提供 strict endpoint、未知字段、usage 和 auxiliary path 的负面样本。

### 已有材料

- [本仓库 Hermes Chat/Responses 分析](hermes-chat-responses-analysis.md)
- Hermes repository：https://github.com/NousResearch/hermes-agent
- Adding providers：https://github.com/NousResearch/hermes-agent/blob/main/website/docs/developer-guide/adding-providers.md
- Provider overview：https://github.com/NousResearch/hermes-agent/blob/main/website/docs/integrations/providers.md

### 重点 issue 类型

- custom Provider transport/api mode 选择；
- Responses-only 模型被错误路由到 Chat，或反之；
- source-specific 字段泄漏到 strict Chat endpoint；
- usage/reasoning 归一；
- Provider 切换后 stale transport state。

### 待补实验

- 记录 Hermes 实际版本；
- Chat/Responses 两种 native path；
- 一个 auxiliary task；
- Provider 切换；
- HTTP 200 中 stream error；
- usage-only final chunk。

## 5. LiteLLM

### 研究职责

LiteLLM 适合作为：

- Provider 参数支持和差异资料库；
- provider-specific request/response transform 样本；
- retry/fallback/error 分类对照；
- Chat/Responses bridge edge-case 来源。

不把 LiteLLM Proxy 的全部控制面、数据库、virtual key、budget 和团队模型作为 OpenBridge 模板。

### 已有材料

- [Chat/Responses 分析](litellm-chat-responses-analysis.md)
- [Proxy 调用链](litellm-proxy-call-chain-analysis.md)
- [Proxy 性能观察](litellm-proxy-performance-bottlenecks.md)
- LiteLLM repository：https://github.com/BerriAI/litellm

### 下一轮问题

- 哪些 Provider 差异属于 Family 代码，哪些只是 Deployment 数据？
- LiteLLM 在 native pass-through 与 full normalization 之间如何选择？
- tool/usage/finish reason 的 provider-specific workaround 能否转为 OpenBridge conformance fixture？
- issue/release 中有哪些“静默丢字段”修复？

## 6. cc-switch

### 研究职责

- 单用户本地服务和客户端配置接管；
- Codex Responses ↔ Chat bridge；
- tool context/history recovery；
- SSE assembly；
- route/usage 的用户可见性。

### 已有材料

- [cc-switch 协议与工具转换分析](cc-switch-chat-responses-tool-conversion-analysis.md)
- cc-switch repository：https://github.com/farion1231/cc-switch

### 需要反证的部分

- history recovery 是否依赖模型/provider 名称或隐式全局 state；
- call identity 是否在并行工具和多轮请求中稳定；
- usage 展示的数据来源和精度；
- 客户端配置写入/恢复失败时的安全行为。

## 7. Bifrost

### 为什么加入

Bifrost 是活跃的多 Provider AI gateway，公开了 Provider 配置、request flow、model catalog 和插件/扩展机制。它适合作为 OpenBridge typed Provider core 与 pipeline 的对照，而不是因为其企业功能或性能宣传。

### 一手入口

- Repository：https://github.com/maximhq/bifrost
- Request flow：https://docs.getbifrost.ai/architecture/core/request-flow
- Provider configuration：https://docs.getbifrost.ai/quickstart/gateway/provider-configuration
- Model catalog：https://docs.getbifrost.ai/architecture/framework/model-catalog
- Provider routing：https://docs.getbifrost.ai/providers/provider-routing

### 研究问题

1. 标准 schema 与 Provider-native schema 的边界在哪里？
2. 同协议 native path 是否绕过完整 normalization？
3. Provider interface 如何拆分 request、response、stream、error 和 auth？
4. model catalog 的 capability 是 Provider、model 还是 endpoint 级？
5. client disconnect 如何传播到上游？
6. Provider onboarding 最小修改面是什么？
7. 插件是否进入 token hot path，OpenBridge 哪些扩展不应复制？

### 负面证据候选

- OpenAI → Anthropic tool payload translation issue：https://github.com/maximhq/bifrost/issues/3511
- Release notes 中 cross-provider reasoning/image translation fixes：https://github.com/maximhq/bifrost/releases
- client disconnect/upstream connection issue：https://github.com/maximhq/bifrost/issues/3164

这些 issue 用于构造 OpenBridge fixture，不表示 Bifrost 当前仍存在相同缺陷；深度调研时必须核对修复 commit 和版本。

## 8. CLIProxyAPI

### 为什么加入

CLIProxyAPI 直接面向 OpenAI/Responses、Codex、Claude、Gemini 等 CLI/client，并包含大量协议 translator 和 stateful routing 失败案例。它与 OpenBridge 的目标客户端和 bridge 问题高度重合，尤其适合研究“看似兼容但在多轮 tool loop 中失败”的边界。

### 一手入口

- Repository：https://github.com/router-for-me/CLIProxyAPI
- Chat → Codex tool output failure：https://github.com/router-for-me/CLIProxyAPI/issues/2132
- Stateful routing affinity：https://github.com/router-for-me/CLIProxyAPI/issues/2594
- `previous_response_id`/WebSocket continuation failure：https://github.com/router-for-me/CLIProxyAPI/issues/2596
- Responses continuation robustness discussion：https://github.com/router-for-me/CLIProxyAPI/issues/1948

### 研究问题

1. translator 如何表示 Chat assistant tool-call group 与 Responses items？
2. `call_id`、item ID、tool index 和 response ID 在何处产生/恢复？
3. `previous_response_id` 被保留、删除或重建时发生什么？
4. round-robin/fallback 为什么破坏 stateful Responses？
5. Responses SSE/WebSocket 与 Chat SSE 的 terminal/usage/error 如何映射？
6. translator 如何处理 unknown items、hosted tools、reasoning 和 compaction？
7. 哪些错误来自 credential/account pooling，而不适用于 OpenBridge 单 credential 范围？

### 使用边界

重点研究 translator 和 issue；不采纳：

- 多 CLI 账号池；
- subscription credential 聚合；
- 未经官方确认的 OAuth/client identity；
- 为账号轮转设计的路由策略。

后续调研应把每个 issue 转为：

```text
failure taxonomy
→ minimal transcript
→ expected OpenBridge eligibility/state rule
→ regression fixture
```

## 9. 问题驱动比较矩阵

| 设计问题 | Codex | Hermes | LiteLLM | cc-switch | Bifrost | CLIProxyAPI |
|---|---:|---:|---:|---:|---:|---:|
| 下游 Responses 契约 | 主证据 | 次证据 | 参考 | 参考 | 参考 | 强负面案例 |
| Codex HTTP/SSE/WebSocket transport | **主证据** | 无 | 弱 | 接入参考 | 弱 | state/continuation 负面案例 |
| 下游 Chat/Agent loop | 弱 | 主证据 | SDK/协议参考 | 参考 | 参考 | 负面案例 |
| Provider adapter 粒度 | 客户端视角 | 客户端视角 | 强 | 中 | **强候选** | 中 |
| Native vs normalization | 主契约 | transport 对照 | 强 | 中 | **强候选** | 强 |
| Tool identity/state | 强 | 强 | 强 | 强 | issue 候选 | **强负面案例** |
| SSE/terminal/cancel | 强 | 强 | 强 | 强 | pipeline/issue 候选 | **强负面案例** |
| 单用户配置/UX | 配置对象 | 配置对象 | 弱 | **强** | 中 | 中 |
| Usage analysis | 客户端观察 | 客户端观察 | 强但过重 | 强 UX | 强但过重 | 中 |
| OAuth 合法边界 | 官方客户端参考 | 实现参考 | 实现参考 | 实现参考 | 非重点 | 风险反例 |

## 10. 调研产物模板

每份深度调研至少包含：

```text
Repository URL
License
Default branch
Full commit SHA
Snapshot date
Files inspected
Issues/releases inspected
Research question
Observed facts
Inferences
Negative evidence
Applicability to OpenBridge
Rejected assumptions
Required experiment
Decision impact
Revalidation trigger
```

本地磁盘路径不能替代 repository/permalink。

## 11. 收敛规则

- Codex/Hermes 决定下游兼容，不由代理项目替代；
- LiteLLM/Bifrost 用于 Provider core 和转换对照；
- cc-switch 用于单用户接入和已有 bridge 深挖；
- CLIProxyAPI 用于 bridge/state 失败 taxonomy；
- 一个核心决策至少需要支持证据、不同方案、负面案例和本地实验；
- 新项目不再提供新架构或失败类型时停止扩展样本。
