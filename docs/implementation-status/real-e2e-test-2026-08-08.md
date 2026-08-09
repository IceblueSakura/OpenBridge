# 真实端到端测试当前最终结果

## 1. 测试范围

本报告只保留当前 checkout 的最终结果，不记录重试历史或修复过程。

- checkout：`main`，commit `f7618de` 加当前已验证 worktree；服务 registry version：`dev-1`。
- 服务：由当前源码执行 `cargo build --locked` 后启动的 `target\debug\openbridge.exe`，地址为
  `http://127.0.0.1:8080`。
- 最终有效证据时间：2026-08-09 01:21:22 至 13:09:46（UTC+08:00）。
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

| 验收面 | 最终通过 | 总单元 | 最终结论 |
|---|---:|---:|---|
| Models 标准/扩展端点 | 2 | 2 | 均为 HTTP 200，ID 集合一致，共 20 个当前可执行模型 |
| 文字 + reasoning | 312 | 312 | 全部返回合法 JSON/SSE 终态 |
| Function tools + 文字 structured outputs | 184 | 200 | 8 个 HTTP 400，8 个 HTTP 200 语义违约 |
| Function tool result 续接 | 14 | 16 | LongCat Responses 与 DeepSeek Flash Responses 各失败 1 个 |
| 严格参数处置 | 16 | 16 | 接受项成功；未知和不支持项均在 egress 前返回精确参数错误 |
| MiMo 图片 | 4 | 4 | Chat/Responses × JSON/SSE 全部通过 |
| Qwen Embeddings | 0 | 10 | 2 个维度被上游拒绝，8 个合法请求被本地响应校验拒绝 |
| MiMo 音频基础/扩展能力 | 15 | 15 | ASR、TTS、VoiceClone、VoiceDesign 全部通过 |
| MiMo 音频 + structured outputs | 0 | 16 | ASR 为 HTTP 500；三个 TTS 类模型未产生结构化文本 |
| **业务请求单元合计（不含 Models 检查）** | **545** | **589** | 无传输错误；最终结果中没有 HTTP 429/503 |

这里的 589 个业务请求单元是不同能力组合，不代表 589 个互不重叠的产品缺陷。多个失败单元可能由同一个能力目录或转换缺陷造成。
严格参数处置中的预期 HTTP 400 记为契约通过，不计作生成成功或产品缺陷。

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

只对 Models 接口明确公开相应能力的文字模型发起请求。Function tool 矩阵覆盖 `none/auto/required/named`、适用接口的
`parallel_tool_calls`、Chat/Responses 与 JSON/SSE；structured output 覆盖 `json_object/json_schema`、Chat/Responses 与 JSON/SSE。

| Model | 通过 | 总单元 | HTTP 错误 | HTTP 200 语义违约 |
|---|---:|---:|---:|---:|
| `LongCat-2.0` | 16 | 16 | 0 | 0 |
| `deepseek-v4-flash` | 4 | 8 | 4 | 0 |
| `gpt-5.5` | 28 | 28 | 0 | 0 |
| `gpt-5.6-luna` | 28 | 28 | 0 | 0 |
| `gpt-5.6-sol` | 28 | 28 | 0 | 0 |
| `gpt-5.6-terra` | 28 | 28 | 0 | 0 |
| `mimo-v2.5` | 20 | 28 | 2 | 6 |
| `mimo-v2.5-pro` | 24 | 28 | 2 | 2 |
| `minimax-m3` | 8 | 8 | 0 | 0 |
| **合计** | **184** | **200** | **8** | **8** |

最终失败边界：

- `deepseek-v4-flash` Responses 的 `required` 与 named tool choice 在 JSON/SSE 下均返回 HTTP 400
  `invalid_request_error`：默认 thinking mode 不支持这两个 choice；当前 Models 却公开了它们。
- `mimo-v2.5` 的 `tool_choice:none` 在部分 Chat/Responses JSON/SSE 请求中仍生成 `report_result` tool call；相同请求有时遵守、
  有时忽略，属于首选 MiMo 上游行为不稳定。
- `mimo-v2.5` 的 Chat `json_object/json_schema` 和 Responses `json_object` 未稳定产生符合声明的 JSON；Responses
  `json_schema` 返回 HTTP 400 `responses_feature_not_supported`。
- `mimo-v2.5-pro` 的 Responses `json_schema` 在 JSON/SSE 下均返回同一 HTTP 400；其余本轮 structured-output 单元通过。

## 6. Function tool result 续接

16 个公开 function-tool 接口均执行一次非流式 required call，再使用原始 call identity 发送 stateless tool result，并明确要求最终回复
`DONE` 且不得再次调用工具。最终 14/16 通过：

- `LongCat-2.0` Responses 初始 call 合法，使用标准 role/content message shorthand 的后续请求被 HTTP 400
  `unsupported_model_capability` 拒绝。仅给首个 user message 补上冗余 `type:"message"` 后，同一续接成功；当前
  Responses-to-Chat fallback converter 没有接受该合法 shorthand，而且一个不可转换的后备 Bridge 会使整个固定 RoutePlan 失败。
- `deepseek-v4-flash` Responses 在初始 `tool_choice:required` 阶段即返回 HTTP 400，原因与上一节的 thinking/tool-choice 冲突相同。
- `gpt-5.5`、GPT-5.6 Luna/Sol/Terra 的 Chat/Responses、`mimo-v2.5`、`mimo-v2.5-pro`、LongCat Chat 和
  MiniMax Responses 均完成 call/result/final-text 续接。

## 7. 严格参数处置

当前最终矩阵只保留严格策略生效后的 16 个真实请求单元。这里的“通过”表示实际状态、错误 code、精确 `param` 和 transport 边界均与
固定契约一致：

| 场景 | 协议与 delivery | 通过/总数 | 最终结果 |
|---|---|---:|---|
| Kimi K3 `temperature:0.2` | Chat/Responses × JSON/SSE | 4/4 | HTTP 200，合法 JSON/SSE 终态，每项 1 次 Provider attempt |
| 未知 `future_parameter` | Chat/Responses JSON | 2/2 | HTTP 400 `unknown_parameter`，精确 `param`，0 次 Provider attempt |
| Kimi K3 `n/logprobs/top_logprobs` | Chat/Responses JSON | 6/6 | HTTP 400 `unsupported_model_capability`，精确 `param`，0 次 Provider attempt |
| GPT-5.6 Luna `seed` | Chat/Responses JSON | 2/2 | HTTP 200，合法 JSON 终态，每项 1 次 Provider attempt |
| GPT-5.6 Luna `include_reasoning` | Chat/Responses JSON | 2/2 | HTTP 400 `unsupported_model_capability`，精确 `param`，0 次 Provider attempt |

Kimi 的四个成功单元确认选中 Chat API 的类型化忽略规则同时适用于 Chat Native 与 Responses-to-Chat Bridge，并覆盖下游
`stream:true/false`。GPT 对照按约定在非 GPT 验收后执行。16 个单元均一次完成，没有最终 429/503、协议或传输错误。测试只保存模型、
协议、参数名、状态、终态和 attempt 数，不保存请求正文、生成正文、reasoning、logprobs、credential 或 Provider request ID。

## 8. 图片、Embeddings 与音频

### 8.1 MiMo 图片

`mimo-v2.5` 使用内联 PNG，Chat/Responses × JSON/SSE 共 4/4 通过；语义检查确认模型识别出合成图中的红、蓝区域。图片正文未保存。

### 8.2 Qwen Embeddings

`qwen3.7-text-embedding` 共 0/10 通过：

- string、string array、显式 float，以及当前目录中的合法维度请求均到达百炼并获得结构正确的 HTTP 200 upstream response，但
  OpenBridge 返回 HTTP 502 `invalid_upstream_response`；直接检查确认上游成功体含标准顶层 `id`，而本地
  `EmbeddingResponseBody` 使用 `deny_unknown_fields` 且未声明 `id`。
- 当前公开维度 `64/128` 被百炼以 HTTP 400 拒绝。
- 百炼实际接受的维度集合为 `256/512/768/1024/1536/2048/2560`；当前目录错误加入 `64/128`，并遗漏
  `1536/2048`。默认维度 `1024` 正确。

因此该 Public Model 当前不可用，同时存在响应解析和维度目录两个独立缺陷。

### 8.3 MiMo 音频

基础与扩展任务共 15/15 通过：

- TTS：JSON/WAV 与 SSE/PCM16；
- ASR：JSON/SSE、`auto/zh` language、data URL 与纯 Base64 source；
- VoiceClone：JSON/SSE，reference WAV 只在进程内存中使用；
- VoiceDesign：JSON/SSE 与 `optimize_text_preview`。

音频与 structured outputs 的交叉矩阵为 0/16：

- `mimo-v2.5-asr` 的 `json_object/json_schema` × JSON/SSE 均返回上游 HTTP 500；
- TTS、VoiceDesign、VoiceClone 的 12 个组合均返回 HTTP 200 且音频有效，但文本通道为空或不符合所请求 JSON 格式。

四个音频 Public Model 当前无条件公开 `json_object/json_schema`，而固定契约不提供“与 audio task 不可组合”的条件 profile，因此这是
能力目录过度声明。

## 9. 当前未解决问题与验收边界

确认的实现或固定契约问题：

1. 百炼 `qwen3.7-text-embedding` 成功体因顶层 `id` 被本地严格解析器拒绝，且维度目录错误。
2. LongCat Responses tool-result 续接会因 Responses message shorthand 无法被后备 Responses-to-Chat Bridge 表示而整体失败。
3. MiMo V2.5/Pro 的 tool-choice/structured-output 契约高于真实首选上游行为；四个专用音频模型还错误公开 structured outputs。
4. DeepSeek V4 Flash Responses 在默认 thinking 下不能执行已公开的 required/named tool choice。

尚未形成真实验收证据：

- 当前 OpenRouter 注册表没有 GPT target；`gpt-5.6-sol` 的第二 source 是 OpenAI，GPT-5.5/Luna/Terra 只有 ChatGPT source。因此“ChatGPT
  优先、不可用时回落 OpenRouter GPT-5.6，尤其 Luna”的既定要求尚未实现，也无法验收。
- 本轮按要求只关注首选路径，没有强制测试 `deepseek-v4-flash` 的 OpenRouter fallback、`minimax-m3` 的 NVIDIA Chat fallback，或
  `gpt-5.6-sol` 的 OpenAI fallback。
- `text-embedding-3-small` 因当前 OpenAI credential pool 未激活而不在 Models 列表中，未执行真实请求。
- 未运行外部 OpenAI SDK、负载、并发稳定性、长期运行或生产环境验收；测试未依赖 Hermes。
