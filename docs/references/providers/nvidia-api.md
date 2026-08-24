# NVIDIA API Catalog / NIM API 协议入口

- Last reverified：外部来源最后复核 2026-08-08；2026-08-24 仅整理本地文档，未刷新外部来源。
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

OpenBridge 当前映射见[Model 与 Provider 映射](../../implementation-status/model-provider-mapping.md)。

## 证据边界

本文未执行真实 NVIDIA 请求，不证明实际响应、错误分类、配额、streaming 或当前账户 entitlement。
