# 功能需求

本目录只定义当前有效的产品行为、失败语义、安全边界和交付证据要求，不记录“代码已经做到什么”或某次测试结果，也不预先固定模块拆分与实施顺序。功能需求是产品承诺的唯一入口；实施计划只能解释某项需求准备如何落地，不能反向扩大兼容承诺。

| 功能域                      | 当前需要回答的问题                                                          | 文档                                                                        |
|-----------------------------|-----------------------------------------------------------------------------|-----------------------------------------------------------------------------|
| 产品范围                    | 服务为谁解决什么问题，哪些能力不做                                          | [产品范围](product-scope.md)                                                |
| 网关 API、兼容与观测        | 客户端可调用什么、JSON/SSE/tool/continuation 如何表现、运行观测如何安全导出 | [网关 API 与客户端兼容](gateway-api-compatibility.md)                       |
| 扩展能力共同规则            | Embeddings 与 Native 媒体如何分层、保守编译、预检、保真和记录证据          | [扩展导航及共同规则](embedding-and-native-multimodal.md)                   |
| Embeddings                  | 向量输入、编码、维度、响应预算与 vector identity                            | [Embeddings 能力](embeddings.md)                                           |
| Native 图片                 | Chat/Responses 图片 source、media type、detail、limit 与 URL policy         | [图片能力](native-image.md)                                                |
| Native 文件                 | Chat/Responses 文件 source、encoding、filename、detail 与 resource identity | [文件能力](native-file.md)                                                 |
| Native 音频                 | 通用音频理解、ASR、TTS、音色条件、stream 与响应预算                         | [音频能力](native-audio.md)                                                |
| 模型与能力契约              | Public Model 如何公开、聚合能力、预检请求且不参与 Route 选择                | [Public Model 与模型能力契约](model-information-and-capability-contract.md) |
| 配置与凭证                  | 配置文件、private secret、API-key pool、header、网络和 reload 如何受信管理  | [配置、凭证与受信运行边界](configuration-and-credentials.md)                |
| ChatGPT subscription OAuth  | 独立 Provider、owned credential、PKCE、refresh 与数据面安全边界             | [ChatGPT subscription OAuth](upstream-oauth-credential-lifecycle.md)         |
| 路由与可用性                | 已接受请求如何按固定 Route 顺序执行有限 retry/fallback、cooldown 和状态亲和 | [路由与 Provider 韧性](provider-resilience.md)                              |
| 交付与证据                  | 如何以 TDD 和分层证据约束声明，并管理预发布破坏性变更                       | [交付与证据要求](delivery-and-evidence.md)                                  |

例外保留一份不属于当前产品承诺的[待定 Model/Provider 配置方案](model-catalog-configuration.md)。该方案暂不实施，
其中的候选字段和验收项不得作为当前功能需求或开发入口。

[ChatGPT subscription OAuth credential lifecycle](upstream-oauth-credential-lifecycle.md)已经实现独立 ChatGPT Provider、
OpenBridge-owned OAuth2 bundle、显式 PKCE 登录、到期驱动 refresh，以及四个固定 Responses-native Public Model 的受控借用和有界
`401` recovery；本机 Codex state 导入已排除。多账号池、动态 credential 控制面和其他应用的 auth cache 仍不属于产品承诺。

功能需求中的“必须”“不得”“只允许”是验收约束，不代表当前实现已经满足；代码、测试、probe 或真实运行已经证明的内容只写入
`../implementation-status/`。

需求变更先在本目录明确用户可观察的结果、失败语义、资源/安全边界与非目标；具体实现方案再写入 `../implementation-plans/`
。当目标或证据冲突时，以产品范围、对应功能需求和实施现状为准，而非历史设计假设。
