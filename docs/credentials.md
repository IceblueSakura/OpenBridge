# 文件授权凭据管理器

独立 `openbridge-auth` 通过同一个 profile-neutral 管理器执行账户登录、显式刷新、本地退出、可选远端撤销和非秘密状态查询。设计归 [ADR 0012](architecture-v2/decisions/0012-grok-personal-credential-pool.md)；具体授权合同归 [Grok](references/grok-login.md)、[ChatGPT/Codex](references/chatgpt-login.md)来源和各自 driver。

Grok 支持个人账户标准设备授权和显式浏览器 PKCE/OIDC；Codex 支持产品私有设备交互和显式浏览器 PKCE/OIDC，不是公开 SIWC。两家缺省仍为 device，browser 必须明确选择。登录不证明订阅推理资格。本组件不接 Gateway、不发现模型/额度、不自动选择账户，也不读取第三方 auth cache 或环境凭据。

## 显式自有文件目录

仅支持具备 advisory locking、原子 rename 与 fsync 的 Unix 本地文件系统。每条命令必须提供 `--store`，父目录须已存在；只创建最后一级目录。相同目录可管理不同 profile 和多个 alias。

按 profile 分目录，每个 alias 对应一个格式化 JSON 账户文件；同一文档原子保存身份绑定、session state、revision、credential generation、login ticket 与 tokens。锁及发布 marker 是内部协调文件，不应手动删除。精确布局、字段和预算由 [store](../src/credential/store.rs)与 [model](../src/credential/model.rs)拥有。

**文件可读不等于内容可公开。** JSON 包含真实 secrets，不能提交 Git、复制到诊断或打印到日志。新目录 `0700`、文件 `0600`，拒绝不安全权限、symlink、非普通或 hardlinked 文件，不自动修复。调试优先使用脱敏 `list`；不要在操作进行中手工编辑文件。

旧 Grok-only 和旧共用 `accounts.json` snapshot 均明确拒绝。新格式不提供兼容层、自动迁移或 `migrate-grok` 命令；使用新的显式自有目录。源代码重构不删除、转换或读取已有真实 store。

## 命令

```sh
cargo build --locked --offline --bin openbridge-auth

STORE="$HOME/.local/share/openbridge/credentials"
mkdir -p -- "$(dirname "$STORE")"

target/debug/openbridge-auth grok login --store "$STORE" --account personal
target/debug/openbridge-auth grok login --store "$STORE" --account personal --method browser
# 如需另一获准 registration，显式覆盖 client；已有账户不自动换绑。
target/debug/openbridge-auth grok login \
  --store "$STORE" --account other --client-id "<approved-xai-client-id>"
target/debug/openbridge-auth codex login --store "$STORE" --account personal
target/debug/openbridge-auth codex login --store "$STORE" --account personal --method browser

target/debug/openbridge-auth list --store "$STORE"
target/debug/openbridge-auth grok list --store "$STORE"
target/debug/openbridge-auth codex refresh --store "$STORE" --account personal
target/debug/openbridge-auth codex logout --store "$STORE" --account personal
# 额外发送一次远端 revoke；默认仅本地清理。
target/debug/openbridge-auth codex logout --store "$STORE" --account personal --revoke
```

login/refresh/revoke 联系真实 authority，需明确授权后执行；示例不会自动执行。本地 alias 不是 email 或已验证身份；允许不同 profile 使用相同 alias，不允许在同 profile/client/principal 下重复登记另一 alias。退出保留身份绑定，不隐式换人、换 client 或重绑定。

Grok 默认使用固定来源的官方产品 client，允许显式 `--client-id` override；Codex 使用固定来源的产品 client，不接受 override。已有账户始终绑定其原 client，省略或更改选项不自动重绑定。公开源码与 metadata 兼容不证明第三方用途已获准，也不能代替 SIWC registration。设备入口不可用时明确失败，不自动改用浏览器或另一 client。

如需代理，显式增加 `--proxy http://127.0.0.1:<port>`；只能是无 userinfo/query/fragment 的受信 HTTP(S) 代理 URL。无环境代理继承、proxy fallback 或自动 retry。代理配置不写入账户文件，也不能由业务 JSON 或 authority 响应选择。

## 交互与身份

设备方法只展示可信第一方验证网址和 user code，不展示 device secret。浏览器方法只展示第一方授权 URL 与准确 redirect，不自动打开浏览器。请只批准自己发起的登录。

两家的浏览器 callback 都只监听 literal `127.0.0.1`。Grok 使用 `/callback`，未指定端口或显式 `--callback-port 0` 时由 OS 选择，可指定其他获准端口。Codex 使用 `/auth/callback`，默认 1455，也允许显式 `--callback-port 1457`；0 和其他端口拒绝。端口占用明确失败，不自动换端口、不取消其他登录进程。

`--callback-port` 仅用于 browser；库级 `LoginOptions.callback_port` 的 `None` 使用 profile 默认，`Some(0)` 是明确请求随机端口，并非缺省。准确 URI 绑定 authorize 与 exchange，监听成功不证明 registration 允许。没有裸 code 或手动 URL 绕过。

callback 校验 method/path/Host、query 唯一性、state 和资源预算；OAuth error 也先校验 state。页面只确认接收 callback，不声明 token 验证或持久化成功。listener/连接由本次 operation future 拥有，在取消、拒绝和超时释放。

Grok browser 的 ES256/P-256 ID token 验证 issuer/audience/expiry/nonce，并与 UserInfo subject 对齐；设备身份由固定 HTTPS UserInfo 验证。Codex 的 RS256 ID token 绑定 subject/workspace，browser 额外验证本次 nonce；device 与 refresh 不套用 browser nonce。JWT payload decode 不是身份验证。浏览器事务的 state/nonce/code/verifier 不单独持久化；Codex 按产品合同保留 ID token 作为 secret，其中可含已经验证的 nonce claim。两家的 claims、scope 与缺字段继承规则互不套用。

Codex browser 使用固定产品 client，只请求 `openid profile email offline_access`，不申请 connectors 权限，不从 ID token 派生 API key。authorization query 与 refresh/revoke 的 `originator` 采用固定 CLI 默认值；设备申请/轮询与 code exchange 不带默认 UA/originator，refresh/revoke 的 UA 按固定 CLI 的系统版本、架构和终端格式构造，不追加项目名，也不读取官方环境 override 或 managed residency/cookie。精确 metadata 归 [Codex owner](../src/credential/codex_metadata.rs)。相对固定官方参考，浏览器 driver 增加并严格验证 nonce；实际 authority 对选定参数的接受及订阅推理资格仍需真实授权验收。

Grok 的默认 client、`grok-build` referrer 与 generic `grok-shell` UA 采用固定官方参考，不追加项目名；版本取独立核实的固定发布参考，不使用源码 crate 的开发版本。设备申请/轮询携带版本及按 stderr TTY 判定的 `cli`/`headless` surface；浏览器 code exchange 只携带版本，refresh/UserInfo/JWKS/revoke 不泛化这些 headers。不读取官方身份环境 override、不冒充 TUI，scope 仍仅六项、不增加 conversations/workspaces 权限。精确 metadata 归 [Grok owner](../src/credential/grok_metadata.rs)。这不是上游登录、推理或合法 registration 用途已验收的声明。

## 生命周期与故障

- `list` 只报告 profile、alias、生命周期、access freshness、revision/generation、expiry 和 pending/recovery 标记，不输出 subject、principal scope、client、token 或 bundle。未知 expiry 不等于已验证可用。
- 同账户竞争返回 busy；其他账户可以继续操作。登录等待人不持锁，提交核对当前 ticket 和身份；失败或取消的重新登录不覆盖旧 session。
- refresh 发送前先持久化移除旧可复用 secrets。取消、身份变化、拒绝或不确定网络结果要求重新登录，不盲目重发旧 refresh token。
- 发布 marker 在替换账户文档前持久化；崩溃或不确定发布留下 marker 时，该账户按需要恢复处理，tokens 不可用。明确 login 或本地 logout 可以完成恢复。已确认 snapshot durability 后的 marker 清理失败只会保守隔离，不恢复旧 token；读回可见性不能代替 fsync。
- Ctrl-C 取消本次网络/轮询，临时授权状态不进入持久化。崩溃留下的登录 ticket 由新登录替代。
- logout 先持久化清除所选 session，再可选 revoke。远端失败、中断或没有 handle 时不会声称 remote 完成；本地 tokens 不恢复。HTTP 200 只是该 token 的协议确认，不证明删除所有 sessions/consent。

存储并不隔离恶意同 UID 进程，也不支持多主机协调或保证 secure erasure。文件故障需要检查文件系统，不能删除 marker 后继续使用不确定 token。

## 组件扩展与验收

[manager](../src/credential/manager.rs)仅依赖共同模型、driver 合同和存储；[显式注册](../src/credential/profiles.rs)负责组合 Grok/Codex。新增 driver 独立拥有授权端点、方法和 wire；不从 Provider 名称推定 grant，不引入任意脚本或配置化 OAuth 工作流。Muse 等后续产品需先确认具体合同。

[开发指南](development.md)列出 synthetic 单元、wire、文件/进程和 CLI gate。真实认证需另行明确账户、store、client/redirect、操作效果与输出边界；推理和额度验证再单独选定 backend、模型和预算。认证成功不是推理接线或订阅额度使用的证明。
