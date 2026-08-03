# Bootstrap、代码注册表、凭证与受信边界

## 状态

**当前约束。** OpenBridge 是单配置所有者管理的 headless 网关。Provider contract、Model、
Upstream Target、Upstream API、Route、Public Model、endpoint、能力和字段转换由 Rust 代码显式注册；运行时配置不提供 Provider DSL，也不支持
route 热重载。

## 1. 所有权划分

| 来源 | 内容 | 能否包含 secret |
|---|---|---|
| `config/bootstrap.toml` | loopback listener、两份私有 credential 文件位置、body/SSE 上限、共享 HTTP client 参数 | 否 |
| 被忽略的 `config/users.toml` | 下游用户、API Key 与启停状态 | 是 |
| 被忽略的 `config/upstream-credentials.toml` | 编译期 credential pool id 与有序上游 API key | 是 |
| `src/models/*` | Model 事实、token 限制、参数和 reasoning | 否 |
| `src/providers/*` | Provider 定义、共享协议机制、request-header hook、target/upstream API、endpoint、credential pool/binding、route 与 Public Model | 否 |
| 下游业务请求 | Public Model 和模型调用参数 | 否；也不能选择 endpoint/credential |

每个运行配置都有同名 `.example` 模板：`config/bootstrap.example.toml`、`config/users.example.toml` 和
`config/upstream-credentials.example.toml`。模板不得包含真实凭证；Bootstrap 模板由测试约束为与默认配置一致。

当前只允许 `OPENBRIDGE_CONFIG` 改变 bootstrap 文件位置；两份私有 credential 文件位置由 bootstrap 固定。不存在
`OPENBRIDGE_ROUTES_CONFIG`，CLI 也不能注入 Provider、URL、header、model id 或转换规则。

## 2. 代码注册表要求

- 每个具体 Provider 由 `src/providers/<provider>.rs` 根模块聚合，并在同名目录内分别拥有静态 definition 与可选 registration；具体 Provider 不使用 `mod.rs`，同一 wire family 的协议机制可以由闭合的编译期模块共享；
- 静态 Provider definition 不自动构成运行链路；只有显式加入 compiled target、Route 与 Public Model 后才可被请求选择；
- 同一模型家族由一个 `src/models/<family>.rs` 根模块聚合；`src/models/<family>/` 下每个扁平叶模块只定义一个具体 Model，并以版本、checkpoint 或命名变体组成模块名；每个具体 Model 仍完整声明自身事实；
- `src/providers/catalog.rs::compiled_config()` 是唯一显式注册入口；
- 不使用运行时插件、链接器自动注册、JSON/TOML 转换模板或脚本；
- Provider contract 定义 adapter 能力上界、endpoint profile 和 credential kind；
- Model 定义模型事实、token 限制、支持参数、reasoning 状态与 reasoning level；
- Credential Pool 绑定 Provider、credential kind 和一个有序 API-key member 集合；多个同 Provider Target
  可引用同一个 pool，但不能跨 Provider 或 credential kind 复用；
- Upstream Target 绑定 Provider、Model、endpoint、credential pool、timeout 和共享故障边界；
- Upstream API 独立声明一个协议的 upstream model、served limit、能力、state affinity，以及可选的 canonical
  reasoning level 到安全上游 wire 值的显式映射；
- 同一 Public Model 可以显式列出多个 Provider route source；相同 canonical Model ID 本身不得触发自动发现、
  隐式 Route 注册或 Provider 聚合；
- Public Model 保存由这些 source 生成的有序完整 Route；对每个下游协议，代码目录先按 source 声明顺序排列
  Native Route，再按相同顺序排列 Bridge Route；
- 启动监听前必须完成唯一性、引用、能力、reasoning、credential pool 和 URL 校验。

修改 Provider、Model 或路由必须重新编译并重启。项目不要求热重载。

## 3. 凭证

- 下游用户表只在启动时读取；用户增删、启停和 API Key 轮换都需要重启；
- 用户 ID 和 API Key 必须唯一，至少有一个启用用户，API Key 不得少于 32 bytes；
- 认证成功后只把不含 Key 的 `Arc<User>` 放入请求上下文；
- 代码注册表只保存非敏感 pool/member id、Provider 和 credential kind，不保存 secret 或 secret locator；
- 服务与 probe 只从 bootstrap 指定的私有 upstream credential TOML 读取上游 API key，不读取 `*_API_KEYS`、旧单值环境变量或 `.env`；
- TOML 只允许声明 `schema_version` 与 `credential_pools`；每个 pool 只包含编译期 pool id 和有序 `api_keys` 数组，不能配置 Provider、credential kind、endpoint、route 或 member id；
- 未由代码注册的 pool、重复 pool、服务所需或 probe 选中但缺失的 pool、空数组、空白成员或 pool 内重复 secret 必须在 listener 绑定或网络 probe 前失败；
- 服务在监听前把已启用用户 Key 与所有已启用 Upstream Target 引用的 pool 一次性装入不可变 `CredentialStore`；
- `CredentialId` 必须区分 `DownstreamUser` 与带 `ProviderKind` 的 `UpstreamPoolMember`，上下游同名 ID 不得造成命名冲突；
- 每个 Store 条目必须冻结受控的 credential type、source、从 1 开始的 generation 与可选过期时间；source 只保存
  `UserConfiguration`、`UpstreamConfiguration` 或 `Programmatic` 类别，不能把文件路径、issuer URL
  或任意业务字符串作为诊断元数据；
- `RuntimeRegistry` 与 `UserRegistry` 不保存 secret；`CredentialStore`、两类注册表、日志、错误响应和 probe report 的 Debug/输出都不得包含 secret；
- 下游认证只能经 Store 的 constant-time 匹配返回用户 ID；上游只能按完整
  `pool_id + member_id + ProviderKind + CredentialKind` 借用短时 credential 视图，不提供通用明文查询；
- 缺失、空值、零 generation、重复下游 Key 或 binding/Provider/credential kind 不匹配时 fail closed；服务所需的上游 Key 缺失或为空时在监听前失败；
- 运行时不得重新读取 `users.toml` 或 `upstream-credentials.toml`；改变任何 Key 必须重启，不支持热更新；
- 业务请求不能提供或覆盖 Authorization、cookie、Host、proxy header 或上游 credential；Provider 的受信代码 hook
  可按编译期规则增添、替换、转换或删除普通 header，共享层不维护普通 header allowlist。具体 Provider 的
  header policy 属于实现事实，不应在本需求文档中固化。

### 3.1 上游 API-key pool

- pool 与 member 都使用稳定、非敏感 ID；member secret 只来自私有 upstream credential TOML，业务请求
  不能提供 pool/member、改变顺序或扩大候选集合；member ID 只能由 pool id 与数组顺序派生，不能
  由 secret 内容派生；
- 一个 pool 至少包含一个 member；member ID 必须唯一，所有 member 必须属于同一 Provider 和 credential
  kind，重复 secret 必须拒绝；单 member pool 与现有单 key 行为等价；
- 同一个 pool 可由同 Provider 的多个 Target 引用，使 key cooldown 与 round-robin cursor 跨模型共享；不得
  为每个模型复制同一组 key 后形成互不知晓的健康状态；
- 每个 pool 只有一个 TOML `api_keys` 数组；未知、缺失或重复 pool、空 pool、空白或重复 member 必须在
  listener 绑定前 fail closed。本阶段不提供环境变量 fallback、member 级 enabled 或热增删；
- `CredentialStore` 继续不可变地持有 secret。运行时可变状态只保存 pool cursor、member binding ID、
  generation 与 cooldown deadline，不保存、复制或重新读取 secret；
- pool 选择只返回短时 credential 借用视图；每次 attempt 必须重新构造敏感认证 header，不能缓存或复用
  上一次 member 的 header；
- `previous_response_id` 等 `TargetBound` Upstream API 在没有 credential affinity 证据或 ledger 时不得引用
  多 member pool，避免 continuation 在不同账号/key 间漂移；
- 更换 secret、改变 pool member 或顺序仍需重启。当前范围不包含 OAuth、余额查询、keyring、加密 secret
  文件、远程 secret manager、动态 reload 或跨进程 pool 状态。

## 4. Endpoint 与出站边界

Endpoint 只来自代码注册项。Registry builder 必须拒绝：

- 非 HTTPS endpoint；
- 缺少 host；
- userinfo、query 或 fragment；
- 双斜线、空 segment、`.`、`..`；
- 编码斜线或不受限字符构成的 path prefix。

共享 transport 只能把 Provider adapter 生成的相对 path 追加到已校验 endpoint base，且禁用
redirect。业务请求、adapter 和 credential 均不能替换 endpoint origin。

## 5. 生命周期

```text
read bootstrap.toml
→ validate BootstrapConfig
→ read users.toml
→ validate UserConfiguration and collect downstream credentials
→ read upstream-credentials.toml
→ validate UpstreamCredentialConfiguration
→ compiled_config()
→ validate and build RuntimeRegistry
→ bind required upstream credential pools by compiled pool id
→ build immutable CredentialStore
→ create shared HTTP client
→ Arc<RuntimeRegistry> + Arc<UserRegistry> + Arc<CredentialStore>
→ start listener
```

注册表启动后不可变。服务没有文件监听、user/route reload、`ArcSwap` 或部分更新语义。运行中的请求和
后续请求都读取同一组 `RuntimeRegistry`、`UserRegistry` 与 `CredentialStore`；改变任一启动输入都必须重启。

## 6. 验收要求

| ID | 行为 |
|---|---|
| CFG-01 | 仓库不存在 Provider/Model route 配置文件或动态 Provider schema。 |
| CFG-02 | 代码注册表中的重复 ID、未知引用、能力扩大、无效 reasoning/level 映射和不安全 URL 在监听前失败。 |
| CFG-03 | 业务请求无法覆盖 endpoint、真实 model、credential、敏感 header 或 candidate 顺序；普通 header 仅能经受信 Provider 代码 hook 转换，不能由业务请求选择转换规则。 |
| CFG-04 | secret 不进入代码注册项、`RuntimeRegistry`、日志、错误或 probe report。 |
| CFG-05 | 每个 Provider 由独立文件实现，并由单一显式 registry 函数注册。 |
| CFG-06 | bootstrap 只控制进程资源策略，不能注册或修改 Provider。 |
| CFG-07 | listener 只允许 loopback；非 loopback 地址必须在监听前拒绝。 |
| CFG-08 | 用户文件中的无效 schema、重复 ID/Key、短 Key 或无启用用户会阻止启动。 |
| CFG-09 | 上下游 secret 只进入启动时不可变 `CredentialStore`；运行时按用途受限接口访问，不重新读取来源。 |
| CFG-10 | 私有 upstream credential TOML 出现未知或重复 pool，或任一已启用 Upstream Target 引用的 API-key pool 缺失、为空或不能解析时，会在 listener 绑定前阻止服务启动。 |
| CFG-11 | 同 Provider 的 Target 可引用共享 API-key pool；启动拒绝空 pool、重复 member、Provider/kind 不匹配或缺失 secret。 |
| CFG-12 | 多 member pool 不得用于缺少 credential affinity 证明的 `TargetBound` Upstream API。 |

## 关联文档

- [Public Model 与模型能力契约](model-information-and-capability-contract.md)
- [当前代码架构](../implementation-status/current-architecture.md)
- [当前实现说明](../implementation-status/current-implementation.md)
- [能力探测](../implementation-status/capability-probing.md)
