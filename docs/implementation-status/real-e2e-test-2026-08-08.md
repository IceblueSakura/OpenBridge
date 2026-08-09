# 最新文字模型 E2E 测试结果

本文只保留当前最新的 `none/high × Chat/Responses × JSON/SSE` E2E 证据：2026-08-09 的 16 模型基础矩阵、
后续修补状态，以及 2026-08-10 新接入 `qwen3.6-27b` 后执行的 8 单元补测。

## 1. 测试边界

- checkout：`main`，commit `ed515cc` 加未修改运行时代码的文档 worktree；服务 registry version：`dev-1`。
- 服务：对当前 checkout 执行 `cargo build --locked --bin openbridge` 后启动 `target\debug\openbridge.exe`，监听
  `http://127.0.0.1:8080`。
- Models 快照：`GET /openbridge/v1/models` 返回 21 个当前可执行 Public Model；其中 16 个同时提供文本 Chat/Responses，进入矩阵；
  其余 5 个按任务和 interface 单列，不发送本矩阵请求。
- 基础矩阵时间：2026-08-09 23:52:24 至 23:56:49（UTC+08:00）；16 个模型各执行
  `reasoning=none/high × Chat/Responses × stream=false/true`，共 128 个请求单元。
- 请求只携带模型、同一条无状态纯文本 prompt、对应协议的标准 reasoning 字段和 `stream`；不添加 token 上限、tools、structured
  outputs 或其他可选参数。Chat 使用 `reasoning_effort`，Responses 使用 `reasoning.effort`。
- `stream:false` 要求合法 JSON、完整协议终态和非空文字；`stream:true` 还要求 Chat `[DONE]` 或 Responses
  `response.completed`。`none` 记录可观察 reasoning 是否为空；`high` 分别记录 reasoning item、可读内容和 usage token 证据。
- 基础 prompt 只要求回复 `OK`。为避免把“任务过于简单”误判成 reasoning 转换缺失，对其中 15 个成功但没有可观察 reasoning 的
  GPT `high` 单元又使用同一条多步整数求解 prompt 定向复测；复测于 2026-08-09 23:59:33（UTC+08:00）完成。
- 结果只保存状态码、终态、语义布尔值、reasoning token 计数和耗时；未保存生成正文、reasoning 正文、Provider request ID 或
  credential。只验收正常首选路径，没有制造故障或强制 fallback。

## 2. 总体结果

| 检查面 | 结果 | 结论 |
|---|---:|---|
| 基础矩阵 | 128 | 16 个模型各 8 个单元全部执行 |
| HTTP 200 | 124 | 124/124 均有非空文字和完整 JSON/SSE 终态 |
| HTTP 400 | 4 | 全部为 `gpt-5.3-codex-spark` 的 `none`，`unsupported_value`，参数 `reasoning.effort` |
| `none` | 60 个 200 + 4 个 400 | 60 个成功体均没有可观察 reasoning；没有“关闭后仍返回 reasoning”的单元 |
| `high` | 64 个 200 | 64/64 均有文字和完整终态；简单 prompt 中 49 个有 reasoning 证据，15 个没有 |
| `high` 定向复测 | 13/15 出现 reasoning | Luna/Sol/Terra 四组合与 GPT-5.5 Chat SSE 均出现；Spark Chat JSON/SSE 仍无可观察 summary |
| HTTP 429/5xx、传输或协议终态错误 | 0 | 未观察到最终限流、服务端错误、连接错误、非法 JSON/SSE 或缺失终态 |

## 3. 请求结果矩阵

缩写：`C` 为 Chat、`R` 为 Responses；`OK` 表示 HTTP 200、非空文字和正确终态；`R-` 表示没有可观察 reasoning，`R+` 表示
存在 reasoning item、可读 reasoning 或正 reasoning token 计数。`*` 表示扩展 Models 没有列出 `none`，但实际请求成功；`‡` 表示
简单 prompt 没有 reasoning、定向复测后出现；`†` 表示定向复测后仍没有可观察 reasoning。

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

`gpt-5.3-codex-spark` 的 Responses `high` 两个基础单元分别返回 reasoning item 和正 reasoning token 计数；同模型 Chat
JSON/SSE 在基础 prompt 和多步求解 prompt 下都只返回完整文字，没有 summary、reasoning item 或 reasoning token 证据。因此这里记录为
Chat Bridge 可见性排查项，而不是 HTTP、文本或终态失败。

## 4. 不进入文字矩阵的模型

| Public Model | 当次运行时状态 | 任务/interface | 排除原因 |
|---|---|---|---|
| `mimo-v2.5-asr` | Models 可见 | `speech_recognition`；Chat-only | 专用 ASR，不是通用文字 Chat/Responses |
| `mimo-v2.5-tts` | Models 可见 | `speech_synthesis`；Chat-only | 专用 TTS，不是通用文字 Chat/Responses |
| `mimo-v2.5-tts-voiceclone` | Models 可见 | `voice_clone`；Chat-only | 专用语音克隆，不是通用文字 Chat/Responses |
| `mimo-v2.5-tts-voicedesign` | Models 可见 | `voice_design`；Chat-only | 专用音色设计，不是通用文字 Chat/Responses |
| `qwen3.7-text-embedding` | Models 可见 | Embeddings-only | 没有 Chat/Responses interface |
| `text-embedding-3-small` | 静态注册、当次 Models 不可见 | Embeddings-only | 当次没有可执行运行时 Route，且不属于文字对话矩阵 |

## 5. 修补状态与剩余证据边界

- 本快照执行时，`deepseek-v4-pro`、`deepseek-v4-flash`、`kimi-k3`、`glm-5.2` 的扩展 Models levels 没有 `none`，但 16 个
  `none` 单元全部成功且 reasoning 为空。2026-08-10 的后续实现已给四个 canonical profile 补入 `none`，并为 Bailian
  DeepSeek Pro/Flash 增加 `none` 到 `enable_thinking:false` 的 Chat egress 转换；表中的 `*` 继续表示测试时状态。
- 当前 preflight 仍把显式 `ReasoningLevel::None` 当作“不要求 reasoning 能力”直接放行。因此 Spark 的 `none` 没有在本地按 Models
  契约拒绝，而是形成四个最终 400；本次后续实现没有给 Spark 增加 `none`，也没有收紧这一准入边界。
- Spark Responses `high` 有 reasoning，而 Chat Bridge 没有可观察 summary；下一步应沿 Responses-to-Chat 转换检查 summary
  生成和 JSON/SSE 两条输出路径，但本轮没有修改实现。
- 2026-08-10 的后续代码修补只运行确定性 Rust 契约和全仓基线，没有重复运行本次 128 单元真实 Provider 矩阵；本次矩阵仍是该修补的
  外部行为依据，而不是修补后运行时的重新验收。
- 本轮没有执行其他 reasoning level、tools、structured outputs、图片、音频、Embeddings、外部 SDK、强制 fallback、负载、长时间
  运行或生产验收；未覆盖范围不能从其他历史测试自动外推到 `ed515cc`。

## 6. `qwen3.6-27b` 接入后补测

2026-08-10 在 `ed515cc` 加当前未提交实现 worktree 上重新构建并启动 OpenBridge。扩展 Models 单模型查询返回 HTTP 200；
`qwen3.6-27b` 的 Chat/Responses 均公开 `none/high`，reasoning output 均为 `plain_text`。随后使用真实下游用户 key 和现有
Bailian credential 对同一正常首选 Route 执行 8 个请求，结果如下：

| Model | C none JSON | C none SSE | R none JSON | R none SSE | C high JSON | C high SSE | R high JSON | R high SSE |
|---|---|---|---|---|---|---|---|---|
| `qwen3.6-27b` | OK R- | OK R- | OK R- | OK R- | OK R+ | OK R+ | OK R+ | OK R+ |

- 8/8 均为 HTTP 200、content type 正确、文字非空且协议终态完整；Chat SSE 收到 `[DONE]`，Responses SSE 收到
  `response.completed`。单请求耗时为 260–2915 ms。
- 四个 `none` 单元均没有 reasoning item、可读 reasoning 或正 reasoning token 证据，确认当前下游 `none` 能关闭 thinking。
- Chat `high` 的 JSON/SSE 均有可读 `reasoning_content` 和正 reasoning token 计数；Responses-via-Chat `high` 的 JSON/SSE 均有
  reasoning item 和可读 reasoning 文本，但没有 reasoning token 计数。本次把它记录为 usage 映射现状，不影响文字、reasoning item
  或终态验收。
- 结果仍只保存状态码、终态、语义布尔值和耗时，没有保存生成正文、reasoning 正文、Provider request ID 或 credential；没有执行
  外部 SDK、强制 fallback、其他 reasoning level、tools、多模态、负载、并发稳定性、长时间运行或生产验收。
