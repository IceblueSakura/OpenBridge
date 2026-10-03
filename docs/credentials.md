# 文件授权凭据管理器

`openbridge-auth` 管理显式自有 store 中的登录、刷新、退出和脱敏状态查询。Grok 个人授权与 Codex 产品授权按 profile 隔离；Codex 不是公开 SIWC，也不是 Platform API key。架构合同见 [ADR 0012](architecture-v2/decisions/0012-grok-personal-credential-pool.md)，精确授权协议与 metadata 来源见 [Grok](references/grok-login.md)、[ChatGPT/Codex](references/chatgpt-login.md)。

登录成功不证明模型、订阅或第三方部署资格。CLI 不发现模型/额度、不自动选择账户、不读取第三方 auth cache；真实 login/refresh/revoke 与推理均需明确授权。

## Gateway access 绑定

[Gateway 启动](http-gateway.md#启动)显式设置 `OPENBRIDGE_CREDENTIAL_STORE`，并通过 catalog 声明的账户变量指定 alias；不默认搜索目录或账户。测试沿用 [统一 probe](probes.md) 的 provider/model 选择，不把账户 alias 暴露为业务参数。

每次请求借用固定 profile/client/principal 的最新 access snapshot，不向推理层提供 refresh token 或 ID token。token rotation 保持 replay scope；过期、退出、quarantine 或身份变化返回脱敏 `credential_unavailable`，不会自动刷新、换账户或重试。未知 expiry 仍为未知；已 dispatch 的请求可完成，logout 不保证停止上游计算。需要恢复时，操作者显式执行 refresh/login。

## 显式自有文件目录

每条 CLI 命令必须提供 `--store`，父目录须存在；只创建最后一级目录。仅支持具备 advisory locking、原子 rename 与 fsync 的 Unix 本地文件系统。逐 profile/alias 的账户文档、锁和发布 marker 由 [store](../src/credential/store.rs)拥有，不支持聚合 `accounts.json`、自动导入或格式迁移。

**账户 JSON 包含真实 secrets，不能提交、复制到诊断或打印。** 新目录 `0700`、文件 `0600`；不安全权限、symlink、非普通或 hardlinked 文件均拒绝，不自动修复。调试使用脱敏 `list`，不要手工删除锁/marker 或在操作期间修改文件。

## 命令

```sh
cargo build --locked --offline --bin openbridge-auth

STORE="$HOME/.local/share/openbridge/credentials"
mkdir -p -- "$(dirname "$STORE")"

target/debug/openbridge-auth grok login --store "$STORE" --account personal
target/debug/openbridge-auth grok login --store "$STORE" --account personal --method browser
target/debug/openbridge-auth codex login --store "$STORE" --account personal
target/debug/openbridge-auth codex login --store "$STORE" --account personal --method browser

target/debug/openbridge-auth list --store "$STORE"
target/debug/openbridge-auth grok list --store "$STORE"
target/debug/openbridge-auth codex refresh --store "$STORE" --account personal
target/debug/openbridge-auth codex logout --store "$STORE" --account personal
# 可选：本地清理后另发一次远端 revoke。
target/debug/openbridge-auth codex logout --store "$STORE" --account personal --revoke
```

- 两家缺省为 device；browser 必须显式选择。设备入口不可用时失败，不自动换方法或 client。
- Grok 可通过 `--client-id "<approved-xai-client-id>"` 指定另一获准 registration；Codex 不接受该 override。已有账户绑定不因省略或更改选项而替换。
- alias 不是 email 或已验证身份。不同 profile 可使用相同 alias；同 profile/client/principal 不能重复登记另一 alias。退出保留身份绑定，不隐式换人或换 client。
- 代理必须显式指定 `--proxy http://127.0.0.1:<port>`，仅接受无 userinfo/query/fragment 的受信 HTTP(S) URL。不继承环境代理，不自动 retry/fallback，不把代理配置写入账户文件。

## 交互与身份

设备登录只展示可信第一方验证网址和 user code，不展示 device secret。浏览器登录只展示第一方授权 URL 与准确 redirect，不自动打开浏览器。请只批准自己发起的登录。

浏览器 callback 仅监听 literal `127.0.0.1`：Grok 使用 `/callback`，缺省或 `--callback-port 0` 由 OS 选端口；Codex 使用 `/auth/callback`，缺省 1455，可显式选择 1457，不接受 0 或其他端口。端口占用直接失败，不换端口、不取消其他进程。`--callback-port` 只用于 browser；库级 `None` 与 `Some(0)` 含义不同。

callback 严格校验 method/path/Host、唯一 query、state 与资源预算；不接受裸 code 或手动 URL 绕过。授权 URL、code exchange 使用同一准确 redirect。callback 页面确认接收不等于身份校验或持久化成功；取消、拒绝与超时释放 listener 和临时事务。

身份由对应 driver 验证，不能只 decode JWT payload。Grok browser 校验 ES256 签名、issuer/audience/expiry/nonce 并与 UserInfo subject 对齐；device 通过固定 UserInfo 验证身份。Codex 验证 RS256 与 subject/workspace，browser 另绑定本次 nonce；device/refresh 不套用 browser nonce。state/code/verifier 不单独持久化，Codex ID token 作为 secret 保存。客户端 metadata 和 scope 的精确 wire 留在 driver 与上述来源 owner；公开源码不构成 registration 或部署资格。

## 生命周期与故障

- `list` 只报告 profile、alias、生命周期、access freshness、revision/generation、expiry 和 pending/recovery 标记，不输出 subject、workspace、client 或 tokens；未知 expiry 不等于有效。
- 同账户竞争返回 busy；其他账户可继续操作。失败或取消的重新登录不覆盖可用 session，提交时重新校验 ticket 与绑定身份。
- refresh 发出前持久化移除可复用 secrets。取消、身份变化、拒绝或不确定网络结果要求重新登录，不能盲目重发 refresh token。
- 发布不确定或遗留 marker 会隔离账户；显式 login 或本地 logout 可恢复。不要删除 marker 后继续使用不确定 token，读回可见性也不能代替 fsync。
- Ctrl-C 取消本次网络/轮询；崩溃遗留的 login ticket 由新登录替代。
- logout 先清除本地 session，再可选 revoke。远端失败、中断或无 handle 不恢复本地 tokens，也不声称远端完成；HTTP 200 不证明所有 sessions/consent 已撤销。

此存储不隔离恶意同 UID 进程，不提供多主机协调或 secure erasure。文件故障应检查文件系统，而非手动绕过恢复标记。

## 组件扩展与验收

新增授权 profile 实现 [driver 合同](../src/credential/driver.rs)并[显式注册](../src/credential/profiles.rs)，不在 manager/store 增加 Provider 分支，不引入任意脚本或配置化 OAuth 工作流。具体回归与离线命令见[开发指南](development.md)；真实操作另行确认账户、store、client/redirect、模型、预算和脱敏边界。
