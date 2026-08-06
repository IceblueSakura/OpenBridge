# 功能：启动配置、用户与受信凭证边界

## 状态

**已完成（当前 checkout）。** OpenBridge 在启动期装配固定注册表、下游用户表和上游 credential store；业务请求不能动态指定
上游 URL、credential、认证 header、Provider 或 Route。

## 已完成内容

- `config/bootstrap.toml` 负责监听地址、请求/响应/replay/SSE 限制、共享 HTTP client 和可选 OTLP trace exporter；当前只接受受校验的
  bootstrap 字段。
- 私有 `config/users.toml` 提供下游用户和 API key，私有 `config/upstream-credentials.toml` 提供按编译期 binding 关联的有序 API-key
  pool 或单一 OAuth2 auth 文件。
- 启动前严格校验未知、重复、类型不匹配和畸形的 credential binding；缺少已注册 pool、source-less pool 或空 API-key 数组会让其
  引用的 Target 在本次启动中未激活，不会要求对应 secret，也不会从 Provider 注册表移除它。
- 服务在 Public Model 编译前应用 active credential pool 集合，再构建不可变 `UserRegistry`、`CredentialStore` 和可用的 OAuth2 credential
  snapshot；缺失的 `auth_json_file` 会创建为空文件并保持待登录，不发布 snapshot，非空损坏文件仍阻止启动。
- 普通 TOML、用户表和 API-key Store 不热重载；OAuth2 manager 只在自己的登录、到期 refresh 或首个预提交 `401` recovery 流程中
  加锁读取、校验并在 rotation 时原子写回。
- 上游 endpoint、认证信息、purpose-bound secret 和普通安全 header 由受信代码注册与 Provider adapter 生成；客户端输入不能覆盖。
- 不从进程环境变量、`.env` 或本机 Codex 状态读取上游 API key、OAuth2 bundle、terminal identity 或 probe 配置。

## 实现边界

- 启动装配入口位于 [`src/main.rs`](../../../src/main.rs)、[`src/config/`](../../../src/config/)、
  [`src/identity.rs`](../../../src/identity.rs)、[`src/upstream_credentials.rs`](../../../src/upstream_credentials.rs) 和
  [`src/credential.rs`](../../../src/credential.rs)。
- 代码注册表和私有凭证文件是两类不同所有权：注册表只保存非敏感 pool/binding 身份，secret 只在启动后的受限 Store 中使用。
- 私有 credential 文件只能激活已经由代码注册的 pool；它不能增加 Provider、Target、Route、endpoint、credential kind 或能力。
- 当前不包含 keyring、加密 secret 文件、远程 secret manager、动态 credential 控制面、非 loopback 部署或多进程凭证协调。

## 验证证据

- [`tests/config_contract.rs`](../../../tests/config_contract.rs) 覆盖 bootstrap 与注册引用边界。
- [`tests/upstream_credential_config.rs`](../../../tests/upstream_credential_config.rs) 覆盖私有上游 credential TOML 的严格解析，以及缺失 auth 文件的启动创建和待登录边界。
- [`tests/upstream_credential_config.rs`](../../../tests/upstream_credential_config.rs) 还覆盖空/缺失 pool 的激活筛选、Provider 注册保留、Target/Public Model 过滤和非激活 pool 不进入服务凭证要求。
- [`tests/credential_store_contract.rs`](../../../tests/credential_store_contract.rs) 覆盖 credential kind、purpose 和 secret 隔离。
- [`tests/startup_contract.rs`](../../../tests/startup_contract.rs) 覆盖启动时 bundle、绑定和拒绝条件。

这些测试证明本地解析、校验和存储边界，不证明真实凭证可用、远程 Provider 可达或长期 refresh 稳定性。

## 相关文档

- [功能需求：Bootstrap、代码注册表、凭证与受信边界](../../functional-requirements/configuration-and-credentials.md)
- [ChatGPT OAuth2 生命周期与 Responses 数据面](chatgpt-oauth-startup.md)
- [当前代码架构](../current-architecture.md)
