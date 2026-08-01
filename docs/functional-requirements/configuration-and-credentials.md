# Bootstrap、代码注册表、凭证与受信边界

## 状态

**当前约束。** OpenBridge 是个人使用的 headless 网关。Provider contract、Model、
Upstream Target、Upstream API、Route、Public Model、endpoint、能力和字段转换由 Rust 代码显式注册；运行时配置不提供 Provider DSL，也不支持
route 热重载。

## 1. 所有权划分

| 来源 | 内容 | 能否包含 secret |
|---|---|---|
| `config/bootstrap.toml` | loopback listener、body/SSE 上限、共享 HTTP client 连接与超时策略 | 否 |
| `src/models/*` | Model 事实、token 限制、参数和 reasoning | 否 |
| `src/providers/*` | Provider 行为、target/upstream API、endpoint、credential binding、route 与 Public Model | 否 |
| 进程环境变量、被忽略的 `.env` 或后续受限 secret backend | 下游 Bearer token、上游 API key/OAuth material | 是 |
| 下游业务请求 | Public Model 和模型调用参数 | 否；也不能选择 endpoint/credential |

当前只允许 `OPENBRIDGE_BOOTSTRAP_CONFIG` 改变 bootstrap 文件位置。不存在
`OPENBRIDGE_ROUTES_CONFIG`，CLI 也不能注入 Provider、URL、header、model id 或转换规则。

## 2. 代码注册表要求

- 每个具体 Provider 位于独立 `src/providers/<provider>.rs` 文件；
- 每个 Model 位于独立 `src/models/<model>.rs` 文件；
- `src/providers/mod.rs::compiled_config()` 是唯一显式注册入口；
- 不使用运行时插件、链接器自动注册、JSON/TOML 转换模板或脚本；
- Provider contract 定义 adapter 能力上界、endpoint profile 和 credential kind；
- Model 定义模型事实、token 限制、支持参数、reasoning 状态与 reasoning level；
- Upstream Target 绑定 Provider、Model、endpoint、credential、timeout 和共享故障边界；
- Upstream API 独立声明一个协议的 upstream model、served limit、能力和 state affinity；
- Public Model 保存有序完整 Route；
- 启动监听前必须完成唯一性、引用、能力、reasoning、credential locator 和 URL 校验。

修改 Provider、Model 或路由必须重新编译并重启。项目不要求热重载。

## 3. 凭证

- 代码只保存非敏感 binding id、credential kind 和环境变量名称；
- 服务与 probe 可选加载 `.env`，已有进程环境变量优先；仓库只提交无真实值的 `.env.example`；
- 当前 OpenAI API key 从 `OPENAI_API_KEY` 获取；
- 当前 LongCat API key 从 `LONGCAT_API_KEY` 获取；
- 下游静态 token 从 `OPENBRIDGE_DOWNSTREAM_TOKEN` 获取；
- `RuntimeRegistry`、Debug、日志、错误响应和 probe report 不得包含 secret；
- secret 只在准备上游请求时解析为短时 `CredentialValue`；
- 缺失、空值或 binding 不匹配时 fail closed；
- 业务请求不能提供或覆盖 Authorization、cookie、Host、proxy header 或上游 credential。

以后增加 keyring、私有文件或 OAuth adapter 时，必须保持 typed locator 和显式 binding；不得增加
任意 shell command secret provider，也不得隐式读取 Codex/Hermes 登录状态。

## 4. Endpoint 与出站边界

Endpoint 只来自代码注册项。Registry builder 必须拒绝：

- 非 HTTPS endpoint；
- 缺少 host；
- userinfo、query 或 fragment；
- 双斜线、空 segment、`.`、`..`；
- 编码斜线或不受限字符构成的 path prefix。

共享 transport 只能把 Provider adapter 生成的相对 path 追加到已校验 endpoint base，且禁用
redirect。业务请求、adapter 和 credential 均不能替换 endpoint origin。若以后需要本地 HTTP
endpoint，必须增加显式、受限的 loopback endpoint 类型和独立测试，不能放宽通用 URL 校验。

## 5. 生命周期

```text
optionally load .env
→ read bootstrap.toml
→ validate BootstrapConfig
→ compiled_config()
→ validate and build RuntimeRegistry
→ create shared HTTP client
→ Arc<RuntimeRegistry>
→ start listener
```

注册表启动后不可变。服务没有文件监听、route reload、`ArcSwap` 或部分更新语义。运行中的请求和
后续请求都读取同一个 `RuntimeRegistry`；改变代码注册表或 `BootstrapConfig` 必须重启。

## 6. 验收要求

| ID | 行为 |
|---|---|
| CFG-01 | 仓库不存在 Provider/Model route 配置文件或动态 Provider schema。 |
| CFG-02 | 代码注册表中的重复 ID、未知引用、能力扩大、无效 reasoning 和不安全 URL 在监听前失败。 |
| CFG-03 | 业务请求无法覆盖 endpoint、真实 model、credential、header 或 candidate 顺序。 |
| CFG-04 | secret 不进入代码注册项、`RuntimeRegistry`、日志、错误或 probe report。 |
| CFG-05 | 每个 Provider 由独立文件实现，并由单一显式 registry 函数注册。 |
| CFG-06 | bootstrap 只控制进程资源策略，不能注册或修改 Provider。 |
| CFG-07 | 非 loopback listener 在当前实现中拒绝启动。 |

## 关联文档

- [配置与路由实施方案](../implementation-plans/configuration-and-routing.md)
- [Provider adapter 与数据流](../implementation-plans/provider-adapters-and-dataflow.md)
- [当前实现说明](../implementation-status/current-implementation.md)
- [能力探测](../implementation-status/capability-probing.md)
