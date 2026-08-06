# 功能：ChatGPT OAuth2 启动凭证生命周期

## 状态

**已完成（启动凭证范围）。** OpenBridge 已具备独立、默认禁用的 ChatGPT Provider OAuth2 bundle 加载、显式 device/PKCE 登录和到期驱动
refresh；这不等于 ChatGPT 数据面已经接入。

## 已完成内容

- `openbridge-auth login chatgpt` 使用固定注册的 device interaction、authorization-code + PKCE 流程，完成 token bundle 校验后事务性写入
  OpenBridge-owned auth 文件。
- 启动时校验 auth 文件的完整性、Provider/context 绑定、token 类型和过期信息，并将可用 bundle 放入独立
  `OAuth2CredentialManager`。
- 到期前 refresh 在进程内 gate 和文件锁内重新加载持久化文档；成功后校验新 bundle、原子写回并发布新的 credential generation。
- ChatGPT Provider target 默认禁用，不加入任何 Route 或 Public Model；服务不会读取本机 Codex auth/cache、terminal identity 或隐式登录。
- 登录、refresh、存储和诊断只输出无 secret 的阶段/状态信息；失败不把 token、authorization code 或完整响应正文写入日志。

## 实现边界

- 登录与 manager 位于 [`src/oauth2_credentials/`](../../../src/oauth2_credentials/)，ChatGPT 注册位于
  [`src/providers/chatgpt/`](../../../src/providers/chatgpt/)。
- 当前范围不包括 ChatGPT 数据面 credential 借用、Route/Public Model、真实请求、401 recovery、多账号 pool 或账号级负载均衡。
- 确定性测试替换 OAuth transport 和 sleep；它们不证明真实 ChatGPT authority、浏览器交互或长期 refresh 成功率。

## 验证证据

- [`tests/oauth2_login_cli.rs`](../../../tests/oauth2_login_cli.rs) 覆盖显式登录 CLI 的阶段、PKCE、响应校验和事务写入。
- [`tests/startup_contract.rs`](../../../tests/startup_contract.rs) 覆盖 startup bundle 加载、校验和拒绝条件。
- [`tests/config_contract.rs`](../../../tests/config_contract.rs) 与 [`tests/credential_store_contract.rs`](../../../tests/credential_store_contract.rs)
  覆盖 credential binding 与存储边界。

## 相关文档

- [功能需求：ChatGPT subscription OAuth credential lifecycle](../../functional-requirements/upstream-oauth-credential-lifecycle.md)
- [功能需求：Bootstrap、代码注册表、凭证与受信边界](../../functional-requirements/configuration-and-credentials.md)
- [Provider 注册表与模型目录](provider-registry-and-model-catalog.md)
