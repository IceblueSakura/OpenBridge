# OpenBridge 文档与源码阅读指引

本页既是文档总索引，也是从产品边界逐步进入架构与实现的阅读手册。OpenBridge 当前是实验性的
Rust/Axum、headless、OpenAI-compatible 多 Provider 网关；阅读时应以当前 checkout、实施现状、测试和源码
为准，不把历史方案、外部参考项目或未实现类型当成现有能力。

## 1. 先选择阅读路线

| 目标 | 建议用时 | 阅读范围 | 完成标志 |
|---|---:|---|---|
| 快速了解项目 | 30 分钟 | 根 README → 产品范围 → 当前实现说明 → 当前代码架构 | 能解释项目解决什么问题、当前只支持什么路径、明确不做什么 |
| 看懂一次请求 | 2～3 小时 | 快速路线 + 启动装配 + Ingress → Pipeline → Provider → Transport | 能从 endpoint 追到上游请求，再追到下游响应 |
| 准备修改代码 | 半天以上 | 请求调用链 + 对应功能需求 + 契约测试 + 当前开发焦点 | 能指出行为需求、失败测试、最小改动面和验证边界 |
| 深挖协议或 Provider | 按专题 | 对应实现模块 + OpenAI 协议参考 + corpus/testkit | 能区分项目事实、外部协议事实和仍待真实验证的结论 |

第一次阅读推荐严格按第 3～9 节顺序进行。已经有明确问题时，可直接使用第 10 节的专题路线。

## 2. 四类文档分别回答什么

| 分类 | 回答的问题 | 入口 |
|---|---|---|
| [功能需求](functional-requirements/README.md) | 产品应保持什么用户行为、安全边界和非目标 | 产品目标与验收依据 |
| [实施现状](implementation-status/README.md) | 当前代码和已执行验证真正证明了什么 | 已实现事实与未验证边界 |
| [实施计划](implementation-plans/README.md) | 当前是否有一个获准实施的短周期行为 | 单一 TDD 开发焦点 |
| [参考文档](references/README.md) | 外部协议、SDK、客户端和参考项目提供了什么事实 | 研究证据，不自动构成功能承诺 |

阅读时保持三个区分：功能需求不等于已经实现；测试通过不等于真实 Provider 或 Agent 已兼容；参考项目的
做法不等于 OpenBridge 应照搬。

## 3. 第一阶段：建立产品坐标

按顺序阅读：

1. [根 README](../README.md)：了解项目定位、运行入口、当前 Native Path、验证基线和非目标。
2. [产品范围](functional-requirements/product-scope.md)：确认服务对象、部署边界和不属于本项目的问题。
3. [当前实现说明](implementation-status/current-implementation.md)：把“目标”与“当前代码事实”分开。
4. [当前代码架构](implementation-status/current-architecture.md)：先看分层图、关键词汇和“尚未实现”。

这一阶段暂时不要钻进具体函数。读完后应能回答：

- 下游客户端看到的是 Public Model，还是上游真实模型？
- 当前是否已经实现 Chat ↔ Responses Protocol Bridge？
- Provider、Upstream Target、Upstream API、Route 和 Public Model 分别拥有哪类事实？
- 哪些结论只由 mock/fixture 证明，哪些仍需要 SDK、独立 Python/curl、目标 Agent 客户端或真实 Provider 验证？

## 4. 第二阶段：看懂启动与装配

先读[配置、凭证与受信运行边界](functional-requirements/configuration-and-credentials.md)，再按以下顺序进入源码：

| 顺序 | 文件 | 重点 |
|---:|---|---|
| 1 | [`src/main.rs`](../src/main.rs) | 进程入口、配置加载、注册表构建、共享 HTTP client、Router 和优雅关闭 |
| 2 | [`src/config/mod.rs`](../src/config/mod.rs)、[`parser.rs`](../src/config/parser.rs) | Bootstrap 基础定义、TOML 解析和边界校验 |
| 3 | [`src/config/source.rs`](../src/config/source.rs) | bootstrap 文件定位、可选 dotenv 加载和错误边界 |
| 4 | [`src/identity.rs`](../src/identity.rs) | 私有用户文件、下游 API Key 匹配和不可变 `UserRegistry` |
| 5 | [`src/providers/catalog.rs`](../src/providers/catalog.rs)、[`catalog/`](../src/providers/catalog) | 编译期 Provider、模型、target、Route 与 Public Model 装配 |
| 6 | [`src/registry/compiler.rs`](../src/registry/compiler.rs)、[`validation.rs`](../src/registry/validation.rs) | 校验 `RegistryConfig` 并生成不可变 `RuntimeRegistry` |

把启动链记成一条线即可：

```text
bootstrap + users + environment credential locators
→ compiled provider registry + startup CredentialStore
→ immutable RuntimeRegistry + UserRegistry + CredentialStore
→ shared UpstreamClient
→ GatewayState
→ Axum Router
```

配套测试先看 [`tests/config_contract.rs`](../tests/config_contract.rs)、
[`tests/downstream_auth_contract.rs`](../tests/downstream_auth_contract.rs) 和
[`tests/example_config.rs`](../tests/example_config.rs)。它们比直接通读 `registry/compiler.rs` 更容易说明哪些校验是契约。

读完后应能回答：为什么业务请求不能动态指定上游 URL、credential、Provider 或 route？为什么用户文件和注册表
变更需要重启？

## 5. 第三阶段：沿一次请求追完整调用链

建议选择一个最简单的 `POST /v1/responses` 非流式请求，从下表自上而下阅读；理解后再换成 streaming 请求。

| 调用阶段 | 代码入口 | 阅读问题 |
|---|---|---|
| Router 与 endpoint | [`ingress::build_router`](../src/ingress/router.rs) | 哪些 endpoint 公开？哪些需要下游认证？ |
| HTTP 基础检查 | `require_user`、`responses`、`has_json_content_type` | 认证、Content-Type 和 body 上限在哪次 egress 前完成？ |
| 请求编排 | [`ingress::forward_request`](../src/ingress/forwarding.rs) | 请求规划、候选循环、retry/fallback 和响应返回如何连接？ |
| 请求事实提取 | [`pipeline::analyze_request`](../src/pipeline/analysis.rs) | 从 JSON 中提取了哪些 capability、limit、reasoning 和 state-affinity 事实？ |
| 路由规划 | [`pipeline::plan_request`](../src/pipeline/planning.rs) | 一条 candidate 为什么必须独立满足完整请求？ |
| 运行事实查询 | [`RuntimeRegistry`](../src/registry/runtime.rs) | Public Model 如何落到 Route、Target 与 Upstream API？ |
| Provider 改写 | [`ProviderAdapter::prepare_request`](../src/provider/adapter.rs) | 上游相对 path、真实 model、普通 header 与认证 header 在哪里产生？ |
| HTTP 发送 | [`UpstreamClient::send`](../src/transport/upstream.rs) | endpoint base、相对 URI、timeout、redirect 和连接复用如何受控？ |
| 响应处理 | `ingress::upstream_response` | status、safe response headers、JSON/SSE body 如何返回下游？ |
| 错误归一 | `route_error`、`upstream_error` | 哪些错误在本地生成，哪些来自上游，哪些信息不得泄露？ |

主链路可以压缩为：

```text
HTTP request
→ authenticate and bound body
→ analyze RequestRequirements
→ plan ordered RouteCandidates
→ select Target + Upstream API + ProviderAdapter
→ prepare relative request and sensitive auth
→ UpstreamClient.send
→ preserve JSON/SSE response within safe header and terminal rules
```

配套阅读 [`tests/native_routing_contract.rs`](../tests/native_routing_contract.rs) 和
[`tests/forwarding_contract.rs`](../tests/forwarding_contract.rs)。前者回答“为什么选择或拒绝 route”，后者回答
“实际转发、fallback、错误、取消和响应是什么”。

## 6. 第四阶段：理解核心数据所有权

这一阶段解决最容易混淆的配置与运行实体问题。

| 问题 | 先读文档 | 再读源码 |
|---|---|---|
| 模型事实放在哪里 | [当前代码架构第 3 节](implementation-status/current-architecture.md#3-注册表层) | [`src/models/`](../src/models)、`ModelConfig`、`ModelInfo` |
| Provider 能力上界是谁定义 | [网关 API 与兼容](functional-requirements/gateway-api-compatibility.md) | [`src/provider/kind.rs`](../src/provider/kind.rs)、[`src/providers/`](../src/providers) |
| target 与 upstream API 为什么分开 | [当前代码架构](implementation-status/current-architecture.md) | `UpstreamTargetConfig`、`UpstreamApiConfig` |
| Public Model 如何选择候选 | [路由与 Provider 韧性](functional-requirements/provider-resilience.md) | `PublicModelConfig`、`RouteConfig`、`plan_request` |
| capability 为什么只能收窄 | [配置与凭证边界](functional-requirements/configuration-and-credentials.md) | [`src/core/capability.rs`](../src/core/capability.rs)、`build_registry` |

建议自己画一条具体映射：

```text
public model name
→ ordered route ids
→ route(downstream protocol + mode)
→ upstream target(endpoint + credential + timeout)
→ upstream API(protocol + upstream model + capabilities + state affinity)
→ canonical model facts
```

如果不能指出每个字段属于哪一层，就先不要修改 registry 或 Provider 配置。

## 7. 第五阶段：专门阅读 Streaming、retry 与安全边界

这部分是当前实现中最需要结合测试阅读的区域：

1. 读[网关 API 与兼容需求第 4、6 节](functional-requirements/gateway-api-compatibility.md)。
2. 读[路由与 Provider 韧性](functional-requirements/provider-resilience.md)。
3. 读 [`src/transport/sse.rs`](../src/transport/sse.rs)：SSE framing、UTF-8、event 大小和 terminal 观察。
4. 读 [`ingress/forwarding.rs`](../src/ingress/forwarding.rs) 与 [`ingress/streaming.rs`](../src/ingress/streaming.rs)：首输出 commit point、EOF、取消与
   retry/fallback 边界。
5. 读 [`src/provider/contracts.rs`](../src/provider/contracts.rs)：safe/sensitive headers、status 分类与 retry hint。
6. 用 [`tests/sse_contract.rs`](../tests/sse_contract.rs)、`forwarding_contract.rs` 中的 streaming cases 逐条反证。

必须能解释以下不变量：

- `output_item.done` 不等于 Responses terminal；EOF-before-terminal 不能伪造成成功。
- 第一个下游业务 body byte 写出后，不能 fallback、retry 或拼接另一上游响应。
- 下游取消必须终止对应上游工作。
- safe headers 与 credential headers 必须分离；业务请求不能控制任意 egress header。

## 8. 第六阶段：理解 Provider 与 Probe

添加或审计 Provider 时，按以下顺序阅读：

1. [`src/provider/kind.rs`](../src/provider/kind.rs) 与 [`src/provider/adapter.rs`](../src/provider/adapter.rs)：闭合 `ProviderKind`、`ProviderContract` 与 `ProviderAdapter`。
2. [`src/providers/openai_compatible.rs`](../src/providers/openai_compatible.rs)：OpenAI-compatible 请求、认证、SSE、错误与 API pair 共享机制。
3. [`src/providers/openai/`](../src/providers/openai) 与 [`src/providers/longcat/`](../src/providers/longcat)：已接入 Provider 如何独立拥有 contract、endpoint path、request-header hook 与注册事实。
4. [`src/providers/deepseek/`](../src/providers/deepseek) 与 [`src/providers/mimo/`](../src/providers/mimo)：尚未接入 registry 的静态 Provider 定义及其协议边界。
5. [`tests/provider_contract.rs`](../tests/provider_contract.rs) 与
   [`tests/provider_boundary_contract.rs`](../tests/provider_boundary_contract.rs)：相对 URI、认证隔离、能力上界和错误分类。
6. [能力探测实施现状](implementation-status/capability-probing.md)、[`src/probe.rs`](../src/probe.rs) 与
   [`src/bin/openbridge-probe.rs`](../src/bin/openbridge-probe.rs)：probe 如何复用受信 target，同时不修改注册表。

注意：当前 OpenAI 与 LongCat 都走 OpenAI-compatible Native Path；这不证明异构 Provider 或 Protocol Bridge
已经实现。

## 9. 第七阶段：用测试理解“已经证明什么”

先读 [TDD 与证据要求](functional-requirements/delivery-and-evidence.md)，再使用下表定位证据：

| 测试资产 | 主要保护内容 | 不证明什么 |
|---|---|---|
| `tests/config_contract.rs` | bootstrap、registry 引用、能力收窄、endpoint 与 credential locator | 真实网络或 Provider 可用性 |
| `tests/native_routing_contract.rs` | 请求事实、capability gate、route 候选和 state affinity | HTTP/SSE 实际发送 |
| `tests/forwarding_contract.rs` | Ingress 到 transport 的 JSON/SSE、fallback、timeout、取消和 header 行为 | 外部 SDK 或真实 Provider 兼容 |
| `tests/provider*_contract.rs` | Provider 请求、认证、能力和错误边界 | 全部 Provider 私有扩展 |
| `tests/sdk_compatibility.rs` | 当前 OpenAI Python/Node SDK 的 loopback 兼容路径 | 默认测试不会执行；需要显式 ignored run |
| [`testdata/`](../testdata/README.md) | canonical Chat/Responses/SSE/tool/error corpus | 任一 case 已经过 OpenBridge runtime |
| [`tools/corpus/`](../tools/corpus/README.md) | Python corpus 管理、Mock Client/Server 与单 case observation 判定 | 自动启动 SUT、多 attempt runner、真实 Agent/Provider |

已执行结果、版本和未接入边界以[实施现状](implementation-status/README.md)为准，不要只根据测试文件存在就宣称
某层已经验证。

## 10. 按问题选择专题路线

| 你要解决的问题 | 阅读路线 |
|---|---|
| 启动失败或配置被拒绝 | 配置需求 → `src/config` → `src/providers/catalog.rs` → `build_registry` → `config_contract.rs` |
| 请求为何没有 route | API 兼容需求 → `analyze_request` → `plan_request` → `native_routing_contract.rs` |
| 模型名或 endpoint 改写错误 | registry ownership → Provider adapter → `UpstreamClient::send` → provider tests |
| SSE 提前结束、重复 terminal 或乱码 | Responses/Chat 协议参考 → `transport/sse.rs` → `ingress/streaming.rs` → SSE/forwarding tests |
| fallback 或 retry 不符合预期 | Provider 韧性需求 → `ingress/forwarding.rs` → status/error classification → forwarding tests |
| credential/header 泄露风险 | 配置与凭证需求 → `identity.rs` → `provider/contracts.rs` → provider boundary tests |
| 新增 Provider | Provider contract → canonical model → compiled registry → adapter → probe → contract tests |
| 扩充协议测试 | [Corpus 指南](../testdata/README.md) → [Testkit 指南](../tools/corpus/README.md) → Python tests |

只有需要核验外部协议或比较实现取舍时，才进入[参考文档](references/README.md)：

- [Chat Completions 协议](references/openai/chat-completions-protocol.md)
- [Responses 协议](references/openai/responses-protocol.md)
- [参考项目比较矩阵](references/project-comparison.md)
- [Chat/Responses、SSE 与工具测试集调研](references/cross-project/chat-responses-sse-tool-test-suite-survey.md)

## 11. 推荐的阅读练习

完成主路线后，用三个小练习检查理解，而不是继续无目的通读：

1. 选一个非流式 Responses 请求，写出从 Public Model 到 upstream model 的完整对象链，并指出 model 改写位置。
2. 选 `eof_before_terminal_does_not_fabricate_a_terminal_event`，从测试反向追到 SSE decoder 和响应关闭路径。
3. 选一个 unsupported capability，请说明它在哪层被发现、为什么没有 egress、下游收到哪类稳定错误。

阅读笔记建议固定记录四项：观察到的当前事实、对应需求、代码/测试证据、尚未验证的边界。这样可以避免把
计划、推论或外部参考误写成当前实现。

## 12. 开始修改代码前

1. 检查工作树和当前源码，不覆盖无关改动。
2. 核对[当前开发焦点](implementation-plans/current-focus.md)是否为空或与任务一致。
3. 从功能需求或已知缺陷选择一个可观察行为。
4. 先写失败测试，再做最小实现；完成后更新实施现状并清空当前焦点。
5. 按改动面运行 Rust 基线；修改 `testdata/` 或 `tools/corpus/` 时追加 Python corpus/testkit 基线。
6. 明确区分静态检查、确定性测试、SDK/独立客户端、目标 Agent、真实 Provider、负载与长期运行证据。

## 13. 文档维护规则

- 产品行为、边界或非目标变化：更新 `functional-requirements/`；
- 已实现行为或已完成验证变化：更新 `implementation-status/`；
- 下一个功能获准实施：只更新 `implementation-plans/current-focus.md`，完成后恢复为空焦点；
- 外部协议、SDK、目标客户端或参考项目事实变化：更新 `references/`，并按影响同步前述文档；
- 不保留远期设计、阶段路线图、目标变迁或淘汰方案；需要实施时再从当前源码建立焦点。
