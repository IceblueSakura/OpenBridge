# 功能：启动配置、用户与受信凭证边界

## 状态

**已完成（当前 checkout）。** OpenBridge 在启动期装配固定注册表、下游用户表和上游 credential store；业务请求不能动态指定
上游 URL、credential、认证 header、Provider 或 Route。

## 已完成内容

- `config/bootstrap.toml` 负责监听地址、请求/响应/replay/SSE 限制、共享 HTTP client、四个在随附开发配置中显式全开的本地下游 HTTP 内容日志开关，
  以及分别可选的 OTLP traces/metrics exporter；当前只接受受校验的 bootstrap 字段。
- 私有 `config/users.toml` 提供下游用户和 API key，私有 `config/upstream-credentials.toml` 提供按编译期 binding 关联的有序 API-key
  pool 或单一 OAuth2 auth 文件。
- `config/upstream-credentials.example.toml` 为每个内置 API-key binding 提供非真实 placeholder；`openai-primary`、NVIDIA 与百炼的
  本地私有 binding 都可以先使用 `api_keys = []` 或省略来保持未激活，稍后填入真实 key 并重启。OpenAI binding 保留在代码注册表中，
  不因当前未启用 `openai-primary` 而被删除；ChatGPT 使用独立的 OAuth2 pool。
- 启动前严格校验未知、重复、类型不匹配和畸形的 credential binding；缺少已注册 pool、source-less pool 或空 API-key 数组会让其
  引用的 Target 在本次启动中未激活，不会要求对应 secret，也不会从 Provider 注册表移除它。
- 服务在 Public Model 编译前应用 active credential pool 集合，再构建不可变 `UserRegistry`、`CredentialStore` 和可用的 OAuth2 credential
  snapshot；缺失的 `auth_json_file` 会创建为空文件并保持待登录，不发布 snapshot，非空损坏文件仍阻止启动。
- 全部私有配置和 credential binding 校验成功后、listener 绑定前，主服务输出 Provider family 与 Public Model 两张配置态
  available/unavailable ASCII 双列表格。Provider 按 enabled/total Target 汇总，Public Model 按已编译 Chat、Responses、Embeddings
  execution interface 分类；标题明确声明没有执行 network probe。
- 普通 TOML、用户表和 API-key Store 不热重载；OAuth2 manager 只在自己的登录、到期 refresh 或首个预提交 `401` recovery 流程中
  加锁读取、校验并在 rotation 时原子写回。
- 上游 endpoint、认证信息、purpose-bound secret 和普通安全 header 由受信代码注册与 Provider adapter 生成；客户端输入不能覆盖。
- 不从进程环境变量、`.env` 或本机 Codex 状态读取上游 API key、OAuth2 bundle、terminal identity 或 probe 配置。
- `[logging]` 可以独立启用认证后 downstream request header/body 与 response header/body。header snapshot 强制脱敏认证、Cookie 和
  secret-like 名称；body wrapper 只在开关启用时保留有界副本，在 EOF/error/cancel 时输出一个带 complete/truncated/byte count 的
  本地 info event。随附开发配置显式全开，缺表/缺字段时解析回退关闭；匿名认证失败不采集，事件不进入 span-only OTLP layer，
  也不改变原始 HTTP frame、Route 或 Provider 行为。

## 实现边界

- 启动装配入口位于 [`src/main.rs`](../../../src/main.rs)、[`src/config/`](../../../src/config/)、
  [`src/identity.rs`](../../../src/identity.rs)、[`src/upstream_credentials.rs`](../../../src/upstream_credentials.rs) 和
  [`src/credential.rs`](../../../src/credential.rs)。
- [`src/registry/availability.rs`](../../../src/registry/availability.rs) 只读取不可变注册表和脱敏 active-pool 集合，稳定排序并渲染摘要；
  输出不包含 pool/Target/Route/endpoint/credential，也不改变 Models API 或请求规划。
- 代码注册表和私有凭证文件是两类不同所有权：注册表只保存非敏感 pool/binding 身份，secret 只在启动后的受限 Store 中使用。
- 私有 credential 文件只能激活已经由代码注册的 pool；它不能增加 Provider、Target、Route、endpoint、credential kind 或能力。
- 当前不包含 keyring、加密 secret 文件、远程 secret manager、动态 credential 控制面、非 loopback 部署或多进程凭证协调。
- 当前本地内容日志不是上游 Provider wire dump，也不包含日志轮转、文件 sink、查询、采样、热重载或 OTLP logs。

## 验证证据

- [`tests/config_contract.rs`](../../../tests/config_contract.rs) 覆盖 bootstrap 与注册引用边界。
- [`tests/observability_contract.rs`](../../../tests/observability_contract.rs) 覆盖四个本地 HTTP 内容开关的真实 Router 接线、完整请求/响应
  body 生命周期、安全 header 保留与 Bearer 值脱敏。
- [`tests/upstream_credential_config.rs`](../../../tests/upstream_credential_config.rs) 覆盖私有上游 credential TOML 的严格解析，以及缺失 auth 文件的启动创建和待登录边界。
- [`tests/upstream_credential_config.rs`](../../../tests/upstream_credential_config.rs) 还覆盖空/缺失 pool 的激活筛选、Provider 注册保留、Target/Public Model 过滤和非激活 pool 不进入服务凭证要求。
- [`tests/credential_store_contract.rs`](../../../tests/credential_store_contract.rs) 覆盖 credential kind、purpose 和 secret 隔离。
- [`tests/startup_contract.rs`](../../../tests/startup_contract.rs) 覆盖启动时 bundle、绑定和拒绝条件。
- `registry::availability::tests::production_report_uses_only_configuration_eligibility_and_redacted_names` 覆盖只激活 MiMo pool 时的
  Provider/Public Model 分类、排序和脱敏；
  `process_reports_configuration_availability_before_bound_listener_failure` 覆盖真实进程在 listener 前输出两张表，并确认缺失 OpenAI pool
  不会错误禁用仍有 ChatGPT source 的 `gpt-5.6-sol`。

这些测试证明本地解析、校验、存储、配置态分类和启动展示边界，不证明真实凭证可借用、远程 Provider/Model 可达、协议能力、配额或长期
refresh 稳定性。配置了 OAuth2 auth-file locator 仍可能处于待登录状态。

2026-08-08 配置态启动摘要验证：

- `cargo test --locked --lib registry::availability::tests`：2 个分类、脱敏和空列渲染测试通过；
- `cargo test --locked --test startup_contract process_reports_configuration_availability_before_bound_listener_failure`：真实进程接线测试通过；
- `cargo fmt -- --check`、`cargo test --locked`、`cargo clippy --locked -- -D warnings` 与 `git diff --check`：通过。

本次没有运行真实 Provider、独立 probe、外部 SDK、目标 Agent、负载或长期运行测试；这些层不由配置态摘要证明。

2026-08-08 NVIDIA 与百炼 credential binding 扩展验证：

- 当时确认两个 API-key pool、固定 Provider instance 与模板 placeholder，并确认模板可按编译期 binding 装载；
- 当前 [`tests/example_config.rs`](../../../tests/example_config.rs) 只保留两个 checked-in Bootstrap profile 的运行时注册表编译烟雾，
  credential 解析和启用/禁用结果由 [`tests/upstream_credential_config.rs`](../../../tests/upstream_credential_config.rs) 覆盖；
- `cargo test --locked`、`cargo clippy --locked -- -D warnings` 与 `cargo fmt -- --check`：通过。

本轮只验证无真实值的模板和本地静态绑定；没有读取、打印或测试真实 key，也没有执行 Provider 网络请求。

2026-08-10 本地下游 HTTP 内容日志开关验证：

- `tests/config_contract.rs`：22 个测试通过，覆盖 `[logging]` 缺表/缺字段回退全关、四个开关独立解析和未知字段拒绝；
- `tests/observability_contract.rs`：15 个测试通过，其中新增契约覆盖真实 Router 的四类本地事件、独立开关、正文生命周期、
  `X-Request-Id` 响应 header 和敏感 header 强制脱敏；
- `tests/otlp_trace_contract.rs`：2 个测试通过，确认即使四个本地内容开关全开，正文和 header marker 也不进入 OTLP traces；
- `cargo fmt -- --check`、`cargo test --locked`、`cargo clippy --locked -- -D warnings` 与 `git diff --check`：通过。

本轮没有运行真实 Provider、外部 SDK、外部 collector/backend、负载或长期运行测试；本地确定性测试不证明生产日志 sink、
部署环境日志保留策略或真实流量下的资源开销。

2026-08-10 Bootstrap 示例说明与开发配置默认值验证：

- `config/bootstrap.example.toml` 与 `config/bootstrap.toml` 的 18 个字段都获得紧邻字段的英文说明，两个文档解析结果保持一致；
- `cargo test --locked --test example_config`：27 个测试通过，新增契约确认每个示例字段都有说明且两份随附配置的四个日志开关全为
  `true`；
- `cargo test --locked --test config_contract`：22 个测试通过，确认自定义配置缺表/缺字段时仍回退关闭并保持独立覆盖；
- `cargo fmt -- --check`、`cargo test --locked`、`cargo clippy --locked -- -D warnings` 与 `git diff --check`：通过。

本轮没有运行真实 Provider、外部日志系统、负载或长期运行性能测试；配置默认值不构成这些层的验收证据。

## 相关文档

- [功能需求：Bootstrap、代码注册表、凭证与受信边界](../../functional-requirements/configuration-credentials/README.md)
- [ChatGPT OAuth2 生命周期与 Responses 数据面](chatgpt-oauth-startup.md)
- [当前代码架构](../current-architecture.md)
