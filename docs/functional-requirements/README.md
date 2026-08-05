# 功能需求

本目录只定义当前有效的产品行为、失败语义、安全边界和交付证据要求，不记录“代码已经做到什么”或某次测试结果，也不预先固定模块拆分与实施顺序。功能需求是产品承诺的唯一入口；实施计划只能解释某项需求准备如何落地，不能反向扩大兼容承诺。

| 功能域                      | 当前需要回答的问题                                                          | 文档                                                                        |
|-----------------------------|-----------------------------------------------------------------------------|-----------------------------------------------------------------------------|
| 产品范围                    | 服务为谁解决什么问题，哪些能力不做                                          | [产品范围](product-scope.md)                                                |
| 网关 API 与兼容             | 客户端可调用什么、JSON/SSE/tool/continuation 如何表现                       | [网关 API 与客户端兼容](gateway-api-compatibility.md)                       |
| Embeddings 与 Native 多模态 | 现阶段两个扩展目标的输入、能力、资源与失败边界                              | [Embeddings 与 Native 多模态扩展](embedding-and-native-multimodal.md)       |
| 模型与能力契约              | Public Model 如何公开、聚合能力、预检请求且不参与 Route 选择                | [Public Model 与模型能力契约](model-information-and-capability-contract.md) |
| 配置与凭证                  | 配置文件、private secret、API-key pool、header、网络和 reload 如何受信管理  | [配置、凭证与受信运行边界](configuration-and-credentials.md)                |
| 路由与可用性                | 已接受请求如何按固定 Route 顺序执行有限 retry/fallback、cooldown 和状态亲和 | [路由与 Provider 韧性](provider-resilience.md)                              |
| 交付与证据                  | 如何以 TDD、fixture、SDK/独立客户端和真实环境证据约束兼容声明               | [交付与证据要求](delivery-and-evidence.md)                                  |

例外保留一份不属于当前产品承诺的[待定 Model/Provider 配置方案](model-catalog-configuration.md)。该方案暂不实施，
其中的候选字段和验收项不得作为当前功能需求或开发入口。

同样保留一份不属于当前产品承诺的[上游 OAuth credential lifecycle 条件性安全边界](upstream-oauth-credential-lifecycle.md)
。OAuth 仍是当前非目标；该文档只约束未来在获得 Provider 正式授权并重新获准实施后的最低安全行为。

功能需求中的“必须”“不得”“只允许”是验收约束，不代表当前实现已经满足；代码、测试、probe 或真实运行已经证明的内容只写入
`../implementation-status/`。

需求变更先在本目录明确用户可观察的结果、失败语义、资源/安全边界与非目标；具体实现方案再写入 `../implementation-plans/`
。当目标或证据冲突时，以产品范围、对应功能需求和实施现状为准，而非历史设计假设。
