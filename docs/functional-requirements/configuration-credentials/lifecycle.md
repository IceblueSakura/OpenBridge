# 生命周期

## 状态

本文是[配置与凭证域](README.md)的生命周期模块：定义启动装配顺序、配置态可用性输出和运行时不可变性。
其他模块见[配置与凭证域](README.md)导航。

## 1. 启动装配顺序

```text
read bootstrap.toml
→ validate BootstrapConfig
→ read users.toml
→ validate UserConfiguration and collect downstream credentials
→ read upstream-credentials.toml
→ validate UpstreamCredentialConfiguration
→ derive redacted active credential-pool set
→ compiled_config()
→ validate and build RuntimeRegistry with active Target eligibility
→ bind active API-key pools and configured OAuth2 auth files by compiled binding id
→ build immutable CredentialStore + OAuth2CredentialManager
→ render configuration-only Provider/Public Model availability without Provider egress
→ create shared HTTP client
→ Arc<RuntimeRegistry> + Arc<UserRegistry> + Arc<CredentialStore> + Arc<OAuth2CredentialManager>
→ start listener
```

完成全部私有配置和 credential binding 校验后、绑定 listener 前，主服务以默认 info 日志输出两张 ASCII 双列表格。Provider family
至少有一个经 active pool 收窄后 enabled 的 Target 时进入可用列；Public Model 至少有一个已编译执行接口时进入可用列。另一列只显示
稳定 Provider/Public Model 名称、Target 计数、接口和脱敏原因，不得显示 pool、Target、Route、endpoint、auth-file locator 或 secret。

表头必须明确标注 `configuration only` 和 `no network probe`。这里的"可用"只表示本次启动配置允许形成执行候选；OAuth2
auth-file locator 会先参与 active-pool 筛选，但主服务必须在输出表格前读取并校验完整 bundle，缺失、空白或损坏文件会阻止启动。
该表不证明当前 credential lease、网络、配额、远端模型或协议能力实际可用；真实探测只能由管理员显式运行独立 probe。

## 2. 冻结 wiring 与可变 OAuth generation

`RuntimeRegistry`、`UserRegistry`、API-key `CredentialStore`、OAuth manager 实例及其 binding/locator/wiring 在启动后
保持不变。服务没有用户、Route、Provider、API-key pool 或 auth-file locator 的文件监听、`ArcSwap` 或部分更新语义；
改变这些启动输入必须重启。

`OAuth2CredentialManager` 内部 token snapshot 与 generation 不是不可变 registry 事实。独立显式登录把完整文件写好后需要
重启服务，由新进程建立初始 snapshot；运行中的 manager 只允许 expiry-driven refresh 或首个预提交 `401` recovery 按同一
Provider/account binding 执行 guarded reload、single-flight 和原子 rotation。该内部更新不得替换 manager、改变 locator、切换账户、
修改 Route 或发布另一套 RuntimeRegistry。

## 关联文档

- [配置与凭证域导航](README.md)
- [所有权划分与代码注册表](ownership-and-registry.md)
- [凭证](credentials.md)
- [Endpoint 与出站边界](endpoint-and-egress.md)
- [ChatGPT subscription OAuth lifecycle](upstream-oauth-credential-lifecycle.md)
- [实施现状](../../implementation-status/README.md)
