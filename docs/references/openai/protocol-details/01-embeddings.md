# OpenAI Embeddings 协议调研

## 1. Wire contract

`POST /v1/embeddings` 是独立 JSON operation。官方 request 的核心字段包括：

- `model`；
- `input`：单个字符串、字符串数组、token 数组或 token-array 批次；
- `encoding_format`：常见为 `float` 或 `base64`；
- 支持模型可接受 `dimensions`；
- 可选 `user`。

成功响应使用 `object: "list"`，包含有序 `data[]`。每项有 `object: "embedding"`、`embedding` 与 `index`；顶层还含 `model` 和
token `usage`。

资料：[Create embeddings](https://developers.openai.com/api/reference/resources/embeddings/methods/create)、[Embeddings guide](https://developers.openai.com/api/docs/guides/embeddings)、[Python SDK request type](https://github.com/openai/openai-python/blob/main/src/openai/types/embedding_create_params.py)。

## 2. 与生成 API 的差异

- response 是向量列表，不是 assistant message、output item 或 SSE event lifecycle；
- input batch 顺序通过 `index` 与 response data 对齐；
- token usage 只有输入/总量语义，不具有 completion output token；
- `dimensions` 与 encoding 是带取值域的参数，不是简单 capability 布尔值；
- vector identity 依赖 model、dimensions、encoding 和模型版本，不能仅因 endpoint 相同就认为可混用。

## 3. 边界

- `float` 与 `base64` response 具有不同大小和 decode 风险。
- 批量 input、token 数和 response vector size 都受模型与服务限制。
- 外部目录声明 embedding mode 不证明某个具体 endpoint/model 当前接受全部 input form。
- 失败、rate limit、batch size 与 dimension domain 需要按目标模型资料分别复核。

