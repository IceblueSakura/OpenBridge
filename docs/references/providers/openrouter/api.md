# OpenRouter API 与模型能力调研

## 来源与检查范围

- [Chat Completions API](https://openrouter.ai/docs/api/api-reference/chat/send-chat-completion-request?explorer=true)
- [Models API](https://openrouter.ai/docs/api/api-reference/models/get-models)
- [Responses API Beta](https://openrouter.ai/docs/api/reference/responses/overview)
- [Nemotron 3 Ultra Free model page](https://openrouter.ai/nvidia/nemotron-3-ultra-550b-a55b%3Afree/api)
- [List all models and their properties](https://openrouter.ai/docs/api/api-reference/models/list-all-models-and-their-properties)
- [Get a model by its slug](https://openrouter.ai/docs/api/api-reference/models/get-a-model-by-its-slug)
- [Get total count of available models](https://openrouter.ai/docs/api/api-reference/models/get-total-count-of-available-models)
- [List models filtered by user preferences](https://openrouter.ai/docs/api/api-reference/models/list-models-filtered-by-user-provider-preferences-privacy-settings-and-guardrails)
- [Models guide](https://openrouter.ai/docs/guides/overview/models)

本文只记录 OpenRouter 模型目录、单模型详情、过滤和字段语义，以及入口/认证与一次固定日期的 live wire 观察；二者都只适用于 OpenRouter。模型目录数据快照见 [models.md](models.md)。

## 1. 接口分层

| 接口                                           | 粒度              | 返回重点                                         |
|------------------------------------------------|-------------------|--------------------------------------------------|
| `GET /api/v1/models`                           | 公开模型目录      | `{data, links, total_count}` 与完整 `Model` 对象 |
| `GET /api/v1/model/{author}/{slug}`            | 单个模型          | 一个 `Model` 对象                                |
| `GET /api/v1/models/count`                     | 目录计数          | 可按输出模态过滤的数量                           |
| `GET /api/v1/models/user`                      | 用户偏好视图      | 叠加 Provider 偏好、隐私、guardrail 或区域约束   |
| `GET /api/v1/models/{author}/{slug}/endpoints` | Provider endpoint | endpoint 价格、参数和供应详情                    |

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

## 3. 官方 API 事实

- API base 为 `https://openrouter.ai/api/v1`。
- Chat Completions、Responses 和 Models 相对 path 分别为 `/chat/completions`、`/responses`、`/models`。
- API key 使用 `Authorization: Bearer <OPENR...Y>`。
- Responses 页面描述 JSON/SSE、reasoning 和 function tool；该 surface 是无状态的，`store: true` 和非空
  `previous_response_id` 返回 400。
- `HTTP-Referer`、`X-Title` 是可选 attribution/routing header，不是 Bearer 认证本身。

## 4. 官方示例与 live wire 差异（2026-08-02 观察）

官方 streaming 示例曾显示顶层 `type: "response.done"`、嵌套 `response.status: "completed"`，随后发送 `[DONE]`。

2026-08-02 对基础模型 `nvidia/nemotron-3-ultra-550b-a55b` 的两次成功 Responses streaming 观察均得到：

- HTTP 200；
- data-only SSE，没有 `event:` line；
- terminal data JSON 顶层 `type: "response.completed"`；
- 嵌套 `response.status: "completed"`；
- terminal 后另有 `[DONE]`；
- 没有出现 `response.done`。

该观察只记录原始 upstream wire 差异。它不证明错误 terminal、其他模型、全部参数或未来版本使用同一事件。

## 5. 目录与 endpoint 的分离

基础 Models API 返回 canonical 模型目录；Provider endpoint 详情通过独立资源访问。用户目录还会叠加账户偏好和政策。因此同一个模型
id 的目录事实、用户可见性和某个 endpoint 的实时供应状态需要分别解释。

## 6. 模型与数据政策边界

- 平台统一 Responses surface 不自动证明每个模型支持所有参数组合。
- 基础模型与 `:free` 变体有不同目录元数据和供应条件。
- `:free` 模型页声明免费 endpoint 会记录会话，不应发送机密或个人信息；该政策不能自动外推到基础模型或其他 endpoint。
- 一次成功请求只证明该模型、账户和时间点的成功流，不证明长期权限、配额、SLA、tool/reasoning 细节或所有错误行为。

## 7. 适用边界

- `Model` 同时包含能力、经济和供应摘要，消费者需要按用途筛选字段。
- `top_provider` 不表示所有可用 Provider endpoint 都具有相同限制。
- user-filtered catalog 是账户视图，不是公开 canonical catalog 的替代。
- Models API 目录不能证明一次具体请求、工具调用或 SSE lifecycle 当前成功。

## 8. 复核条件

endpoint、认证、模型 id、Responses beta 行为、数据政策、attribution header 或具体模型页面变化时，需要重新采集官方资料和 wire
transcript，并分别记录模型变体。
