# 2026-08-09 文字 Generation `none/high` 真实 Provider 矩阵

本文是一次不可变验证记录，只描述 2026-08-09 实际执行的请求和结果，不代表当前 checkout 已重新验收。

## 执行边界

- checkout：`main@ed515cc`，叠加只修改文档、未修改运行时代码的 worktree；registry version 为 `dev-1`。
- 服务：执行 `cargo build --locked --bin openbridge` 后启动本地 `target\debug\openbridge.exe`，监听
  `http://127.0.0.1:8080`。
- 时间：2026-08-09 23:52:24 至 23:56:49（UTC+08:00）；定向 reasoning 复测于 23:59:33 完成。
- 当时扩展 Models 返回 21 个可执行 Public Model；其中 16 个文字 Generation 模型进入矩阵。
- 每个模型执行 `reasoning=none/high × Chat/Responses × stream=false/true`。请求只携带模型、同一条无状态纯文本
  prompt、对应协议的标准 reasoning 字段和 `stream`。
- JSON 要求合法响应、完整协议终态和非空文字；SSE 还要求 Chat `[DONE]` 或 Responses
  `response.completed`。
- 基础 prompt 只要求回复 `OK`。15 个成功但没有可观察 reasoning 的 GPT `high` 单元另以同一条多步整数求解
  prompt 复测，避免把任务过于简单误判成转换缺失。
- 只保存状态码、终态、语义布尔值、reasoning token 计数和耗时；未保存正文、Provider request ID、账户或 credential。
- 只验收正常首选路径，没有制造故障或强制 fallback。

## 总体结果

| 检查面 | 结果 | 当次结论 |
|---|---:|---|
| 基础矩阵 | 128 | 16 个模型各 8 个单元全部执行 |
| HTTP 200 | 124 | 成功单元均有非空文字和完整 JSON/SSE 终态 |
| HTTP 400 | 4 | 全部为 `gpt-5.3-codex-spark` 的 `none`，错误为 `unsupported_value`，参数为 `reasoning.effort` |
| `none` | 60 个 200 + 4 个 400 | 60 个成功体均无可观察 reasoning |
| `high` | 64 个 200 | 全部有文字和完整终态；简单 prompt 中 49 个有 reasoning 证据 |
| `high` 定向复测 | 13/15 出现 reasoning | Spark Chat JSON/SSE 仍无可观察 summary |
| 最终 429/5xx、传输或协议终态错误 | 0 | 未观察到这些最终错误 |

## 请求结果矩阵

缩写：`C` 为 Chat，`R` 为 Responses；`OK` 表示 HTTP 200、非空文字和正确终态；`R-` 表示没有可观察
reasoning，`R+` 表示存在 reasoning item、可读 reasoning 或正 reasoning token 计数。`*` 表示测试时扩展 Models 没有
列出 `none`，但实际请求成功；`‡` 表示定向复测后出现 reasoning；`†` 表示定向复测后仍没有可观察 reasoning。

| Model | C none JSON | C none SSE | R none JSON | R none SSE | C high JSON | C high SSE | R high JSON | R high SSE |
|---|---|---|---|---|---|---|---|---|
| `deepseek-v4-flash` | OK R-* | OK R-* | OK R-* | OK R-* | OK R+ | OK R+ | OK R+ | OK R+ |
| `deepseek-v4-pro` | OK R-* | OK R-* | OK R-* | OK R-* | OK R+ | OK R+ | OK R+ | OK R+ |
| `glm-5.2` | OK R-* | OK R-* | OK R-* | OK R-* | OK R+ | OK R+ | OK R+ | OK R+ |
| `gpt-5.3-codex-spark` | 400 `unsupported_value` | 400 `unsupported_value` | 400 `unsupported_value` | 400 `unsupported_value` | OK R?† | OK R?† | OK R+ | OK R+ |
| `gpt-5.5` | OK R- | OK R- | OK R- | OK R- | OK R+ | OK R+‡ | OK R+ | OK R+ |
| `gpt-5.6-luna` | OK R- | OK R- | OK R- | OK R- | OK R+‡ | OK R+‡ | OK R+‡ | OK R+‡ |
| `gpt-5.6-sol` | OK R- | OK R- | OK R- | OK R- | OK R+‡ | OK R+‡ | OK R+‡ | OK R+‡ |
| `gpt-5.6-terra` | OK R- | OK R- | OK R- | OK R- | OK R+‡ | OK R+‡ | OK R+‡ | OK R+‡ |
| `kimi-k3` | OK R-* | OK R-* | OK R-* | OK R-* | OK R+ | OK R+ | OK R+ | OK R+ |
| `LongCat-2.0` | OK R- | OK R- | OK R- | OK R- | OK R+ | OK R+ | OK R+ | OK R+ |
| `mimo-v2.5` | OK R- | OK R- | OK R- | OK R- | OK R+ | OK R+ | OK R+ | OK R+ |
| `mimo-v2.5-pro` | OK R- | OK R- | OK R- | OK R- | OK R+ | OK R+ | OK R+ | OK R+ |
| `minimax-m3` | OK R- | OK R- | OK R- | OK R- | OK R+ | OK R+ | OK R+ | OK R+ |
| `qwen3.7-max` | OK R- | OK R- | OK R- | OK R- | OK R+ | OK R+ | OK R+ | OK R+ |
| `qwen3.7-plus` | OK R- | OK R- | OK R- | OK R- | OK R+ | OK R+ | OK R+ | OK R+ |
| `qwen3.8-max` | OK R- | OK R- | OK R- | OK R- | OK R+ | OK R+ | OK R+ | OK R+ |

Spark Responses `high` 的两个基础单元分别返回 reasoning item 和正 reasoning token 计数；Spark Chat JSON/SSE 在基础
prompt 和多步 prompt 下都只有完整文字，没有 summary、reasoning item 或 reasoning token 证据。这是可见性观察，不是
HTTP、文本或终态失败。

## 当时未进入矩阵的模型

| Public Model | 当次状态 | 排除原因 |
|---|---|---|
| `mimo-v2.5-asr` | Models 可见 | Chat-only 专用 ASR |
| `mimo-v2.5-tts` | Models 可见 | Chat-only 专用 TTS |
| `mimo-v2.5-tts-voiceclone` | Models 可见 | Chat-only 专用语音克隆 |
| `mimo-v2.5-tts-voicedesign` | Models 可见 | Chat-only 专用音色设计 |
| `qwen3.7-text-embedding` | Models 可见 | Embeddings-only |
| `text-embedding-3-small` | 静态注册、Models 不可见 | 当次没有可执行运行时 Route，且不是文字 Generation |

## 证据边界

- 表中的 `*` 只描述当时 Models 投影与真实请求的差异；后续 canonical profile 变化不改写本记录。
- Spark `none` 的四个上游 400 与 Chat `high` reasoning 可见性只描述该 checkout 和 payload。
- 未执行其他 reasoning level、tools、structured outputs、图片、音频、Embeddings、外部 SDK、强制 fallback、负载、
  长期运行或生产验收。

当前实现解释见 [Provider 状态目录](../../providers/README.md)和
[Models/能力预检状态](../../features/models-api-and-capability-preflight.md)。
