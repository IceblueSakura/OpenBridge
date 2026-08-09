# 真实端到端测试当前最终结果

## 1. 测试范围

本报告只保留当前 checkout 的最终结果，不记录重试历史或修复过程。

- checkout：`main`，commit `90180d8` 加当前已验证 worktree；服务 registry version：`dev-1`。
- 服务：由当前源码执行 `cargo build --locked` 后启动的 `target\debug\openbridge.exe`，地址为
  `http://127.0.0.1:8080`。
- 最终有效证据时间：2026-08-09 01:21:22 至 14:42:35（UTC+08:00）。
- 下游认证：使用私有 `config/users.toml` 中启用用户的真实 Bearer key；key 未输出、复制或写入结果。
- 路由范围：只验收固定顺序下正常可用的首选路径，不制造故障、不强制 fallback。下游响应不公开实际 Target，因此本报告不把
  fallback、NVIDIA 后备或其他第二候选描述为已验收。
- Models：检查 `GET /v1/models` 与 `GET /openbridge/v1/models` 的状态、集合完整性、重复项和 ID 格式。
- 文字生成：覆盖 15 个模型的 Chat/Responses、JSON/SSE、reasoning 字段省略以及全部公开 reasoning level。
- 扩展能力：覆盖 function tools、tool choice、parallel calls、structured outputs、工具结果续接、严格参数处置、MiMo 图片、
  MiMo 专用音频和 Qwen Embeddings。
- 所有测试只保留状态码、协议终态、语义布尔值、错误分类和媒体字节数；不保存生成文本、reasoning、tool arguments、向量、音频、
  transcript、Provider request ID 或 credential。

## 2. 总体结果

| 验收面 | 通过 | 总单元 | 最终结论 |
|---|---:|---:|---|
| Models 标准/扩展端点 | 2 | 2 | 均为 HTTP 200，ID 集合一致，共 20 个当前可执行模型 |
| 文字 + reasoning | 312 | 312 | 全部返回合法 JSON/SSE 终态 |
| 当前公开的 function tools + 文字 structured outputs | 164 | 164 | 全部符合当前公开能力；含 8 个 strict function-schema 单元 |
| 已移除 generation 能力的本地拒绝 | 44 | 44 | 全部在 egress 前返回 HTTP 400 |
| Function tool result 续接 | 16 | 16 | 全部完成 call/result/final-text 续接 |
| 严格参数处置 | 16 | 16 | 接受项成功；未知和不支持项均在 egress 前返回精确参数错误 |
| MiMo 图片 | 4 | 4 | Chat/Responses × JSON/SSE 全部通过 |
| Qwen Embeddings | 10 | 10 | 默认/合法维度成功；非法维度在本地被拒绝 |
| MiMo 音频基础/扩展能力 | 15 | 15 | ASR、TTS、VoiceClone、VoiceDesign 全部通过 |
| MiMo 音频 structured-output 能力拒绝 | 16 | 16 | 全部在 egress 前返回 HTTP 400 |
| **业务请求单元合计（不含 Models 检查）** | **597** | **597** | **全部符合当前公开契约；没有最终 429/503 或传输错误** |

597 个业务请求单元是能力组合，不代表相同数量的模型或产品功能。预期的 HTTP 400 表示 OpenBridge 正确拒绝未公开、未知或不支持的
能力，因此计为契约通过，不计作生成成功。

## 3. Models 端点

当前静态目录有 21 个 Public Model。私有配置没有激活 `text-embedding-3-small` 的唯一 OpenAI 来源，因此两个 Models 端点均返回
其余 20 个当前可执行模型。

| 检查项 | 最终结果 |
|---|---:|
| `GET /v1/models` | HTTP 200，20 项 |
| `GET /openbridge/v1/models` | HTTP 200，20 项 |
| 两个 list response 的 `object` | 均为 `list` |
| 标准/扩展端点 ID 集合差异 | 0 |
| 相对当前可执行目录缺失或多余 ID | 0 |
| 重复 ID | 0 |
| 不符合 `[A-Za-z0-9][A-Za-z0-9._:-]{0,127}` 的 ID | 0 |

最终可见的 20 个 ID：

`LongCat-2.0`、`deepseek-v4-flash`、`deepseek-v4-pro`、`glm-5.2`、
`gpt-5.3-codex-spark`、`gpt-5.5`、`gpt-5.6-luna`、`gpt-5.6-sol`、`gpt-5.6-terra`、
`kimi-k3`、`mimo-v2.5`、`mimo-v2.5-asr`、`mimo-v2.5-pro`、`mimo-v2.5-tts`、
`mimo-v2.5-tts-voiceclone`、`mimo-v2.5-tts-voicedesign`、`minimax-m3`、`qwen3.7-max`、
`qwen3.7-plus`、`qwen3.7-text-embedding`。

## 4. 文字与 reasoning

每个单元均覆盖一个协议、delivery mode 和 reasoning 配置。字段省略表示使用 Provider 默认行为；显式配置覆盖每个模型公开的全部
level，包括适用模型的 `none`。Chat 使用 `reasoning_effort`，Responses 使用 `reasoning.effort`。

| Model | 单元 | 合法成功 | HTTP/协议/传输错误 |
|---|---:|---:|---:|
| `LongCat-2.0` | 12 | 12 | 0 |
| `deepseek-v4-flash` | 16 | 16 | 0 |
| `deepseek-v4-pro` | 12 | 12 | 0 |
| `glm-5.2` | 12 | 12 | 0 |
| `gpt-5.3-codex-spark` | 20 | 20 | 0 |
| `gpt-5.5` | 24 | 24 | 0 |
| `gpt-5.6-luna` | 28 | 28 | 0 |
| `gpt-5.6-sol` | 28 | 28 | 0 |
| `gpt-5.6-terra` | 28 | 28 | 0 |
| `kimi-k3` | 16 | 16 | 0 |
| `mimo-v2.5` | 20 | 20 | 0 |
| `mimo-v2.5-pro` | 20 | 20 | 0 |
| `minimax-m3` | 12 | 12 | 0 |
| `qwen3.7-max` | 32 | 32 | 0 |
| `qwen3.7-plus` | 32 | 32 | 0 |
| **合计** | **312** | **312** | **0** |

这组结果确认当前首选路径上的基础 Chat/Responses 转换、`stream:true` 直传、ChatGPT 上游强制流式到下游非流式的缓冲转换，以及公开
reasoning level 的请求映射均可完成。reasoning 内容只做存在性检查；opaque continuation 不伪装为可读 reasoning。

## 5. Function tools 与 structured outputs

### 5.1 当前公开能力

正向矩阵只请求 Models 接口当前公开的能力。Function tool 覆盖公开的 tool choice、适用接口的 `parallel_tool_calls`、
Chat/Responses 与 JSON/SSE；structured output 覆盖公开的 `json_object/json_schema`、Chat/Responses 与 JSON/SSE。

| Model | 通过 | 总单元 | 最终公开边界 |
|---|---:|---:|---|
| `LongCat-2.0` | 16 | 16 | 维持当前公开能力 |
| `deepseek-v4-flash` | 4 | 4 | Responses 仅公开 `none/auto` tool choice |
| `gpt-5.5` | 28 | 28 | 维持当前公开能力 |
| `gpt-5.6-luna` | 28 | 28 | 维持当前公开能力 |
| `gpt-5.6-sol` | 28 | 28 | 维持当前公开能力 |
| `gpt-5.6-terra` | 28 | 28 | 维持当前公开能力 |
| `mimo-v2.5` | 8 | 8 | `tool_choice:auto`；Chat/Responses 支持 `json_object` |
| `mimo-v2.5-pro` | 8 | 8 | `tool_choice:auto`；Chat/Responses 支持 `json_object` |
| `minimax-m3` | 8 | 8 | 维持当前公开能力 |
| **公开能力矩阵小计** | **156** | **156** | **全部通过** |
| MiMo strict function schema | 8 | 8 | 两个文字模型的 Chat/Responses × JSON/SSE 全部通过 |
| **合计** | **164** | **164** | **全部通过** |

MiMo 的自然多调用输出仍可被 OpenBridge 保真转发，但官方没有声明 `parallel_tool_calls` 请求参数，因此 Public Model 不把它公开为
可控能力。
MiMo 两款文字模型的 `json_object` 按官方前提使用明确 JSON-only prompt；Chat/Responses × JSON/SSE 共 8/8 返回合法 JSON、完整终态，
且合成字段和值与请求一致。该结果不外推为 `json_schema` 字段约束。

### 5.2 未公开能力的拒绝边界

| 场景 | 通过/总数 | 最终结果 |
|---|---:|---|
| DeepSeek Flash Responses `required/named` × JSON/SSE | 4/4 | HTTP 400 `unsupported_model_capability` |
| MiMo V2.5 `none/required/named` × Chat/Responses × JSON/SSE | 12/12 | HTTP 400 `unsupported_model_capability` |
| MiMo V2.5 Pro `none/required/named` × Chat/Responses × JSON/SSE | 12/12 | HTTP 400 `unsupported_model_capability` |
| MiMo 两模型 `parallel_tool_calls:true` × Chat/Responses × JSON/SSE | 8/8 | HTTP 400 `unsupported_model_capability` |
| MiMo 两模型 `json_schema` × Chat/Responses × JSON/SSE | 8/8 | HTTP 400 `unsupported_model_capability` |
| **合计** | **44/44** | **全部在本地能力门禁拒绝** |

## 6. Function tool result 续接

16 个公开 function-tool 接口均完成一次非流式 call、stateless tool result 和最终文本回复：

- `LongCat-2.0` Responses 接受标准的 `{role, content}` message shorthand；后备 Responses-to-Chat Bridge 能保留原始 call identity，
  并完成最终回复。
- `deepseek-v4-flash` Responses 使用当前允许的初始 `tool_choice:auto`，续接时使用 `tool_choice:none`；包含 reasoning history 的
  tool-result 请求完成最终回复且没有重复调用工具。
- GPT-5.5、GPT-5.6 Luna/Sol/Terra 的 Chat/Responses、LongCat Chat 和 MiniMax Responses 均完成相同续接契约。MiMo V2.5/Pro
  的四个接口以首轮 `tool_choice:auto` 产生合法 call，续接轮不再提交 tools，最终均返回 `DONE` 且没有重复调用。

最终结果为 16/16，没有 HTTP、协议或传输错误。

## 7. 严格参数处置

“通过”表示实际状态、错误 code、精确 `param` 和 transport 边界均与固定契约一致：

| 场景 | 协议与 delivery | 通过/总数 | 最终结果 |
|---|---|---:|---|
| Kimi K3 `temperature:0.2` | Chat/Responses × JSON/SSE | 4/4 | HTTP 200，合法 JSON/SSE 终态，每项 1 次 Provider attempt |
| 未知 `future_parameter` | Chat/Responses JSON | 2/2 | HTTP 400 `unknown_parameter`，精确 `param`，0 次 Provider attempt |
| Kimi K3 `n/logprobs/top_logprobs` | Chat/Responses JSON | 6/6 | HTTP 400 `unsupported_model_capability`，精确 `param`，0 次 Provider attempt |
| GPT-5.6 Luna `seed` | Chat/Responses JSON | 2/2 | HTTP 200，合法 JSON 终态，每项 1 次 Provider attempt |
| GPT-5.6 Luna `include_reasoning` | Chat/Responses JSON | 2/2 | HTTP 400 `unsupported_model_capability`，精确 `param`，0 次 Provider attempt |

Kimi 的四个成功单元确认选中 Chat API 的类型化忽略规则同时适用于 Chat Native 与 Responses-to-Chat Bridge，并覆盖下游
`stream:true/false`。16 个单元均一次完成，没有最终 429/503、协议或传输错误。

## 8. 图片、Embeddings 与音频

### 8.1 MiMo 图片

`mimo-v2.5` 使用内联 PNG，Chat/Responses × JSON/SSE 共 4/4 通过；语义检查确认模型识别出合成图中的红、蓝区域。图片正文未保存。

### 8.2 Qwen Embeddings

`qwen3.7-text-embedding` 共 10/10 符合当前契约：

- 默认请求和 `256/512/768/1024/1536/2048/2560` 七个显式合法维度均返回 HTTP 200，向量长度与请求一致；默认维度为
  `1024`。
- 百炼成功体的顶层 `id` 可被严格解析器接受，但不会作为 OpenBridge 下游 Embeddings 字段输出。
- `64/128` 均在本地返回 HTTP 400 `unsupported_model_capability`，精确 `param` 为 `dimensions`。

### 8.3 MiMo 音频

基础与扩展任务共 15/15 通过：

- TTS：JSON/WAV 与 SSE/PCM16；
- ASR：JSON/SSE、`auto/zh` language、data URL 与纯 Base64 source；
- VoiceClone：JSON/SSE，reference WAV 只在进程内存中使用；
- VoiceDesign：JSON/SSE 与 `optimize_text_preview`。

`mimo-v2.5-asr`、`mimo-v2.5-tts`、`mimo-v2.5-tts-voiceclone`、`mimo-v2.5-tts-voicedesign` 的 Models 接口均不再公开
structured outputs。四个模型的 `json_object/json_schema` × JSON/SSE 共 16/16 在本地返回 HTTP 400
`unsupported_model_capability`；确定性 forwarding 契约同时确认这些请求不会产生 Provider egress。

## 9. 剩余验收边界

本报告原列出的 Qwen Embeddings、LongCat Responses shorthand、MiMo 能力过度声明以及 DeepSeek Flash Responses tool-choice 四类问题
均已关闭。仍未形成真实验收证据的范围如下：

- 当前 OpenRouter 注册表没有 GPT target；`gpt-5.6-sol` 的第二 source 是 OpenAI，GPT-5.5/Luna/Terra 只有 ChatGPT source。因此
  “ChatGPT 优先、不可用时回落 OpenRouter GPT-5.6，尤其 Luna”的既定要求尚未实现，也无法验收。
- 当前只验收正常首选路径，没有强制测试 `deepseek-v4-flash` 的 OpenRouter fallback、`minimax-m3` 的 NVIDIA Chat fallback，或
  `gpt-5.6-sol` 的 OpenAI fallback。
- `text-embedding-3-small` 因当前 OpenAI credential pool 未激活而不在 Models 列表中，未执行真实请求。
- 未运行外部 OpenAI SDK、负载、并发稳定性、长期运行或生产环境验收；测试未依赖 Hermes。
