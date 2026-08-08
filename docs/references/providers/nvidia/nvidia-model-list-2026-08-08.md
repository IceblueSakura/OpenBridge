# NVIDIA API Catalog Models 列表

## 范围与快照

- 快照日期：2026-08-08。
- 观察入口：NVIDIA API Catalog OpenAI-compatible Models endpoint，https://integrate.api.nvidia.com/v1/models。
- 观察结果：HTTP 200，返回 100 个模型 ID。
- 观察方式：使用当前项目已配置的 NVIDIA API key 通过受信的 Models probe 获取；本文不记录 key、请求头或响应正文。
- `minimaxai/minimax-m3` 出现在本次返回列表中。
- 下方按模型 ID 中第一个 `/` 之前的 namespace 分组，仅用于阅读；namespace 不是 Models API 明示的供应商、授权或能力字段。

模型列表、账号权限、地域、配额和模型生命周期可能变化。本快照不代表每个模型均已通过 Chat、Responses、Embeddings、工具调用、多模态或其它能力请求。

## Namespace 汇总

| Namespace | 数量 |
|---|---:|
| `01-ai` | 1 |
| `adept` | 1 |
| `ai21labs` | 1 |
| `aisingapore` | 1 |
| `baai` | 1 |
| `bigcode` | 1 |
| `databricks` | 1 |
| `deepseek-ai` | 2 |
| `google` | 9 |
| `ibm` | 4 |
| `meta` | 10 |
| `microsoft` | 3 |
| `minimaxai` | 1 |
| `mistralai` | 6 |
| `moonshotai` | 1 |
| `nv-mistralai` | 1 |
| `nvidia` | 44 |
| `openai` | 2 |
| `poolside` | 1 |
| `snowflake` | 1 |
| `stepfun-ai` | 1 |
| `thinkingmachines` | 1 |
| `writer` | 4 |
| `z-ai` | 1 |
| `zyphra` | 1 |
| **合计** | **100** |

## 完整模型列表

### `01-ai`（1）

- `01-ai/yi-large`

### `adept`（1）

- `adept/fuyu-8b`

### `ai21labs`（1）

- `ai21labs/jamba-1.5-large-instruct`

### `aisingapore`（1）

- `aisingapore/sea-lion-7b-instruct`

### `baai`（1）

- `baai/bge-m3`

### `bigcode`（1）

- `bigcode/starcoder2-15b`

### `databricks`（1）

- `databricks/dbrx-instruct`

### `deepseek-ai`（2）

- `deepseek-ai/deepseek-coder-6.7b-instruct`
- `deepseek-ai/deepseek-v4-flash-0731`

### `google`（9）

- `google/codegemma-1.1-7b`
- `google/codegemma-7b`
- `google/deplot`
- `google/diffusiongemma-26b-a4b-it`
- `google/gemma-2b`
- `google/gemma-3-12b-it`
- `google/gemma-3-4b-it`
- `google/gemma-4-31b-it`
- `google/recurrentgemma-2b`

### `ibm`（4）

- `ibm/granite-3.0-3b-a800m-instruct`
- `ibm/granite-3.0-8b-instruct`
- `ibm/granite-34b-code-instruct`
- `ibm/granite-8b-code-instruct`

### `meta`（10）

- `meta/codellama-70b`
- `meta/llama-3.1-70b-instruct`
- `meta/llama-3.1-8b-instruct`
- `meta/llama-3.2-11b-vision-instruct`
- `meta/llama-3.2-1b-instruct`
- `meta/llama-3.2-3b-instruct`
- `meta/llama-3.2-90b-vision-instruct`
- `meta/llama-3.3-70b-instruct`
- `meta/llama-guard-4-12b`
- `meta/llama2-70b`

### `microsoft`（3）

- `microsoft/kosmos-2`
- `microsoft/phi-3-vision-128k-instruct`
- `microsoft/phi-3.5-moe-instruct`

### `minimaxai`（1）

- `minimaxai/minimax-m3`

### `mistralai`（6）

- `mistralai/codestral-22b-instruct-v0.1`
- `mistralai/mistral-7b-instruct-v0.3`
- `mistralai/mistral-large`
- `mistralai/mistral-large-2-instruct`
- `mistralai/mistral-nemotron`
- `mistralai/mixtral-8x22b-v0.1`

### `moonshotai`（1）

- `moonshotai/kimi-k2.6`

### `nv-mistralai`（1）

- `nv-mistralai/mistral-nemo-12b-instruct`

### `nvidia`（44）

- `nvidia/ai-synthetic-video-detector`
- `nvidia/cosmos-reason2-8b`
- `nvidia/embed-qa-4`
- `nvidia/ising-calibration-1.5-31b`
- `nvidia/llama-3.1-nemoguard-8b-content-safety`
- `nvidia/llama-3.1-nemoguard-8b-topic-control`
- `nvidia/llama-3.1-nemotron-51b-instruct`
- `nvidia/llama-3.1-nemotron-70b-instruct`
- `nvidia/llama-3.1-nemotron-nano-8b-v1`
- `nvidia/llama-3.1-nemotron-nano-vl-8b-v1`
- `nvidia/llama-3.1-nemotron-safety-guard-8b-v3`
- `nvidia/llama-3.1-nemotron-ultra-253b-v1`
- `nvidia/llama-3.2-nemoretriever-1b-vlm-embed-v1`
- `nvidia/llama-3.2-nv-embedqa-1b-v1`
- `nvidia/llama-3.3-nemotron-super-49b-v1`
- `nvidia/llama-3.3-nemotron-super-49b-v1.5`
- `nvidia/llama-nemotron-embed-1b-v2`
- `nvidia/llama-nemotron-embed-vl-1b-v2`
- `nvidia/llama3-chatqa-1.5-70b`
- `nvidia/mistral-nemo-minitron-8b-8k-instruct`
- `nvidia/nemoretriever-parse`
- `nvidia/nemotron-3-embed-1b`
- `nvidia/nemotron-3-nano-30b-a3b`
- `nvidia/nemotron-3-nano-omni-30b-a3b-reasoning`
- `nvidia/nemotron-3-super-120b-a12b`
- `nvidia/nemotron-3-ultra-550b-a55b`
- `nvidia/nemotron-3.5-content-safety`
- `nvidia/nemotron-4-340b-instruct`
- `nvidia/nemotron-4-340b-reward`
- `nvidia/nemotron-mini-4b-instruct`
- `nvidia/nemotron-nano-12b-v2-vl`
- `nvidia/nemotron-nano-3-30b-a3b`
- `nvidia/nemotron-parse`
- `nvidia/neva-22b`
- `nvidia/nv-embed-v1`
- `nvidia/nv-embedcode-7b-v1`
- `nvidia/nv-embedqa-e5-v5`
- `nvidia/nv-embedqa-mistral-7b-v2`
- `nvidia/nvclip`
- `nvidia/nvidia-nemotron-nano-9b-v2`
- `nvidia/riva-translate-4b-instruct`
- `nvidia/riva-translate-4b-instruct-v1.1`
- `nvidia/riva-translate-4b-instruct-v2`
- `nvidia/vila`

### `openai`（2）

- `openai/gpt-oss-120b`
- `openai/gpt-oss-20b`

### `poolside`（1）

- `poolside/laguna-xs-2.1`

### `snowflake`（1）

- `snowflake/arctic-embed-l`

### `stepfun-ai`（1）

- `stepfun-ai/step-3.7-flash`

### `thinkingmachines`（1）

- `thinkingmachines/inkling`

### `writer`（4）

- `writer/palmyra-creative-122b`
- `writer/palmyra-fin-70b-32k`
- `writer/palmyra-med-70b`
- `writer/palmyra-med-70b-32k`

### `z-ai`（1）

- `z-ai/glm-5.2`

### `zyphra`（1）

- `zyphra/zamba2-7b-instruct`

## 证据边界

- 本文记录一次 Models 列表响应中的模型 ID 和 namespace 分组；没有对每个 ID 发起 Chat、Responses、Embeddings 或其它能力请求。
- Models 列表中的模型不等同于当前账号一定具备调用权限，也不等同于项目已经注册了对应的下游 Public Model。
- 出现图像、音频、Embedding、安全检测或其它专用模型名称，不代表兼容入口、请求字段、响应形状或能力已被本次观察验证。
- 本文不记录 API key、认证头、请求正文、响应正文、价格、配额、可用区域或模型生命周期结论。

## 参考来源

- NVIDIA API Catalog Models endpoint: https://integrate.api.nvidia.com/v1/models
- [NVIDIA NIM LLM APIs](https://docs.api.nvidia.com/nim/reference/llm-apis)
- [MiniMax M3 model reference](https://docs.api.nvidia.com/nim/reference/minimaxai-minimax-m3)
