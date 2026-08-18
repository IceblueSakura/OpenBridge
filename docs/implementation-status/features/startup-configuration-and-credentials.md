# 功能：启动配置、用户与受信凭证边界

## 当前行为

- `config/bootstrap.toml` 拥有 listener、limits、共享 HTTP client、项目默认 instructions、四个本地下游内容日志开关与可选
  OTLP traces/metrics；随附两个开发 profile 显式打开四个日志开关，并显式把两种 telemetry signal 指向
  `http://127.0.0.1:4318`。自定义配置省略日志表/字段时回退为 `false`，省略 signal table 时禁用对应 exporter。
- 私有 `users.toml` 提供下游用户/API key，私有 `upstream-credentials.toml` 只激活代码注册的 API-key pool 或 OAuth auth-file
  locator。主服务构建 OAuth manager 时要求该文件已存在且包含完整有效 bundle；缺失、空白或损坏文件会阻止启动。
- 未知、重复、类型/Provider 不匹配或损坏 binding 在 listener 前失败；缺失/source-less/空 API-key pool 只禁用引用 Target。
- active pool 在 Public Model 编译前单向收窄 Target；私有文件不能新增 Provider、Target、Route、endpoint 或能力。
- API-key Store 与用户表不热重载。独立 `openbridge-auth login chatgpt` 可以把完整登录结果事务写入尚不存在的目标文件；常驻服务的
  OAuth manager 只在到期 refresh 或首个预提交 401 recovery 内 guarded reload/atomic rotate 自己的 OpenBridge-owned 文件。
- 启动在 listener 前输出脱敏的 Provider/Public Model 配置态 available/unavailable 摘要；它不执行 network probe。
- 本地 request/response header/body snapshot 只在认证后和显式开关下采集，受 body budget 限制并强制脱敏；不进入 reviewed OTLP trace layer。
- 上游 endpoint、credential、认证 header 与 purpose 由注册表/adapter 固定；不从环境、`.env` 或本机 Codex 状态读取上游 secret。

## 所有权

装配位于 [`src/main.rs`](../../../src/main.rs)、[`src/config/`](../../../src/config/)、[`src/identity.rs`](../../../src/identity.rs)、
[`src/upstream_credentials.rs`](../../../src/upstream_credentials.rs)、[`src/credential/`](../../../src/credential/) 与
[`src/oauth2_credentials/`](../../../src/oauth2_credentials/)。配置态摘要由 `src/registry/availability.rs` 拥有。

## 确定性证据

- `tests/config_contract.rs`、`tests/example_config.rs`：严格 Bootstrap schema、默认、注释与随附 profile。
- `tests/upstream_credential_config.rs`：私有 binding、激活筛选、OAuth 文件与禁用 Target/Public Model。
- `tests/credential_store_contract.rs`、`tests/startup_contract.rs`：credential kind/purpose、bundle 与进程启动拒绝。
- `tests/observability_contract.rs`、`tests/otlp_trace_contract.rs`：四类本地 snapshot、body 生命周期、redaction 与 OTLP 排除。
- registry availability 单元测试与 startup process test：配置态分类、排序、脱敏和 listener 前输出。

## 未证明范围

确定性测试不证明真实 credential 可借用、Provider 可达、配额、OAuth 长期 refresh、外部 collector/sink、生产日志保留策略、
真实流量资源开销、负载或长期运行。配置 auth-file locator 本身不证明登录有效；主服务只会在完整 bundle 通过启动校验后监听。

## 相关文档

- [配置与凭证需求](../../functional-requirements/configuration-credentials/README.md)
- [ChatGPT OAuth2](chatgpt-oauth-startup.md)
- [当前代码架构](../current-architecture.md)
