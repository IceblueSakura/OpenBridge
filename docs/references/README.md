# 参考文档

本目录只保存外部协议、SDK、目标客户端、Provider 和参考项目的调研结果。每份文档应记录来源、快照日期或 commit、观察事实、推论与证据边界。

本目录不记录本仓库的当前实现状态、源码结构、已执行测试、目标数据类型或实施方案；这些内容按[文档分类规则](../README.md)
写入其他目录。

## 1. 目录规则

| 位置                           | 内容                                                                           |
|--------------------------------|--------------------------------------------------------------------------------|
| `openai/`                      | OpenAI 官方 API/SDK 与 gpt-oss 测试资产调研                                    |
| `openrouter/`                  | OpenRouter Models/Provider API 与固定目录快照                                  |
| `providers/`                   | 其他上游 Provider 的独立官方协议快照，以及已满足项目级前置条件的 Provider 对照 |
| `codex/`                       | Codex 源码、认证、SSE、tool lifecycle 和 tests                                 |
| `hermes/`                      | Hermes Agent 的协议与 credential lifecycle                                     |
| `litellm/`                     | LiteLLM Proxy、adapter、model、observability、performance 和 tests             |
| `cc-switch/`                   | cc-switch protocol bridge 与 retry/failover                                    |
| `cliproxyapi/`                 | CLIProxyAPI state、credential 与 OAuth scheduler                               |
| `open-responses/`              | Open Responses 规范与 compliance tests                                         |
| `responses-proxy/`             | CallOrRet/responses-proxy 转换与 tests                                         |
| `openai-compatibility-tester/` | beranekio compatibility tester                                                 |
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

## 2. OpenAI 官方协议

- [API 规范目录](openai/api-specification-catalog.md)
- [Chat Completions 协议](openai/chat-completions-protocol.md)
- [Responses 协议](openai/responses-protocol.md)
- [Embeddings 与多模态 API 关系](openai/embedding-and-multimodal-forwarding.md)
- [扩展协议调研索引](openai/protocol-details/README.md)

测试资产：

- [gpt-oss compatibility-test](openai/gpt-oss-compatibility-test-analysis.md)
- [OpenAI SDK streaming consumers](openai/openai-sdk-stream-test-assets-analysis.md)

## 3. Provider 官方资料

### OpenRouter

- [Models API 与能力字段](openrouter/model-information-api-analysis.md)
- [模型目录快照（2026-08-02）](openrouter/model-catalog-2026-08-02.md)
- [Provider API 快照（2026-08-02）](openrouter/provider-api-2026-08-02.md)

### DeepSeek、Xiaomi MiMo、NVIDIA 与阿里云百炼

- [DeepSeek 协议入口快照（2026-08-08）](providers/deepseek-protocol-2026-08-08.md)
- [Xiaomi MiMo 协议入口与文本生成快照](providers/xiaomi-mimo-protocol-2026-08-02.md)
- [Xiaomi MiMo 图片理解协议与真实观察](providers/xiaomi-mimo-image-protocol-2026-08-07.md)
- [Xiaomi MiMo 音频理解、ASR/TTS 协议与真实观察](providers/xiaomi-mimo-audio-protocol-2026-08-08.md)
- [DeepSeek/MiMo 综合对照（2026-08-02 历史快照）](providers/deepseek-mimo-protocols-2026-08-02.md)
- [NVIDIA MiniMax M3 与百炼 GLM/Qwen Chat 模型入口（2026-08-08）](providers/nvidia-bailian-chat-models-2026-08-08.md)

## 4. 参考项目

参考项目的许可证以各仓库根 `LICENSE` 为准；此处不是依赖引入或法律意见。

| 项目         | 许可证入口                                                                              | 项目级调研                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
|--------------|-----------------------------------------------------------------------------------------|------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| Codex        | [Apache-2.0](https://github.com/openai/codex/blob/main/LICENSE)                         | [SSE/tool lifecycle](codex/codex-sse-and-tool-lifecycle-analysis.md)、[browser OAuth/tool](codex/codex-oauth-and-tool-call-analysis.md)、[device auth/refresh](codex/codex-device-auth-token-refresh-analysis.md)、[tests](codex/codex-protocol-test-assets-analysis.md)                                                                                                                                                                                                                                         |
| Hermes Agent | [MIT](https://github.com/NousResearch/hermes-agent/blob/main/LICENSE)                   | [Chat/Responses](hermes/hermes-chat-responses-analysis.md)、[Codex OAuth lifecycle](hermes/hermes-codex-oauth-refresh-analysis.md)                                                                                                                                                                                                                                                                                                                                                                               |
| LiteLLM      | [MIT; enterprise subtree另有条款](https://github.com/BerriAI/litellm/blob/main/LICENSE) | [Chat/Responses](litellm/litellm-chat-responses-analysis.md)、[call chain](litellm/litellm-proxy-call-chain-analysis.md)、[performance](litellm/litellm-proxy-performance-bottlenecks.md)、[observability](litellm/litellm-observability-analysis.md)、[model info](litellm/litellm-model-information-analysis.md)、[credential retry](litellm/litellm-credential-pool-retry-analysis.md)、[OAuth](litellm/litellm-chatgpt-oauth-refresh-analysis.md)、[tests](litellm/litellm-protocol-test-assets-analysis.md) |
| cc-switch    | [MIT](https://github.com/farion1231/cc-switch/blob/main/LICENSE)                        | [Chat/Responses tool conversion](cc-switch/cc-switch-chat-responses-tool-conversion-analysis.md)、[retry/failover](cc-switch/cc-switch-retry-failover-analysis.md)                                                                                                                                                                                                                                                                                                                                               |
| CLIProxyAPI  | [MIT](https://github.com/router-for-me/CLIProxyAPI/blob/main/LICENSE)                   | [stateful bridge](cliproxyapi/cliproxyapi-stateful-bridge-analysis.md)、[credential retry](cliproxyapi/cliproxyapi-credential-pool-retry-analysis.md)、[Codex OAuth scheduler](cliproxyapi/cliproxyapi-codex-oauth-refresh-analysis.md)                                                                                                                                                                                                                                                                          |

其他测试项目：

- [Open Responses Compliance](open-responses/open-responses-compliance-analysis.md)
- [CallOrRet/responses-proxy](responses-proxy/responses-proxy-test-assets-analysis.md)
- [beranekio/openai-compatibility-tester](openai-compatibility-tester/openai-compatibility-tester-analysis.md)

## 5. 综合调研

| 功能                 | 综合文档                                                                                         | 项目级前置                                                                                |
|----------------------|--------------------------------------------------------------------------------------------------|-------------------------------------------------------------------------------------------|
| Protocol tests       | [Chat/Responses、SSE 与工具测试资产](cross-project/chat-responses-sse-tool-test-suite-survey.md) | OpenAI gpt-oss/SDK、Open Responses、Codex、LiteLLM、responses-proxy、compatibility-tester |
| Credential retry     | [Pool、cooldown 与有限重试](cross-project/credential-pool-retry-analysis.md)                     | CLIProxyAPI、LiteLLM、cc-switch                                                           |
| Model information    | [LiteLLM/OpenRouter 模型信息](cross-project/model-information-comparison.md)                     | LiteLLM、OpenRouter                                                                       |
| OAuth device/refresh | [设备登录与 token refresh](cross-project/upstream-oauth-device-code-token-refresh-analysis.md)   | Codex、CLIProxyAPI、Hermes、LiteLLM                                                       |

## 6. 固定快照索引

下面只记录外部项目本地 checkout 的 2026-08-01/05 复核位置，不覆盖各文档中的原始逐行 commit：

| 项目        | 分支与提交                                                              | 已复核主题                                                     |
|-------------|-------------------------------------------------------------------------|----------------------------------------------------------------|
| Codex       | `main` @ `ee0247f95a6fe2b094ba2253d82cae2a2b4c2dff`                     | device/browser auth、refresh、Responses SSE/tool tests         |
| Hermes      | `main` @ `470cf66b039c73bdd2c21d43094ce41a4db74eae`                     | Chat/Responses mode、Codex credential lifecycle                |
| LiteLLM     | `litellm_internal_staging` @ `23de7a15d9d40006ee596e617475ba101d60c5e9` | Responses routes、ChatGPT authenticator、model/metrics modules |
| cc-switch   | `main` @ `ebbf141fc71547a99f669df1be8e345130d1d890`                     | bridge state、history、retry/failover                          |
| CLIProxyAPI | `main` @ `bc71c77f5cc42f3fbe1bf040cf14d4f166894835`                     | stateful translator、credential retry、OAuth scheduler         |

## 7. 新增文档检查表

- 对应单一项目或官方来源目录；
- 记录 source URL、snapshot date/commit 和阅读范围；
- 区分观察事实、推论、未验证项与适用边界；
- 不包含本仓库源码路径、当前实现状态、目标类型或实施步骤；
- 多项目综合先链接每个项目的独立调研；
- 不记录 credential、私有配置、敏感请求或未脱敏 production transcript；
- 动态官方事实使用固定日期理解，升级结论前重新复核。
