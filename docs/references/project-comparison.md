# 参考项目调研总览

## 文档元数据

| 字段 | 值 |
|---|---|
| Source snapshot | 本页链接的项目级叶文档及其固定 URL、commit、版本与采集日期；本页不另行拥有源码快照 |
| Last reverified | 2026-08-24：纳入 new-api 固定源码调研，并按当前 `docs/references/` 树核对分类、前置关系与链接；未刷新其他外部仓库、网页或 Provider |
| Scope | 比较外部项目的证据角色、已调研主题、互证关系和不可外推项 |
| Evidence boundary | 导航与综合比较不替代官方协议、项目叶文档、真实 Provider 验证或任何本地产品合同 |
| Recheck trigger | 项目版本、许可证、协议面、认证 flow 或综合文档的前置集合变化时 |

## 1. 文档用途

本页比较外部项目提供的证据类型、已调研主题与局限。它不定义任何本地产品角色、架构选择或实施任务。

项目级事实以各自目录中的固定快照为准；本页只提供导航和跨项目差异概览。

## 2. 项目与证据类型

| 项目                        | 主要产品形状                      | 已调研证据                                                                            | 不能由此推导                                                                   |
|-----------------------------|-----------------------------------|---------------------------------------------------------------------------------------|--------------------------------------------------------------------------------|
| Codex                       | Rust CLI/Agent client             | Responses SSE consumer、tool lifecycle、browser/device auth、refresh、client tests    | 完整 OpenAI 规范、第三方 OAuth 授权、server-side tool execution contract       |
| Hermes Agent                | Python Agent runtime              | Chat/Responses mode、history normalization、Codex credential lifecycle                | 通用 gateway IR、分布式 credential manager、所有 Agent 客户端语义              |
| LiteLLM                     | 多 Provider Proxy/SDK             | Chat/Responses adapter、Proxy call chain、model catalog、metrics、retry、OAuth、tests | 其 deployment/team/budget/control-plane 字段是通用协议                         |
| [new-api](new-api/README.md) | 多租户 LLM gateway 与运营平台    | 多协议 converter registry、渠道 priority/weight、计费结算、巡检和后台任务            | 动态控制面、余额支付、自动封禁或转换兼容行为是通用 gateway contract             |
| cc-switch                   | 桌面 Code Agent router/bridge     | Responses↔Chat conversion、tool context、history、SSE state、retry/failover           | Provider-name heuristic、UI/config takeover 或 call-id fallback 具有通用正确性 |
| CLIProxyAPI                 | 多协议 subscription/account proxy | state mapping、translator failures、credential cooldown、Codex OAuth scheduler        | account rotation、私有 client identity 或 WebSocket state 可移植               |
| OpenRouter                  | 聚合 Provider/API 与模型目录      | Model object、filters、Chat/Responses path、fixed model/wire snapshots                | 目录字段等于每个 endpoint 的实际 capability                                    |
| Open Responses              | 独立 Responses 规范/生态          | HTTP/SSE/WebSocket compliance scenarios                                               | 与 OpenAI 官方 Responses 完全相同或覆盖 Chat bridge                            |
| gpt-oss compatibility-test  | model/API-shape smoke             | Chat/Responses、streaming、function-call smoke                                        | 确定性 semantic oracle                                                         |

## 3. 功能分工

| 研究问题                                   | 直接项目证据                              | 综合文档                                                                               |
|--------------------------------------------|-------------------------------------------|----------------------------------------------------------------------------------------|
| Responses SSE 与 client tool lifecycle     | Codex                                     | [Protocol test assets](cross-project/chat-responses-sse-tool-test-suite-survey.md)     |
| Chat/Responses request/response conversion | LiteLLM、new-api、cc-switch          | [Protocol test assets](cross-project/chat-responses-sse-tool-test-suite-survey.md)     |
| Stateful continuation 与 opaque identity   | CLIProxyAPI、cc-switch、Codex             | 各项目 state/tool 文档；尚无单一通用 state contract                                    |
| Credential retry/cooldown                  | CLIProxyAPI、LiteLLM、new-api、cc-switch   | [Credential retry comparison](cross-project/credential-pool-retry-analysis.md)         |
| OAuth device login/refresh                 | Codex、CLIProxyAPI、Hermes、LiteLLM       | [OAuth comparison](cross-project/upstream-oauth-device-code-token-refresh-analysis.md) |
| Model information                          | LiteLLM、OpenRouter                       | [Model information comparison](cross-project/model-information-comparison.md)          |
| Observability/TTFT                         | LiteLLM、Codex                            | 各自 project document；口径尚未统一                                                    |
| Provider protocol                          | OpenAI、OpenRouter、DeepSeek、Xiaomi MiMo | 对应官方协议目录                                                                       |

## 4. 互证关系

### 4.1 Chat/Responses

- OpenAI official docs 定义公开 wire。
- Codex 说明一个具体 Responses client 如何消费 typed SSE 和 tool lifecycle。
- Hermes 说明完整 Agent loop 如何在 Chat history 与 Responses items 间归一化。
- LiteLLM、new-api 和 cc-switch 提供三种不同 converter/state 实现；new-api 还显式注册 direct 与 multi-hop conversion。
- Open Responses、gpt-oss 和 OpenAI SDK consumer tests 提供不同强度的协议与客户端测试。

这些证据角色互补，但没有任何一个项目可以同时替代官方 wire、client behavior、converter contract 和真实 Provider 验证。

### 4.2 OAuth

- RFC 定义标准 device authorization、refresh grant 和 rotation security。
- Codex 定义其 CLI 产品 flow。
- CLIProxyAPI 展示后台到期调度器。
- Hermes 展示同主机跨进程 auth-store lock。
- LiteLLM 展示简单按需 JSON authenticator 及其并发缺口。

四个项目复现相似私有 flow 不等于形成公共 client registration。

### 4.3 Retry

- CLIProxyAPI 的隔离单位偏 credential/account。
- LiteLLM 的隔离单位偏 deployment。
- new-api 的隔离单位偏 channel/group，并叠加 priority、weight、affinity 和自动禁用。
- cc-switch 的隔离单位偏 Provider。

相似的“换下一个候选”行为建立在不同资源身份和控制面上，不能只合并 status code 表。

### 4.4 Model information

- LiteLLM 明确区分兼容 model list、deployment、model group、global catalog 与 runtime metrics。
- OpenRouter 用丰富 `Model` 对象组织 canonical catalog，并把 endpoint detail 和 user-filtered view 分开。

两者共同说明模型身份、能力、供应、经济与运行时观测是不同信息层。

## 5. 研究维护规则

1. 单项目观察先进入对应项目目录，并固定 source/date/commit。
2. 综合文档只引用已经存在的项目级调研，不在综合文档首次引入项目事实。
3. issue 必须记录触发条件、失败 transcript 和项目版本；不能只引用结论。
4. “可借鉴”只表示研究价值，不改变该项目事实或形成实施承诺。
5. 外部 tests 需要区分 schema smoke、client consumption、deterministic state contract 与 real Provider E2E。
6. 动态模型、endpoint、SDK、policy 和 license 变化时重新复核，不用总览日期覆盖固定快照。
7. 本仓库的需求、当前代码和验证结果不写入本目录。
