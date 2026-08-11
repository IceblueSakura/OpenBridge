# Model 目录与 Provider 接入配置（待定）

## 状态

**待定方案，暂不实施。** 本文保留 `config/models.toml`、Canonical Model 目录、Provider-Model 接入和 Public Model
暴露的候选契约，供以后重新评估；它不构成当前产品承诺、功能验收要求或实施任务。除非配置所有者
再次明确批准，否则不得据此修改代码、配置文件、OpenAPI、实施现状或
[当前开发焦点](../../implementation-plans/current-focus.md)。当前 checkout 的真实行为和验证结果仍以
[当前实现总览](../../implementation-status/current-implementation.md)链接的功能专题为准。

本文后续“必须”“不得”“只允许”均描述该方案若重新获批时的候选边界，不适用于当前实现。当前继续采用 Rust 代码显式注册
Model、Provider Target/API、Route 与 Public Model 的方式。

## 1. 用户结果与静态装配原则

配置所有者可以在一个非敏感、版本化的模型目录中新增或修改 Canonical Model 事实，声明代码中已有 Provider 分别接入哪些模型，并把一个或多个
Provider 接入按固定优先级暴露成 Public Model。所有变更都通过重启生效。 OpenBridge 必须只在启动时读取、解析、校验并转换该文件；listener
启动后，请求路径只能读取已经构建完成的 不可变 registry snapshot。

该扩展遵循“尽可能少的运行时决策”原则：

- 不监视文件，不提供热重载、局部更新、`ArcSwap` 或管理 API；
- 不按请求动态发现、选择、打分、加权或重排 Model、Provider 或 Route；
- 不引入用户配额、计费、租户策略或动态 credential 控制面；
- 不在请求路径解析 TOML、查找原始文档、合并配置或计算新的能力与候选集合；
- 已有固定候选上的有限 retry、fallback、credential rotation 与 cooldown 仍只属于可用性执行，不能改变 启动时确定的候选资格、顺序或公共能力契约。

## 2. 事实所有权与信任边界

| 来源                 | 拥有的事实                                                                                                                                      | 明确不拥有的事实                                                                |
|----------------------|-------------------------------------------------------------------------------------------------------------------------------------------------|---------------------------------------------------------------------------------|
| `config/models.toml` | Canonical Model 事实、Provider-Model binding、真实 upstream model、Public Model 元数据和有序 source                                             | Provider 实现、endpoint、credential、header/wire mapping、任意能力或 Route 定义 |
| Rust Provider 目录   | 闭合 Provider 与 integration profile、受信 endpoint、credential binding、Native API/保守能力基线、typed state ownership、固定 Route 生成规则和协议转换 | secret；也不能从远程目录自动发现 Model                                          |
| 私有 credential TOML | 下游用户 Key 与已由代码声明的上游 credential pool secret                                                                                        | Model、Provider、endpoint、Route 或能力                                         |
| 启动构建器           | 校验上述静态输入并生成不可变 registry 和 credential snapshot                                                                                    | listener 启动后的配置变更或动态策略                                             |

三类记录必须保持分层：Canonical Model 只声明模型本体事实；Provider-Model binding 只选择一个代码内 integration profile 并绑定
Canonical Model 与真实 upstream model；Public Model 只声明下游身份和有序 binding source。只有三层引用 完整且通过启动校验，启动编译器才生成
Upstream Target/API、Route 与 Public Model。单独新增 Canonical Model 记录或未被 Public Model 引用的 binding 都不得形成可调用入口。

## 3. 首版文档契约

首版文件使用 TOML，根对象必须包含整数 `schema_version = 1`，以及类型相互独立的 `models`、
`provider_models` 和 `public_models` 数组。服务配置至少包含一个完整 Public Model；每条 Canonical Model 记录必须 使用显式
`task` 标签，并只携带该任务允许的子表。以下示例不包含真实 credential 或任意 endpoint：

```toml
schema_version = 1

[[models]]
id = "example-designer/example-chat-v1"
name = "Example Chat V1"
description = "Synthetic chat model used to illustrate the catalog schema."
task = "chat"
tokenizer = "example-tokenizer"
knowledge_cutoff = "2026-01"

[models.chat]
max_context_tokens = 128000
max_input_tokens = 128000
max_output_tokens = 8192
input_modalities = ["text", "image"]
output_modalities = ["text"]
supported_parameters = ["max_tokens", "temperature", "tools"]
reasoning = "unknown"
reasoning_levels = []

[[models]]
id = "example-designer/example-embedding-v1"
name = "Example Embedding V1"
task = "embedding"

[models.embedding]
max_input_tokens = 8192
input_modalities = ["text"]
native_dimensions = 1024

[[provider_models]]
id = "openai-example-chat-v1"
provider_instance = "openai"
integration_profile = "default"
canonical_model = "example-designer/example-chat-v1"
provider_model = "openai/example-chat-v1"
upstream_model = "example-chat-v1"

[[provider_models]]
id = "openrouter-example-chat-v1"
provider_instance = "openrouter"
integration_profile = "default"
canonical_model = "example-designer/example-chat-v1"
provider_model = "openrouter/example-chat-v1"
upstream_model = "vendor/example-chat-v1"

[provider_models.model_rules]
max_context_tokens = 64000
max_input_tokens = 64000
max_output_tokens = 8192
disabled_parameters = ["tools"]

[[public_models]]
id = "example-chat"
model = "example-designer/example-chat-v1"
name = "Example Chat"
description = "Synthetic public model backed by two ordered Provider sources."
created = 1785715200
status = "active"
routing_strategy = "source_first"
sources = [
    "openai-example-chat-v1",
    "openrouter-example-chat-v1",
]
```

示例中的 `integration_profile = "default"` 只是结构占位；实际值必须来自对应 Provider 在当前二进制中注册的 闭合
integration profile 目录。

### 3.1 公共字段

| 字段               | 要求                                                                                |
|--------------------|-------------------------------------------------------------------------------------|
| `id`               | 必填、全局唯一、稳定的 `designer/model` Canonical Model ID；不是 Public Model id 或 upstream model。 |
| `name`             | 必填、非空的展示名称。                                                              |
| `description`      | 可选；缺失表示没有公开描述。                                                        |
| `task`             | 必填的闭合枚举；首版只允许 `chat` 或 `embedding`。                                  |
| `tokenizer`        | 可选；缺失表示未知，不得推测。                                                      |
| `knowledge_cutoff` | 可选；缺失表示未知，不得使用当前日期代替。                                          |

### 3.2 Chat 子表

`task = "chat"` 必须带一个 `chat` 子表。该子表只包含 total/input/output token 上限、输入/输出模态、 OpenAI-compatible
参数名、reasoning 状态和 reasoning levels。各项模型事实都可以缺失：token 上限或模态缺失 表示未知，`supported_parameters` 与
`reasoning_levels` 缺失等价于空数组，`reasoning` 缺失等价于
`unknown`。任一已提供的 token 上限必须为正，input/output 不得超过 total context。

`reasoning` 使用 `supported`、`unsupported` 或 `unknown`。只有 `supported` 可以携带非空
`reasoning_levels`；`unknown` 表示证据不足，不得被解释为支持或不支持。`supported_parameters` 只能引用 OpenBridge
代码维护的已知请求参数词汇，配置文件不能仅凭一个新字符串启用未建模行为。

### 3.3 Embedding 子表

`task = "embedding"` 必须带一个 `embedding` 子表。该子表只包含输入模态、最大输入 token 和模型的原生向量
维度；各项都可以缺失并表示未知，已提供的 token 或维度必须为正。它不得携带生成输出、stream、tool、reasoning
或生成参数。可变输出维度、encoding、批量限制和 Provider 专有限制属于具体 Upstream API 与接口契约，不能仅由 Canonical Model
目录启用。

Chat 记录出现 `embedding` 子表、Embedding 记录出现 `chat` 子表，或记录同时出现两个子表时，启动必须失败。

### 3.4 Provider-Model binding

每个 `[[provider_models]]` 表示“一个代码内 Provider integration profile 提供一个 Canonical Model 接入”。它不是任意 Target
或 Upstream API 文档，首版只包含：

| 字段                  | 要求                                                                                           |
|-----------------------|------------------------------------------------------------------------------------------------|
| `id`                  | 必填、全局唯一且稳定的 binding ID；用于 Public Model source 引用和确定性派生内部 ID。          |
| `provider_instance`   | 必填的闭合 Provider instance ID，必须已经由 Rust 代码注册并唯一拥有受信 BaseURL。               |
| `integration_profile` | 必填的 Provider family 内闭合接入 profile ID，必须属于实例绑定的 `ProviderKind`。               |
| `canonical_model`     | 必填的 `designer/model` Canonical Model ID 引用。                                            |
| `provider_model`      | 必填的 `provider/model` routing identity；Provider 前缀和 model basename 必须由启动校验确认。 |
| `upstream_model`      | 必填、非空且有长度上限的真实上游模型名；只写入受信 integration profile 生成的请求 model 字段。 |
| `model_rules`         | 可选的 Provider 接入收窄规则；只允许更小的 token 上限、禁用已知参数或收窄 reasoning 状态。     |

Provider instance 必须冻结 `ProviderKind` 与唯一 endpoint origin；代码内 integration profile 必须冻结 operation 相对路径、
credential pool/kind、timeout、quota/fault 边界、每个 `OperationKind` 至多一份的 Native API 集合、受 Provider contract 约束的保守
能力基线、typed state ownership，以及可生成的 Native/Bridge Route surface。配置只能引用已注册实例并选择其 family 的 integration
profile，不能覆盖上述任一项，也不能扩大 integration profile 或 Canonical Model 能力。

同一个 `upstream_model` 适用于该 integration profile 生成的全部 Native API。可选 `model_rules` 也统一作用于 这些 API，并与
profile 自身的每协议规则继续保守相交；它只允许 `max_context_tokens`、`max_input_tokens`、
`max_output_tokens`、`disabled_parameters` 和 `reasoning`。已提供的 limit 必须为正，已知 Canonical Model 上限存在
时不得超过它；禁用参数必须由 Canonical Model 声明；reasoning 按
`unsupported < unknown < supported` 排序时不得高于 Canonical Model。首版不允许在配置中提供 reasoning wire mapping、模态或接口
capability。

`reasoning_levels` 是 Canonical Model 事实；绑定同一模型的 Chat/Responses Native API 必须继承同一集合，代码内 integration
profile 不得定义每协议子集。profile 只能降低完整 reasoning 支持状态，或把已声明 level 编码到该协议的固定 wire 形状；只有
thinking 开关的 Chat API 将 `none` 编码为关闭、其余已声明 level 编码为开启，支持标准 effort 的 Responses API 保留原值。

首版不提供 `enabled`、weight、priority、capability enablement、timeout、endpoint、credential pool 或自定义 route 字段。binding
出现在文件中即表示启动时启用；停用通过删除记录并重启完成。需要不同协议 surface、endpoint 或 credential binding
时，必须先在代码中增加并验证新的 Provider instance 或闭合 integration profile。

### 3.5 Public Model

每个 `[[public_models]]` 表示一个下游可见身份，首版包含：

| 字段                          | 要求                                                                                    |
|-------------------------------|-----------------------------------------------------------------------------------------|
| `id`                          | 必填、全局唯一并满足不带 namespace 的 Public Model ID 契约；这是下游 Models、请求和响应中的 model。 |
| `model`                       | 必填的 Canonical Model ID；所有 source 必须引用同一 `designer/model` ID，供内部关联使用。       |
| `name`                        | 必填、非空的下游展示名称。                                                              |
| `description`                 | 可选的下游说明。                                                                        |
| `created`                     | 必填、非零的稳定 Unix 秒；不得使用启动时间。                                            |
| `status`                      | 必填的 `active`、`deprecated` 或 `retired`。                                            |
| `deprecated_at`、`retired_at` | 与状态一致的可选稳定 Unix 秒。                                                          |
| `routing_strategy`            | 必填的 `native_first` 或 `source_first`；决定每个下游协议如何展开 source 与 Native/Bridge 优先级。 |
| `sources`                     | 必填、非空、无重复的 `provider_models.id` 数组；数组顺序就是 Provider fallback 优先级。 |

启动编译器按 `routing_strategy` 与 `sources` 生成候选：`native_first` 对每个下游协议先生成所有 source 允许的 Native Route，再按
同一 source 顺序生成 Bridge；`source_first` 对每个下游协议先遍历 source，再在同一 source 内生成 Native、随后生成显式或允许的
Bridge。Embedding 只生成 Native Route。配置不得提供 Route ID、route prefix、协议转换方向或另一套 priority。每个 binding 首版
必须且只能被一个 Public Model 引用，避免孤立 Target 或隐式 alias。

## 4. 缺失值、严格解析与启动校验

- 缺失的可选事实统一表示未知；不得为兼容旧记录自动填充“支持”或伪造数值。
- 已显式提供的模态集合必须非空；所有数组必须去重并在转换后形成确定性顺序。
- 重复 Model/binding/Public Model ID、未知字段、未知 task/枚举、空白字符串、越界数值和跨字段矛盾必须在 listener 或网络 probe
  开始前失败。
- 未知 Provider/integration profile/Model/source 引用、integration profile 与 Provider 不匹配、source 指向不同 Canonical
  Model、 source 重复或 binding 未被恰好引用一次时必须失败。
- 文档解析类型必须与内部 `RegistryConfig`、`ModelConfig` 和公开 API DTO 分离，并采用严格未知字段拒绝； 内部 Rust
  类型新增字段不得静默扩大外部配置格式。
- 文件字节数、模型数量、字符串长度和数组长度必须具有启动时固定上限，防止不受限配置消耗。
- Provider integration profile/API 与 binding `model_rules` 只能收窄 Canonical Model 事实，不能扩大；目录中未知的能力在公共契约中继续
  fail closed。

错误必须指出配置文件、记录 ID（若已经可安全确定）和字段路径，但不得包含 credential、私有请求正文或内部 敏感值。解析或校验失败时不得回退到代码内置
Model，也不得带着部分模型启动。

## 5. 启动构建与不可变性

bootstrap 必须解析出唯一受信的模型目录路径；服务与 probe 使用同一来源，且不允许业务请求、CLI 参数或额外
环境变量注入第二份模型目录。目标启动链为：

```text
read and validate bootstrap.toml
→ read, strictly parse and validate models.toml
→ convert document DTOs into Canonical Models, Provider-Model bindings and Public Models
→ load the closed compiled Provider integration profile catalog
→ generate Target/API/Route definitions from validated bindings and ordered sources
→ validate the complete graph and build immutable RuntimeRegistry
→ bind private credentials and build immutable CredentialStore
→ start probe egress or listener
```

转换完成后不保留供请求路径查询的原始 TOML 对象。首次迁移必须把现有 Canonical Model、Provider 模型 binding、 Public Model 和
source 顺序整体迁入该文件；Provider contract/integration profile、adapter 与 Route 生成规则继续保留在代码。
运行时不提供“代码默认注册 + 文件覆盖”、多文件 merge、环境覆盖或部分成功语义。文件缺失、不可读或无效时服务 和需要 registry 的
probe 都必须 fail closed。任何目录变更都需要重启。

## 6. 兼容性规则

- 首个对外契约固定为 `schema_version = 1`；在该契约尚未发布或被外部部署依赖前，修正首版字段不制造无意义的 版本迁移。
- 首版正式形成兼容承诺后，新增字段、task、枚举值或改变字段语义/结构都必须使用明确的新 schema version；
  不得让旧二进制在同一版本下猜测新语义，也不得实现猜测式通用迁移。
- `schema_version = 1` 只接受本文明确列出的字段和枚举；未知字段始终拒绝，避免程序静默忽略配置意图。
- 首版已经定义的可选字段可以缺失；缺失时必须得到本文规定的未知/空值语义，不得通过默认值扩大能力。
- Canonical Model ID 改名按删除旧 ID、新增新 ID 处理；首版不提供 alias resolution 或自动迁移。
- 收窄已公开事实可能使启动失败或缩小 Public Model 契约，必须按客户端可见行为变更审查；扩大能力必须有明确证据， 并同时受
  Provider contract 和 Upstream API 上界约束。
- 只有确认代表相同 checkpoint、语义和向量身份的部署才能引用同一 Canonical Model ID；Provider 专有 served limit 放在
  Upstream API 收窄规则中，不复制成新的 Canonical Model。不同向量身份必须使用不同 Model ID。

## 7. 接入与可调用边界

在代码已经提供对应 Provider integration profile 的前提下，配置所有者可以通过新增 Canonical Model、Provider-Model binding 和
Public Model 三类记录，并重启进程，完成一个新模型接入，无需再为该模型编写 Rust registration。启动编译器 只能按 integration
profile 的闭合模板生成 Target、Upstream API 与 Route；配置中的 `upstream_model` 只能成为上游请求 body 中的模型值，不能影响
URL、path、header、credential 或 adapter。

以下各层都必须显式存在，不做基于同名 ID 的自动推导：

1. Canonical Model 提供模型事实；
2. Provider-Model binding 证明配置所有者选择了哪个受信 Provider/integration profile 和 upstream model；
3. Public Model 通过有序 `sources` 决定下游身份、Provider 优先级和是否可见。

新增 Provider、认证机制、endpoint/wire family、integration profile 或 Route surface 仍然需要代码、测试、 重新编译和重启。模型配置不能定义任意
Provider DSL、协议转换脚本或原始 Route，也不能把 integration profile 未声明的接口或能力打开。

## 8. 功能验收要求

| ID      | 应被保护的行为                                                                                                                                                             |
|---------|----------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| MCFG-01 | 有效模型目录只在启动时读取一次，并被转换为请求共享的不可变 Model、binding、Public Model 和 Route 图。                                                                      |
| MCFG-02 | 缺失文件、未知 schema/字段、重复 ID、任务字段混用或跨字段矛盾在 listener/probe egress 前阻止启动。                                                                         |
| MCFG-03 | binding 只能选择代码注册的 Provider/integration profile、Canonical/upstream model 和安全收窄规则；不能覆盖 endpoint、credential、header、能力上界、timeout 或 Route 规则。 |
| MCFG-04 | Canonical Model、Provider-Model binding 和 Public Model source 任一层缺失时，不会形成可见、可执行模型。                                                                    |
| MCFG-05 | Chat 与 Embedding 字段严格隔离；未知事实保持未知并在能力预检中 fail closed。                                                                                               |
| MCFG-06 | 不存在代码默认模型注册与文件的 merge/override、文件监听、热重载或请求时配置读取。                                                                                          |
| MCFG-07 | 模型目录不能发明参数词汇、Provider/integration profile、接口或能力；`model_rules` 只能收窄，也不能绕过代码内 Route 生成规则。                                              |
| MCFG-08 | 同一有效输入确定性地产生同一模型集合和公开事实；除显式 `sources` 数组外，不依赖 TOML 表声明顺序、网络目录或当前时间。                                                      |
| MCFG-09 | `public_models.routing_strategy` 只能是 `native_first`/`source_first`，`sources` 决定 Provider 优先级；启动按所选类型化策略固定展开，运行时不重新打分或重排。                  |
| MCFG-10 | 未知或错配的 Provider/integration profile/Model/source、跨 Canonical Model 聚合、重复或孤立 binding 会在 egress 前阻止启动。                                               |

确定性配置与 registry 测试只能证明解析、校验、装配和不可变性；不能证明目录事实与真实 Provider 当前能力一致。 真实能力扩大仍需独立
Provider 证据。

## 9. 非目标

- 任意 Provider、Target、Upstream API、Route 或协议转换 DSL；
- Provider `/models`、LiteLLM、OpenRouter 或远程目录的自动同步与动态发现；
- 多文件继承、overlay、环境变量逐字段覆盖、运行时 patch 或远程配置中心；
- 配置任意 endpoint、认证/header、credential pool、timeout、capability enablement、wire mapping 或 Provider 私有扩展；
- 动态路由、质量/价格/负载打分、用户配额、计费、ACL 或租户策略；
- 在同一进程内变更模型目录，或保证不重启迁移；
- 自动 alias、schema 猜测迁移或 Provider 专有任意扩展字段。

## 关联文档

- [产品范围](../product-scope/product-scope.md)
- [Public Model 与模型能力契约](../model-capability/README.md)
- [配置、凭证与受信运行边界](../configuration-credentials/README.md)
- [扩展能力导航及共同规则](../extended-capabilities/embedding-and-native-multimodal.md)
- [当前实现总览](../../implementation-status/current-implementation.md)
