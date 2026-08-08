# NVIDIA MiniMax M3 与百炼 GLM/Qwen Chat 模型入口

## 范围与快照

- 快照日期：2026-08-08。
- 只读取 NVIDIA 与阿里云官方文档；未执行带认证请求、模型列表探测或 SDK 调用。
- 本页记录固定 API 入口与模型 ID，不从模型介绍页推断全部 wire capability。

## NVIDIA API Catalog

官方 LLM API reference 将托管推理入口描述为 OpenAI-compatible Chat Completions API，固定根地址为
`https://integrate.api.nvidia.com/v1`，Chat path 为 `/chat/completions`，使用 Bearer API key。MiniMax M3 的官方模型页使用
`minimaxai/minimax-m3` 作为模型标识。

来源：

- [NVIDIA NIM LLM APIs](https://docs.api.nvidia.com/nim/reference/llm-apis)
- [MiniMax M3 model reference](https://docs.api.nvidia.com/nim/reference/minimaxai-minimax-m3)

模型页还描述多模态、长上下文等模型级特征，但没有经过当前账号与固定 Chat payload 的真实观察；这些描述不能单独证明图片、工具、
结构化输出、reasoning wire、streaming terminal 或额度边界。

## 阿里云百炼 Model Studio

官方 OpenAI Chat 兼容文档给出北京地域兼容根地址
`https://dashscope.aliyuncs.com/compatible-mode/v1`，Chat path 为 `/chat/completions`。官方 GLM 页面列出模型 ID
`glm-5.2`；Qwen 模型目录与 Chat 调用文档列出 `qwen3.7-plus` 和 `qwen3.7-max`。

来源：

- [GLM model API reference](https://help.aliyun.com/en/model-studio/glm)
- [Qwen model catalog](https://help.aliyun.com/en/model-studio/text-generation-model/)
- [Qwen via OpenAI Chat Completions](https://help.aliyun.com/en/model-studio/qwen-api-via-openai-chat-completions)
- [百炼子业务空间模型调用与北京兼容入口](https://help.aliyun.com/zh/model-studio/model-calling-in-sub-workspace)

百炼文档中的地域、workspace、模型权限、工具、视觉、reasoning 与具体参数支持可能随账户和模型变化。本快照没有验证这些模型是否已对某个
真实账号开放，也没有验证 SSE 终态、错误信封、模型列表或高级能力。

## 证据边界

这些官方页面足以确定本次静态接入所需的 Provider 根地址、Chat path 与四个上游模型 ID。真实可达性、账号授权、配额、当前响应形状及任何
高级能力仍需要使用无敏感输出的显式 Provider 验收单独确认。
