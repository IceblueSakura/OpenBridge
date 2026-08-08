# NVIDIA API Catalog / NIM API 协议入口调研

## 来源与范围

本文只记录 NVIDIA 托管推理（API Catalog / NIM hosted endpoint）的协议入口、认证与 wire 事实，不包含本地接入状态。模型目录见 [models.md](models.md)。

- [NVIDIA NIM for Large Language Models API Reference](https://docs.nvidia.com/nim/large-language-models/latest/api-reference.html)（2026-08-08 抓取）
- [NeMo Retriever Authentication and API keys](https://docs.nvidia.com/nemo/retriever/26.5.0/extraction/api-keys)
- NVIDIA API Catalog Models endpoint 观察（见 [models.md](models.md)，2026-08-08）

## 观察事实

### 入口与认证

- OpenAI-compatible base URL 为 `https://integrate.api.nvidia.com/v1`（API Catalog 托管推理）；本地/自托管 NIM 通常使用 `http://<host>:8000/v1`。
- API key 前缀为 `nvapi-`，标准环境变量为 `NVIDIA_API_KEY`，通过 `Authorization: Bearer $NVIDIA_API_KEY` 传递。
- `NVIDIA_API_KEY`（build.nvidia.com 生成）与 NGC personal key（`nvcr.io` 用）不是同一个字符串，不能互换。
- 模型 ID 采用 `vendor/model` 命名空间格式（如 `z-ai/glm4.7`、`zhipuai/glm-4`、`minimax/minimax-m3`），请求 `model` 字段直接传完整 ID。

### 端点（NIM API Reference）

| Endpoint | 说明 |
|---|---|
| `POST /v1/chat/completions` | 多轮对话补全，支持 streaming 与 tool calling |
| `POST /v1/completions` | 单轮文本补全 |
| `POST /v1/responses` | OpenAI Responses API 入口 |
| `GET /v1/responses/{response_id}` | 获取已创建的 response |
| `POST /v1/responses/{response_id}/cancel` | 取消进行中的 streaming response |
| `GET /v1/models` | 模型目录（API Catalog） |
| `POST /v1/chat/completions/render` | 渲染 chat template，不执行推理（NIM） |
| `POST /v1/completions/render` | 渲染 prompt template，不执行推理（NIM） |

API Catalog 与 NIM 都声明 OpenAI-compatible 表面；API Catalog 是 NVIDIA 托管的多模型聚合入口（一次 key 访问全部目录模型），NIM 是单模型/自部署服务（`/docs` 提供交互式 OpenAPI explorer）。

### 速率与可用性

- API Catalog 速率限制以请求头与 429 响应体现（`x-ratelimit-*` 计数、retry window），per-account 上限在 build.nvidia.com 账户页显示。
- 免费/付费模型、BYOK endpoint 与各模型支持端点集合以模型页为准；模型存在不自动证明某个端点、参数或配额可用。

## 证据边界

NIM API Reference 描述的是 NIM 运行时的 OpenAI-compatible 表面，API Catalog 托管推理的具体模型端点支持（如哪些模型支持 Responses）未在本快照逐模型核验。本文没有执行真实 NVIDIA 请求，不证明实际响应、错误分类、配额或 streaming 行为；认证约定来自官方文档与社区实现观察，使用前以 build.nvidia.com 模型页为准。
