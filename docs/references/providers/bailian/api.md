# 阿里云百炼 API 协议入口调研

## 来源与范围

本文只记录阿里云百炼（Model Studio）对外暴露的协议入口、认证与 wire 事实，不包含本地接入状态。模型目录见 [models.md](models.md)。

- [OpenAI 兼容-Chat](https://help.aliyun.com/zh/model-studio/qwen-api-via-openai-chat-completions)（2026-08-08 抓取）
- [OpenAI 兼容-Responses](https://help.aliyun.com/zh/model-studio/qwen-api-via-openai-responses)
- [深度思考](https://help.aliyun.com/zh/model-studio/deep-thinking)
- [文本生成模型 API 参考](https://help.aliyun.com/zh/model-studio/qwen-api-reference)
- [OpenAI 兼容-Batch Chat](https://help.aliyun.com/zh/model-studio/openai-compatible-batch-chat)

## 协议面总览

百炼提供四类文本生成调用接口（官方 API 参考页声明）：

1. **OpenAI 兼容 Chat Completions**——与 OpenAI 客户端库直接兼容；
2. **OpenAI 兼容 Responses**——内置联网搜索、代码解释器和网页内容提取工具，自动管理对话历史；
3. **Anthropic 兼容 Messages**——兼容 Anthropic Messages API，支持思考和工具调用；
4. **DashScope 原生**——百炼原生接口，功能集与参数最完整（Qwen-Audio 等模型仅支持此协议，不支持 OpenAI 兼容协议）。

## 观察事实

### 入口与地域

OpenAI 兼容 base URL（`compatible-mode/v1`），`{WorkspaceId}` 为业务空间 ID：

| 地域 | base_url（新专属域名） | 旧域名（仍可用） |
|---|---|---|
| 华北2（北京） | `https://{WorkspaceId}.cn-beijing.maas.aliyuncs.com/compatible-mode/v1` | `https://dashscope.aliyuncs.com/compatible-mode/v1` |
| 新加坡 | `https://{WorkspaceId}.ap-southeast-1.maas.aliyuncs.com/compatible-mode/v1` | `https://dashscope-intl.aliyuncs.com/compatible-mode/v1` |
| 美国（弗吉尼亚） | `https://dashscope-us.aliyuncs.com/compatible-mode/v1` | — |
| 德国（法兰克福） | `https://{WorkspaceId}.eu-central-1.maas.aliyuncs.com/compatible-mode/v1` | — |
| 日本（东京） | `https://{WorkspaceId}.ap-northeast-1.maas.aliyuncs.com/compatible-mode/v1` | — |

百炼官方建议迁移至业务空间专属域名（推理性能与稳定性更优）；现有域名仍可正常使用。

Chat 请求地址：`POST {base_url}/chat/completions`。

### 认证

`Authorization: Bearer $DASHSCOPE_API_KEY`（API key 前缀 `sk-`）。各地域 API key 不同，获取入口为百炼控制台。

### 请求能力面（OpenAI 兼容 Chat）

- 文本输入、图像输入（`image_url`）、视频输入（`video` 图片列表）、工具调用（`tools` function）、联网搜索（`enable_search`）、流式输出（`stream` + `stream_options.include_usage`）。
- 流式 chunk 的 `choices[].delta` 支持 `content`、`reasoning_content`、`function_call`、`tool_calls`、`audio`（Qwen-Omni）、`refusal`。
- `usage` 在最后一个 chunk 返回，含细粒度 token 分类：`prompt_tokens_details.{text, audio, video, image, cached}`、`completion_tokens_details.{text, reasoning, audio}`、`cache_creation.{ephemeral_5m_input_tokens, ...}`。
- 文档理解仅 `qwen-long` 支持（`fileid://` system message）；PPT 生成仅 `qwen-doc-turbo` 支持。
- 支持模型类别：Qwen 系列（LLM、VL、Coder、Omni、Math）、DeepSeek（阿里云直供、硅基流动直供、快手万擎直供）、Kimi（阿里云直供、月之暗面直供）、GLM（阿里云直供）、MiniMax（阿里云直供、稀宇科技直供）。**三方直供模型仅在中国站华北2（北京）地域可用**，调用前需在百炼控制台开通对应服务。

### Qwen3.7 reasoning 控制

- Qwen3.7 Max 与 Plus 当前模型属于 hybrid thinking，默认开启；Chat 使用非标准布尔字段 `enable_thinking` 显式开启或关闭。
- Chat 的 `thinking_budget` 是 reasoning token 上限，不是离散 effort。Chat 参数表中的 `reasoning_effort` 当前明确用于
  DeepSeek V4 与 GLM 系列，不能据此给 Qwen3.7 推导多档 Chat effort。
- Qwen Responses API 对包含 Qwen3.7 Max/Plus 的支持模型集合声明七个递增取值：`none`、`minimal`、`low`、`medium`、
  `high`、`xhigh`、`max`；其中 `xhigh`、`max` 只在华北2（北京）和新加坡支持。
- Responses reasoning 通过 `type=reasoning` output item 返回；其 `summary` 数组元素为 `type=summary_text` 与 `text`，因此这是
  summary channel，不是 Chat `reasoning_content` 的 plain-text wire。
- 官方资料对同一 Qwen3.7 模型在 Responses 中列出七档，在 Chat 中只提供开关 wire；资料没有说明这是两个不同 checkpoint，
  也没有证明 Chat 能区分七种强度。`thinking_budget` 仍不能作为额外离散档位的依据。

## 证据边界

官方文档说明公开协议面，不证明任一具体 API key、账户、地域或模型当前可用。本文没有执行真实百炼请求，因而不证明实际响应内容、错误分类、配额或长时间 streaming 行为；三方直供模型的可用性与开通状态需在控制台逐一确认。模型列表与计费以百炼控制台为准。
