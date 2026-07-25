# 配置、凭证与受信运行边界

## 状态

**当前目标。** OpenBridge 是配置文件优先的单用户 headless 网关。本文件定义配置层、secret 解析、reload、安全 header 与部署信任边界；不规定 TOML 的最终字段布局或 credential adapter 的内部类型。当前原型仍以环境变量为基线，实际实现范围见[当前实现说明](../implementation-status/current-implementation.md)。

## 1. 配置所有权与优先级

服务所有者是唯一的配置所有者。客户端业务请求只能表达模型调用，不拥有 Provider、路由或 credential 选择权。

配置按用途分层：

| 层 | 建议位置 | 内容 | 版本控制与权限 |
|---|---|---|---|
| 基础启动配置 | `config/bootstrap.toml` | listener、body/SSE/resource 上限、受信网络与 telemetry 基础策略。 | 可提交；不得含 secret。 |
| 路由配置 | `config/routes.toml` | provider family、deployment、alias candidates、能力、timeout、固定 allowlist header 与 secret reference。 | 可提交；不得含 secret。 |
| 私有本地配置 | `config/local.toml` | 下游静态 Bearer token、上游 API key 或受支持 secret source 的本地值。 | 必须被忽略且仅服务所有者可读；不得进入测试 artifact、日志或提交。 |

运行时值的优先级必须明确且可在启动诊断中显示其**来源类别**（不显示值）：

1. 私有配置中为该字段配置的值或明确指向的 private secret；
2. 受信基础/路由配置中对该字段声明的非敏感默认或 secret reference；
3. 仅当配置显式使用 `env://NAME` 时读取的环境变量；
4. 没有值或 reference 时 fail closed。

环境变量不能按同名规则隐式覆盖任何配置文件值；它只是某些部署环境无法挂载私有文件时的显式适配器。CLI 参数只能选择受信配置文件位置或执行受限管理动作，不能直接注入 API key、上游 URL、任意 header 或路由 JSON。

## 2. 凭证要求

- 下游认证 token、Provider API key、OAuth refresh material、cookie 和其他 secret 必须只来自 private config 或其显式 secret reference；公共配置、Git、错误响应、metrics label、fixture 和普通日志不得包含其明文。
- credential 必须绑定到明确 deployment；同一请求选择 deployment 后不得在 retry、fallback 或 reload 中改用未获 RoutePlan 许可的 credential。
- secret 解析应尽可能短时、最小作用域。调用记录只可包含 credential source 类别或稳定的非秘密 binding id，不能含 secret 值、header、cookie 或可逆派生值。
- 启动、reload 或调用时发现 secret 缺失、空值、权限不符合要求或 reference 无效，应以安全错误拒绝相关服务/route；不得悄悄回退到同名环境变量或其他 deployment credential。
- OAuth 是可选 credential adapter，不是当前 API-key 路径的隐式替代。它不得借用 Codex/Hermes 本地登录状态、`auth.json`、cookie 或 client identity，也不引入账号池/轮换池。

## 3. 路由、header 与网络信任

- deployment 的 base URL、协议、真实模型、认证方式、timeout 与固定 header 只来自受信配置；endpoint 必须在加载时验证为允许的 absolute origin/path prefix，调用时只允许 adapter 生成的相对路径。
- 默认监听 loopback。若配置非 loopback listener，必须同时满足静态高熵下游 token 与 TLS/可信反向代理要求；不满足时应拒绝启动或业务访问。
- Provider redirect 默认关闭；代理不得接受业务请求提供的 URL、Authorization、cookie、proxy、callback、Host override 或任意 `x-*` 出站 header。
- 固定 header 必须是每个 adapter/profile 的最小 allowlist。`x-codex-turn-state` 仅可为已启用 Codex Native Responses profile 透明传递；其他私有 header 需要独立证据、方向、生命周期和安全审查，不能通过通配规则放行。
- 配置可以收紧安全、资源、retry 和 telemetry 边界，但不能把没有代码/fixture 证据的 protocol bridge 或 Provider capability 宣称为已支持。

## 4. 生命周期与 reload

| 行为 | 功能要求 |
|---|---|
| 启动 | 完整读取、合并并验证各配置层后才开始监听；错误消息说明配置位置/字段类别，不回显 secret。 |
| route reload | 构建并验证完整新 snapshot 后原子替换。失败时继续使用上一个有效 snapshot，不产生半更新状态。 |
| in-flight request | 始终持有开始时的 route/credential/config snapshot；reload 不改变其 alias、deployment、timeout、header 或 fallback 边界。 |
| bootstrap policy 变化 | listener、网络保护、全局资源限制等启动级策略不应通过普通 route reload 改变；需要受控重启或明确的生命周期操作。 |
| 私有配置变化 | 必须有明确的重新加载/重启语义与失败处理；不能因文件观察或环境变量变化在请求中途更换 secret。 |

配置校验应同时覆盖 schema、重复 id、alias candidate、provider/protocol 上界、URL/origin、header allowlist、资源限制、secret reference 和相互依赖的 feature。缺失、未知或矛盾配置默认 fail closed，而非猜测 Provider 行为。

## 5. Headless 管理与诊断

服务不提供 GUI 或客户端管理面，但服务所有者需要最小的本地运维能力：

- 启动/重载结果、配置版本、启用 deployment 数、私有配置是否成功解析等可通过受保护的本地日志或 CLI 查看；输出只能显示 id、来源类别和状态，不能显示 secret 或业务正文。
- 显式 probe 可使用已配置 deployment 与 credential 产生脱敏 report，但不得写回路由、自动改变 capability、扩展 public alias 或暴露为下游管理 API。
- 调用统计/telemetry 的 sink、保留与导出同样由配置定义，且不得由业务请求改变；详细要求见[调用统计与可观测性](observability.md)。

## 6. 功能验收要求

| ID | 应被保护的用户可观察行为 |
|---|---|
| CFG-01 | 受版本控制的配置可启动示例不包含 secret；`config/local.toml` 不被 Git 跟踪，且文档说明其权限要求。 |
| CFG-02 | 下游 token 与上游 API key 可从 private config 解析；环境变量只有在显式 `env://` reference 时才被读取。 |
| CFG-03 | 缺失/无效/不安全的 secret 或 network 配置会安全 fail closed，不回显值，也不回退到其他 deployment 或同名环境变量。 |
| CFG-04 | 业务请求无法覆盖 URL、真实模型、认证、credential、redirect、header allowlist 或路由策略。 |
| CFG-05 | 成功 reload 原子影响后续请求；失败 reload 和 in-flight 请求均保持原有效 snapshot。 |
| CFG-06 | private header 仅按 profile allowlist 保留；Codex turn state 不会进入日志、Bridge、跨 deployment fallback 或普通配置通配规则。 |
| CFG-07 | 非 loopback 部署缺少 token/TLS 信任前置条件时不能以“单用户”名义无保护暴露。 |

## 7. 非目标

- 多用户身份、client registration、动态下游 key 管理、配额、账单或审计；
- Vault/云 secret manager 的默认强依赖、任意 shell command secret provider 或自动 credential rotation；
- 从本地 Codex/Hermes 状态文件导入登录凭证；
- 热加载任意代码、路由脚本、HTTP transformer 或未经审查的 header；
- 用环境变量约定替代可审阅的配置来源与优先级。

## 关联文档

- [产品范围](product-scope.md)
- [网关 API 与客户端兼容](gateway-api-compatibility.md)
- [路由与 Provider 韧性](provider-resilience.md)
- [调用统计与可观测性](observability.md)
- [当前实现说明](../implementation-status/current-implementation.md)
- [配置与路由实施方案](../implementation-plans/configuration-and-routing.md)
