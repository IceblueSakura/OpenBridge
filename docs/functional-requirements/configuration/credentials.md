## 1. 凭证总则

- 下游用户表只在启动时读取；用户增删、启停和 API Key 轮换都需要重启；
- 用户 ID 和 API Key 必须唯一，至少有一个启用用户，API Key 不得少于 32 bytes；
- 认证成功后只把不含 Key 的 `Arc<User>` 放入请求上下文；
- 代码注册表只保存非敏感 pool/member id、Provider 和 credential kind，不保存 secret 或 secret locator；
- 服务与常规 API-key probe 只从 bootstrap 指定的私有 upstream credential TOML 读取上游 API key，不读取 `*_API_KEYS`、旧单值
  环境变量或 `.env`；任何 probe 都不得发现或导入本机 Codex credential、环境或 terminal 状态；
- 管理员 probe 只能从所选 Provider 的已启用 Generation Target 继承 trusted origin、Provider operation path、timeout 与 credential
  binding；只有全部候选归属同一 Provider instance 与 credential binding 时才能自动选定一个 Target，存在多个 trusted deployment 时
  必须用 `--target` 显式消歧。candidate model 不能借 Embeddings/Images/Audio Target 扩大 operation；model ID 只覆盖固定合成请求的
  `model` 字段，并且不继承所借 Target 现有模型的 ignored-parameter、reasoning mapping、output ceiling 或 delivery narrowing；它仍不能
  覆盖 endpoint、path、credential、header、prompt、schema 或任意 JSON；
- 每次 Generation probe 只能选择一个协议、一个 delivery 和一个闭合 `case`，并只发送一个请求；CLI/库不接受 `all`、列表或
  reasoning × capability 笛卡尔积。reasoning level 必须编码在 `reasoning-*` case 中，外部矩阵脚本通过多次独立调用编排。
- 固定 Generation probe 的所有 bounded case 使用 4096-token accuracy-oriented upstream output limit；探测 Target 自身已注册 upstream
  model 时按其 trusted output ceiling 下调，显式 candidate model 不得继承另一模型的 ceiling。不能仅为减少 token 消耗使用易截断的
  默认上限。只有显式 `--allow-unbounded-streaming-output` 才能为拒绝该字段的
  streaming backend 省略限制，报告和使用说明必须暴露该计费/长 reasoning 风险；
- structured oracle 可以在完整有界 response 生命周期内瞬时组合标准 output text；无状态 function-tool oracle 只能用内置固定 prompt、
  最多两个固定工具和固定 schema 探测首轮 Auto/None/Required/Named、strict schema 与 `parallel_tool_calls=false/true`。它不得执行工具、
  发送 tool result、构造 continuation，或发送 `previous_response_id`、background/conversation 等状态字段。除下述固定 inline PNG case
  外，其他多模态不在本阶段。报告、日志和
  错误不得保留生成正文、tool arguments、call/item identity、固定 prompt/schema 或完整请求/响应；
- `image-input-inline-png` 只能发送代码内固定、已视觉复核的 PNG data URL 与 OCR prompt；Chat/Responses 使用各自标准 image content part。
  只有完整响应精确匹配固定可见 token 才能记为 `supported`；请求成功但未匹配必须保守记为 `inconclusive`。报告、日志和错误不得保留
  图片 data URL、图片 bytes、prompt 或输出正文；remote URL、detail 差分与其他媒体必须作为后续独立 case。
- Models probe 必须在完整有界 response 内计算总 ID 数和 candidate 可见性，但报告中的 ID sample 最多保留 1024 项并显式标记截断；
- TOML 只允许声明 `schema_version` 与 `credential_pools`；每项包含编译期 binding id，并且可以选择有序 `api_keys` 数组、单一
  `auth_json_file` locator 或不提供 source（未激活），不能配置 Provider、credential kind、endpoint、route 或 member id；
- 未由代码注册的 pool、重复 pool、空白成员或 pool 内重复 secret 必须在 listener 绑定或网络 probe 前失败；缺少已注册 pool、无 source
  的已知 pool 或空 API-key 数组表示该 pool 本次启动未激活，不构成动态 Provider 注册；
- 服务在监听前把已启用用户 Key 与所有已激活 API-key Target 引用的 pool 一次性装入不可变 `CredentialStore`，并把所有显式配置的
  OAuth2 auth 文件装入内部可变、对外 snapshot 化的 `OAuth2CredentialManager`；完整过期 bundle 作为立即 refresh 输入而不是损坏文档；
- `CredentialId` 必须区分 `DownstreamUser` 与带 `ProviderKind` 的 `UpstreamPoolMember`，上下游同名 ID 不得造成命名冲突；
- 每个 credential 条目必须冻结受控的 type、source、从 1 开始的 generation 与可选过期时间；source 只保存
  `UserConfiguration`、`UpstreamConfiguration`、`OAuth2AuthJsonFile` 或 `Programmatic` 类别，不能把文件路径、
  issuer URL 或任意业务字符串作为诊断元数据；
- `RuntimeRegistry` 与 `UserRegistry` 不保存 secret；`CredentialStore`、两类注册表、日志、错误响应和 probe report 的
  Debug/输出都不得包含 secret；probe report 也不得包含认证 header、完整合成请求正文或完整 upstream response body；
- 下游认证只能经 Store 的 constant-time 匹配返回用户 ID；上游只能按完整
  `pool_id + member_id + ProviderKind + CredentialKind` 借用短时 credential 视图，不提供通用明文查询；
- 缺失、空值、零 generation、重复下游 Key 或 binding/Provider/credential kind 不匹配时 fail closed；已注册但未激活的 API-key
  pool 只会让其引用的 Target 在本次启动中不可执行。显式配置 OAuth2 `auth_json_file` 会激活对应 binding；主服务要求文件已存在且
  包含完整 bundle，缺失、空白或损坏文件都在 listener 前失败；
- 运行时不得重新读取 `users.toml` 或 `upstream-credentials.toml`；改变用户、API Key 或 locator 必须重启。OAuth2 manager 只可在
  expiry-driven refresh 或首个预提交 `401` recovery transaction 中通过同主机 advisory lock guarded reload 自有 auth 文件，并将完整
  rotation 原子写回后发布新 generation；普通成功路径不读文件，任何请求都不得触发交互式登录。这是同一 binding 内的
  credential lifecycle，不是通用配置热更新；
- 业务请求不能提供或覆盖 Authorization、cookie、Host、proxy header 或上游 credential；Provider 的受信代码可声明固定的非敏感
  `User-Agent` 与普通 header，也可通过 hook 按编译期规则增添、替换、转换或删除普通 header。固定 header 在 hook 后应用，业务请求
  不能覆盖；authentication header 最后从 purpose-bound credential 生成。共享层不维护普通 header allowlist，具体 Provider 的 header
  值属于实现事实，不应在本需求文档中固化。

## 2. 上游 API-key pool

- pool 与 member 都使用稳定、非敏感 ID；member secret 只来自私有 upstream credential TOML，业务请求 不能提供
  pool/member、改变顺序或扩大候选集合；member ID 只能由 pool id 与数组顺序派生，不能 由 secret 内容派生；
- 一个激活的 API-key pool 至少包含一个 member；member ID 必须唯一，所有 member 必须属于同一 Provider 和 credential kind，重复
  secret 必须拒绝；单 member pool 与现有单 key 行为等价；未激活 pool 可以没有 member；
- 同一个 pool 可由同 Provider 的多个 Target 引用，使 key cooldown 与 round-robin cursor 跨模型共享；不得 为每个模型复制同一组
  key 后形成互不知晓的健康状态；
- 每个 API-key pool 只有一个 TOML `api_keys` 数组；未知或重复 pool、空白或重复 member 必须在 listener 绑定前 fail closed；缺少
  pool、source-less pool 或空数组只表示该 pool 未激活。不提供环境变量 fallback、member 级 enabled 或热增删；
- `CredentialStore` 继续不可变地持有 secret。运行时可变状态只保存 pool cursor、member binding ID、 generation 与 cooldown
  deadline，不保存、复制或重新读取 secret；
- pool 选择只返回短时 credential 借用视图；每次 attempt 必须重新构造敏感认证 header，不能缓存或复用 上一次 member 的
  header；
- 上游有状态 API 是永久非目标：`TargetBoundContinuation` Responses executable profile 不可用于任何公开能力；
  若注册代码声明该变体，其启用条件仍必须像其他非法注册一样在启动时拒绝（例如多 member pool 漂移风险），
  且该拒绝不构成对任何下游参数的放行；普通 `TargetBound` 不虚构该限制；
- 更换 API key、改变 pool member 或顺序仍需重启。API-key pool 不承担 OAuth、余额查询、keyring、加密 secret 文件、远程 secret
  manager、动态 reload 或跨进程 pool 状态；ChatGPT OAuth 使用独立 credential kind 和生命周期要求。

## 3. ChatGPT 本地状态隔离

- 五个 ChatGPT Responses-native Target 使用同一个独立 `OAuth2BearerAccessToken` pool。Spark、GPT-5.5、Luna 与
  Terra 分别只为一个 ChatGPT-only Public Model 提供 source；Sol Target 则是还包含 OpenAI 后备 source 的
  `gpt-5.6-sol` Public Model 的 ChatGPT source。通用 API-key probe 不借用 OAuth manager credential，ChatGPT
  probe 只能显式借用所选 Target 的 manager lease；
- OpenBridge 不搜索 `$CODEX_HOME`、Codex auth cache 或其他本机 Agent 状态，不接受 probe 专用 Codex auth file 或 executable selector；
- OpenBridge 不读取 terminal 相关环境变量，不根据本机 OS、architecture 或 terminal 构造 Codex-compatible 请求身份，也不启动 Codex
  CLI 或 app-server；
- ChatGPT credential 只能来自下节定义的 OpenBridge-owned OAuth2 auth 文件；服务数据面和显式 ChatGPT Models probe 只可通过 manager 的短生命周期
  lease 借用当前 generation，不能通过 CLI 参数或本机 Agent 状态隐式获取 credential；
- 显式登录、可刷新 bundle、持久化、数据面借用和 guarded reload/refresh/401 recovery 以
  [ChatGPT subscription OAuth lifecycle](oauth-chatgpt.md)为准。

## 4. OpenBridge-owned OAuth2 auth 文件

- OAuth2 auth 文件路径只来自 private upstream credential TOML 的 `auth_json_file`；相对路径以该 TOML 所在目录为基准，业务请求、
  Provider response 和 probe 参数不能覆盖；
- 配置项仍使用编译期 credential binding id，loader 必须从 `RuntimeRegistry` 解析唯一 Provider 与
  `OAuth2BearerAccessToken` kind；TOML 不获得动态 Provider 选择权；
- 每个 OAuth2 Provider 最多配置一个 auth 文件，并派生一个稳定的内部 member id；不提供 auth 文件数组、账号 pool、轮转、
  cooldown 或负载均衡；
- ChatGPT 文件使用当前兼容的 OAuth 字段形状，但由 OpenBridge 独立拥有；不得默认、搜索、导入或回退到
  `$CODEX_HOME/auth.json`；
- 主服务在 listener 绑定前完成首次读取并要求完整校验；缺失、空白或损坏文件均阻止启动。显式 login CLI 可以在成功取得并校验
  bundle 后，从 missing version 事务性创建完整文件；之后只有 expiry-driven refresh 或首个预提交 `401` recovery transaction 在
  advisory lock 内 guarded reload，rotation 只能原子替换。错误、`Debug`、日志和 metric 不得包含 locator、token、账户或完整 auth record；
- `OAuth2CredentialManager` 对外只发布脱敏 snapshot，对内维护 guarded reload、single-flight、refresh、generation 与后台调度；
  数据面只能取得不暴露 locator/完整 bundle 的短生命周期 credential lease，并按同一账户/Provider 边界执行一次有界 `401` recovery。
- 不提供运行中换账户 API 或配置热重载。换账户必须先停止服务，手动删除该 binding 的 OpenBridge-owned `auth_json_file` 及同一登录流程明确
  创建的其他 OpenBridge-owned 授权文件（如有），再显式登录并重启；不得借此搜索、导入或删除本机 Codex auth cache。
