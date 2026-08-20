# 当前源码阅读指引

本页把产品合同、当前架构、源码入口和验证资产串成一条阅读路线。它只描述当前 checkout 的导航方式，不替代功能需求、实施状态或
测试结果；源码路径发生职责变化时应同步更新本页。

## 1. 先建立产品坐标

按顺序阅读：

1. [根 README](../../README.md)：安装、配置、启动和最小调用；
2. [功能需求](../functional-requirements/README.md)：产品行为、安全边界与非目标；
3. [实施现状](README.md)：当前代码已经实现和验证的范围；
4. [当前代码架构](current-architecture.md)：模块所有权、装配链和请求数据流；
5. [当前开发焦点](../implementation-plans/current-focus.md)：当前保留的实施优先级与焦点边界。

读完后应能区分：

- Public Model 与上游真实模型；
- Native Path 与 Protocol Bridge；
- 固定能力预检与 Route 执行顺序；
- 确定性测试、独立客户端、真实 Provider、负载和长期运行证据。

## 2. 启动与装配

先读[配置与凭证需求](../functional-requirements/configuration-credentials/README.md)，再查看：

| 顺序 | 源码 | 责任 |
|---:|---|---|
| 1 | [`src/main.rs`](../../src/main.rs) | 进程入口、配置加载、注册表构建、共享 client、Router 与关闭 |
| 2 | [`src/config/`](../../src/config) | Bootstrap 类型、严格解析和进程策略 |
| 3 | [`src/identity.rs`](../../src/identity.rs) | 私有下游用户与不可变用户注册表 |
| 4 | [`src/upstream_credentials.rs`](../../src/upstream_credentials.rs) | 私有上游 credential binding 解析 |
| 5 | [`src/oauth2_credentials/`](../../src/oauth2_credentials)、[`src/bin/openbridge-auth.rs`](../../src/bin/openbridge-auth.rs) | ChatGPT auth 文件、显式登录、refresh 与 manager snapshot |
| 6 | [`src/providers/catalog.rs`](../../src/providers/catalog.rs) | 可信 Provider、Target、Route 与 Public Model 装配 |
| 7 | [`src/registry/`](../../src/registry) | 完整图校验和不可变运行时 snapshot |
| 8 | [`src/ingress/router.rs`](../../src/ingress/router.rs) | 公开资源、认证边界和 handler wiring |

启动主线为：

```text
Bootstrap + users + upstream credential bindings
→ compiled Provider catalog
→ validated RuntimeRegistry + UserRegistry
→ CredentialStore + OAuth2CredentialManager
→ shared UpstreamClient
→ GatewayState
→ Axum Router
```

配置与注册表构建不执行普通业务 egress。业务请求不能动态指定 Provider、URL、credential、header policy 或 Route。

## 3. 沿一次请求阅读

建议从一个最小的非流式 `POST /v1/responses` 开始：

| 阶段 | 入口 | 阅读问题 |
|---|---|---|
| Router 与认证 | [`ingress::build_router`](../../src/ingress/router.rs) | 哪些资源公开，哪些需要 Bearer token？ |
| Body 生命周期 | [`src/ingress/lifecycle.rs`](../../src/ingress/lifecycle.rs) | 请求上限、重放预算和下游取消如何传播？ |
| 请求编排 | [`src/ingress/forwarding.rs`](../../src/ingress/forwarding.rs) | attempt、retry/fallback 与 commit point 如何连接？ |
| 事实提取 | [`src/pipeline/generation/analysis.rs`](../../src/pipeline/generation/analysis.rs) | Generation 请求携带了哪些 capability、limit 和 state 事实？ |
| 固定能力预检 | [`src/pipeline/generation/preflight.rs`](../../src/pipeline/generation/preflight.rs) | 为什么 Generation 不支持能力会在查看候选前失败？ |
| Route 计划 | [`src/pipeline/generation/planning.rs`](../../src/pipeline/generation/planning.rs) | 为什么 Generation 预检后仍保持静态候选顺序？ |
| 运行实体 | [`src/registry/runtime.rs`](../../src/registry/runtime.rs) | Public Model 如何关联 Target、API 与 Route？ |
| Provider 改写 | [`src/provider/adapter.rs`](../../src/provider/adapter.rs) | 相对 path、真实 model 和认证 header 在哪里产生？ |
| HTTP 发送 | [`src/transport/upstream.rs`](../../src/transport/upstream.rs) | endpoint、redirect、timeout 和连接池如何受控？ |
| 响应与错误 | [`src/ingress/response.rs`](../../src/ingress/response.rs) | safe headers、JSON/SSE、错误与 request id 如何返回？ |

压缩后的数据流：

```text
HTTP request
→ authenticate and bound body
→ analyze operation-specific requirements
→ preflight one fixed Public Model interface
→ build ordered RoutePlan
→ select Target + typed Upstream API
→ prepare trusted relative request and credential
→ send through shared transport
→ buffer bounded JSON or incrementally validate/render SSE under a per-event limit
```

## 4. 核心事实所有权

| 问题 | 文档入口 | 源码 owner |
|---|---|---|
| Canonical Model 事实 | [模型能力需求](../functional-requirements/model-capability/README.md) | [`src/models/`](../../src/models) |
| Public Model 固定接口 | 同上 | [`src/registry/public_model.rs`](../../src/registry/public_model.rs)及其子模块 |
| Provider 能力上界 | [网关 API 需求](../functional-requirements/gateway-api/README.md) | [`src/provider/`](../../src/provider)、[`src/providers/`](../../src/providers) |
| Target 与 Upstream API | [当前架构](current-architecture.md) | 静态注册在 [`src/providers/*/registration.rs`](../../src/providers)，已解析实体在 [`src/registry/runtime.rs`](../../src/registry/runtime.rs)；`definition.rs` 只定义配置类型 |
| Route ordering | [路由与韧性需求](../functional-requirements/routing-resilience/README.md) | [`src/providers/catalog/route_compiler.rs`](../../src/providers/catalog/route_compiler.rs)、[`src/pipeline/generation/planning.rs`](../../src/pipeline/generation/planning.rs)、[`src/pipeline/embeddings/planning.rs`](../../src/pipeline/embeddings/planning.rs) |
| Attempt 与 cooldown | 同上 | [`src/execution/coordinator.rs`](../../src/execution/coordinator.rs)、[`src/ingress/health.rs`](../../src/ingress/health.rs)、[`src/ingress/credential_health.rs`](../../src/ingress/credential_health.rs) |
| Bootstrap 与 secret | [配置与凭证需求](../functional-requirements/configuration-credentials/README.md) | config、identity、upstream credentials、credential stores |
| OTLP 与本地内容日志 | [观测需求](../functional-requirements/observability/README.md) | [`src/observability.rs`](../../src/observability.rs)及其子模块 |

需要记住的映射：

```text
Public Model
→ fixed operation interface
→ ordered Route candidates
→ Upstream Target
→ typed Upstream API
→ canonical Model + Provider contract
```

同一 Public Model 的全部固定候选共同参与能力交集；请求能力不会成为跳过、筛选或重排候选的理由。

## 5. Streaming、Bridge 与韧性

结合[网关 API 需求](../functional-requirements/gateway-api/README.md)和[路由与韧性需求](../functional-requirements/routing-resilience/README.md)，按顺序阅读：

1. [`src/transport/sse.rs`](../../src/transport/sse.rs)：SSE framing、UTF-8 与单 event 上限；
2. [`src/provider/adapter.rs`](../../src/provider/adapter.rs)和 [`src/providers/openai_compatible.rs`](../../src/providers/openai_compatible.rs)：Provider event 的 terminal/error 分类；
3. [`src/ingress/streaming.rs`](../../src/ingress/streaming.rs)：增量分类、首输出 commit point、EOF、body error 和取消；
4. [`src/bridge.rs`](../../src/bridge.rs)及 [`src/bridge/`](../../src/bridge)：Chat ↔ Responses 请求/响应转换；
5. [`src/execution/coordinator.rs`](../../src/execution/coordinator.rs)：请求级 attempt budget 与 capped backoff；
6. [`src/ingress/health.rs`](../../src/ingress/health.rs)及 [`src/ingress/credential_health.rs`](../../src/ingress/credential_health.rs)：Target 与 credential member cooldown；
7. [`src/provider/contracts.rs`](../../src/provider/contracts.rs)：safe/sensitive headers 与错误分类。

重点核对四个不变量：

- 第一个下游业务 body byte 写出后不能 retry、fallback 或拼接另一响应；
- EOF-before-terminal 不能伪造成成功 terminal；
- 下游取消必须终止对应上游工作；
- safe header 与 credential header 必须保持隔离。

## 6. Provider 与 Probe

添加或审计 Provider 时，先读 [`src/provider/`](../../src/provider)，再进入对应的 [`src/providers/`](../../src/providers) family：

1. `ProviderDefinition` 与 operation surface；
2. 固定 endpoint、credential kind 和错误边界；
3. canonical Model 与 Target/API registration；
4. Public Model source 与 Route surface；
5. Provider contract、boundary、forwarding 测试；
6. [Provider 当前状态](providers/README.md)与[外部 Provider 资料](../references/providers/README.md)。

管理员 probe 位于 [`src/probe.rs`](../../src/probe.rs)及其子模块。它复用固定 Target、adapter 和 transport，但不修改注册表，也不证明
tool、多模态、SDK/Agent、retry/fallback、负载或长期稳定性。

MCP 是独立本地服务：从 [`src/mcp/`](../../src/mcp)读取 dual-era transport、session 和 `hello` tool，再用
[`tests/mcp_contract.rs`](../../tests/mcp_contract.rs)与 [`tests/mcp_dual_era.rs`](../../tests/mcp_dual_era.rs)核对认证、Origin、stateless metadata
和 legacy lifecycle；它不进入 generation Provider pipeline。

## 7. 用测试理解证据边界

当前测试资产入口见 [test-assets](test-assets/)；常用源码包括：

- [`tests/config_contract.rs`](../../tests/config_contract.rs)：Bootstrap 与 registry 引用；
- [`tests/ingress_contract.rs`](../../tests/ingress_contract.rs)：认证、body、JSON 和 pre-egress 错误；
- [`tests/forwarding_contract.rs`](../../tests/forwarding_contract.rs)：实际 Router、transport、retry/fallback、取消和响应；
- [`tests/provider_contract.rs`](../../tests/provider_contract.rs)：Provider request/wire 合同；
- [`tests/sse_contract.rs`](../../tests/sse_contract.rs)：增量 SSE decoder、UTF-8 与单 event 上限；
- [`tests/provider_boundary_contract.rs`](../../tests/provider_boundary_contract.rs)、[`tests/forwarding_contract/resilience.rs`](../../tests/forwarding_contract/resilience.rs)、[`tests/bridge_forwarding_contract.rs`](../../tests/bridge_forwarding_contract.rs)与 [`tests/protocol_bridge_replay.rs`](../../tests/protocol_bridge_replay.rs)：terminal/EOF、commit、replay 与 Bridge 边界；
- [`tests/oauth2_login_cli.rs`](../../tests/oauth2_login_cli.rs)：显式 ChatGPT login CLI 边界；
- [`tests/mcp_contract.rs`](../../tests/mcp_contract.rs)与 [`tests/mcp_dual_era.rs`](../../tests/mcp_dual_era.rs)：MCP stateless/legacy lifecycle；
- [`testdata/`](../../testdata)：canonical wire oracle；
- [`tools/corpus/`](../../tools/corpus)：Python corpus/testkit。

测试文件存在不代表相应层已经运行。确定性 Rust、fixture、Python loopback、外部 SDK、目标 Agent、真实 Provider、负载和长期运行是
相互独立的证据层；实际记录见[证据目录](evidence/README.md)。

## 8. 按问题定位

| 问题 | 推荐路线 |
|---|---|
| 启动失败 | 配置需求 → `src/config` → catalog → registry validation → config/startup tests |
| 模型不可见 | active pool → Target/API → Route → Public Model compiler → Models tests |
| 请求 pre-egress 拒绝 | request analysis → Public Model interface → preflight → ingress tests |
| model/path/header 改写错误 | registry entity → Provider adapter → transport → provider tests |
| SSE 提前结束或重复 terminal | transport SSE → ingress streaming → Bridge renderer → SSE tests |
| retry/fallback 不符合预期 | RoutePlan → attempt manager → error classification → resilience tests |
| credential 泄露风险 | config/credential owner → adapter auth → header redaction → boundary tests |
| trace/metric 缺失 | observability owner → OTLP wiring → observability/OTLP contract tests |
| 外部协议差异 | 对应 reference snapshot → requirement owner → current status → focused test |

## 9. 开始修改前

1. 检查工作树，保留无关改动；
2. 核对[当前开发焦点](../implementation-plans/current-focus.md)和对应产品需求；
3. 明确一个可观察行为、失败语义、不做项和证据边界；
4. 先建立失败测试或最小复现，再做最小实现；
5. 更新对应功能状态页和实际验证记录；
6. 按改动面运行聚焦测试与仓库基线，并明确未运行的外部验收层。

源码、当前测试和实际运行结果高于历史说明；外部参考和旧验证记录只能提供定位线索。
