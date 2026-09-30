# Flash / LongCat 双协议接入与百炼受控验收

- 日期：2026-09-30 UTC。
- 实现：`semantic-v2`，基线 `4397183` 加随本文交付的绑定、bootstrap、probe 选择与 usage view 适配改动；不是未修改基线的验收。
- 主要路径：固定 OpenAI SDK `3.19.0` → 临时 loopback `openbridge` binary → 固定 Provider；三次独立库级 capture 诊断单列，不冒充 binary 验收。
- 仅 synthetic 文本、JSON object、`lookup` Schema 和数字结果，无媒体、实际工具执行或用户会话。每请求至多 2048 输出 tokens，截断场景固定 8；串行、零自动重试、不跟随重定向，保留 120 秒服务 deadline。
- 凭据只在进程内加载；未改写私有配置或系统服务。SDK 不继承环境代理，可信网关出站显式配置代理。报告只保留白名单元数据；诊断 capture 每份最多 2 MiB，仅落本地 ignored 目录，剔除已知 credential/opaque 字段，不进入独立 fixtures。

## 固定目标与接入范围

| 对外模型 | 上游模型 | 实际入口 |
|---|---|---|
| `mimo-v2.6-flash` | `mimo-v2.6-flash` | Xiaomi `/v1/chat/completions`、`/v1/responses` |
| `longcat-2.5-preview` | `LongCat-2.5-Preview` | LongCat `/openai/v1/chat/completions`、`/openai/v1/responses` |
| `qwen3.8-max` | `qwen3.8-max` | 北京百炼 `/compatible-mode/v1/chat/completions` |

Flash 为正式双协议绑定，与 Pro 共享凭据但保持独立模型/Route/Endpoint；本次未调用 Pro。百炼只验收既有 Chat，不因此开放 Responses。精确来源、映射和准入见 [API-key profiles](../../architecture-v2/api-key-text-profiles.md)、[LongCat 来源](../../references/providers/longcat-api.md)、[百炼来源](../../references/providers/bailian-api.md)。

## 场景及结果

正常矩阵的每个协议/交付组合包含 8 请求：文本 1、JSON object 1、工具调用及续轮 2、两次 lookup 的四轮历史 4。正文 oracle 分别检查精确文本、独立 JSON 解析与整数值、工具名/参数/call identity，以及续轮的准确数字结果；历史在 SDK 实际 send 前与预期序列化值比较。JSON/SSE 均检查实际响应消费，SSE 额外检查终态及 HTTP EOF，不以 HTTP 200 判成功。

| 模型 / 路径 | 已执行的通过范围 | 未完成或失败 |
|---|---|---|
| Flash，Chat JSON/SSE + Responses JSON/SSE | 正常矩阵 32/32；Chat SSE 提前关闭后另一次普通请求成功，原取消请求记录 `cancelled` 而非成功终态 | Chat JSON 8-token 截断被拒绝；独立诊断确认非法 usage。Chat SSE 8-token 截断中途失败，未捕获该次完整 wire，不能直接认定与 JSON 同因 |
| LongCat，Chat JSON | 文本、JSON object、工具首轮通过 | 工具续轮在响应头前超时；四轮历史未执行 |
| LongCat，Responses JSON | 修复重复 usage view 后，新的文本及 JSON object 请求通过 | 工具首轮在响应头前超时；Responses SSE、Chat SSE 和相应续轮/边界未完成 |
| 百炼，Chat JSON/SSE | 修复重复 text usage view 后，正常矩阵 16/16 | Chat JSON 8-token 截断在响应头前超时；SSE length、取消及随后请求未执行 |

百炼的新一轮请求已取得上游 HTTP 200，并完成正常 JSON/SSE/工具历史验收；[先前 invalid_api_key](2026-09-29-provider-followup.md)不能继续描述本次所用凭据。新成功不证明其他地域、账号或全部模型 entitlement。

### 全部实际尝试（不隐藏修复前失败）

| 模型 | 通过 | 失败 | 主动取消 | 总请求 |
|---|---:|---:|---:|---:|
| Flash | 33 | 3 | 1 | 37 |
| LongCat | 5 | 4 | 0 | 9 |
| 百炼 | 16 | 3 | 0 | 19 |
| 合计 | 54 | 10 | 1 | 65 |

其中 62 请求经 SDK/binary，3 请求为库级诊断（均在修复前失败）。修复后的离线 replay 不产生额外 Provider 调用，也不改写原请求的失败记录。未发送场景保留 `not_run`，不能算通过；预算未用尽不触发自动重试。

## 已定位的 wire 差异

1. **LongCat Responses 的重复 usage details**：标准 `input_tokens_details` 之外另有相等的 `prompt_tokens_details`，导致原 adapter 拒绝。现在仅该具名规则检查重复对象与标准对象完全相等后丢弃 view；缺少标准对象、null 或冲突都拒绝，不把它当第二个 Usage 权威。真实脱敏 capture 离线 intake/projection 通过，随后新的 SDK/binary 文本与 JSON object 请求通过。
2. **百炼纯文本的 `text_tokens`**：观察到输入 text count 等于 prompt total，输出 text count 等于 completion total。官方 Chat 文档明确 output text count 已包含 reasoning count。adapter 只在对应 unsigned integer 完全等于总计时丢弃冗余 view；不估算正文 tokens、不叠加 reasoning、不接受非等值媒体细分。真实 capture 离线通过，随后正常矩阵 16/16 通过。
3. **Flash 截断 usage 矛盾**：独立 8-token Chat JSON 诊断报告 `completion_tokens=8`、`reasoning_tokens=12`、`finish_reason=length`。子计数大于总计不满足既有语义不变量，网关拒绝正确；没有 clamp、补造或放宽验证。首批 JSON 失败只有阶段元数据，不能声称该次具体字段也已被捕获。新的 synthetic 回归保护合法边界与非法 JSON/SSE usage、poisoning。

适配规则仅在边界验证重复表示，不改变 core schema、标准/client 准入或全局缺省策略。独立测试在 `tests/semantic/provider_profiles.rs` 与 `tests/semantic/adapters.rs`；完整 Rust locked/offline、clippy、fmt 及三个固定 synthetic SDK gates 均已执行通过。回放 capture 不是独立 wire oracle。

## 仍未定位的失败

- 首批 LongCat Chat 工具续轮，以及修复后的 LongCat Responses 工具首轮：120 秒 deadline 到达前均未收到上游响应头（操作者 stage 为 `connect`，status 未知）。这不能区分连接、代理、Provider 排队或生成等待，更不能据此认定 codec 问题。
- 百炼 Chat JSON 8-token 截断也在响应头前超时，没有响应 body 可供语义诊断。
- Flash Chat SSE 8-token 截断：上游 HTTP 200 后在 `intake` 失败，已经交付部分字节，下游连接中止，没有伪造成功终态。该次无 capture，原因仍需独立确认。

用户报告期间手动重启过系统服务；时间上的并存不是因果证据。后续又出现上述超时，因此既不能断言全由重启造成，也不能直接归咎 Provider。未延长服务 deadline、未自动重试/fallback，未为得到全绿而改请求控制。

## 本地证据与复现边界

白名单账本与报告分别位于以下 ignored run 目录；不依赖这些目录运行默认 tests：

- `testdata/runtime/flash-longcat-20260930-0654`：首批 13 请求。
- `testdata/runtime/flash-length-20260930-0704`：1 次 Flash 截断诊断。
- `testdata/runtime/flash-longcat-bailian-20260930-0704`：18 请求，含 Flash Responses 正常矩阵及两个修复前失败。
- `testdata/runtime/longcat-bailian-wire-20260930-0708`：2 次 adapter 差异诊断。
- `testdata/runtime/flash-longcat-bailian-closure-20260930-0714`：修复后 31 请求；其源码 fingerprint 为 `3f04a370d16c092d704b02e877aaf0d8d13a1c169bc841ee0f74661988f549a5`。

计划、逐请求计数和原调用诊断解释见 [probe 指南](../../probes.md)。测试选取 `text,json,tool,history` 的正常路径，再单独选取 `cancel,length`，所有请求仍受同一批次的总预算约束。重新执行需要新的明确授权，不复用账本重放同一 scenario。

这不是全部协议分支、任意 Agent、媒体、模型质量、缓存命中收益、负载/长稳或生产部署验收。客户端提前关闭及后续请求成功也不证明 Provider 已停止计算或计费。没有独立核验账单。
