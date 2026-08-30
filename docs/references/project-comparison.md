# 参考项目调研总览

## 文档元数据

| 字段 | 值 |
|---|---|
| Source snapshot | 本页链接的项目级叶文档及其固定 URL、commit、版本与采集日期；本页不另行拥有源码快照 |
| Last reverified | 2026-08-30：纳入protocol gateway/semantic model固定源码调研与OpenRouter公开资料复核；旧项目的原始快照仍由各叶文档维护 |
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
| LiteLLM                     | 多 Provider Proxy/SDK             | Chat/Responses adapter、Proxy call chain、model catalog、metrics、retry、OAuth、tests | 其 deployment/team/budget/control-plane字段是通用协议                         |
| [Bifrost](protocol-gateways/bifrost.md) | 多协议、多Provider gateway | operation-specific schemas、Provider converters、Responses/server-tool/stream tests | 其协议schema可直接作为protocol-neutral canonical IR |
| [TensorZero](protocol-gateways/tensorzero.md) | LLM inference/optimization platform | semantic content、Provider tool scope、reasoning/state和Provider adapters | arbitrary Provider tool payload可跨Route安全转发 |
| [Vercel AI SDK](protocol-gateways/vercel-ai-sdk.md) | 多Provider应用SDK | provider-neutral static content、stream parts、Provider tools/options | SDK warnings/options/headers满足不受信Gateway的fail-closed边界 |
| [Portkey Gateway](protocol-gateways/portkey.md) | 多Provider gateway | config-driven Provider adapter、middleware、response/stream/error transforms | silent field drop、clamp或synthetic stream具有语义等价性 |
| [Helicone AI Gateway](protocol-gateways/helicone.md) | Rust runtime gateway | weighted/latency routing、retry、cache、metrics和integration tests | endpoint相同即可证明候选capability等价 |
| [new-api](new-api/README.md) | 多租户 LLM gateway 与运营平台    | 多协议 converter registry、渠道 priority/weight、计费结算、巡检和后台任务            | 动态控制面、余额支付、自动封禁或转换兼容行为是通用 gateway contract             |
| cc-switch                   | 桌面 Code Agent router/bridge     | Responses↔Chat conversion、tool context、history、SSE state、retry/failover           | Provider-name heuristic、UI/config takeover 或 call-id fallback 具有通用正确性 |
| CLIProxyAPI                 | 多协议 subscription/account proxy | state mapping、translator failures、credential cooldown、Codex OAuth scheduler        | account rotation、私有 client identity 或 WebSocket state 可移植               |
| OpenRouter                  | 聚合 Provider/API 与模型目录      | Model object、filters、Chat/Responses path、fixed model/wire snapshots                | 目录字段等于每个 endpoint 的实际 capability                                    |
| Open Responses              | 独立 Responses 规范/生态          | HTTP/SSE/WebSocket compliance scenarios                                               | 与 OpenAI 官方 Responses 完全相同或覆盖 Chat bridge                            |
| gpt-oss compatibility-test  | model/API-shape smoke             | Chat/Responses、streaming、function-call smoke                                        | 确定性 semantic oracle                                                         |

## 3. 综合调研入口

跨项目共性、差异和未知项只由[综合调研索引](cross-project/README.md)下的主题文档维护：

- Chat/Responses、SSE 与 tool 测试资产；
- 富语义 IR、Provider extensions、server tools 与 protocol/runtime 边界；
- credential retry/cooldown；
- OAuth device login/refresh；
- model information。

Stateful continuation、observability 与 Provider protocol 目前只有项目级或官方来源，没有独立综合 owner。

## 4. 研究维护规则

1. 单项目观察先进入对应项目目录，并固定 source/date/commit。
2. 综合文档只引用已经存在的项目级调研，不在综合文档首次引入项目事实。
3. issue 必须记录触发条件、失败 transcript 和项目版本；不能只引用结论。
4. “可借鉴”只表示研究价值，不改变该项目事实或形成实施承诺。
5. 外部 tests 需要区分 schema smoke、client consumption、deterministic state contract 与 real Provider E2E。
6. 动态模型、endpoint、SDK、policy 和 license 变化时重新复核，不用总览日期覆盖固定快照。
7. 本仓库的需求、当前代码和验证结果不写入本目录。
