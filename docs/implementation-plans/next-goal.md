# 下一步目标

**用仅文本 Generation 验证 IR 化网关的整体流程，再扩展 Provider 和多模态。** 最终产品是[多模态、Chat Completions/Responses 兼容、Agent 友好且缓存亲和性强的网关](../architecture-v2/README.md#产品目标与阶段判据)，不是离线 codec 集合。Generation 仍采用 Responses-first 标准语义 + scoped extensions；Chat 是同一 IR 的第二协议，不另建 Chat IR。完成度及证据统一见[实施基线与差距](../architecture-v2/migration.md)。

文本是阶段性验证载体，不是把 IR 永久缩成 text/common subset，也不要求先穷尽纯文本 API 才接线。当前暂缓广泛 Provider 接入、多模态、hosted tool 执行与有状态 API；旧运行时已[归档](../archive.md)，最小网关流程不等于恢复整个旧入口或追求旧功能对等。

## 优先顺序

| 优先级 | 建议切片 | 预期可观察结果 / 退出条件 |
|---|---|---|
| 1 | **Responses 完整 SSE 必填性闭合** | 以[已复现反例](../architecture-v2/migration.md#当前正确性缺口)起步：required `sequence_number` 缺失、initial snapshot 缺 `output` 不再成功；同类已准入生命周期分支按固定 schema 审查。保留合法空 output、正常 JSON/SSE 与低层接口的明确边界；错误后不可恢复为成功 |
| 2 | **最小文本端到端网关切片设计与接线** | 定义受信 Public Model/task、固定 Route/目标与最小 Provider adapter，以 synthetic upstream 验证真实请求/响应处理链，而非只在 SDK 测试 handler 里直接构造回答。独立立项实现，不恢复整个旧运行时 |
| 3 | **沿端到端场景补必要文本投影** | 优先候选为 Chat cache-write usage、content_filter、选定服务返回的 service tier / metadata / system fingerprint；每个 owner 独立切片，按所选场景需要先后接入，不把整个字段表作为步骤 2 的前置条件 |
| 4 | **固定真实 Provider 的受控验收** | 文本 loopback 全流程通过后，另行选择精确 Provider/account/model/请求矩阵，验证实际协议与缓存行为；需要明确网络/付费授权。通过一个目标不代表其他 Provider |
| 5 | **扩展 Provider 与多模态** | 复用已验证的 IR / lowering / execution 边界，按来源 profile 与任务域逐项扩展；每域覆盖 request/response/event、资源安全、变换与失败，不另起 Native 旁路 |

步骤 3 的具体可验收候选：

- **Chat cache-write usage**：已有 `Usage.input_cache_write_tokens` 投影到标准 `prompt_tokens_details.cache_write_tokens`；缺省/null/0、usage tail、Static/Event 和双协议投影不丢计数、不估算。此项帮助观测缓存，不等于已经提高命中率。
- **Chat content_filter**：保留过滤原因和 partial output，JSON/SSE 一致，finish_reason 不替代 DONE，不退化为 length/stop。
- **标准上下文**：先定各 wire 位置、request hint/reported fact 和流内变化规则，不用 opaque 透传放行未知键。

其他 usage 细分、文本 content-array/logprobs/控制和扩展 response context 留在[缺口表](../architecture-v2/migration.md#尚未映射的文本能力)。没有实际流程阻塞或具体反例，不扩张 Schema、不追求“完整 Chat API 对等”。

## 紧接着建议选定的切片

**选择优先级 1，而不是继续笼统“完善 Schema / configuration update”。** 原因是已有准入路径仍能把缺必填字段的 wire 接受到成功终态；这比扩大支持集合更直接影响边界可信度。

- **要求**：固定 SDK/schema 的标准 SSE required/presence 规则与完整 Response snapshot 一致；完整字节边界不能借低层简写补字段。
- **失败用例**：从独立合法 wire 删除 sequence 或初始 output，分别断言即时拒绝、poisoning、EOF/materialization 失败；保留合法空数组及分片对照。相关 queued/in_progress 生命周期需按各自 schema 验证，不新增状态资源操作。
- **非目标**：不重写 framer/reducer，不放开 hosted/media/全部 event union，不把本地 `response.cancelled` 改称标准，不顺带补 Chat 或扩张 Schema。
- **验证**：最低 owning layer 的负例 + 完整 Responses SSE 字节入口；Rust locked/offline 基线。若涉及 SDK 生成的正常 wire，固定消费者 gate 是另一个显式验收层，不能替代缺字段负例。
- **完成标准**：已列反例转为拒绝且正常已准入分支无回归，profile 与剩余缺口同步；不以“全部纯文本标准已完成”为退出条件。

这是下一步建议，不是已经获准的行为实现；[current-focus](current-focus.md)保持无进行中切片。选定实现范围后，再将可观察结果、失败用例、非目标与验证边界落到 current-focus。

## 文本端到端验证的完成门槛

整体流程至少包含以下实际调用链；设计合同见[execution model](../architecture-v2/execution-model.md)，当前尚未接线：

```text
Chat / Responses client
 → 受信 Public Model/task 绑定与入口准入
 → request codec → IR validation / transform → requirements
 → 固定 Route / candidate lowering → upstream codec
 → 可信 Provider transport adapter
 → response / event decode → IR → downstream lowering / codec
 → JSON / SSE client consumption → 下一轮历史回放
```

1. **语义与双协议**：关闭已知已准入边界缺口；对可表示的文本/function/structured-output 子集验证同协议与跨协议路径，并实际修改 IR 验证最终 upstream/downstream wire。Responses-only 字段不能用静默删除换取 Chat 成功。
2. **Agent 续轮**：固定 SDK 经完整链路消费工具调用，客户端执行 synthetic tool 并回传结果，再取得最终回答；保留 call identity、工具定义/Schema 顺序、reasoning replay 的可信 origin 和派生 view 的原文权威。至少覆盖 JSON/SSE 与非成功终态；不要求网关自己执行工具或支持完整 Agent 产品。
3. **缓存亲和性**：同一合法 scope 的续轮保持约定的 cache key、前缀内容/工具/Schema 顺序及有效上下文；IR 修改/删除仍是权威，不为缓存恢复旧内容。logical session、cache affinity、turn state 分开，跨 origin/认证所有权不沿用 opaque replay。先验收稳定投影与隔离，不用 synthetic usage 宣称真实 cache hit、成本或延迟改善。
4. **执行与安全**：最小 ingress 明确其语义承诺与拒绝项；业务请求不能选择 URL/credential/profile。固定目标、认证绑定、body/stream 预算、取消、错误分类和 downstream commit 需要实际 owner；首个可见语义输出后禁止重试/fallback。首片可使用单固定候选，不要求先建凭据池或自动故障转移。
5. **证据分层**：独立语义/字节测试 + 同一接线入口的 synthetic upstream + 显式固定 SDK gate。现有 SDK handler 直接构造 fixture 回答，不能代替 Provider adapter/执行链验收；真实 Provider 与缓存效果另行验证。

不以通用 Schema 引擎、program/hosted-tool 执行、Codex turn 管理、state/WS、多模态或其他任务完成作为门槛。新场景暴露反例时按 owner 补具体切片，不先建通用插件框架。

## 实现前需选定的边界

下一行为切片仍由 [current-focus](current-focus.md)明确范围。最小执行切片需先确定：Public Model 的文本承诺、固定上下游 profile/目标、ingress 与 credential/origin owner，以及哪些缓存/Agent 场景必须首片覆盖。第一阶段可以完全 synthetic loopback；真实 Provider 的账号、模型和调用矩阵未选择，也未获调用授权。

本页只记录方向；不授权行为修改、真实 Provider/付费调用、部署、提交或推送。
