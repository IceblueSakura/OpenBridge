# 当前开发焦点

## Responses 纯文本协议与 HTTP/SSE 验收

用户授权补全 Responses 的纯文本协议，包含 instructions、content、reasoning、tools，然后完善 HTTP/SSE；本阶段不接实际 Provider。允许破坏性修订 v2 内部接口，不保留无意义兼容层，不自动提交或推送。

当前只有 v2 语义库和独立验收；旧运行时已按用户决定[整体归档](../archive.md)，不要求恢复旧服务可用性。离线测试和固定 SDK synthetic loopback 是局部证据，不等于全部字段/事件准入或本切片完成。继续完成字段覆盖审查及生命周期/资源反例后，才可清空焦点。生产执行不属于本切片完成条件。

### 验收目标

普通 Responses 客户端不必人工裁剪请求/响应字段，就能在明确的无状态纯文本 profile 中经过 v2 完成 JSON/SSE 两轮交互。最终 wire 必须由最终 IR 与合法表示元数据决定，不能依赖原 JSON 透传。

### 实施入口与剩余验收

先检查分支、工作区和目标 diff，保留现有草稿，不从零重写或假定已有代码通过验收。恢复入口：

- 任务类型与 reducer：`src/semantic/task/generation/`、`src/semantic/value/presence.rs`。
- 静态/事件 codec 与完整 envelope：`src/protocol/openai/`，重点是新增的 `envelope.rs`、`settings.rs`、`text.rs`、`sse.rs`；同时检查 `src/lowering/` 和 `src/protocol/fidelity.rs`。
- 纯 framing：`src/transport/sse.rs`。保留独立 `sse_contract` 验证，不能只验证 Responses 新入口；旧运行时消费者已归档。
- 离线验收：`tests/semantic_v2_text_profile.rs`、`tests/semantic_v2_responses_sse.rs`、`tests/semantic_v2_body_lifecycle.rs` 和 `tests/support/responses_profile.rs`；后者是独立编写的 synthetic wire 预期，不由 Rust encoder 生成。SDK 专用入口为 `tests/semantic_v2_responses_sdk_loopback.rs` 和 `tests/sdk/semantic_v2_responses_text_loop.py`，不经过生产 Router。
- `Cargo.toml` 与 `Cargo.lock` 已加入直接 `mime` 依赖，用于 HTTP media type 解析；后续继续使用 locked 检查。

按以下依赖顺序推进：

1. **恢复可执行检查并固定准入矩阵。** 先编译、执行新 profile 测试及受影响的 `semantic_v2_*` 测试，区分实现缺陷与旧子集预期。基于 SDK `3.10.0` 类型和仓库固定资料，逐项确认字段/事件的 owner、支持形态、缺失/null/空值/默认值归一化及明确拒绝项；不能把 SDK 类型存在或宽松解析成功当作已支持。覆盖矩阵应有规范落点，本页不复制另一套 schema。
2. **关闭静态与事件语义缺口。** 复核 instructions 字符串、分段消息及响应中的 instruction echo；function/custom 定义、选择、调用和文本数组结果；Structured Output、reasoning 控制、refusal、annotations/logprobs、usage 和完整 envelope。为准入语义补独立 decode/encode 预期及插入、替换、删除测试，并检查最终 requirements 与 lowering 拒绝边界同步变化。
   - 完整 HTTP envelope 与低层 task snapshot 的职责须明确；缺失的 reported settings、usage 明细或概率字节不能靠猜默认值补齐。
   - SDK 的 readable reasoning text 使用 delta/done，不应生成 message 的 `content_part` 事件；当前修订仍须通过 SDK 验证。
   - 区分增量 logprobs 与静态概率记录的字段要求，允许有证据的缺失详情补全，不允许最终 snapshot 改写已观察概率。核对 `file_path` 的静态 annotation 与 annotation-added 事件准入差异。
   - 正文替换必须清除依赖旧正文的引用/概率；cache breakpoint、wire identity 和 replay 不得重新附着到已删除或错配的 owner。partial encrypted replay 可观察不等于可用于后续请求。
   - 补齐 malformed/null、身份和状态冲突、未知字段、累计预算与嵌套 schema 等资源反例；检查错误后是否仍可继续使用 decoder/encoder。
3. **完成 HTTP/SSE framing 的独立验收。** 对 `ResponsesSseDecoder`/`ResponsesSseEncoder` 和共用 framer 覆盖逐字节 UTF-8、LF/CRLF/裸 CR、BOM、多行 data、comment、event 名与 JSON type 一致性、HTTP status/content type、EOF 与终态。确认分片方式不改变预算结果，缺失 delimiter/terminal、重复终态和终态后业务数据均不能变成成功；明确 v2 strict EOF 与既有允许 EOF dispatch 的调用边界，并回归已有 transport 测试。
   - 分别验证单事件、总 wire、事件数和语义累计预算，以及 obfuscation 的显式开关与独立预算。测试 seed 只能使用 synthetic 数据，不能据此宣称生产随机性或侧信道保证。
   - 默认每次消费有界帧并增量产出，不先收集整条 wire 流再转换。
4. **建立独立固定 SDK 的两轮 loopback。** 维护只调用 v2 的专用测试 Router/handler，不恢复已归档的 predecessor Router gate。先绑定固定 synthetic model，再做 envelope decode → IR → lowering/encode；响应来源使用独立 JSON/SSE fixture。覆盖 reasoning → 并行 function/custom calls → 客户端合成工具结果 → 最终文本/Structured Output 的两轮 JSON 与 SSE 交互，检查 SDK 的最终响应和关键事件。
   - 在 v2 IR 中修改最终正文（例如将 fixture 的 `{"ok":false}` 改为 `{"ok":true}`），并清除旧引用/概率；SDK 观察结果必须来自最终 IR，而非源 snapshot 或原 JSON 透传。
   - 固定 `openai==3.10.0`，使用严格响应验证；所有 HTTP 仅到临时 loopback listener，禁用自动重试和环境代理，使用 synthetic Bearer，不继承 Provider credential、组织/项目等私有环境配置，不读取认证缓存或隐式启动实际 OpenBridge 服务。
5. **用实际 body 生命周期验证取消、背压和失败。** 先用 channel/readiness 和有界 timeout 建立确定性测试，再让专用 loopback 覆盖：首帧之后可立即消费、慢消费者不导致无限预取、客户端提前关闭后生产者被丢弃、截断/非法帧/超限不能补发成功终态。SDK 子进程、listener 和 body producer 都须有有界等待与清理；不能用 sleep 隐藏竞争，也不能以 codec 单测替代这些 I/O 证据。
6. **收敛文档并执行最终门槛。** 审查全部 diff，修复本次引入的失败，更新受影响的 v2 所有权/协议说明和具体迁移边界；不要把此专用测试接线写成生产 Router 已迁移。通过下述门槛并确认本切片完成后才清空 current focus；未完成时保留明确缺口，不自动提交或推送。

### 字段归属与限制

- 任务语义：instructions、input/messages、文本/refusal、readable reasoning、工具定义/选择/调用/结果、生成与文本输出控制。
- 表示：response/item identity、时间戳、bounded owner-bound replay 和协议元数据；不得覆盖 typed 内容。
- 交付：stream 与 stream options、HTTP/SSE；不混入消息历史。
- 执行：model 绑定及 session/cache/service tier/safety metadata 位于任务 IR 之外。网关不按业务 payload 选择 upstream URL 或 credential。
- 本阶段保持客户端完整历史的无状态模式。store/background/previous_response_id/conversation 的非活动形态与拒绝边界必须显式，而不是悄悄开启资源服务。
- 不执行 hosted tools、remote MCP、服务端 conversation/background、moderation、prompt templates 或 compaction；不接真实 Provider，不部署。媒体和独立推理任务不在本次范围。
- Structured Output 是纯文本控制的相关独立语义域，需在覆盖矩阵中明确准入，不能用 schema 字段透传冒充实现。

剩余准入审查以 [Responses text profile](../architecture-v2/responses-text-profile.md)为落点；SSE padding 独立预算的当前规则由该页和 `semantic_v2_responses_sse` 维护，不在本页重复 schema。

### 验证门槛

每个准入字段/事件须有 owner、独立 decode/encode 预期、变换与删除规则、不可表示结果、资源边界。正常/异常 SSE 和 JSON materialize 保持一致；unknown fields、非法序列、错配 identity、超限、缺失 terminal 必须失败关闭。

先执行 focused tests，再执行 `cargo fmt -- --check`、`cargo test --locked --offline`、`cargo clippy --locked --offline -- -D warnings`、相关文档链接及 `git diff --check`。本机 linker 如需覆盖，仅使用当前命令环境。SDK 依赖固定版本，所有业务 HTTP 仅使用 synthetic loopback，不读取私有配置或认证缓存。
