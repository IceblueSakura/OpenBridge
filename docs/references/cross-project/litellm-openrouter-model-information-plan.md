# LiteLLM / OpenRouter 模型信息模型调研与 OpenBridge 接口计划

## 文档状态与范围

- **调研时间**：2026-08-03。
- **文档性质**：外部参考事实、模型能力字段对照和 OpenBridge 已采纳的核心模型与接口计划；live source 与实施现状文档仍是当前行为依据。
- **调研重点**：能返回单个或全部模型信息的接口，以及其中可用于描述模型能力的字段。
- **明确排除**：部署配置、凭据、上游地址、Provider/target 选择、路由健康、租户权限、运行时指标和控制面 UI。
- **复核条件**：LiteLLM 的 `main` 源码、模型价格/上下文目录和官方文档会持续演进；OpenRouter Models API 也可能增加字段。升级实现前必须重新核对官方资料和当前 OpenBridge checkout。

本文把“模型能力”定义为客户端可以安全依赖的静态或准静态事实，例如输入输出模态、上下文限制、工具/结构化输出/reasoning 支持、协议接口和可接受参数；价格、排行和某一次 Provider 部署的可用性不属于第一版能力契约。

## 1. LiteLLM 的模型信息模型

### 1.1 接口分层

LiteLLM 同时存在兼容性模型列表、Proxy 部署信息、逻辑模型组和独立 Model Catalog。它们的对象粒度不同，不能把返回字段直接当成同一个“模型”对象。

| 接口 | 粒度 | 单个/全部 | 返回重点 | 对 OpenBridge 的参考价值 |
|---|---|---|---|---|
| `GET /models`、`GET /v1/models` | OpenAI 兼容模型条目 | 全部 | `id`、`object`、`created`、`owned_by` 等列表字段；可按当前 Proxy 选项筛选健康或通配路由 | 说明兼容列表的最小外形；不适合作为详细能力来源 |
| `GET /models/{model_id}`、`GET /v1/models/{model_id}` | OpenAI 兼容模型条目 | 单个 | 单个模型对象，访问不到或不存在时返回错误 | 可作为单个资源语义参考；仍不是详细能力模型 |
| `GET /model/info`、`GET /v1/model/info` | Proxy deployment | 全部或按 `litellm_model_id` 单个 | `{data: [{model_name, litellm_params, model_info}]}`；`model_info` 可被自定义字段扩展，并可能合并 LiteLLM 成本/上下文目录 | 字段丰富，但明确包含 deployment 边界，不能直接对外复用 |
| `GET /v2/model/info` | 分页 deployment 元数据 | 全部或按模型/团队/搜索条件筛选 | `data`、总数、页码、大小等分页结果；需要 DB 的 Proxy 元数据 | 说明大目录查询和分页语义；不应把 DB/deployment 字段引入公共能力接口 |
| `GET /model_group/info` | 逻辑模型组及其多个部署 | 全部或按 `model_group` 单个 | Provider 列表、上下文/成本、TPM/RPM、`supports_*`、`supported_openai_params` 等聚合结果 | 说明“逻辑模型组”聚合方式；Provider 列表和限流/成本字段超出本项目暴露边界 |
| `GET /model_catalog` | 独立模型目录 | 全部，支持分页和过滤 | 定价、上下文、`mode`、模态和大量 `supports_*` 能力字段 | 最适合参考静态能力字段集合，但仍是 LiteLLM 全局目录，不等于当前网关的有效能力 |
| `GET /model_catalog/{model_id}` | 独立模型目录条目 | 单个 | 一个目录模型的能力/成本元数据 | 说明单模型详情语义；不能绕过 OpenBridge 的注册和路由约束 |
| `GET /model/metrics`、`/model/metrics/slow_responses`、`/model/metrics/exceptions`、`/model/streaming_metrics` | 运行时观测 | 全部或按模型键 | 延迟、TTFT、失败、异常和流式统计 | 不是静态能力，不纳入模型信息接口 |

当前 LiteLLM Proxy 源码还存在模型设置或某些部署/版本提供的公共 Model Hub、成本地图相关扩展；这些接口的版本和部署条件不稳定，且通常返回管理面或目录面数据。它们不是本计划的稳定依赖。`/model/settings` 属于 Provider 参数设置，不是模型能力详情。

当前源码中与这些接口关联的查询参数也反映了粒度差异：

| 接口 | 当前可观察的参数类别 | 语义边界 |
|---|---|---|
| `/models`、`/v1/models` | `return_wildcard_routes`、`team_id`、`include_model_access_groups`、`only_model_access_groups`、`include_metadata`、`fallback_type`、`scope=expand`、`healthy_only` | 控制 Proxy 列表中的路由、团队、元数据、fallback 或健康过滤；不是能力声明 |
| `/models/{model_id}`、`/v1/models/{model_id}` | `team_id`、`healthy_only` | 控制单个兼容模型条目的访问和健康条件 |
| `/model/info`、`/v1/model/info` | `litellm_model_id`、`include_team_models`、`teamId` | 通过 deployment id 或团队范围选择 deployment；无参数时通常返回全部可见记录 |
| `/v2/model/info` | `model`、`user_models_only`、`include_team_models`、`debug`、`page`、`size`、`search`、`modelId`、`teamId`、`sortBy`、`sortOrder`、`exclude_auto_routers` | DB-backed deployment 的分页、搜索、团队和自动路由筛选 |
| `/model_group/info` | 可选 `model_group` | 选择一个逻辑模型组或返回全部逻辑组 |
| `/model_catalog` | 目录分页及 `provider`、`supports_vision`、`supports_reasoning` 等能力过滤 | 全局目录筛选；不改变 OpenBridge 的本地注册结果 |

这些参数名称和可用性属于当前源码/部署版本观察，不应直接复制成 OpenBridge 的公共查询契约。OpenBridge 第一版只需要一个可选的 Public Model id 过滤器。

### 1.2 LiteLLM 的三种“模型”粒度

#### A. 兼容列表模型

`/models` 的职责是让 OpenAI 兼容客户端知道“可以请求哪些 model id”。它通常只给出最小的 OpenAI 模型对象，不承诺完整的上下文、工具、reasoning 或模态能力。LiteLLM 当前源码还明确将 `/model/info` 作为详细信息入口，因此不应通过扩展 `/models` 的少量字段来承载完整能力模型。

#### B. Proxy deployment 与 model group

`/model/info` 的核心结构是：

```json
{
  "data": [
    {
      "model_name": "public-or-proxy-name",
      "litellm_params": {
        "model": "provider/model"
      },
      "model_info": {
        "id": "deployment-id",
        "base_model": "provider/model",
        "max_input_tokens": 128000,
        "max_output_tokens": 16384,
        "supports_function_calling": true
      }
    }
  ]
}
```

这类结构把“调用名”“上游调用参数”和“模型信息”放在同一个 deployment 记录中。`model_info` 可以包含自定义键，源码还会从 LiteLLM 模型成本目录补齐缺失信息，并对 API key、base 等敏感或部署字段做处理。它适合 Proxy 管理面、路由管理和诊断，不适合作为 OpenBridge 的公共响应，因为会把部署拓扑与能力事实混在一起。

`/model_group/info` 则把同一个逻辑组下的多个 deployment 聚合起来，典型字段包括：

- 身份与聚合：`model_group`、`providers`、公开性或团队可见性；
- 限制与经济：`max_input_tokens`、`max_output_tokens`、输入/输出成本、TPM/RPM/ITPM/OTPM；
- 能力：`supports_parallel_function_calling`、`supports_vision`、`supports_web_search`、`supports_url_context`、`supports_reasoning`、`supports_function_calling`、`supported_openai_params`；
- 管理字段：客户端认证参数、团队或访问组信息。

其中能力字段有参考意义，但聚合结果可能是不同 Provider 或 deployment 的混合；它没有自动等价于“每条完整路由都保证的能力”。

#### C. LiteLLM Model Catalog

独立的 `api.litellm.ai` Model Catalog 更接近模型能力目录，而不是 Proxy deployment。官方 API 文档提供：

- `GET /model_catalog`：全部目录，支持 `provider`、`supports_vision`、`supports_reasoning` 等过滤和分页；
- `GET /model_catalog/{model_id}`：单个目录条目；
- 目录数据从 LiteLLM GitHub 模型成本/上下文数据定期刷新，因此适合作为外部能力字段的参考源，而不是 OpenBridge 运行时的动态发现源。

模型目录样例中可观察到的字段集合包括：

| 能力维度 | 代表字段 | 语义 |
|---|---|---|
| 身份与生命周期 | `model_name`、`litellm_provider`、`source`、`deprecation_date` | 目录标识、Provider 来源和生命周期提示 |
| 任务模式 | `mode` | `chat`、`completion`、`embedding`、`image_generation`、`audio_transcription`、`audio_speech`、`moderation`、`rerank`、`search` 等 |
| Token 限制 | `max_input_tokens`、`max_output_tokens`、兼容旧字段 `max_tokens` | 输入、输出和历史兼容限制 |
| 模态 | `supports_vision`、`supports_audio_input`、`supports_audio_output`、部分条目的 `supports_pdf_input`、`supports_video_input`、`supports_image_input` | 输入输出模态能力；可选字段不应缺失即视为不支持 |
| 工具与结构化 | `supports_function_calling`、`supports_parallel_function_calling`、`supports_response_schema`、`supports_system_messages` | 工具调用、并行工具、结构化输出和 system message |
| Reasoning 与缓存 | `supports_reasoning`、`supports_prompt_caching`、`output_cost_per_reasoning_token` | reasoning 能力和缓存/推理成本元数据 |
| 外部工具与端点 | `supports_web_search`、`supported_endpoints` | 目录或 Provider 端点层面的附加信息 |
| 其他目录元数据 | `supported_regions`、向量存储成本、`output_vector_size` | 区域、embedding 和经济性信息 |

LiteLLM 目录的优点是能力字段较宽，缺点是它表达的是全局目录上限或目录声明。OpenBridge 若采用这些字段，必须经过本地注册、Provider API 和路由执行能力的再次收窄，不能把目录字段原样转发给客户端。

### 1.3 LiteLLM 模型能力模型的归纳

LiteLLM 的可复用结构可以抽象为：

```text
ModelCatalogEntry
├── identity: model id / provider / source / lifecycle
├── task: mode
├── limits: max input / max output
├── modalities: vision / audio / image / pdf / video ...
├── interaction: function calling / parallel tools / response schema / system messages
├── reasoning: supported / reasoning token cost / provider-specific extensions
├── endpoint: supported endpoints / OpenAI parameter set
└── economics-and-operations: price / region / rate limit / metrics
```

其中最后一层应与公共能力对象隔离。特别是 `/model/info` 的 `litellm_params`、deployment id、base URL、credential locator 和 Provider 信息，不应成为 OpenBridge 的响应字段。

## 2. OpenRouter 的模型信息模型

### 2.1 接口分层

OpenRouter 的 Models API 更接近公开模型目录：基础列表和单模型详情都围绕同一种 `Model` 对象展开，并把 Provider endpoint 详情作为链接或独立资源处理。

| 接口 | 粒度 | 单个/全部 | 返回重点 | 对 OpenBridge 的参考价值 |
|---|---|---|---|---|
| `GET /api/v1/models` | 公开 canonical 模型目录 | 全部或分页/过滤 | `{data, links, total_count}`；每项为完整 `Model` 对象 | 最适合参考“模型能力详情”对象的外形和过滤思路 |
| `GET /api/v1/model/{author}/{slug}` | canonical 模型详情 | 单个 | `{data: Model}`；支持模型别名和部分变体 slug | 适合作为单模型详情语义参考 |
| `GET /api/v1/models/count` | 目录计数 | 全部的数量 | `{data: {count}}`，可按输出模态过滤 | 只有在目录分页或统计需要时才有价值 |
| `GET /api/v1/models/user` | 按用户偏好过滤后的目录 | 全部或分页/过滤 | 与 Models API 相同的模型对象，但叠加 Provider 偏好、隐私、guardrail 或区域约束 | 属于用户/账号视图；不作为 OpenBridge 第一版公共契约 |
| `GET /api/v1/models/{author}/{slug}/endpoints` | 一个模型的 Provider endpoint 详情 | 全部 endpoint | 每个 Provider endpoint 的价格、参数、可用性等 | 能解释模型背后的 Provider 差异，但属于部署/供应面，按本请求排除 |

`GET /api/v1/models` 当前 API 参考文档支持 `offset`、`limit`（上限 1000）、`q`、`category`、`supported_parameters`、`input_modalities`、`output_modalities`、`context`、价格上下限、`arch`、`model_authors`、`providers`、`distillable`、`zdr`、`region`、模型年龄、多个 intelligence/coding/agentic 指标和 `sort` 等过滤。`output_modalities=all` 可请求完整输出模态范围。过滤能力很丰富，但不等于返回对象中的每个字段都属于运行时可保证的能力。

### 2.2 OpenRouter `Model` 对象

OpenRouter 基础模型对象可以按以下维度理解：

```json
{
  "id": "author/model",
  "canonical_slug": "author/model",
  "name": "Model display name",
  "created": 1730000000,
  "description": "Model description",
  "context_length": 128000,
  "architecture": {
    "input_modalities": ["text", "image"],
    "output_modalities": ["text"],
    "tokenizer": "Tokenizer name",
    "instruct_type": "instruction"
  },
  "pricing": {
    "prompt": "0.000001",
    "completion": "0.000005",
    "request": "0",
    "image": "0",
    "web_search": "0",
    "internal_reasoning": "0"
  },
  "top_provider": {
    "context_length": 128000,
    "max_completion_tokens": 16384,
    "is_moderated": false
  },
  "per_request_limits": null,
  "supported_parameters": [
    "tools",
    "tool_choice",
    "reasoning",
    "structured_outputs"
  ],
  "default_parameters": {},
  "expiration_date": null,
  "links": {
    "details": "https://openrouter.ai/api/v1/models/author/model/endpoints"
  }
}
```

字段可归纳为：

| 能力维度 | 代表字段 | 说明与边界 |
|---|---|---|
| 身份与描述 | `id`、`canonical_slug`、`name`、`created`、`description` | canonical 模型身份和展示信息；slug 变体可能是独立可请求对象 |
| 生命周期 | `expiration_date`、可选 `knowledge_cutoff` | 过期和知识截止提示；不等价于当前服务健康 |
| 模态与 tokenizer | `architecture.input_modalities`、`architecture.output_modalities`、`tokenizer`、`instruct_type` | 明确区分输入/输出模态，适合归一化到能力对象 |
| 上下文与输出 | `context_length`、`top_provider.context_length`、`top_provider.max_completion_tokens` | 模型级和 top Provider 级限制可能不同；应定义优先级，不能简单相加 |
| 参数能力 | `supported_parameters`、`default_parameters` | 工具、`tool_choice`、`reasoning`、`structured_outputs`、采样和停止等参数集合 |
| Reasoning | `supported_parameters` 中的 `reasoning`/`include_reasoning`，部分返回还含 `reasoning` 对象及 effort 信息 | 既有开关能力，也可能有默认、必选和 effort 枚举；缺少对象时不能推断更细粒度等级 |
| 供应限制 | `per_request_limits`、`is_moderated` | 供应或请求限制，不是模型本体能力；是否公开应单独决定 |
| 成本 | `pricing` 的 prompt/completion/image/search/cache/reasoning 等字段 | 经济性目录，不纳入本项目第一版能力契约 |
| 质量指标 | 可选 `benchmarks`、intelligence/coding/agentic 等指数 | 质量/排行信号，不等于协议能力或成功保证 |
| Provider 细节入口 | `links.details` | 指向 endpoint 级详情；不要把链接内容展开成公共 deployment 配置 |

OpenRouter 的基础 `Model` 对象把“目录能力”和“经济/质量/供应信息”并列返回，这对探索和筛选很方便；但 OpenBridge 的公共接口需要更窄的能力契约，尤其要避免把 `top_provider` 或 endpoint 信息解释成网关所有 fallback 路径都保证的属性。

### 2.3 OpenRouter 模型能力模型的归纳

OpenRouter 的核心可复用结构是：

```text
Model
├── identity: id / canonical slug / name / description / lifecycle
├── architecture: input modalities / output modalities / tokenizer
├── limits: context length / max completion tokens
├── parameters: supported parameters / defaults
├── reasoning: optional default / mandatory / effort details
├── provider-summary: top provider / per-request limits
├── economics: pricing
├── quality: benchmarks and indexes
└── links: endpoint detail resource
```

其中 `architecture`、`limits`、`parameters` 和 reasoning 是本计划最有价值的能力参考；`pricing`、`benchmarks`、`top_provider` 和 endpoint links 需要保留在目录或供应面，不应无条件成为 OpenBridge 公共模型信息。

## 3. LiteLLM 与 OpenRouter 的综合比较

### 3.1 共同抽象与差异

| 维度 | LiteLLM | OpenRouter | 综合结论 |
|---|---|---|---|
| 公开列表 | `/models` 偏 OpenAI 兼容最小条目；`/model/info` 偏 Proxy deployment | `/api/v1/models` 直接返回完整目录模型 | OpenBridge 保留 `/v1/models` 的最小兼容职责，新增独立详情接口 |
| 单个详情 | `/model/info?litellm_model_id=...` 是 deployment；`/model_catalog/{id}` 是目录条目 | `/api/v1/model/{author}/{slug}` 返回一个 `Model` | 单个详情必须先明确是公共模型、目录模型还是 deployment；本项目选择公共模型 |
| 能力来源 | `model_prices_and_context_window.json` 的能力旗标较多 | `architecture` 与 `supported_parameters` 结构较清晰 | 采用分层能力对象；不复制 Provider 专属字段的长尾 |
| 输入输出模态 | `supports_vision`、audio 等分散旗标，部分条目有扩展字段 | `architecture.input_modalities/output_modalities` | 优先采用显式 `input_modalities`/`output_modalities`，无法确认时为未知 |
| 上下文限制 | `max_input_tokens`、`max_output_tokens` | `context_length`、`top_provider.max_completion_tokens` | OpenBridge 使用已有输入/输出 token 上限模型，避免把总上下文和最大输出混淆 |
| 工具/结构化 | `supports_function_calling`、parallel、response schema、supported params | `supported_parameters` 中的 tools、tool choice、structured outputs 等 | 同时保留布尔能力与可接受参数集合，但语义必须是网关有效能力 |
| Reasoning | `supports_reasoning`，可能附 effort 或成本字段 | `reasoning` 参数及可选默认/必选/effort 对象 | 采用 `unknown/supported/unsupported`，等级和输出形态仅在有证据时暴露 |
| 接口差异 | 目录能力与 Proxy deployment/路由信息混在不同接口中 | 基础模型与 Provider endpoint 分开但可链接 | 公共模型信息不得跨越部署边界；endpoint 能力不直接外泄 |
| 价格/质量/运行时 | 成本、区域、TPM/RPM、指标等分散在不同接口 | `pricing`、`benchmarks`、供应限制并列在 `Model` | 作为内部目录或未来独立资源候选，第一版公共能力接口不包含 |
| 聚合语义 | model group 可能聚合多个 deployment | `top_provider` 是供应摘要 | 不能把“存在一个支持路径”当成“所有 fallback 路径均支持” |

### 3.2 推荐的能力语义

综合两者后，模型信息应至少区分以下三个层次：

1. **模型身份与静态事实**：公共模型 id、名称、描述、输入/输出 token 限制、输入/输出模态、任务模式、reasoning 支持与等级。
2. **协议接口能力**：`chat_completions` 和 `responses` 是否启用、是否支持 streaming、function calling、parallel tools、structured outputs、image input、store、previous response 等。它们不是模型目录的简单字段，而是 OpenBridge 实际可提供的协议表面。
3. **能力证据与不确定性**：字段来自静态注册、Provider API 注册和 route 约束中的哪一类；未知必须与不支持区分。公共响应只给非敏感、面向客户端的证据类别，不给内部拓扑。

不建议把以下内容塞入同一个公共模型对象：Provider 名称、上游 model 名称、route/deployment id、base URL、credential locator、`litellm_params`、团队/访问组、TPM/RPM、健康状态、延迟、价格、质量排行和 endpoint 级供应详情。
## 4. OpenBridge 最终模型设计

### 4.1 设计结论与边界

Public Model 是客户端主动选择的稳定服务契约，不是 Router 临时选择出的模型组。OpenBridge 为每个 Public Model 编译恰好一份 Chat Completions 契约和一份 Responses 契约；客户端请求只针对所选模型做能力预检，不根据能力切换 Public Model，也不根据能力跳过、重排或筛选 Route。

最终信息分为三层：

| 层次 | 公共对象 | 语义 | 对外返回 |
|---|---|---|---|
| 标准身份 | StandardModel | id、object、created、owned_by | 是 |
| 模型事实 | ModelCapabilities | 任务、上下文、模态、参数和 reasoning | 是 |
| 接口契约 | ModelInterfaceCapabilities | Chat/Responses 实际保证的 stream、tools、结构化输出、reasoning 和 state | 是 |
| 私有执行 | Canonical Model、Provider、Target、Upstream API、Route、credential | 收窄、转换、顺序、retry、fallback 与 cooldown | 否 |

公共能力描述调用方在当前 OpenBridge 构建中可依赖的静态边界，不描述瞬时健康、延迟、配额、价格或部署数量。Route 仍负责执行和韧性，但不能扩大 Public Model 契约。

### 4.2 值对象与未知语义

| 值对象 | wire 值 | 语义 |
|---|---|---|
| SupportState | supported、unsupported、unknown | 只有 supported 能通过请求预检；unknown 必须 fail closed |
| ModelTask | chat、text_generation | 当前 Public Model 的任务类别 |
| InputModality | text、image、audio、file | 模型或接口可接收的内容类型 |
| OutputModality | text、image、audio | 模型或接口可生成的内容类型 |
| ReasoningLevel | none、minimal、low、medium、high、xhigh、max | 标准下游 reasoning 强度 |
| ReasoningOutputMode | unsupported、plain_text、summary、opaque、unknown | 下游接口可观察的 reasoning 输出形态 |
| LifecycleStatus | active、deprecated、retired | retired 不再进入可调用目录 |

统一规则：

- 未知 token 限制、tokenizer、知识截止和日期使用 JSON null。
- 明确不支持使用 unsupported 或空集合，不能伪装成 unknown。
- 数组只列出已确认值，必须去重并确定性排序。
- 某协议没有可执行 Route 时，interfaces 中对应值为 null。
- 扩展对象使用 schema_version 进行兼容演进；第一版值为 1。
- 公共对象不出现 Provider、Target、Route、upstream model、endpoint、credential 或 wire mapping。

### 4.3 完整公共数据模型

    PublicModelInfo
    ├── schema_version: "1"
    ├── id: string
    ├── object: "model"
    ├── created: int64
    ├── owned_by: "openbridge"
    ├── name: string
    ├── description: string | null
    ├── lifecycle: ModelLifecycle
    ├── capabilities: ModelCapabilities
    └── interfaces: ModelInterfaces
        ├── chat_completions: ModelInterfaceCapabilities | null
        └── responses: ModelInterfaceCapabilities | null

    ModelCapabilities
    ├── tasks: ModelTask[]
    ├── context_window: ContextWindow
    ├── modalities: ModelModalities
    ├── supported_parameters: string[]
    ├── tokenizer: string | null
    ├── knowledge_cutoff: string | null
    └── reasoning: ModelReasoningCapabilities

    ModelInterfaceCapabilities
    ├── context_window: ContextWindow
    ├── modalities: ModelModalities
    ├── supported_parameters: string[]
    ├── streaming: SupportState
    ├── system_messages: SupportState
    ├── tools: ToolCapabilities
    ├── structured_outputs: StructuredOutputCapabilities
    ├── reasoning: InterfaceReasoningCapabilities
    ├── prompt_caching: SupportState
    └── state: StateCapabilities

| 子对象 | 字段 |
|---|---|
| ContextWindow | max_context_tokens、max_input_tokens、max_output_tokens |
| ToolCapabilities | support、types、tool_choice_modes、parallel_calls、strict_schema |
| StructuredOutputCapabilities | support、modes、strict_schema |
| InterfaceReasoningCapabilities | support、levels、output |
| StateCapabilities | store、previous_response_id、background |
| ModelLifecycle | status、deprecated_at、retired_at |

capabilities 表达模型本体事实上界；interfaces 表达具体下游协议的固定可调用契约。模型本体支持 image 或 reasoning，不代表两个接口都开放该能力，调用方应以目标 interfaces 项为准。

### 4.4 固定契约的编译与请求规则

OpenBridge 不返回 guaranteed 加 profiles，也不保留 conditional。每个协议只有一个固定契约，由该 Public Model 对应协议的全部静态启用、可执行 Route 保守相交得到：

| 字段类型 | 固定契约计算 |
|---|---|
| 布尔能力 | 所有 Route 都支持才是 supported；任一 Route 明确不支持则为 unsupported；仅有未知证据时为 unknown |
| token 上限 | 所有 Route 都有已知值时取最小值；任一未知则为 null |
| 模态、参数、reasoning level | 集合交集，并确定性排序 |
| reasoning 输出 | 所有 Route 形态相同才公开该形态，否则为 unknown |
| Bridge | 只保留转换器完整支持的公共子集；不能因某条 Native Route 更强而扩大契约 |

例如首条 Chat Route 不支持 function tools、第二条 Route 支持 tools，Public Model 的 Chat tools.support 仍为 unsupported。工具请求在任何上游调用前返回错误，不能跳过首条 Route 去选择第二条。

固定契约只影响请求是否被接受：

1. 解析客户端明确指定的 Public Model。
2. 用目标接口的唯一契约完成一次能力预检。
3. 不支持或未知时返回 HTTP 400 和 unsupported_model_capability。
4. 通过后严格按配置顺序构造 Route 候选。
5. Route 只因协议不匹配、Target 静态禁用或 Upstream API 静态禁用而不可执行；请求能力不得改变候选顺序。
6. retry、fallback、cooldown 和 state-affinity 继续属于执行层。

### 4.5 编译期不变量

registry 构建必须拒绝：

1. Public Model id 不是安全单段资源 id，或 created 为零。
2. 展示名称为空，描述为空白字符串。
3. 生命周期状态与时间字段不一致，或时间早于 created。
4. 上下文、输入或输出限制为零，或输入/输出上限超过总上下文。
5. 显式模态集合为空或重复。
6. Public Model 没有 Route、重复引用 Route 或引用未知对象。
7. Upstream API 规则扩大 canonical 模型事实，或收窄后产生不一致上下文。
8. reasoning 支持状态、level 与参数声明不一致。
9. 公共序列化对象中出现任何部署、凭据或 wire mapping 字段。

retired 或没有任何可执行接口的 Public Model 不进入下游可见目录。未知能力保持 unknown 或 null，不能提升为 supported。

### 4.6 当前代码映射

- PublicModelConfig 声明稳定 id、created、display_name、description、lifecycle 和私有 Route 顺序。
- ModelContextLength 独立保存 total、input、output 三项限制。
- registry 构建阶段从有效 Model、Upstream API 和 Native/Bridged Route 编译 PublicModelInfo。
- RuntimeRegistry 只向 handler 和 planner 暴露不可变 Public Model；HTTP handler 不临时遍历 Route。
- planner 先读取 Public Model 的接口契约，能力通过后才按原 Route 顺序生成执行计划。
- StandardModel 是完整 PublicModelInfo 的严格四字段投影。
- mode 与模态已进入模型信息；未实现的 hosted/custom tool、audio/file 请求、prompt caching 等协议能力仍在请求分析阶段拒绝。

## 5. 三类模型信息接口

### 5.1 接口矩阵

| 类别 | HTTP 接口 | 返回对象 |
|---|---|---|
| OpenAI 标准 Models | GET /v1/models | StandardModel list envelope |
| OpenAI 标准 Models | GET /v1/models/{model} | 单个 StandardModel |
| OpenBridge 扩展列表 | GET /openbridge/v1/models | PublicModelInfo list envelope |
| OpenBridge 扩展单模型 | GET /openbridge/v1/models/{model} | 单个 PublicModelInfo |

Public Model id 采用安全单段格式 [A-Za-z0-9][A-Za-z0-9._:-]{0,127}。包含斜杠的 canonical 或上游模型 id 只能留在私有 registry 中。

### 5.2 OpenAI 标准 Models 契约

标准对象严格只有四个字段：

    {
      "id": "code-primary",
      "object": "model",
      "created": 1785715200,
      "owned_by": "openbridge"
    }

列表使用 object=list 与 data 数组；retrieve 直接返回对象。标准接口不能附加 capabilities。created 是 Public Model 契约首次创建的稳定 Unix 秒，不使用进程启动时间。

### 5.3 OpenBridge 扩展列表和详情

GET /openbridge/v1/models 返回 object=list 与完整 PublicModelInfo 数组。GET /openbridge/v1/models/{model} 直接返回对应元素；同一 registry snapshot 下，详情必须与列表中的元素逐字段相同。

第一版返回全部可见静态 Public Models，不分页、不搜索、不按 Provider 过滤，也不在请求期间调用外部目录。未来需要分页时只能扩展列表 envelope，不能改变单个 PublicModelInfo 的语义。

### 5.4 共同安全和错误边界

- 四个 operation 使用与 Chat/Responses 相同的 Bearer 认证。
- 模型按 Public Model id 确定性排序。
- 标准与扩展接口读取同一不可变 registry snapshot。
- 未知模型返回 404 model_not_found，param 为 model。
- 请求能力不支持时返回 400 unsupported_model_capability，并在 egress 前结束。
- 不返回 Provider、canonical/upstream model id、Target、Route、Native/Bridge 模式、base URL、endpoint profile、credential、header 或 wire mapping。
- 不返回价格、成本、TPM/RPM、健康、延迟、指标、排行或 benchmark。
- 不通过 LiteLLM/OpenRouter 目录动态发现或自动注册模型。

### 5.5 验证边界

确定性 Rust 测试至少覆盖：

1. 标准 list/retrieve 的严格四字段投影。
2. 扩展 list/detail 逐字段一致和私有部署字段缺失。
3. 非法 Public Model 身份、上下文、模态和生命周期在 registry 构建时失败。
4. 较弱首选 Route 与较强后续 Route 的交集仍拒绝能力请求。
5. 能力错误为 unsupported_model_capability，且上游 transport 调用次数为零。
6. 能力中立请求保持原 Route 顺序和 fallback 候选。
7. OpenAPI 同时包含四个模型信息 operation。

这些检查只证明 OpenBridge 自有 registry、planner 和 HTTP 契约，不证明 LiteLLM/OpenRouter 目录新鲜度、真实 Provider 能力、外部 SDK、负载或长期运行兼容性。

## 6. 结论

LiteLLM 最值得借鉴的是能力目录字段宽度与 mode、supports 类字段的可枚举化；其 model/info 和 model_group/info 混有 deployment 信息，不能原样公开。OpenRouter 最值得借鉴的是用同一个 Model 对象组织 identity、architecture、context、supported parameters 和 reasoning；pricing、benchmarks、top_provider 与 endpoint 详情仍不属于 OpenBridge 的模型能力契约。

OpenBridge 的落点是一份由 registry 预编译的 PublicModelInfo：标准接口只投影四个身份字段，扩展接口返回模型事实与每协议唯一固定契约，请求预检读取同一对象。能力用于正确拒绝客户端已选择模型不支持的请求，不用于选模、Route 筛选或 Route 重排。

## 7. 来源与复核入口

### OpenAI 标准 Models

- [OpenAI Models list](https://developers.openai.com/api/reference/resources/models/methods/list)：官方 `GET /v1/models` operation；标准元素包含 `id`、`object`、`created` 和 `owned_by`。
- [OpenAI Models retrieve](https://developers.openai.com/api/reference/resources/models/methods/retrieve)：官方 `GET /v1/models/{model}` operation；直接返回单个标准 `Model`。
- [OpenAI Models catalog](https://developers.openai.com/api/docs/models)：官方模型目录入口；目录页面不是 Models API 的详细 capability schema。

### LiteLLM

- [LiteLLM Proxy model management](https://docs.litellm.ai/docs/proxy/model_management)：`/models`、`/model/info` 和 Proxy 模型管理说明。
- [LiteLLM Model Catalog API](https://api.litellm.ai/docs)：`/model_catalog` 全部目录和单模型详情。
- [LiteLLM Proxy route source](https://github.com/BerriAI/litellm/blob/main/litellm/proxy/proxy_server.py)：当前 `models`、`model/info`、`v2/model/info`、`model_group/info` 和 metrics route 的源码入口；分支 `main` 会变化。
- [LiteLLM model price and context catalog](https://github.com/BerriAI/litellm/blob/main/model_prices_and_context_window.json)：模型模式、上下文、价格和 `supports_*` 字段样例；不能当作 OpenBridge 运行时注册表。
- [LiteLLM router types](https://github.com/BerriAI/litellm/blob/main/litellm/types/router.py)：`ModelInfo`、`ModelGroupInfo` 和 `Deployment` 类型边界。

### OpenRouter

- [List all models and their properties](https://openrouter.ai/docs/api/api-reference/models/list-all-models-and-their-properties)：`GET /api/v1/models` 的过滤、分页和 `Model` 字段。
- [Get a model by its slug](https://openrouter.ai/docs/api/api-reference/models/get-a-model-by-its-slug)：单模型详情接口。
- [Get total count of available models](https://openrouter.ai/docs/api/api-reference/models/get-total-count-of-available-models)：模型计数接口。
- [List models filtered by user preferences](https://openrouter.ai/docs/api/api-reference/models/list-models-filtered-by-user-provider-preferences-privacy-settings-and-guardrails)：用户视图目录及其过滤边界。
- [Models guide](https://openrouter.ai/docs/guides/overview/models)：模型目录概念和 Models API 总览。

### OpenBridge 当前边界

- [Gateway API compatibility](../../functional-requirements/gateway-api-compatibility.md)：Public Model、`/v1/models` 和不暴露上游枚举/部署选择的产品边界。
- [Capability probing status](../../implementation-status/capability-probing.md)：canonical model facts、有效 Provider/API capability 和 probe 不做动态发现的实现事实。
