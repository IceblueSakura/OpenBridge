# 参考文档

本目录只保存外部协议、SDK、目标客户端、Provider 和参考项目的调研结果。每份文档应记录来源、快照日期或 commit、观察事实、推论与证据边界。

本目录不记录本仓库的当前实现状态、源码结构、已执行测试、目标数据类型或实施方案；这些内容按[文档分类规则](../README.md)
写入其他目录。

## 1. 目录规则

| 位置                           | 内容                                                                           |
|--------------------------------|--------------------------------------------------------------------------------|
| `openai/`                      | OpenAI 官方 API/SDK 与 gpt-oss 测试资产调研                                    |
| `providers/<provider>/`       | 每个上游 Provider 的独立官方协议快照，以及已满足项目级前置条件的 Provider 对照 |
| `codex/`                       | Codex 源码、认证、SSE、tool lifecycle 和 tests                                 |
| `hermes/`                      | Hermes Agent 的协议与 credential lifecycle                                     |
| `litellm/`                     | LiteLLM Proxy、adapter、model、observability、performance 和 tests             |
| `cc-switch/`                   | cc-switch protocol bridge 与 retry/failover                                    |
| `cliproxyapi/`                 | CLIProxyAPI state、credential 与 OAuth scheduler                               |
| `mcp/`                         | MCP 协议规范与 Rust 生态 SDK 调研                                              |
| `cross-project/`               | 已经存在各项目独立调研后的综合比较                                             |
| 当前目录                       | 总索引与[参考项目调研总览](project-comparison.md)                              |

单项目事实只能写入对应项目目录。一个功能同时参考多个项目时，顺序必须是：

```text
project A research
+ project B research
+ ...
→ cross-project synthesis linking every prerequisite document
```

综合文档不得替代项目级来源，不得把比较结论写回任一项目的“实现事实”。

## 2. 语音资料的两类入口

语音资料固定按协议规范与 Provider 能力分开，避免把“模型能处理音频”误写成“兼容某个标准 endpoint”：

| 类别                 | 回答的问题                                                                    | 唯一入口                                                                                         |
|----------------------|-------------------------------------------------------------------------------|--------------------------------------------------------------------------------------------------|
| 标准 Audio/Speech 协议 | `/audio/*`、Chat audio、Realtime 的 path、wire、media response 和生命周期是什么 | [OpenAI 音频细粒度索引](openai/README.md#6-音频与语音)与[Realtime 索引](openai/README.md#7-realtime) |
| MiMo 模型语音能力    | 当前六个 MiMo model 分别支持什么、通过哪个 endpoint/字段调用、证据到哪一层      | [Xiaomi MiMo 全模型语音能力与调用途径](providers/xiaomi/audio.md)    |

功能需求和实施现状只引用这两个入口，不在同一文档中混写标准协议与 Provider capability。

## 3. OpenAI 官方协议

- [细粒度协议调研总索引](openai/README.md)
- [API 规范目录](openai/api-specification-catalog.md)

测试资产：

- [gpt-oss compatibility-test](openai/gpt-oss-compatibility-test-analysis.md)
- [OpenAI SDK streaming consumers](openai/openai-sdk-stream-test-assets-analysis.md)

## 4. MCP 协议与 Rust SDK

- [MCP Rust 生态调研索引](mcp/README.md)（生态全景、协议支持矩阵、选型观察；快照 2026-08-08）
  - [rmcp 官方 Rust SDK](mcp/rmcp-official-sdk.md)——唯一完整实现 2026-07-28 现行规范
  - [rust-mcp-sdk（rust-mcp-stack）](mcp/rust-mcp-sdk-community.md)——axum 一站式托管，锚定 2025-11-25
  - [pmcp（Pragmatic AI Labs）](mcp/pmcp.md)——传输/认证面最广，锚定 2025-11-25
  - [fastmcp_rust](mcp/fastmcp-rust.md)——实验性，asupersync 运行时，不构成生产选型

## 5. Provider 官方资料

Provider 文档按"调研方向"组织：每个 provider 一个目录，目录内至少拆分为 `api.md`（协议入口、认证与 wire 事实）与 `models.md`（模型目录与能力字段）；专项能力面（如 MiMo 图片/音频）独立成文；不保留多份日期快照，快照日期记录在各文档"来源与范围"节内。

### OpenRouter

- [API 与模型能力调研](providers/openrouter/api.md)——接口分层、`Model` 对象字段语义、入口/认证、live wire 观察
- [模型目录](providers/openrouter/models.md)——全模态目录、精确匹配与 endpoint 参数差异（复核 2026-08-09）

### DeepSeek、LongCat、Xiaomi MiMo、Kimi、NVIDIA 与阿里云百炼

- [DeepSeek API 协议入口](providers/deepseek/api.md)（2026-08-08）——endpoint、认证、Responses 约束
- [DeepSeek 模型目录与定价](providers/deepseek/models.md)（2026-08-08）——官方模型表、特性矩阵、OpenRouter 补充
- [LongCat API 与 reasoning](providers/longcat/api.md)（2026-08-08）——Chat thinking 开关、Native Responses 与官方 Codex 配置
- [LongCat 2.0 模型事实](providers/longcat/models.md)（2026-08-08）——官方模型详情与 OpenRouter reasoning 交叉证据
- [Xiaomi MiMo API 协议入口](providers/xiaomi/api.md)——origin、Chat/Responses 入口、双认证方式
- [Xiaomi MiMo 模型目录](providers/xiaomi/models.md)——官方 6 模型、V2 下线、OpenRouter 补充
- [Xiaomi MiMo 图片理解协议与真实观察](providers/xiaomi/image.md)（2026-08-07）
- [Xiaomi MiMo 全模型语音能力与调用途径](providers/xiaomi/audio.md)（2026-08-08）
- [Kimi CN API 协议入口](providers/kimi/api.md)（2026-08-09）
- [Kimi K3 模型参数](providers/kimi/models.md)（2026-08-09）
- [NVIDIA API Catalog / NIM API 协议入口](providers/nvidia/api.md)——base URL、`nvapi-` 认证、端点表
- [NVIDIA API Catalog Models 列表](providers/nvidia/models.md)（2026-08-08）
- [阿里云百炼 API 协议入口](providers/bailian/api.md)——多地域 base URL、四类协议面、请求能力面
- [阿里云百炼 Models 列表前缀与研发者分类](providers/bailian/models.md)（2026-08-08）

## 6. 参考项目

参考项目的许可证以各仓库根 `LICENSE` 为准；此处不是依赖引入或法律意见。

| 项目         | 许可证入口                                                                              | 项目级调研                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
|--------------|-----------------------------------------------------------------------------------------|------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| Codex        | [Apache-2.0](https://github.com/openai/codex/blob/main/LICENSE)                         | [SSE/tool lifecycle](codex/codex-sse-and-tool-lifecycle-analysis.md)、[browser OAuth/tool](codex/codex-oauth-and-tool-call-analysis.md)、[device auth/refresh](codex/codex-device-auth-token-refresh-analysis.md)、[tests](codex/codex-protocol-test-assets-analysis.md)                                                                                                                                                                                                                                         |
| Hermes Agent | [MIT](https://github.com/NousResearch/hermes-agent/blob/main/LICENSE)                   | [Chat/Responses](hermes/hermes-chat-responses-analysis.md)、[Codex OAuth lifecycle](hermes/hermes-codex-oauth-refresh-analysis.md)、[Models endpoint schema](hermes/hermes-models-endpoint-schema.md)、[Provider plugin 能力与 aux 分派](hermes/hermes-provider-plugin-capabilities.md)、[网关插件化扩展全景](hermes/hermes-gateway-plugin-capabilities.md)                                                                                                                                                                                                                                                                        |
| LiteLLM      | [MIT; enterprise subtree另有条款](https://github.com/BerriAI/litellm/blob/main/LICENSE) | [Chat/Responses](litellm/litellm-chat-responses-analysis.md)、[call chain](litellm/litellm-proxy-call-chain-analysis.md)、[performance](litellm/litellm-proxy-performance-bottlenecks.md)、[observability](litellm/litellm-observability-analysis.md)、[model info](litellm/litellm-model-information-analysis.md)、[credential retry](litellm/litellm-credential-pool-retry-analysis.md)、[OAuth](litellm/litellm-chatgpt-oauth-refresh-analysis.md)、[tests](litellm/litellm-protocol-test-assets-analysis.md) |
| cc-switch    | [MIT](https://github.com/farion1231/cc-switch/blob/main/LICENSE)                        | [Chat/Responses tool conversion](cc-switch/cc-switch-chat-responses-tool-conversion-analysis.md)、[retry/failover](cc-switch/cc-switch-retry-failover-analysis.md)                                                                                                                                                                                                                                                                                                                                               |
| CLIProxyAPI  | [MIT](https://github.com/router-for-me/CLIProxyAPI/blob/main/LICENSE)                   | [stateful bridge](cliproxyapi/cliproxyapi-stateful-bridge-analysis.md)、[credential retry](cliproxyapi/cliproxyapi-credential-pool-retry-analysis.md)、[Codex OAuth scheduler](cliproxyapi/cliproxyapi-codex-oauth-refresh-analysis.md)                                                                                                                                                                                                                                                                          |

其他测试项目：

- [Open Responses Compliance](openai/open-responses-compliance-analysis.md)

## 7. 综合调研

| 功能                 | 综合文档                                                                                         | 项目级前置                                                                                |
|----------------------|--------------------------------------------------------------------------------------------------|-------------------------------------------------------------------------------------------|
| Protocol tests       | [Chat/Responses、SSE 与工具测试资产](cross-project/chat-responses-sse-tool-test-suite-survey.md) | OpenAI gpt-oss/SDK、Open Responses、Codex、LiteLLM |
| Credential retry     | [Pool、cooldown 与有限重试](cross-project/credential-pool-retry-analysis.md)                     | CLIProxyAPI、LiteLLM、cc-switch                                                           |
| Model information    | [LiteLLM/OpenRouter 模型信息](cross-project/model-information-comparison.md)                     | LiteLLM、OpenRouter                                                                       |
| OAuth device/refresh | [设备登录与 token refresh](cross-project/upstream-oauth-device-code-token-refresh-analysis.md)   | Codex、CLIProxyAPI、Hermes、LiteLLM                                                       |

## 8. 固定快照索引

下面只记录外部项目本地 checkout 的 2026-08-01/05 复核位置，不覆盖各文档中的原始逐行 commit：

| 项目        | 分支与提交                                                              | 已复核主题                                                     |
|-------------|-------------------------------------------------------------------------|----------------------------------------------------------------|
| Codex       | `main` @ `ee0247f95a6fe2b094ba2253d82cae2a2b4c2dff`                     | device/browser auth、refresh、Responses SSE/tool tests         |
| Hermes      | `main` @ `470cf66b039c73bdd2c21d43094ce41a4db74eae`                     | Chat/Responses mode、Codex credential lifecycle                |
| LiteLLM     | `litellm_internal_staging` @ `23de7a15d9d40006ee596e617475ba101d60c5e9` | Responses routes、ChatGPT authenticator、model/metrics modules |
| cc-switch   | `main` @ `ebbf141fc71547a99f669df1be8e345130d1d890`                     | bridge state、history、retry/failover                          |
| CLIProxyAPI | `main` @ `bc71c77f5cc42f3fbe1bf040cf14d4f166894835`                     | stateful translator、credential retry、OAuth scheduler         |

## 9. 新增文档检查表

- 对应单一项目或官方来源目录；
- 记录 source URL、snapshot date/commit 和阅读范围；
- 区分观察事实、推论、未验证项与适用边界；
- 不包含本仓库源码路径、当前实现状态、目标类型或实施步骤；
- 多项目综合先链接每个项目的独立调研；
- 不记录 credential、私有配置、敏感请求或未脱敏 production transcript；
- 动态官方事实使用固定日期理解，升级结论前重新复核。
