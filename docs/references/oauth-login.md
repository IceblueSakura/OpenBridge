# 账户登录来源与采用边界

本页是登录参考的导航与共用标准 owner，不维护认证实现、模型/账号状态、研究过程或运行报告。具体 authority、flow、固定客户端版本和采用差异分别由下列参考维护：

- [Grok Build / xAI 账户登录](grok-login.md)：官方浏览器 OIDC、标准设备授权、企业路径、pi 内置登录与补充插件。
- [ChatGPT 登录：Codex 与 pi](chatgpt-login.md)：Codex 产品浏览器/私有设备交互、公开 SIWC 动态 registration、refresh 与账户边界。
- [扩展与上下文](extensions-and-context.md#2-codex-session_id-的实际含义)：logical session、cache affinity、thread/turn、连接级 continuation 和选定客户端投影；不是登录协议的第二份定义。

方向归 [next-goal](../implementation-plans/next-goal.md)，有效决策归 [ADRs](../architecture-v2/README.md#架构决策)，获准行为切片归 [current-focus](../implementation-plans/current-focus.md)。这些文档均不授予登录、凭据操作、网络调用或部署权限。

## 共用标准

- [RFC 6749](https://www.rfc-editor.org/rfc/rfc6749.html)：authorization-code 与 refresh grant。
- [RFC 7636](https://www.rfc-editor.org/rfc/rfc7636.html)：PKCE，约束 authorization code 被截获后的使用。
- [RFC 8252](https://www.rfc-editor.org/rfc/rfc8252.html)：native app、系统浏览器与 loopback callback；仍受具体 client registration 约束。
- [RFC 8628](https://www.rfc-editor.org/rfc/rfc8628.html)：标准设备授权、pending、slow-down、拒绝与过期。设备交互不必然采用此 grant。
- [RFC 9700](https://www.rfc-editor.org/rfc/rfc9700.html)：OAuth 安全最佳实践、refresh rotation 与重放风险。
- [OpenID Connect Core](https://openid.net/specs/openid-connect-core-1_0.html)、[Discovery](https://openid.net/specs/openid-connect-discovery-1_0.html)：issuer/metadata、ID-token 验证及 nonce；JWT payload decode 不是签名验证。
- [RFC 7009](https://www.rfc-editor.org/rfc/rfc7009.html)：token revocation；本地清理、远端撤销与删除 registration 不是同一操作。
- [RFC 8707](https://www.rfc-editor.org/rfc/rfc8707.html)：resource indicators；scope、resource/audience 和推理 backend 必须分别绑定。

标准不决定具体产品的 client registration、订阅资格、scope、账户/workspace、redirect URI 或 endpoint。公开源码中的 client ID 不是 secret，也不是第三方复用资格的授权。

## 共用采用边界

1. **协议与用途分开**：登录成功不证明模型/operation 准入、订阅推理、计费或 SDK/Agent 闭环。API key、产品 session 和公开 SIWC token 不按同名 Bearer 混用。
2. **secret 与 runtime 分开**：凭据、locator、选定账户、refresh/retry state 属于独立认证/执行 owner，不进入 Task IR；纯 codec/lowering 不访问 credential、registry 或网络。业务 JSON 不选择 authority、账户、认证 headers 或脚本。
3. **事务与生命周期闭合**：授权事务、credential generation、registration、host identity 及 replay scope 分开。secret owner 在锁内重读并检查 source version，成功 rotation 原子发布整套 credential；并发更新或结果不确定不能盲目复用旧 refresh token。数据面只借用短生命周期、账户绑定的 credential 视图，不取得 locator/完整 bundle。取消、失败或晚到结果不覆盖另一登录或 generation。
4. **来源与可执行证据分开**：公开 metadata、固定源码和 synthetic 验证不证明真实登录或上游接受。真实登录、refresh、revocation、credential-bearing 发现和推理分别取得目标、效果与预算授权。
5. **独立存储**：不自动发现、读取、导入或修改 Codex、Grok、pi、Hermes、LiteLLM、浏览器或 OS 的真实 auth cache。默认拥有自己的 store；凭据迁移需另外授权。
6. **不恢复旧运行时**：不从参考客户端照搬账号池、脚本加载、动态 registry、隐式 API-key fallback 或业务请求 retry。publication/commit 后不能认证恢复重放，不根据 canonical model 相同允许跨 credential replay。

若存在方案选择，先固定具体 authority/client/resource、secret store、权限与失败边界，再在 owning ADR/行为切片内定稿；本入口不选择 Rust 类型或配置格式。

## pi 公用 credential 生命周期

固定 pi-ai `0.99.2`、`earendil-works/pi@005af57d88ee23b33778f343a9595b32e67ff788`（[MIT](https://github.com/earendil-works/pi/blob/005af57d88ee23b33778f343a9595b32e67ff788/LICENSE)）的 [resolver](https://github.com/earendil-works/pi/blob/005af57d88ee23b33778f343a9595b32e67ff788/packages/ai/src/auth/resolve.ts)通过 credential-store `modify` 在锁内重读、判断有效期、限定 refresh timeout 并持久化，存储 OAuth credential 的 refresh 失败不会静默退回环境 API key。各 authority 的 token 字段、rotation 与 adjusted expiry 仍由各自模块拥有，不能统一缺省 TTL 或机械叠加余量。

pi `/logout` 清理自身 stored credential，不撤销 Provider credential，也不移除环境变量或其他配置认证；产品端的 revocation 合同见各自参考。源码定位为 [credential store](https://github.com/earendil-works/pi/blob/005af57d88ee23b33778f343a9595b32e67ff788/packages/coding-agent/src/core/auth-storage.ts)，实际 store 必须单独核对跨进程协调和发布保证，不能从一次 `modify` 调用推定多主机锁或事务 durability。

## 归档与补充源码导航

以下只用于来源定位，不代替上面两份参考或当前产品合同，也不恢复旧配置、实现或研究报告：

- [OpenBridge v0.1](https://github.com/IceblueSakura/OpenBridge/tree/adff062e3412760ce5a66ae2e7506d22b142e0b1)，固定 `adff062e3412760ce5a66ae2e7506d22b142e0b1`，[MIT](https://github.com/IceblueSakura/OpenBridge/blob/adff062e3412760ce5a66ae2e7506d22b142e0b1/LICENSE)：[旧登录入口](https://github.com/IceblueSakura/OpenBridge/blob/adff062e3412760ce5a66ae2e7506d22b142e0b1/src/bin/openbridge-auth.rs)、[旧 refresh owner](https://github.com/IceblueSakura/OpenBridge/blob/adff062e3412760ce5a66ae2e7506d22b142e0b1/src/oauth2_credentials/manager/refresh.rs)；[归档说明](../archive.md)拥有历史定位。
- [Codex 旧认证定位](https://github.com/openai/codex/tree/ee0247f95a6fe2b094ba2253d82cae2a2b4c2dff/codex-rs/login)，固定 `ee0247f95a6fe2b094ba2253d82cae2a2b4c2dff`，[Apache-2.0](https://github.com/openai/codex/blob/ee0247f95a6fe2b094ba2253d82cae2a2b4c2dff/LICENSE)：只保留旧引用的可追溯性，不作为新的认证基线，也不改写 [upstream-sync](upstream-sync.md)。
- [Hermes auth](https://github.com/NousResearch/hermes-agent/blob/470cf66b039c73bdd2c21d43094ce41a4db74eae/hermes_cli/auth.py)，固定 `470cf66b039c73bdd2c21d43094ce41a4db74eae`，[MIT](https://github.com/NousResearch/hermes-agent/blob/470cf66b039c73bdd2c21d43094ce41a4db74eae/LICENSE)：credential ownership 与协调入口。
- [LiteLLM ChatGPT authenticator](https://github.com/BerriAI/litellm/blob/23de7a15d9d40006ee596e617475ba101d60c5e9/litellm/llms/chatgpt/authenticator.py)，固定 `23de7a15d9d40006ee596e617475ba101d60c5e9`，[许可入口](https://github.com/BerriAI/litellm/blob/23de7a15d9d40006ee596e617475ba101d60c5e9/LICENSE)：采用具体文件前核对许可范围。
- [CLIProxyAPI device login](https://github.com/router-for-me/CLIProxyAPI/blob/bc71c77f5cc42f3fbe1bf040cf14d4f166894835/sdk/auth/codex_device.go)、[refresh scheduler](https://github.com/router-for-me/CLIProxyAPI/blob/bc71c77f5cc42f3fbe1bf040cf14d4f166894835/sdk/cliproxy/auth/auto_refresh_loop.go)，固定 `bc71c77f5cc42f3fbe1bf040cf14d4f166894835`，[MIT](https://github.com/router-for-me/CLIProxyAPI/blob/bc71c77f5cc42f3fbe1bf040cf14d4f166894835/LICENSE)：来源导航，不采用其账号池或 fallback 为默认策略。

不据此建立客户端兼容矩阵或宣称已成功执行。具体 OpenBridge 实现查 [provider auth](../../src/provider/auth.rs)、[bootstrap](../../src/gateway/bootstrap.rs)和 [Generation 缺口](../implementation-status/generation.md)。
