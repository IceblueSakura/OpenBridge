# OpenBridge 基础目标

## 状态

**Working scope。** 本文只定义当前需要稳定保持的产品目标、边界和非目标。它不定义开发阶段、实现顺序、全局验收清单或完成日期；当前代码已经证明的行为以[当前实现说明](../implementation-status/current-implementation.md)为准。

## 产品目标

OpenBridge 是由单个用户管理、以单个服务部署的 headless Provider 聚合网关。它让本地正在使用的 Agent 客户端通过一个稳定的 OpenAI-compatible HTTP 地址访问受信配置中的上游 Provider，而不暴露上游 credential、URL 或路由细节；服务本身不提供 GUI、Web 控制台或客户端管理功能。

初期希望持续实现和验证的用户结果是：

- 客户端可以使用稳定的 Public Model 调用 `POST /v1/responses` 或 `POST /v1/chat/completions`，并获得 HTTP JSON 或 SSE 响应；主要调用路径不要求客户端知道上游 Provider 实际支持的协议或能力差异；
- 服务所有者能在显式 Rust 注册表中管理真实模型、Provider、Upstream Target、credential binding、Public Model、原生/桥接 route 和候选顺序；
- 服务所有者通过显式 credential binding 与受限 secret source 管理上游 credential 和静态下游 Bearer token；secret 不进入代码注册表、版本控制或日志；
- 下游和上游协议一致时，代理优先做最小改写的原生转发，尽量保留未知但合法的 wire 字段与流式语义；
- 协议不一致时，代理按受信配置选择已实现且已验证的受限转换；转换不声明天然无损，无法按配置安全表达时应拒绝或给出清楚错误；
- Public Model 的能力由至少一条完整原生/桥接执行路径能否满足当前请求决定，不能把不同 Provider 的独立能力字段简单求并集；
- tool call、tool result、流结束、取消和必要的 continuation 信息在已支持的路径中保持可预测；
- 多个候选上游并存时，路由保持确定性，并能在安全边界内处理临时不可用、限流和最终错误；
- 服务所有者能通过无界面的、本地受保护的输出查看调用量、上游报告的 token usage、流式首输出时间和按稳定错误类别聚合的终态错误率；
- 下游客户端只需要 OpenBridge 地址和可选静态 token，不需要获得上游 credential。

首要日常互操作对象是 OpenAI SDK 和 Codex CLI。Hermes 是可选目标：只有明确作出 Hermes 兼容声明时，才为相应行为补充验证。

## 部署与信任边界

- 单用户、单配置所有者、单进程/单服务是默认模型；不建立 tenant、team、principal、成员或客户端管理模型。
- 服务所有者通过受信配置和 headless CLI 管理网关；不提供面向 Agent 客户端的注册、配置下发、状态管理或图形化界面。
- 本地默认监听 loopback。非 loopback 部署应使用 HTTPS 或可信反向代理，并使用至少一个静态高熵 Bearer token；未满足时拒绝启动或拒绝业务请求。
- Provider endpoint、认证、credential reference、静态下游 Bearer token 和允许的固定 header 只来自受信配置；业务请求不能覆盖这些值。
- 配置文件是运行时值和 credential 的首要来源：受版本控制的基础配置不含 secret，当前用户可读的私有配置保存实际密钥或其引用；`env://` 只在配置明确选择时作为迁移/部署兼容来源，不能无提示地覆盖配置值。
- 普通配置、日志、调用统计和测试证据不得保存明文密钥、cookie 或私人 prompt。调用统计不记录请求/响应正文或 tool 参数。
- 一次请求选择的 Upstream Target、Upstream API、协议模式和 fallback 边界应在请求生命周期内稳定；配置更新只影响后续请求。

## 初始接口边界

| 接口 | 用途 |
|---|---|
| `POST /v1/responses` | Codex custom Provider 的首要 HTTP/SSE 入口。Responses WebSocket 不包含在初期兼容承诺中。 |
| `POST /v1/chat/completions` | OpenAI-compatible Chat 客户端的入口。 |
| `GET /v1/models` | 返回服务所有者配置的 Public Models。 |

初期 Provider 形态聚焦 OpenAI Responses 原生上游和 generic OpenAI-compatible Chat 上游。一个 Public Model 可以映射到有序 routes；每条 route 固定 Upstream Target/Upstream API、下游协议和 Native/Bridge 模式，上游协议由 Upstream API 确定。候选不隐含完全等价，仍须按整个请求的能力组合、上下文、状态亲和和可用性筛选。

## 明确非目标

- 多租户、团队成员、principal/ACL、下游用户配额、计费、合规审计或独立控制面；
- 同一 Provider 的多账号池、credential 轮换池或账号级负载均衡；
- OpenAI 全部资源 API、Realtime、Files、Conversations 或管理 API；
- 初期 Responses WebSocket transport；
- 将 Chat 与 Responses 转换承诺为无损，或让转换路径静默丢弃无法表达的语义；
- 让业务请求动态指定任意上游 URL、认证 header、credential 或转换脚本；
- 让 OpenBridge 执行 Agent 返回的通用 function tool；协议转换只处理 wire-level tool call/result；
- GUI、Web 控制台、客户端注册/配置管理，或作为独立客户端管理服务；
- 以调用统计替代多租户账单、用户配额、合规审计，或记录完整 prompt/completion/tool payload；
- 用一次 mock、SDK 或 CLI 成功运行推断所有 Provider、模型、工具循环或长时间运行场景均已兼容。

## 后续方向，不构成当前计划

以下方向可以按实际用户价值和测试发现单独选择，但没有预定义顺序、阶段门或交付承诺：

- 多 Provider 聚合下的 capability、session affinity、cooldown、有限重试和错误传播；
- 扩大 Chat ↔ Responses 受限 Protocol Bridge 的已验证语义覆盖；
- Provider-hosted tool facade 与 Anthropic Messages 协议兼容。两者同为后续方向；
- 本地/MCP Tool Bridge、headless 的健康观测、OAuth credential adapter 和更多路由策略。

各方向的技术假设可见架构、设计和研究文档，但这些材料不替代本文，也不自动形成待办。

## 术语

- **Provider Family**：代码中实现的一类协议与认证行为，例如 `openai`、`openai-compatible`、`anthropic`。
- **Model**：注册表中与具体供应商调用方式分离的真实模型身份及内在事实；同名但 revision、tokenizer 或语义不同的供应应使用不同身份。
- **Upstream Target**：受信配置中的真实上游调用边界，绑定 Provider Family、base URL、credential reference、真实模型以及共享 quota/fault/state scope；它可以包含多个协议级 Upstream API。
- **Upstream API**：Upstream Target 下的一条原生协议供应，分别记录上游协议、model id、transport、served limits、能力证据和 state affinity；同一 Upstream Target 可同时包含 Chat 与 Responses Upstream API。
- **Public Model**：客户端使用的稳定服务模型名，映射到有序的完整 routes；它是 OpenBridge 的下游服务契约，不等同于任一 Provider 的模型名或能力声明。
- **Serving route / Execution Plan**：固定 Public Model、Upstream Target/Upstream API、上下游协议、Native/Bridge 模式、转换约束、credential binding 和 fallback 边界的可执行路径。
- **Native path**：下游与上游协议一致时的最小改写转发路径。
- **Protocol Bridge**：仅在协议不一致时执行的受限语义转换。
- **Hosted Tool Facade**：将 Provider 原生托管工具规范化为独立工具接口；它不等同普通 function tool。
