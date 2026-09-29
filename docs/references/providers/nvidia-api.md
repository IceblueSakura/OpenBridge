# NVIDIA API Catalog / NIM API 协议入口

- Last reverified：2026-09-29 UTC；读取当前 NIM API Reference、选定模型官方页并核对托管目录。
- Recheck trigger：API Catalog/NIM endpoint、认证或 hosted/self-hosted 边界变化。

## 来源与范围

- [NVIDIA NIM LLM API Reference](https://docs.nvidia.com/nim/large-language-models/latest/api-reference.html)
- [NeMo Retriever Authentication](https://docs.nvidia.com/nemo/retriever/26.5.0/extraction/api-keys)

本文只记录托管 API Catalog 与 NIM 的入口、认证和部署边界，不保存 Models 全量目录或逐模型能力。

## 入口与认证

- API Catalog 托管推理使用 `https://integrate.api.nvidia.com/v1`；本地或自托管 NIM 通常使用部署方提供的 `/v1` base。
- API key 通过 `Authorization: Bearer ***` 传递。build.nvidia.com 的 API key 与访问 `nvcr.io` 的 NGC personal key 不是同一种凭证。
- API Catalog 是多模型聚合入口；NIM 是单模型或自部署服务。两者都可能暴露 OpenAI-compatible endpoint，但可用操作和限制由具体部署决定。

## 常见协议入口

NIM API Reference 描述 Chat Completions、Completions、Responses、Models 与 template render 入口；目录可见不证明某模型支持全部 endpoint。具体模型支持、配额、价格和部署条件应直接读取 NVIDIA 当前官方模型页。

OpenBridge 旧运行时映射见[固定归档](https://github.com/IceblueSakura/OpenBridge/blob/4f13ecefa21265a6ec5aa967278e81f03039f585/docs/implementation-status/model-provider-mapping.md)，不是 v2 当前能力。

## 证据边界

选定 [Nemotron 3 Super 官方页](https://build.nvidia.com/nvidia/nemotron-3-super-120b-a12b)声明 Chat 与工具能力；本轮固定绑定的实际 JSON/SSE、工具选择差异与未解决的失败见 [onboarding evidence](../../implementation-status/evidence/2026-09-29-api-key-provider-onboarding.md)。该证据不能外推其他 NIM 部署、模型、配额或长期可用性。
