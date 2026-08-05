# 当前开发焦点

## 状态

**活动焦点：ChatGPT subscription OAuth 显式 device login 与 PKCE credential 持久化。**

用户已授权完成 ChatGPT subscription 的登录与自动 refresh 生命周期；本焦点只实现第一个可观察行为。完成并清空后，下一焦点再实现
expiry-driven refresh、进程/跨进程 single-flight 和原子 rotation 写回。两项行为串行实施，不在同一失败测试下并行展开。

## 行为

管理员显式运行 `openbridge-auth login chatgpt` 时，命令只从 bootstrap、编译期 ChatGPT OAuth registration 与 private
`upstream-credentials.toml` 解析受信 issuer、client registration 和 OpenBridge-owned `auth_json_file`；它请求一次 device user
code，向当前终端显示 verification URI、一次性 code、有效期和防钓鱼提示，随后有界轮询授权结果。上游返回 authorization code 与
PKCE material 后，命令验证 `S256(code_verifier) == code_challenge`，交换并校验完整 token/account bundle，再以跨进程锁、源版本比较和
同目录原子替换写入目标文件。只有完整写入成功才报告登录成功。

该 ChatGPT 流程是 LiteLLM、Hermes 与 Codex 当前实现使用的私有 device interaction：轮询私有 endpoint 得到 authorization code 与
PKCE material，再执行 authorization-code exchange；它不是原样 RFC 8628 的 device-code token polling。实现必须使用独立、明确命名的
ChatGPT adapter，不能伪装成通用 Device Authorization Grant。

## 对应功能需求

- 主需求：[OAUTH-03、OAUTH-06 与 OAUTH-08](../functional-requirements/upstream-oauth-credential-lifecycle.md#8-功能验收要求)
- secret 与配置边界：[配置、凭证与受信运行边界](../functional-requirements/configuration-and-credentials.md)
- 证据分层：[交付与证据要求](../functional-requirements/delivery-and-evidence.md)

当前功能需求中的 OAUTH-09 属于下一焦点；本焦点不得以登录实现宣称 token 已自动 refresh。

## 外部实现基线

规划时已只读复核以下 clean checkout；实现前若 checkout 变化，重新定位模块和测试，不沿用历史行号：

| 项目 | commit | 本焦点采用的事实 |
|------|--------|------------------|
| LiteLLM | `23de7a15d9d40006ee596e617475ba101d60c5e9` | 私有 device request/poll、15 分钟上限、PKCE code exchange、完整 token bundle 与 account claim |
| Hermes Agent | `470cf66b039c73bdd2c21d43094ce41a4db74eae` | 显式 device login、429/`Retry-After` 分类、OpenBridge 自有 store 类比、跨进程文件锁与原子 replace 边界 |
| Codex | `757c151a0e920c6238801866a3d13e010dfeddb8` | 当前 private endpoint 字段、verification URI、poll pending status、PKCE exchange 与 workspace/account 检查 |

参考实现只证明相应源码快照的行为。私有 endpoint、public client registration、scope 或 header 不构成通用 OAuth 协议承诺；OpenBridge
把它们固定在受信 Rust registration 中，CLI、TOML、环境变量和业务请求均不能覆盖。

## 先失败的测试或复现

先增加以下聚焦测试并确认旧实现失败：

1. `tests/upstream_credential_config.rs::oauth2_login_target_resolves_without_opening_or_requiring_the_auth_file`
   - 旧配置 API 只能在服务启动时读取一个已存在 auth file，不能为显式登录安全解析尚未创建的 OpenBridge-owned locator。
2. `oauth2_credentials::login::tests::chatgpt_device_login_polls_then_persists_one_validated_bundle_atomically`
   - fake transport 依次返回 device session、pending、包含 authorization code/PKCE material 的成功结果和合成 token；旧实现没有 login
     state machine、PKCE 校验或持久化。
3. `oauth2_credentials::login::tests::pkce_mismatch_or_invalid_token_preserves_the_previous_bundle`
   - challenge 不匹配、token 字段缺失、account mismatch 或过期 access token 必须在写入前失败；旧实现没有交换后验证边界。
4. `oauth2_credentials::storage::tests::compare_and_replace_serializes_writers_and_never_exposes_secrets`
   - 同一 locator 的两个写入者只有源版本仍匹配者可原子替换；错误和 `Debug` 不含路径、code、verifier、token 或 account。
5. `tests/oauth2_login_cli.rs`
   - 新 CLI 只接受固定 `login chatgpt` 操作；没有 `--issuer`、`--client-id`、`--token-endpoint`、`--auth-file`、header 或 Codex-cache
     selector，取消或网络失败不创建/修改 credential。

确定性测试不得访问真实网络、真实 auth file 或本机 Codex/Hermes/LiteLLM cache，也不得用 sleep 制造时序。

## 最小实现边界

- 将 `src/oauth2_credentials.rs` 保持为公共 facade，把 document validation、file storage 与 ChatGPT login state machine 分到按职责命名的
  子模块；保留现有公共 crate path。
- 在 `src/providers/chatgpt/` 中登记固定 OAuth issuer、device endpoints、token endpoint、verification URI、redirect URI、public
  client registration 与协议时限；这些事实不进入 TOML 或 CLI 参数。
- 为 private upstream credential configuration 增加 purpose-bound login target resolution：校验 registry 的 Provider/kind/pool
  ownership，解析相对 locator，但不要求目标文件已存在，也不向通用调用方显示 locator。
- 增加独立 headless `openbridge-auth` binary。它只在管理员显式执行时发起登录，普通服务启动和模型请求绝不隐式开始交互式登录。
- device session 只在内存存在；device auth ID、authorization code 与 PKCE verifier 使用 zeroizing/secret 类型，失败、超时、Ctrl+C 或
  drop 后清除。只向发起终端显示 verification URI 与 user code，并显示 device-code phishing 提示。
- 对 device request、poll 与 token exchange 设置固定 HTTPS origin、禁用 redirect、有界 timeout/body、15 分钟总 deadline 与受限 poll
  interval；pending 只接受当前 Provider profile 明确的 status，429 只按有界 `Retry-After` 处理。
- token exchange 前验证 PKCE `S256`；交换后要求非空 id/access/refresh token、未来 access-token expiry 和一致的 account binding。
  当前参考实现没有提供可独立验证第三方 JWT 签名的 OpenBridge trust store，因此本焦点只校验 HTTPS 交换结果和已知 claim/binding，不能
  宣称完成离线 JWT signature verification。
- 持久化使用 auth-file 同目录临时文件、owner-only 权限、flush/sync 与原子 replace；写入前在跨进程 advisory lock 内重新读取并比较源
  version，防止并发 login 静默覆盖另一成功会话。完整 bundle、account 和 `last_refresh` 一次写入，不保存半成品 session。
- 保持现有 auth JSON envelope 兼容：`auth_mode`、`OPENAI_API_KEY`、`tokens` 与 `last_refresh`；Provider、endpoint、pool、status 和 locator
  继续由受信注册表/private TOML 持有，不写入 token 文件。

明确不做：自动 refresh、refresh scheduler、401 recovery、manager 到数据面 credential 借用、启用 ChatGPT target、Route/Public
Model、多账号 pool、导入 Codex/Hermes/LiteLLM cache、浏览器 callback server、通用 RFC 8628 adapter、动态 registration/endpoint、
环境变量 issuer 覆盖、keyring、真实账户自动登录或任何业务请求触发登录。

## 本次验证

### 确定性本地验证

先运行聚焦测试，再运行 Rust baseline：

```powershell
cargo test --locked --test upstream_credential_config oauth2_login_target_resolves_without_opening_or_requiring_the_auth_file
cargo test --locked oauth2_credentials::login::tests
cargo test --locked oauth2_credentials::storage::tests
cargo test --locked --test oauth2_login_cli
cargo fmt -- --check
cargo test --locked
cargo clippy --locked -- -D warnings
git diff --check
```

测试只使用合成 JWT/token、process-unique 临时目录、fake clock 与 fake transport。确定性成功只证明状态机、文件事务和脱敏边界，不证明真实
ChatGPT subscription、当前 external client registration 或网络可用。

### CLI 与真实 Provider 验收

- 无网络 CLI contract 验证命令形状、未知 selector 拒绝、缺失/无效 private binding 的安全失败和无 secret 输出。
- 真实 `openbridge-auth login chatgpt` 会创建外部 device session、显示一次性 code 并要求用户在浏览器确认，属于交互式外部验收；本次默认
  不自动执行。只有用户明确选择当次真实登录时才运行，并只记录日期、平台、源码 commit、成功/失败分类与文件前后是否原子替换，不记录
  URI query、user code、device auth ID、account、token、PKCE material 或文件 hash。

### 本焦点不运行的层

- 自动 refresh、rotation 并发、refresh backoff、401 recovery 与长时间运行；
- ChatGPT 数据面、OpenAI SDK、Hermes/LiteLLM runtime 或通用 Codex-as-client compatibility；
- Python protocol corpus；本焦点不修改 `testdata/` 或 `tools/corpus/`；
- 真实 Provider 登录，除非用户在实现完成后明确选择交互式验收。

## 结果记录

- 已证明的事实：完成后写入[当前实现说明](../implementation-status/current-implementation.md)，只记录 CLI、受信 registration、device/PKCE
  状态机、transactional store、确定性测试和实际执行的外部验收。
- 下一焦点：以写入后的 expiry 为输入，实现 OAUTH-09 的 guarded reload、single-flight refresh、rotated token 原子写回、失败状态与
  expiry-driven scheduler；不得在本焦点提前实现。
- 仍另起焦点：manager 到 ChatGPT Provider data-plane credential borrow 与 bounded 401 replay。
- 完成后把本文恢复为无活动焦点，不保留本次实施历史。

## 关联文档

- [ChatGPT subscription OAuth credential lifecycle](../functional-requirements/upstream-oauth-credential-lifecycle.md)
- [OAuth 设备登录与 token refresh 综合调研](../references/cross-project/upstream-oauth-device-code-token-refresh-analysis.md)
- [LiteLLM ChatGPT OAuth 调研](../references/litellm/litellm-chatgpt-oauth-refresh-analysis.md)
- [Hermes Codex OAuth 调研](../references/hermes/hermes-codex-oauth-refresh-analysis.md)
- [Codex 设备登录与 token refresh 调研](../references/codex/codex-device-auth-token-refresh-analysis.md)
- [当前实现说明](../implementation-status/current-implementation.md)
