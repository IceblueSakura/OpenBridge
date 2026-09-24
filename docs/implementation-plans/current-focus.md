# 当前开发焦点

## 下一切片：Chat response_format 的双协议闭合

本页记录经本次审阅收敛的下一步建议；本次授权为文档整理，不据此自动启动代码实现、提交或推送。整体方向由 [next-goal](next-goal.md)维护：Responses-first Generation，以单候选 Chat 验证同一 IR 的协议投影，不扩展多模态或恢复旧运行时。

### 起点与实际缺口

已有完整 JSON/SSE 字节入口、共享 reducer、Schema 结构/strict/default/本地引用验证，以及两套固定 SDK loopback。复用这些机制，不重新建立 framer、Schema 引擎或测试 Router。当前准入分别由 [Responses](../architecture-v2/responses-text-profile.md)、[Chat](../architecture-v2/chat-text-profile.md)、[Schema profile](../architecture-v2/schema-profile.md)拥有。

具体缺口：`chat.rs` 尚不接收/编码 `response_format`；`lowering/generation.rs::lower_request` 对 Chat 的 structured output 仍整体拒绝。IR 已有 `OutputConstraint::{Text, JsonObject, JsonSchema}` 和 `TextOptions`，无需另建 Chat 输出语义。SDK 派生 view 是独立缺口，不作为本切片的前置条件。

### 可观察结果与范围

- 在明确准入的无状态单候选文本请求中，Chat `response_format` 的 text、json_object、json_schema 三分支与 Responses `text.format` 映射到同一语义 owner；分别验证独立的 decode/encode 预期，而非只做 round trip。
- JSON Schema 的 name、description、strict 与有序 schema 来自最终 IR；插入、替换、删除控制后，requirements/lowering 和最终 wire 同步变化，源元数据不能补回旧值。
- 固定 SDK 经完整 envelope → IR → lowering → JSON/SSE 消费合成结构化文本；通过修改最终 IR 正文证明响应并非源 JSON 透传。请求约束不是响应事实，不给 Chat response/chunk 伪造 format/settings echo。

只补此控制的协议映射和必要拒绝边界。保持现有 Schema profile、function strict 默认差异及 structured_output capability 检查；不通过放宽 Schema 验证或全部解除 Chat lowering 拒绝来实现映射。

### 实施顺序与失败用例

1. **固定准入与表示规则。** 核对固定 `openai==3.19.0` 类型及[上游基线](../references/upstream-sync.md)：Chat json_schema 的嵌套外壳与 Responses 的平铺形态分别解析；确认外层缺失/null、显式 text、内层 strict/description 的缺失/null/default 等价或区别。检查 `TextOptions.presence` 与 format presence 的依赖；不得把仅有 verbosity 或空 text 容器猜成用户显式指定了 response_format。准入细则更新到对应 profile，不在本页复制另一套 schema。
2. **先写失败测试，再接入 codec/lowering。** 在最低拥有层覆盖三分支的独立双向预期、完整 envelope 字节入口、错误嵌套/字段类型/未知字段和 Schema 拒绝传播。只开放已实现的 Chat 输出控制映射；目标 capability 不支持、其余不可表示语义和 function strict 默认不匹配仍须拒绝。
3. **验证变换与响应闭合。** 对最终 schema 做插入/替换/删除并断言属性顺序；删除或切换输出控制后不得复活旧 schema。JSON/SSE 保持同一最终文本与终态；refusal、length/截断、流失败不得被包装成成功的结构化结果。复用已有终态/资源测试，只为新增行为补独立反例；不实现模型输出的通用 Schema adherence 检查。
4. **扩展固定消费者并收敛文档。** 在现有 Chat SDK 两轮 JSON/SSE gate 中加入 response_format 请求和最终文本断言，保留 Responses gate。使用 create/typed chunks 即可证明本切片的请求和 wire 合同，不以此宣称 parse/parsed replay 已支持。更新受影响的 profiles、迁移缺口与测试预期，不写完成日志。

主要入口：`src/protocol/openai/{chat,chat_envelope,settings}.rs`、`src/lowering/generation.rs`，以及 `src/semantic/task/generation/{request,requirements,schema}.rs`。测试复用 `tests/semantic/{schema,text_profile,tools}.rs`、`tests/transport/chat.rs`、`tests/sdk/chat.rs` 和 `tests/sdk/chat_text_loop.py`；实现前检查实际调用链与目标 diff，保留已有工作。

### 验证与完成条件

先运行受影响的 semantic/transport 用例，再执行 `cargo fmt -- --check`、`cargo test --locked --offline`、`cargo clippy --locked --offline --all-targets -- -D warnings`。按[开发指南](../development.md#固定-openai-sdk-loopback)显式运行 Responses/Chat 两套 SDK gates，并检查文档链接和 `git diff --check`。全部业务 HTTP 仅使用 synthetic loopback，不读取私有配置、调用真实 Provider 或部署；离线验收不证明生产兼容。

完成条件为：准入规则明确、三分支独立映射与变换成立、拒绝边界不退化、JSON/SSE 与固定消费者验收通过、相关文档与实现一致。届时清空本切片；无需等整个纯文本标准覆盖完成，也不得同时抹去 next-goal 中未完成的方向。

### 后续而非本切片

下一切片再处理 SDK 派生视图回放：分别确定 Responses output_text.parsed 与 Chat parsed 消费对象的实际形态、原始正文权威、派生值一致性/失效规则，以及 static/event/history 各自的准入位置。不能直接照搬 parsed_arguments 规则或允许任意 SDK 对象作为协议 payload。

其余字段/事件审查、phase/configuration update、Schema regex/format 与模型级限制、媒体/tools/state 和生产执行继续由 [next-goal](next-goal.md)与[迁移缺口](../architecture-v2/migration.md#当前已知闭合缺口)排序，不纳入本切片的完成条件。
