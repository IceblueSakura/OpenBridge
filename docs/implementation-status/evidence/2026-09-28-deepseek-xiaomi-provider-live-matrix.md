# 2026-09-28 DeepSeek / Xiaomi MiMo Provider 接入与双协议真实矩阵

## 记录类型

**接入验收记录 + 差异记录**。验收 v2 新增的 Provider/Topology/最小执行链对真实 wire 的处理结果；同时记录固定标准准入与两处已执行观察不一致（Xiaomi Chat `reasoning_content`、Xiaomi Responses reported settings echo）。

## 执行边界

- **工具**：`examples/live_probe.rs`（本 checkout，经 `OPENBRIDGE_PROBE=1` 显式 gate），请求、intake 与下游交付全部走 `openbridge::execution` 同一链入口；HTTP 经环境出站代理。
- **矩阵**：2 模型 × {Chat, Responses} × {text, json_object, tool} × {JSON, SSE} = 24 场景（tool 场景含一次工具结果续轮，共 32 次请求的设计）；固定 synthetic prompt/schema/工具定义，`max_completion_tokens`/`max_output_tokens` 64–192，单次超时 120s，调用间隔 2s。
- **模型目录预检**（免费 GET，付费调用前）：Xiaomi `/v1/models` 含 `mimo-v2.6-pro`（9 ids）；DeepSeek `/models` 仅返回 `deepseek-flash`、`deepseek-v4-pro`，**不含 `deepseek-v4.1-flash`**，按约定停止 DeepSeek 付费调用。
- **凭证/账号/网络**：本地私有凭据池（单一账号），环境出站代理；凭证仅进程内用于认证头，未进入任何日志或报告。
- **脱敏报告**：`testdata/runtime/probe-2026-09-28/`（gitignored）`calls.jsonl` + `summary.md`，仅含 HTTP 状态、阶段、错误类、usage 与耗时。

## 执行结果

### 第 1 轮（mimo-v2.6-pro）

12 次真实请求（mimo-v2.6-pro 全部 12 场景 round 1）HTTP 全部 200，模型正常生成（含 7–13s 的结构化/工具响应）；OpenBridge 链在 decode/terminal 阶段确定性拒绝，两类错误各占 6 格：

| 观察 | 覆盖 | 错误 | 阶段 |
|---|---|---|---|
| Xiaomi Chat 返回 `reasoning_content` | chat × {text, json_object, tool} × {JSON, SSE} | `unsupported field or representation: reasoning_content` | 静态 terminal / 流式 intake |
| Xiaomi Responses 不回显完整 reported settings | responses × 3 cases × 2 交付 | `invalid missing reported settings` | 静态 terminal / 流式 intake |

### 第 2 轮（deepseek-flash，用户选定；原指定的 `deepseek-v4.1-flash` 不在目录中）

12 次真实请求 HTTP 全部 200、模型正常生成；三类新拒绝，各占 4 格：

| 观察 | 覆盖 | 错误 | 阶段 |
|---|---|---|---|
| DeepSeek Chat 返回 `system_fingerprint` | chat × 3 cases × 2 交付 | `unsupported field or representation: system_fingerprint` | 静态 terminal / 流式 intake |
| DeepSeek Responses usage 缺细分 | responses × 3 cases × JSON | `invalid incomplete usage details` | 静态 terminal |
| DeepSeek Responses SSE 返回 `content_filters` | responses × 3 cases × SSE | `unsupported field or representation: content_filters` | 流式 intake |

### 第 3 轮（收敛后复测）

Chat 侧准入按 [Chat profile](../../architecture-v2/chat-text-profile.md#provider-stream-shapes-and-normalization)收敛后复测：**两模型 Chat × {text, json_object, tool} × {JSON, SSE} 全部 16 格（含 4 个工具续轮 round）真实消费通过**，HTTP 200、终态合法、usage 原样、下游交付经独立 decode 验收。收敛内容：`reasoning_content` → reasoning 语义映射（Static/Event 顺序一致）、`system_fingerprint` 标准 reported fact、usage 别名归一（`prompt_cache_hit_tokens`/`miss` 与标准细分一致性校验）、finish 帧 usage、null 身份/role 复用、空开篇 fragment 延迟归属、tool_calls `index`/身份规范化、`created`/fingerprint 首值绑定。

Responses 侧 12 格保持确定性拒绝，阻塞为 provider 与固定标准/固定 SDK 的不一致（实测 `openai` 固定版本模型）：

- DeepSeek Responses 不报告 `usage.input_tokens_details.cache_write_tokens`（固定 SDK 为 required int，null 亦不可），另有 `content_filters`/`frequency_penalty`/`presence_penalty` 等扩展字段；
- Xiaomi Responses 完全不回显 settings（固定 SDK 要求 `tools`/`tool_choice`/`parallel_tool_calls` 为 required 非空），usage 细分同样缺失，另有 `output_text` 扩展字段。

标准明文禁止用请求复制冒充 response facts（[responses-standard](../../references/responses-standard.md)），网关不估算、不伪造：在 provider 补齐报告事实或产品合同另行裁决前，Responses 交付保持拒绝。

## 处置：双轨实现（2026-09-28 修订）

不向 Provider 提一致性要求。处置改为[双轨](../../architecture-v2/decisions/0008-stable-core-and-vendor-adapters.md)：稳定核心 IR + 厂商适配 encode/decode——usage 缺省按三态忠实保留（两家无 cache-write 计费维度，缺席属预期）、settings 回显缺省保留不伪造、`output_text` 按派生 view 校验后丢弃、`content_filters`/`frequency_penalty`/`presence_penalty` 走有界 classified response fidelity。本文上方的 wire 观察保留为适配层的事实依据。

### 第 4 轮（双轨适配后复测，2026-09-29）

**矩阵 32/32 场景全部真实消费通过**：两模型 × {Chat, Responses} × {text, json_object, tool（含工具续轮）} × {JSON, SSE}。适配层收敛的厂商形状：usage 缺省三态（无 cache-write 计费维度）、settings 回显缺省、`output_text` 派生 view 校验后丢弃、`content_filters`/`frequency_penalty`/`presence_penalty` 有界 classified fidelity 同源保留、`reasoning_content` 与流式 `reasoning_text` part、DeepSeek encrypted reasoning 的同源 replay scope 绑定、usage 别名与流形状规范化。核心 IR 未做任何厂商化修改。
