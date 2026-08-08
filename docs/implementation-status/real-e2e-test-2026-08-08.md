# 2026-08-08 真实端到端测试最终结果

## 1. 测试范围

本报告记录当前 checkout、当前私有配置和真实 Provider 在 2026-08-08 的最终端到端结果。

- checkout：`main`；服务 registry version：`dev-1`。
- 服务：`target\debug\openbridge.exe`，地址：`http://127.0.0.1:8080`。
- 下游认证：使用私有 `config/users.toml` 中启用用户的真实 Bearer key；key 未输出、未复制到文档或日志。
- 请求文本：`Reply with exactly OK.`。
- Models：检查 `GET /v1/models` 和 `GET /openbridge/v1/models`。
- Generation：对 18 个声明 text input 的 Public Model 执行 Chat/Responses × stream off/on × reasoning 字段省略/high，
  共 144 个组合。
- `mimo-v2.5-asr` 只有 audio input，不进入文字输入矩阵。
- 三个 MiMo TTS 模型虽然声明 text input，但需要 task-specific audio output 参数；本报告仍保留通用文字生成矩阵的能力预检结果。
- 每个矩阵单元只保留一个最终结果；测试客户端不执行自动重试。

矩阵列含义：

- `C` = Chat Completions，`R` = Responses。
- `N` = 省略 reasoning 字段，表示使用 Provider 默认值，不等于显式关闭 reasoning。
- `H` = 显式 high；Chat 使用 `reasoning_effort: "high"`，Responses 使用
  `reasoning: {"effort":"high"}`。
- `0` = `stream: false`，`1` = `stream: true`。

GPT/ChatGPT 前提：当前 ChatGPT source 只接受 streaming Responses 上游请求，因此由该 source 执行的请求不支持
`stream: false`。当前私有配置未激活 OpenAI API-key pool，`gpt-5.6-sol` 也只使用 ChatGPT source。

## 2. 总体结果

| 检查项 | 最终结果 |
|---|---:|
| Models 标准端点 | HTTP 200，19 项 |
| Models 扩展端点 | HTTP 200，19 项 |
| Generation 矩阵单元 | 144 |
| 2xx 成功 | 64 |
| 非流式可解析 JSON | 27 |
| 流式 SSE 成功 | 37 |
| HTTP 错误 | 80 |
| 传输错误 | 0 |
| `unsupported_model_capability` | 62 |
| `unsupported_request` | 14 |
| `invalid_upstream_response` | 4 |
| 429 / 503 | 0 / 0 |

37 个 SSE 成功中，36 个保存了显式 terminal 判定；`minimax-m3 C-N/1` 保存了 HTTP 200、SSE content type
和完整响应 body，但没有单独保存 terminal 判定。

测试结束时 `GET /healthz` 为 HTTP 200，服务保持运行正常。

## 3. Models 端点最终结果

当前静态目录有 20 个 Public Model。`text-embedding-3-small` 只绑定未激活的 OpenAI API-key pool，因此当前私有配置下
两个 Models 端点均返回其余 19 个可执行 generation model。

| 检查项 | 最终结果 |
|---|---:|
| `GET /v1/models` | HTTP 200，19 项 |
| `GET /openbridge/v1/models` | HTTP 200，19 项 |
| 标准/扩展端点 ID 集合差异 | 0 |
| 当前可见目录缺失或多余 ID | 0 |
| 不符合 `[A-Za-z0-9][A-Za-z0-9._:-]{0,127}` 的 ID | 0 |
| 两个 list response 的 `object` | 均为 `list` |

最终可见的 19 个 ID：

`LongCat-2.0`、`chatgpt-gpt-5.3-codex-spark`、`chatgpt-gpt-5.5`、`deepseek-v4-flash`、
`deepseek-v4-pro`、`glm-5.2`、`gpt-5.6-luna`、`gpt-5.6-sol`、`gpt-5.6-terra`、`kimi-k3`、
`mimo-v2.5`、`mimo-v2.5-asr`、`mimo-v2.5-pro`、`mimo-v2.5-tts`、
`mimo-v2.5-tts-voiceclone`、`mimo-v2.5-tts-voicedesign`、`minimax-m3`、`qwen3.7-max`、
`qwen3.7-plus`。

扩展 Models 对 `glm-5.2`、`kimi-k3`、`qwen3.7-max`、`qwen3.7-plus` 的 Chat/Responses reasoning
输出均声明为 `plain_text`。GLM 与 Kimi 的公开 levels 包含 `high`；两个 Qwen3.7 模型的 levels 为空，因此显式
`high` 不属于公开能力。

## 4. Chat/Responses 最终矩阵

| Model | C-N/0 | C-N/1 | C-H/0 | C-H/1 | R-N/0 | R-N/1 | R-H/0 | R-H/1 |
|---|---|---|---|---|---|---|---|---|
| `chatgpt-gpt-5.3-codex-spark` | 400/USR | 502/IUR | 400/UMC | 400/UMC | 400/USR | 200/SSE | 400/USR | 200/SSE |
| `chatgpt-gpt-5.5` | 400/USR | 502/IUR | 400/UMC | 400/UMC | 400/USR | 200/SSE | 400/USR | 200/SSE |
| `deepseek-v4-flash` | 200/JSON | 200/SSE | 200/JSON | 200/SSE | 200/JSON | 200/SSE | 200/JSON | 200/SSE |
| `deepseek-v4-pro` | 200/JSON | 200/SSE | 200/JSON | 200/SSE | 200/JSON | 200/SSE | 400/UMC | 400/UMC |
| `glm-5.2` | 200/JSON | 200/SSE | 200/JSON | 200/SSE | 200/JSON | 200/SSE | 200/JSON | 200/SSE |
| `gpt-5.6-luna` | 400/USR | 502/IUR | 400/UMC | 400/UMC | 400/USR | 200/SSE | 400/USR | 200/SSE |
| `gpt-5.6-sol` | 400/UMC | 400/UMC | 400/UMC | 400/UMC | 400/USR | 200/SSE | 400/USR | 200/SSE |
| `gpt-5.6-terra` | 400/USR | 502/IUR | 400/UMC | 400/UMC | 400/USR | 200/SSE | 400/USR | 200/SSE |
| `kimi-k3` | 200/JSON | 200/SSE | 200/JSON | 200/SSE | 200/JSON | 200/SSE | 200/JSON | 200/SSE |
| `LongCat-2.0` | 200/JSON | 200/SSE | 400/UMC | 400/UMC | 200/JSON | 200/SSE | 400/UMC | 400/UMC |
| `mimo-v2.5` | 200/JSON | 200/SSE | 400/UMC | 400/UMC | 200/JSON | 200/SSE | 400/UMC | 400/UMC |
| `mimo-v2.5-pro` | 200/JSON | 200/SSE | 400/UMC | 400/UMC | 200/JSON | 200/SSE | 400/UMC | 400/UMC |
| `mimo-v2.5-tts` | 400/UMC | 400/UMC | 400/UMC | 400/UMC | 400/UMC | 400/UMC | 400/UMC | 400/UMC |
| `mimo-v2.5-tts-voiceclone` | 400/UMC | 400/UMC | 400/UMC | 400/UMC | 400/UMC | 400/UMC | 400/UMC | 400/UMC |
| `mimo-v2.5-tts-voicedesign` | 400/UMC | 400/UMC | 400/UMC | 400/UMC | 400/UMC | 400/UMC | 400/UMC | 400/UMC |
| `minimax-m3` | 200/JSON | 200/SSE | 400/UMC | 400/UMC | 200/JSON | 200/SSE | 400/UMC | 400/UMC |
| `qwen3.7-max` | 200/JSON | 200/SSE | 400/UMC | 400/UMC | 200/JSON | 200/SSE | 400/UMC | 400/UMC |
| `qwen3.7-plus` | 200/JSON | 200/SSE | 400/UMC | 400/UMC | 200/JSON | 200/SSE | 400/UMC | 400/UMC |

符号：

- `200/JSON`：HTTP 200，非流式 JSON 可解析。
- `200/SSE`：HTTP 200，流式 SSE 成功。
- `400/UMC`：`code=unsupported_model_capability`。
- `400/USR`：`code=unsupported_request`。
- `502/IUR`：`code=invalid_upstream_response`。

## 5. 按模型汇总

| Model | 组合数 | 2xx 成功 | HTTP 错误 | 传输错误 |
|---|---:|---:|---:|---:|
| `chatgpt-gpt-5.3-codex-spark` | 8 | 2 | 6 | 0 |
| `chatgpt-gpt-5.5` | 8 | 2 | 6 | 0 |
| `deepseek-v4-flash` | 8 | 8 | 0 | 0 |
| `deepseek-v4-pro` | 8 | 6 | 2 | 0 |
| `glm-5.2` | 8 | 8 | 0 | 0 |
| `gpt-5.6-luna` | 8 | 2 | 6 | 0 |
| `gpt-5.6-sol` | 8 | 2 | 6 | 0 |
| `gpt-5.6-terra` | 8 | 2 | 6 | 0 |
| `kimi-k3` | 8 | 8 | 0 | 0 |
| `LongCat-2.0` | 8 | 4 | 4 | 0 |
| `mimo-v2.5` | 8 | 4 | 4 | 0 |
| `mimo-v2.5-pro` | 8 | 4 | 4 | 0 |
| `mimo-v2.5-tts` | 8 | 0 | 8 | 0 |
| `mimo-v2.5-tts-voiceclone` | 8 | 0 | 8 | 0 |
| `mimo-v2.5-tts-voicedesign` | 8 | 0 | 8 | 0 |
| `minimax-m3` | 8 | 4 | 4 | 0 |
| `qwen3.7-max` | 8 | 4 | 4 | 0 |
| `qwen3.7-plus` | 8 | 4 | 4 | 0 |
| **合计** | **144** | **64** | **80** | **0** |

## 6. Reasoning 关闭能力最终结果

Reasoning 关闭能力使用 Native Chat JSON/SSE 直接验证，并以 Hermes 的 off 行为作为 wire-shape 对照。

- Hermes `0.20.0` 将 `reasoning_effort: none`、`false` 或 `disabled` 解析为关闭状态；省略配置表示未设置。
- Hermes custom OpenAI-compatible profile 的 off 请求同时包含 `reasoning_effort: "none"` 与 `think: false`。

| Provider | Model | 标准 `reasoning_effort: none`（JSON/SSE） | Hermes off wire shape（JSON/SSE） |
|---|---|---|---|
| Bailian | `glm-5.2` | 200/200，reasoning 内容为空 | 200/200，reasoning 内容为空 |
| Kimi CN | `kimi-k3` | 200/200，reasoning 内容为空 | 200/200，reasoning 内容为空 |
| Bailian | `qwen3.7-max` | 200/200，reasoning 内容为空 | 200/200，reasoning 内容为空 |
| Bailian | `qwen3.7-plus` | 200/200，reasoning 内容为空 | 200/200，reasoning 内容为空 |

最终结论：这四个当前真实部署都能关闭 reasoning。矩阵中的 `N` 是字段省略，Provider 可以使用默认 reasoning 行为，
不能把 `N` 解释为 off。该结论只适用于本报告记录的 endpoint、账号和时间点。

## 7. 最终错误归类

| 错误 | 单元数 | Provider / 模型 | 最终归类 |
|---|---:|---|---|
| `400/USR` | 14 | ChatGPT：`chatgpt-gpt-5.3-codex-spark`、`chatgpt-gpt-5.5`、`gpt-5.6-luna`、`gpt-5.6-terra`；当前仅有 ChatGPT source 的 `gpt-5.6-sol` | 当前 source 不支持 `stream:false` |
| `502/IUR` | 4 | ChatGPT：`chatgpt-gpt-5.3-codex-spark`、`chatgpt-gpt-5.5`、`gpt-5.6-luna`、`gpt-5.6-terra` | Chat streaming + reasoning 字段省略返回 `invalid_upstream_response` |
| `400/UMC` | 12 | ChatGPT/GPT models | Chat interface 或 reasoning 组合不在公开能力内 |
| `400/UMC` | 2 | DeepSeek：`deepseek-v4-pro` | Responses high 不在公开能力内 |
| `400/UMC` | 4 | LongCat：`LongCat-2.0` | Chat/Responses high 不在公开能力内 |
| `400/UMC` | 32 | Xiaomi MiMo：`mimo-v2.5`、`mimo-v2.5-pro` 和三个 TTS models | high 或通用文字生成形状不在对应公开能力内 |
| `400/UMC` | 4 | NVIDIA：`minimax-m3` | Chat/Responses high 不在公开能力内 |
| `400/UMC` | 8 | Bailian：`qwen3.7-max`、`qwen3.7-plus` | 模型未声明 high reasoning level |

`400/UMC` 在公共能力预检阶段产生，不形成对应 Provider egress。最终没有非 GPT Provider HTTP/transport
运行错误；Bailian Qwen3.7 的 8 个错误均为本地 high capability 拒绝。

## 8. 证据边界

- 真实 Provider 结果只证明当前私有配置、账号、endpoint、请求形状和执行时间点。
- Hermes off 结论来自本机 Hermes source 所定义 wire shape 的直接重放；未执行 Hermes runtime 用户会话。
- 未执行外部 OpenAI SDK、负载测试、长期运行、并发稳定性或生产环境验收。
- 没有保存生成文本、完整上游响应、Provider request ID、credential 或其他敏感数据。
