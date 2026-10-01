# Codex / SuperGrok 登录来源与边界

本页从 `v0.1` 的旧资料提取必要标准出处、固定源码入口和采用边界，不恢复历史调研报告、比较表、测试结果或旧实现合同。下一步方向由 [next-goal](../implementation-plans/next-goal.md) 维护；当前凭据种类和启动路径查 [auth](../../src/provider/auth.rs) 与 [bootstrap](../../src/gateway/bootstrap.rs)。资料存在不代表当前 Gateway 已支持 OAuth 登录。

## 标准来源

- [RFC 6749](https://www.rfc-editor.org/rfc/rfc6749.html)：OAuth 2.0 authorization-code 与 refresh grant。
- [RFC 7636](https://www.rfc-editor.org/rfc/rfc7636.html)：PKCE，防止 authorization code 被截获后滥用。
- [RFC 8628](https://www.rfc-editor.org/rfc/rfc8628.html)：标准设备授权及 pending、slow-down、拒绝和过期语义。
- [RFC 9700](https://www.rfc-editor.org/rfc/rfc9700.html)：OAuth 2.0 安全最佳实践，含 refresh-token rotation 与重放风险。

标准不能替代具体 Provider 的 client registration、scope、audience、账号绑定和产品使用政策。设备交互不必然使用 RFC 8628 token polling；必须按目标合同区分。

## Codex / ChatGPT

这里的目标是 Codex 使用的 ChatGPT 账户登录，不是给普通 OpenAI API key 增加刷新，也不把 Codex executable 或本机 auth cache 作为依赖。旧资料的官方产品入口为 [Codex authentication](https://learn.chatgpt.com/docs/auth)，采用前需重核当前内容。

旧资料固定的认证定位快照为 `openai/codex@ee0247f95a6fe2b094ba2253d82cae2a2b4c2dff`，许可证为 [Apache-2.0](https://github.com/openai/codex/blob/ee0247f95a6fe2b094ba2253d82cae2a2b4c2dff/LICENSE)：

- [device login](https://github.com/openai/codex/blob/ee0247f95a6fe2b094ba2253d82cae2a2b4c2dff/codex-rs/login/src/device_code_auth.rs)：私有设备交互先轮询取得 authorization code 和 PKCE 材料，再进行 authorization-code exchange；不是标准设备 grant 的直接 token 返回。
- [auth manager](https://github.com/openai/codex/blob/ee0247f95a6fe2b094ba2253d82cae2a2b4c2dff/codex-rs/login/src/auth/manager.rs)：定位使用时刷新、guarded reload、账户边界和有界认证恢复。
- [login module](https://github.com/openai/codex/tree/ee0247f95a6fe2b094ba2253d82cae2a2b4c2dff/codex-rs/login)：定位浏览器 authorization-code + PKCE、state 和 loopback callback，以及 credential backend。
- [device login tests](https://github.com/openai/codex/blob/ee0247f95a6fe2b094ba2253d82cae2a2b4c2dff/codex-rs/login/tests/suite/device_code_login.rs)：核对该快照的设备交互与失败边界，不作为 OpenBridge 的执行证据。

该旧认证快照只用于源码导航，不替换 [upstream-sync](upstream-sync.md) 的主线语义基线。该入口不声明已重新核验官方网页、当前 Codex 源码、client registration 或真实 token flow。实现前应重新固定认证来源，并在 owning code 保留必要出处；不从旧资料复制 endpoint、client ID、scope、header 或超时常量作为新合同。

Codex 产品实现和其他客户端复现私有 flow 不保证第三方复用资格。账户登录、OAuth authority 与订阅推理 backend 是不同边界，取得 token 不等于获得任意 API/模型/自动化用途的访问权。

## SuperGrok

SuperGrok 是待调研的账户/订阅接入目标，不等同于通过 OpenRouter 调用 Grok，也不等同于 xAI API-key 接入。

`v0.1` 的 [Hermes 插件资料](https://github.com/IceblueSakura/OpenBridge/blob/adff062e3412760ce5a66ae2e7506d22b142e0b1/docs/references/hermes/hermes-provider-plugin-capabilities.md) 仅提供 `xai-oauth` 的定位线索，没有充分说明其 authority、grant、client registration、token exchange 或 refresh 合同。不能据此认定 SuperGrok 的当前登录就是标准 OAuth2、与 Codex 共用设备流程，或可通过读取其他应用 cache 接入。

下一次专项调研必须先确认官方支持的登录方式和订阅推理用途，再定位合法客户端及其固定源码；未取得充分证据的字段和流程保持未定，不预造端点或把 Cookie/session 登录包装为 OAuth2。

## 旧 OpenBridge 与补充客户端来源

选定旧参考为 [`OpenBridge v0.1@adff062e3412760ce5a66ae2e7506d22b142e0b1`](https://github.com/IceblueSakura/OpenBridge/tree/adff062e3412760ce5a66ae2e7506d22b142e0b1)，许可证为 [MIT](https://github.com/IceblueSakura/OpenBridge/blob/adff062e3412760ce5a66ae2e7506d22b142e0b1/LICENSE)。只通过 Git 查阅受版本管理的旧资料，不读取私有配置或真实授权文件。

- [旧 OAuth 生命周期合同](https://github.com/IceblueSakura/OpenBridge/blob/adff062e3412760ce5a66ae2e7506d22b142e0b1/docs/functional-requirements/configuration-credentials.md#chatgpt-oauth-credential-生命周期)：定位显式登录、文件所有权、短期 lease、refresh/rotation、账户隔离与提交前恢复边界；不是当前主线的配置格式或已接受设计。
- [旧登录入口](https://github.com/IceblueSakura/OpenBridge/blob/adff062e3412760ce5a66ae2e7506d22b142e0b1/src/bin/openbridge-auth.rs)、[设备登录 transport](https://github.com/IceblueSakura/OpenBridge/blob/adff062e3412760ce5a66ae2e7506d22b142e0b1/src/oauth2_credentials/login/transport.rs)、[refresh manager](https://github.com/IceblueSakura/OpenBridge/blob/adff062e3412760ce5a66ae2e7506d22b142e0b1/src/oauth2_credentials/manager/refresh.rs)：定位原实现的职责分工，不直接移植旧 registry、配置、文件 schema 或 retry 策略。
- [Hermes auth](https://github.com/NousResearch/hermes-agent/blob/470cf66b039c73bdd2c21d43094ce41a4db74eae/hermes_cli/auth.py)，[MIT](https://github.com/NousResearch/hermes-agent/blob/470cf66b039c73bdd2c21d43094ce41a4db74eae/LICENSE)：补充 credential-store ownership 与同文件系统并发协调的源码入口。
- [LiteLLM ChatGPT authenticator](https://github.com/BerriAI/litellm/blob/23de7a15d9d40006ee596e617475ba101d60c5e9/litellm/llms/chatgpt/authenticator.py)，[许可证](https://github.com/BerriAI/litellm/blob/23de7a15d9d40006ee596e617475ba101d60c5e9/LICENSE)：补充 token resolution 与持久化的源码入口；采用具体文件前核对许可范围。
- [CLIProxyAPI device login](https://github.com/router-for-me/CLIProxyAPI/blob/bc71c77f5cc42f3fbe1bf040cf14d4f166894835/sdk/auth/codex_device.go)、[refresh scheduler](https://github.com/router-for-me/CLIProxyAPI/blob/bc71c77f5cc42f3fbe1bf040cf14d4f166894835/sdk/cliproxy/auth/auto_refresh_loop.go)，[MIT](https://github.com/router-for-me/CLIProxyAPI/blob/bc71c77f5cc42f3fbe1bf040cf14d4f166894835/LICENSE)：补充登录与后台刷新 owner 的源码入口，不采用其账号池或 fallback 策略作为主线默认值。

这些入口不组成兼容矩阵。具体授权合同必须重新核实；登录、凭据操作和真实请求仍需明确授权。未来实现必须保持纯 codec/lowering 不访问网络或秘密、认证绑定不进入 Task IR、客户端不能选择账户/Provider，以及 publication/commit 后不能认证恢复重放的边界。
