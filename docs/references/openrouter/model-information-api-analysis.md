# OpenRouter Models API 与模型能力字段调研

## 状态与证据

- 调研日期：2026-08-03
- 来源：OpenRouter 官方 Models API 与模型指南
- 本文只记录 OpenRouter 模型目录、单模型详情、过滤和字段语义。

## 1. 接口分层

| 接口 | 粒度 | 返回重点 |
| --- | --- | --- |
| `GET /api/v1/models` | 公开模型目录 | `{data, links, total_count}` 与完整 `Model` 对象 |
| `GET /api/v1/model/{author}/{slug}` | 单个模型 | 一个 `Model` 对象 |
| `GET /api/v1/models/count` | 目录计数 | 可按输出模态过滤的数量 |
| `GET /api/v1/models/user` | 用户偏好视图 | 叠加 Provider 偏好、隐私、guardrail 或区域约束 |
| `GET /api/v1/models/{author}/{slug}/endpoints` | Provider endpoint | endpoint 价格、参数和供应详情 |

列表接口支持文本、category、supported parameters、输入输出模态、context、价格、作者、Provider、区域、模型年龄和若干质量指标过滤。过滤条件不等于每个返回字段都属于稳定协议能力。

## 2. `Model` 对象

OpenRouter 的模型对象可归纳为：

```text
Model
├── identity: id / canonical slug / name / description
├── lifecycle: created / expiration / optional knowledge cutoff
├── architecture: input/output modalities / tokenizer / instruct type
├── limits: context length / top-provider max completion tokens
├── parameters: supported parameters / defaults
├── reasoning: parameter support and optional effort metadata
├── provider summary: top provider / per-request limits
├── economics and quality: pricing / benchmarks / indexes
└── links: endpoint details
```

重点字段边界：

- `context_length` 是目录模型的上下文值；`top_provider.max_completion_tokens` 是供应摘要中的输出限制，两者不能相加。
- `supported_parameters` 描述工具、reasoning、structured output、采样和停止等参数入口，但不自动给出每个参数的完整取值域。
- `architecture.input_modalities` 与 `output_modalities` 明确区分方向。
- `top_provider`、pricing、per-request limit 和 endpoint link 属于供应或经济信息，不是模型本体的全部稳定能力。
- benchmarks 与 intelligence/coding/agentic 指标是质量信号，不是协议兼容保证。

## 3. 目录与 endpoint 的分离

基础 Models API 返回 canonical 模型目录；Provider endpoint 详情通过独立资源访问。用户目录还会叠加账户偏好和政策。因此同一个模型 id 的目录事实、用户可见性和某个 endpoint 的实时供应状态需要分别解释。

## 4. 适用边界

- `Model` 同时包含能力、经济和供应摘要，消费者需要按用途筛选字段。
- `top_provider` 不表示所有可用 Provider endpoint 都具有相同限制。
- user-filtered catalog 是账户视图，不是公开 canonical catalog 的替代。
- Models API 目录不能证明一次具体请求、工具调用或 SSE lifecycle 当前成功。

## 一手资料

- [List all models and their properties](https://openrouter.ai/docs/api/api-reference/models/list-all-models-and-their-properties)
- [Get a model by its slug](https://openrouter.ai/docs/api/api-reference/models/get-a-model-by-its-slug)
- [Get total count of available models](https://openrouter.ai/docs/api/api-reference/models/get-total-count-of-available-models)
- [List models filtered by user preferences](https://openrouter.ai/docs/api/api-reference/models/list-models-filtered-by-user-provider-preferences-privacy-settings-and-guardrails)
- [Models guide](https://openrouter.ai/docs/guides/overview/models)
