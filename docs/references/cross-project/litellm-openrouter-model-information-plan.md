# LiteLLM / OpenRouter 模型信息模型调研与 OpenBridge 候选接口计划

## 文档状态与范围

- **调研时间**：2026-08-03。
- **文档性质**：外部参考事实、模型能力字段对照和一个候选接口计划；不构成当前实现授权，也不修改 `current-focus.md`。
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

### 4.1 设计优先级与四层边界

本节先定义最终公共契约，再把当前代码视为迁移输入。现有 `ModelConfig`、`PublicModelConfig` 或 capability 布尔字段不限制最终数据模型；实现阶段可以大范围调整类型和 registry 编译流程。

最终模型分为四层：

| 层 | 核心对象 | 职责 | 是否公开 |
|---|---|---|---|
| 公共身份 | `PublicModelIdentity` | 稳定 Public Model id、展示名称、描述、创建时间、所有者和生命周期 | 是 |
| 模型固有能力 | `ModelCapabilities` | 任务、上下文窗口、输入/输出模态、tokenizer、知识截止和 reasoning 上界 | 是 |
| 协议可调用能力 | `ModelInterfaceCapabilities` | Chat/Responses 上实际可依赖的参数、stream、tools、结构化输出、reasoning 输出和状态能力 | 是 |
| 私有供应与部署 | `CanonicalModel`、Provider、Target、Upstream API、Route、credential | 证明和实现公共契约，完成收窄、wire mapping、重试和 fallback | 否 |

Public Model 应成为独立的客户端服务契约，而不是从第一条 route 或某个 Provider deployment 临时拼出的别名。最终 registry 中应显式声明 Public Model 的身份和模型固有能力，再由编译器验证每个协议 capability profile 至少存在一条完整 route 可以满足。底层 route 可以拥有额外能力，但不能扩大未声明的公共契约。

`CanonicalModel` 继续保存内部、Provider-independent 的参考事实；`PublicModel` 可以绑定多个 canonical model 或 deployment，但对客户端只呈现自己的稳定身份和已验证能力。这样即使 fallback 指向不同模型，也不会把某个上游名称、描述或上下文限制误当成 Public Model 的唯一真相。

### 4.2 基础值对象与未知语义

最终 schema 使用有界、可扩展的类型，不接收或透传 LiteLLM 的任意 `model_info` 键。

| 值对象 | 建议值 | 语义 |
|---|---|---|
| `SupportState` | `supported`、`unsupported`、`conditional`、`unknown` | `conditional` 仅用于聚合结果并要求检查完整 profiles；`unknown` 表示证据不足。两者在请求规划中都不能直接当作已支持 |
| `ModelTask` | `chat`、`text_generation`、`embedding`、`rerank`、`image_generation`、`audio_transcription`、`audio_speech`、`moderation`、`search` | 模型能完成的任务，不等同于 HTTP API 名称 |
| `Modality` | `text`、`image`、`audio`、`video`、`file` | 输入/输出内容类型；后续只能以新增枚举值扩展 |
| `ReasoningLevel` | `none`、`minimal`、`low`、`medium`、`high`、`xhigh`、`max` | 下游统一 reasoning 强度；`none` 表示显式禁用 |
| `ReasoningOutput` | `unsupported`、`plain_text`、`summary`、`opaque`、`unknown` | 模型/接口可观察的 reasoning 输出形态 |
| `LifecycleStatus` | `active`、`deprecated`、`retired` | `retired` 不再出现在可路由列表；`deprecated` 仍可调用但有生命周期提示 |
| `ApiInterface` | `chat_completions`、`responses` | 当前公开协议；未来增加新键，不复用 `ModelTask` |

统一空值规则：

- 未知数值、日期、tokenizer 或知识截止使用 JSON `null`；
- 已知不支持使用 `unsupported` 或空集合，不能用 `null` 代替；
- `conditional` 只允许出现在 `interfaces.<api>.guaranteed` 的聚合字段中，模型固有能力和单个完整 profile 不得使用；
- 模态和参数数组只列出**已保证支持**的值，未列出不代表模型本体永久不支持；
- 某个 API 完全不可用时，`interfaces` 中对应值为 `null`；
- 所有数组使用稳定、去重顺序，所有枚举 wire 值保持小写 snake_case。

### 4.3 完整公共 `ModelInfo` 数据模型

公共对象由同一份 `PublicModelDefinition` 编译产生。OpenAI 标准接口只投影其中四个标准字段，两个扩展接口返回完整对象。

```text
PublicModelInfo
├── id: string
├── object: "model"
├── created: int64
├── owned_by: "openbridge"
├── name: string
├── description: string | null
├── lifecycle: ModelLifecycle
├── capabilities: ModelCapabilities
└── interfaces: map<ApiInterface, ModelInterfaceCapabilities | null>

ModelCapabilities
├── tasks: ModelTask[]
├── context_window: ContextWindow
├── modalities: Modalities
├── tokenizer: string | null
├── knowledge_cutoff: string | null
└── reasoning: ModelReasoningCapabilities

ModelInterfaceCapabilities
├── guaranteed: AggregatedCapabilityProfile
└── profiles: CapabilityProfile[]

AggregatedCapabilityProfile
└── 与 CapabilityProfile 同形，但 SupportState 可使用 conditional

CapabilityProfile
├── context_window: ContextWindow
├── modalities: Modalities
├── parameters: ParameterCapabilities
├── streaming: SupportState
├── system_messages: SupportState
├── tools: ToolCapabilities
├── structured_outputs: StructuredOutputCapabilities
├── reasoning: InterfaceReasoningCapabilities
├── prompt_caching: SupportState
└── state: StateCapabilities
```

完整 JSON 示例：

```json
{
  "id": "code-primary",
  "object": "model",
  "created": 1730000000,
  "owned_by": "openbridge",
  "name": "Code Primary",
  "description": "面向编码和工具调用的公共模型",
  "lifecycle": {
    "status": "active",
    "deprecated_at": null,
    "replacement_model": null
  },
  "capabilities": {
    "tasks": ["chat", "text_generation"],
    "context_window": {
      "max_context_tokens": 128000,
      "max_input_tokens": 120000,
      "max_output_tokens": 16000
    },
    "modalities": {
      "input": ["text", "image"],
      "output": ["text"]
    },
    "tokenizer": null,
    "knowledge_cutoff": null,
    "reasoning": {
      "support": "supported",
      "required": false,
      "levels": ["none", "low", "medium", "high"],
      "default_level": null
    }
  },
  "interfaces": {
    "chat_completions": {
      "guaranteed": {
        "context_window": {
          "max_context_tokens": 128000,
          "max_input_tokens": 120000,
          "max_output_tokens": 16000
        },
        "modalities": {
          "input": ["text"],
          "output": ["text"]
        },
        "parameters": {
          "supported": [
            "max_completion_tokens",
            "reasoning_effort",
            "stream",
            "temperature",
            "tool_choice",
            "tools"
          ],
          "constraints": {
            "reasoning_effort": {
              "type": "enum",
              "values": ["none", "low", "medium", "high"]
            },
            "temperature": {
              "type": "number",
              "minimum": 0.0,
              "maximum": 2.0
            }
          }
        },
        "streaming": "supported",
        "system_messages": "supported",
        "tools": {
          "support": "supported",
          "types": ["function"],
          "tool_choice_modes": ["none", "auto", "required", "named"],
          "parallel_calls": "supported",
          "strict_schema": "supported"
        },
        "structured_outputs": {
          "support": "supported",
          "modes": ["json_object", "json_schema"],
          "strict_schema": "supported"
        },
        "reasoning": {
          "support": "supported",
          "levels": ["none", "low", "medium", "high"],
          "output_modes": ["summary"]
        },
        "prompt_caching": "unknown",
        "state": {
          "store": "unsupported",
          "previous_response_id": "unsupported",
          "background": "unsupported"
        }
      },
      "profiles": [
        {
          "context_window": {
            "max_context_tokens": 128000,
            "max_input_tokens": 120000,
            "max_output_tokens": 16000
          },
          "modalities": {
            "input": ["text", "image"],
            "output": ["text"]
          },
          "parameters": {
            "supported": [
              "max_completion_tokens",
              "reasoning_effort",
              "stream",
              "temperature",
              "tool_choice",
              "tools"
            ],
            "constraints": {
              "reasoning_effort": {
                "type": "enum",
                "values": ["none", "low", "medium", "high"]
              },
              "temperature": {
                "type": "number",
                "minimum": 0.0,
                "maximum": 2.0
              }
            }
          },
          "streaming": "supported",
          "system_messages": "supported",
          "tools": {
            "support": "supported",
            "types": ["function"],
            "tool_choice_modes": ["none", "auto", "required", "named"],
            "parallel_calls": "supported",
            "strict_schema": "supported"
          },
          "structured_outputs": {
            "support": "supported",
            "modes": ["json_object", "json_schema"],
            "strict_schema": "supported"
          },
          "reasoning": {
            "support": "supported",
            "levels": ["none", "low", "medium", "high"],
            "output_modes": ["summary"]
          },
          "prompt_caching": "unknown",
          "state": {
            "store": "unsupported",
            "previous_response_id": "unsupported",
            "background": "unsupported"
          }
        }
      ]
    },
    "responses": {
      "guaranteed": {
        "context_window": {
          "max_context_tokens": 128000,
          "max_input_tokens": 120000,
          "max_output_tokens": 16000
        },
        "modalities": {
          "input": ["text"],
          "output": ["text"]
        },
        "parameters": {
          "supported": ["input", "max_output_tokens", "reasoning", "stream", "tools"],
          "constraints": {}
        },
        "streaming": "supported",
        "system_messages": "supported",
        "tools": {
          "support": "supported",
          "types": ["function"],
          "tool_choice_modes": ["none", "auto", "required", "named"],
          "parallel_calls": "supported",
          "strict_schema": "supported"
        },
        "structured_outputs": {
          "support": "supported",
          "modes": ["json_schema"],
          "strict_schema": "supported"
        },
        "reasoning": {
          "support": "supported",
          "levels": ["none", "low", "medium", "high"],
          "output_modes": ["summary"]
        },
        "prompt_caching": "unknown",
        "state": {
          "store": "unsupported",
          "previous_response_id": "unsupported",
          "background": "unsupported"
        }
      },
      "profiles": []
    }
  }
}
```

示例中的值只用于展示 schema，不是任何当前 Public Model 的事实。`responses.profiles` 为空表示没有高于 `guaranteed` 的附加完整能力组合；当存在条件能力时，`profiles` 必须列出每个高于 guaranteed、去重后的完整 profile。

主要字段约束：

| 字段 | 最终语义 |
|---|---|
| `id` | 下游稳定 Public Model id；必须是安全 URL path segment，不能使用上游 model id |
| `created` | Public Model 契约首次创建时间的稳定 Unix 秒；不得使用进程启动时间或每次构建时间 |
| `owned_by` | 固定为 `openbridge` 或另一个明确公共所有者；不得返回实际 Provider |
| `capabilities.tasks` | 模型任务类别；与 Chat/Responses HTTP interface 分开 |
| `context_window.max_context_tokens` | 输入和输出合计窗口上限 |
| `max_input_tokens` / `max_output_tokens` | 输入和输出各自上限；两个最大值不保证可同时使用，也不能简单相加 |
| `modalities` | 模型固有、已声明的输入/输出模态上界；接口 profile 只能收窄 |
| `parameters.supported` | 该协议 profile 保证接受的 OpenAI-compatible 参数名；Provider 私有参数不进入此集合 |
| `parameters.constraints` | 可选的公开数值/枚举约束；只允许 typed constraint，不返回 wire mapping 或默认 Provider 参数 |
| `tools` | tool 类型、`tool_choice` 模式、并行调用和 strict function schema 能力 |
| `structured_outputs` | JSON object、JSON schema 和 strict schema 等结构化输出模式 |
| `reasoning` | 模型支持/必需状态、标准 effort 等级，以及具体接口可返回的 reasoning 输出形态 |
| `state` | `store`、`previous_response_id`、`background` 等 API 状态能力；它们不是模型固有能力 |

`capabilities` 适合模型目录展示和粗粒度筛选，实际构造 Chat/Responses 请求时必须以目标 `interfaces.<api>.guaranteed` 和 `profiles` 为准。模型固有能力中出现某个模态或 reasoning，不代表每个 HTTP interface 都开放该能力，也不证明任意两个字段可以组合使用。

### 4.4 `guaranteed` 与完整 capability profiles

模型固有能力与协议可调用能力使用不同聚合语义：

- `capabilities` 是 Public Model 显式声明的模型固有能力上界；编译器验证每个接口 profile 都是其子集；
- `interfaces.<api>.guaranteed` 是该 API 所有完整可执行 profile 的保守交集，适合只需要简单布尔判断的客户端；
- `interfaces.<api>.profiles` 只列出**高于 guaranteed 基线**的、去重后的完整能力组合；没有条件能力时返回空数组；
- 一个请求能力组合可被声明为支持，当且仅当它完全落在 `guaranteed` 内，或至少有一个完整 profile 同时覆盖全部要求；
- profile 不带 route id、Provider、上游模型、优先级、权重、健康或 deployment 数量，多个等价 route 只生成一个 profile；
- profile 仅描述静态服务契约，不反映瞬时 cooldown、限流或故障。

交集规则必须固定：

| 字段类型 | `guaranteed` 计算 |
|---|---|
| `SupportState` | 全部为 `supported` 才是 `supported`；全部明确为 `unsupported` 才是 `unsupported`；已知 profile 在支持与不支持之间分化时为 `conditional`；证据不足时为 `unknown` |
| token 上限 | 取所有已知上限中的最小值；任一完整 profile 未知时，guaranteed 对应值为 `null` |
| 模态、参数、tool 类型、choice、reasoning level/output mode | 集合交集 |
| typed parameter constraint | 取安全交集；无法得到非空有效区间时该参数退出 guaranteed 参数集合 |
| `lifecycle` | 属于 Public Model 本身，不从 route 聚合 |

例如，一个完整 profile 同时支持 image+tools，另一个只支持 text+tools，则 guaranteed 可以把 tools 标为 `supported`，但 image 只存在于第一个附加 profile。若两个 profile 分别只支持 image 和 tools，guaranteed 中两者都不能标为 `supported`，相应聚合状态为 `conditional`，附加 profiles 必须分别保持完整组合，绝不能合成 image+tools。

### 4.5 编译期不变量

最终 registry 必须在启动时拒绝以下不一致：

1. Public Model id 不是安全单段资源 id，或 `created` 不是稳定的非负 Unix 时间；
2. interface profile 的 task、上下文、模态、reasoning 等能力扩大了 Public Model 固有能力上界；
3. advertised profile 没有任何一条完整 Native/Bridged route 能同时满足；
4. `tools.support = supported` 但没有 tool 类型或缺少相应公共参数；
5. parallel tool calls 已支持但 function tool 本身不支持；
6. reasoning 为 `unsupported` 却声明 levels、默认 level 或输出模式；
7. `default_level` 不属于声明的 reasoning levels；
8. structured output mode、state 能力或参数 constraint 与协议 schema 冲突；
9. 模型固有能力或完整 profile 使用仅允许聚合结果出现的 `conditional`；
10. 数组重复、顺序不稳定、出现未注册枚举或任意 Provider 私有字段；
11. 公共序列化对象中出现 Provider、Target、Route、upstream model、endpoint、credential 或 wire mapping。

每个附加 profile 还必须完整包含 guaranteed 基线，并至少严格增加一个已验证能力；否则应在编译时去重或拒绝。

### 4.6 当前代码的迁移方向

当前代码只作为迁移差距，不作为最终 schema 的约束。后续实现至少需要：

- 将现在只含 `name + routes` 的 `PublicModelConfig` 拆为公共 catalog definition 与私有 routing binding；
- 为 Public Model 增加稳定 `created`、展示元数据、生命周期和完整 `ModelCapabilities`；
- 将现有 `ModelContextLength` 扩展为 total/input/output 三个独立 token 上限；
- 增加 typed task、modality、tool type/choice、structured output、parameter constraint、prompt caching 和状态能力；
- 保留 canonical model、Provider 和 Upstream API 的逐层收窄，但让编译器验证公共 profile，而不是由 handler 临时遍历 route 拼 JSON；
- 在不可变 `RuntimeRegistry` 中预编译 `PublicModelInfo`、标准 `Model` 投影和协议 capability profiles；
- 让请求规划与模型信息接口共享同一 profile 判定逻辑，避免“目录宣称支持、planner 却拒绝”或反向漂移。

本轮只确定最终设计并修改调研文档，不修改这些 Rust 类型或运行时行为。

## 5. 三类模型信息接口

### 5.1 接口矩阵

产品上分为三类接口。为了完整覆盖官方 OpenAI Models resource，第一类包含 list 和 retrieve 两个标准 operation；扩展列表与扩展单个详情分别是第二、第三类。

| 类别 | HTTP 接口 | 返回对象 | 目标 |
|---|---|---|---|
| 1. OpenAI 标准 Models | `GET /v1/models` | 标准 list envelope | 兼容 OpenAI SDK 和只需要 model id 的客户端 |
| 1. OpenAI 标准 Models | `GET /v1/models/{model}` | 标准 `Model` 对象 | 完整标准 retrieve 语义；不返回扩展能力 |
| 2. 扩展 Model 列表 | `GET /v1/model/info` | `PublicModelInfo` list envelope | 一次读取所有公共模型的完整能力 |
| 3. 扩展单模型详情 | `GET /v1/model/info/{model}` | 单个 `PublicModelInfo` | 精确读取一个公共模型的完整能力 |

Public Model id 必须验证为安全 URL path segment，例如 `[A-Za-z0-9][A-Za-z0-9._:-]{0,127}`。上游可能带 `/` 的 model id 继续留在私有 binding 中，不得直接成为公共资源 id。

### 5.2 OpenAI 标准 Models 契约

官方 OpenAI Models API 的标准 `Model` 只包含基本身份字段。OpenBridge 的最终标准投影为：

```json
{
  "id": "code-primary",
  "object": "model",
  "created": 1730000000,
  "owned_by": "openbridge"
}
```

列表响应：

```json
{
  "object": "list",
  "data": [
    {
      "id": "code-primary",
      "object": "model",
      "created": 1730000000,
      "owned_by": "openbridge"
    }
  ]
}
```

规则：

- `/v1/models` 和 `/v1/models/{model}` 只使用这四个标准字段，不偷偷加入 `capabilities`；
- `created` 来自 Public Model 定义中的稳定时间；当前实现需要补齐该字段；
- 标准 retrieve 直接返回一个 `Model`，不包 `{data: ...}`；
- list 与 retrieve 必须来自同一份 `PublicModelInfo` 编译结果，不能维护两份模型目录；
- 未知或不可见模型返回 404，并使用稳定 OpenAI-compatible error，不暴露它是否存在于内部 deployment。

### 5.3 扩展 Model 列表契约

`GET /v1/model/info` 返回完整 `PublicModelInfo`：

```json
{
  "object": "list",
  "data": [
    {
      "id": "code-primary",
      "object": "model",
      "created": 1730000000,
      "owned_by": "openbridge",
      "name": "Code Primary",
      "description": "面向编码和工具调用的公共模型",
      "lifecycle": {},
      "capabilities": {},
      "interfaces": {}
    }
  ]
}
```

上例中的空对象仅用于缩短 envelope 示例，实际每个元素必须使用 4.3 节的完整 schema。第一版返回全部静态 Public Models，不分页、不按 Provider 过滤，也不在请求期间访问外部目录。若未来模型数量确实需要分页，应在不改变单个 `PublicModelInfo` 的前提下增加 cursor envelope。

### 5.4 扩展单模型详情契约

`GET /v1/model/info/{model}` 直接返回与扩展列表元素完全相同的 `PublicModelInfo`，不再包一层 `data`：

```json
{
  "id": "code-primary",
  "object": "model",
  "created": 1730000000,
  "owned_by": "openbridge",
  "name": "Code Primary",
  "description": "面向编码和工具调用的公共模型",
  "lifecycle": {},
  "capabilities": {},
  "interfaces": {}
}
```

单模型接口与列表接口必须逐字段一致；同一 registry snapshot 下，`GET /v1/model/info/{id}` 应等于 `GET /v1/model/info` 中对应 `data[]` 元素。

### 5.5 三类接口的共同边界

- 三类接口使用相同 Bearer 认证和用户可见范围，模型集合与业务请求可选择的 Public Models 一致；
- 模型按 Public Model id 确定性排序，单个请求读取同一不可变 registry snapshot；
- 返回值不因瞬时 Provider 健康、credential 轮换、cooldown、延迟或配额变化而抖动；
- 标准接口是完整对象的严格投影，扩展列表和扩展单个接口共享一个 serializer/DTO；
- 不包含 Provider、canonical/upstream model id、Target、Route、Native/Bridge 模式、base URL、endpoint profile、credential、header、wire mapping、团队或访问组；
- 不包含价格、成本、TPM/RPM、实时健康、延迟、TTFT、错误统计、质量排行或 benchmark；
- 不通过 LiteLLM/OpenRouter 的目录动态发现或自动注册模型；外部目录只作为人工核实来源；
- 未知能力保持 `unknown`/`null`，请求规划继续 fail closed。

建议未知模型错误：

```json
{
  "error": {
    "message": "The requested model does not exist or is not available",
    "type": "invalid_request_error",
    "param": "model",
    "code": "model_not_found"
  }
}
```

### 5.6 实施前置条件与验证边界

只有在后续明确批准实现时，才进入代码调整。最小验证范围包括：

1. registry 定义和编译失败测试：非法 public id、不稳定/缺失 `created`、能力扩大、无完整 route 的 profile、reasoning/tools/constraint 不一致；
2. capability profile 测试：交集、unknown、数值上限、集合交集、去重，以及 image/tools 分属不同 profile 时不能合成；
3. 三类接口测试：认证、稳定排序、空列表、标准四字段投影、扩展完整 schema、标准/扩展单模型 404；
4. 一致性测试：标准列表、标准 retrieve、扩展列表、扩展单个详情共享同一模型集合和 snapshot；
5. 安全测试：序列化 JSON 中不得出现 Provider、Target、Route、upstream model、URL、credential 或 wire mapping 字段；
6. 新的客户端可见验收优先使用 OpenAI SDK 或独立 Python/curl 验证标准 Models API，再运行聚焦 Rust 契约测试；
7. 按仓库默认基线运行 `cargo fmt -- --check`、`cargo test --locked`、`cargo clippy --locked -- -D warnings` 和 `git diff --check`。

这些检查只能证明 OpenBridge 自有 registry 和 HTTP 契约，不证明 LiteLLM/OpenRouter 目录新鲜度、真实 Provider 的长期能力、负载表现或生产兼容性。当前只完成设计文档，尚未修改代码或执行运行时验证。

## 6. 结论

LiteLLM 最值得借鉴的是能力目录的字段宽度和 `mode`/`supports_*` 的可枚举化；但 `/model/info` 和 `/model_group/info` 明确展示了 deployment/聚合边界，不能原样对外。OpenRouter 最值得借鉴的是以单一 `Model` 对象组织 canonical identity、architecture、context、supported parameters 和 reasoning；但 `pricing`、`benchmarks`、`top_provider` 与 endpoint 详情应与协议能力隔离。

OpenBridge 的最终落点不是“给现有 `ModelConfig` 多序列化几个字段”，而是建立独立的 Public Model capability contract：公共身份和模型固有能力显式声明，Chat/Responses 用 guaranteed + anonymous complete profiles 表达真正可调用的组合，私有 route/deployment 只负责证明和实现该契约。对外提供 OpenAI 标准 Models、扩展模型列表和扩展单模型详情三类接口，且所有响应来自同一份编译后的 `PublicModelInfo`。

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
