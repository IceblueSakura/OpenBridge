# 模型交互实施边界与缺口

本页只记录尚未闭合的实现边界，不维护完成度矩阵、动态 Provider/model 清单或测试结果。设计权威归[语义设计](../architecture-v2/semantic-ir.md)，当前接线归[架构](../architecture.md)，推进顺序归[next-goal](../implementation-plans/next-goal.md)。

实际准入须分别核对 [adapters](../../src/adapter/mod.rs)、[codecs](../../src/protocol/openai/mod.rs)、[lowering](../../src/lowering/generation.rs)、[HTTP activation](../../src/gateway/config.rs)和独立预期；库类型、目标表示、实例启用和上游接受不是同一层。现有 Chat/Responses profiles 仍有约束力，新的协议中立设计不自动扩大准入。

## 文件范围与延期边界

文件仅维持既有 Responses user inline/URL 基础输入及必要正确性、安全维护。Issuer-bound 文件 ID、工具文件结果、生成文件产物、更多文件格式/目标、Chat 文件投影及文件管理 API 等扩展继续暂停，恢复评估按[计划的有限首批范围与重评节点](../implementation-plans/next-goal.md#推进顺序与退出条件)执行，不等待所有模态完成，也不自动恢复实施。下列缺口继续保留，不表示下一片实施优先级，也不作为其他模态的前置；延期不放宽现行拒绝或授权边界。

## 语义与表示缺口

- **Text/Image/File 承载**：[Resource](../../src/semantic/task/generation/resource.rs) 的来源与类型化描述尚未闭合一般产物 identity、issuer-bound 引用及生命周期。标准 user inline 与 URL 文件分别以 [inline profile](../architecture-v2/responses-text-profile.md#user-inline-file-input)和 [URL profile](../architecture-v2/responses-text-profile.md#user-file-url-input)为界；issuer-bound 文件 ID、工具文件结果、资源操作及更多目标的文件载体仍缺主链。按[当前主线](../implementation-plans/next-goal.md)分别选片，不从共享 source 或文件输入推定所有用途已准入，不以 adapter 或私有字段补偿必要承载不足。
- **后续媒体与 Embedding**：[生成音频](../../src/semantic/task/generation/audio.rs)仍采用要求 reference/expiry/transcript 的固定 Chat 值组合；生成图片的流式产物与事件、vector 所需独立请求/结果合同尚未闭合；静态有序产物的独立 ImageGeneration task 不补足这些分支。[TaskKind](../../src/semantic/task/mod.rs)枚举不证明实现。这些缺口保留，但不作为本阶段 Text/Image/File 的前置。
- **标准客户端目标**：[HTTP 配置](../../src/gateway/config.rs)仍选择 MorphieCore 具名规则，reasoning/usage/cache 等差异与 Responses [本地兼容形式](../architecture-v2/responses-text-profile.md#control-message-and-annotation-admission-details)仍需逐项收敛。没有独立 `_openbridge` carrier 不等于已经完整标准化。[Chat 有损投影](../architecture-v2/protocol-and-lowering.md#semantic-loss)的具体白名单、观察和静态/事件实现尚需选片；标准缺少载体时不伪造字段。

- **交互与依赖主链**：跨 response 的逻辑 turn/continuation、跨协议 typed group 与非 reasoning attachment 的 replay 尚无完整主链。[消息组视图](../../src/semantic/task/generation/group.rs)、[pending-call 视图](../../src/semantic/task/generation/continuation.rs)、[本地 response 关联](../../src/semantic/task/generation/turn.rs)和[进程内依赖证明](../../src/semantic/task/generation/dependency.rs)以及[reported progress](../../src/semantic/task/generation/progress.rs)/[格式绑定值](../../src/semantic/task/generation/replay.rs)只是有界库级能力，不补足真实上游 turn 映射、全链身份、opaque/目标/执行权限的整体判据，或部分结果 history 的执行准入。结果关联齐备与 `Unreported` 都不能证明 turn 已结束或下一请求已就绪；具体边界见[continuation profile](../architecture-v2/responses-text-profile.md#response-outcome-and-continuation)。
- **Replay 最终化与凭据更新**：[事件模型](../../src/semantic/task/generation/event.rs)尚未独立表达 item 闭合后的 replay 产物最终化，或同一 owner 下完整回放凭据的更新/重签发合同；当前终态与已闭合 item 的 replay 不一致仍拒绝。来源/scope 相同不证明不同 opaque 值等价，也不能用覆盖 final 值、丢弃载体或恢复旧 fidelity 补偿。需先定清值/凭据身份、更新 authority 和依赖失效，再选最小语义事件及 wire 接线；文件输入准入不补足该合同。
- **分组与客户端 replay**：[客户端边界](../architecture-v2/client-generation-profile.md)没有 message-call membership、其他格式 replay、跨响应身份或 authenticated dependency proof 的私有位置。标准 Responses 无载体的关系仍拒绝；不得以邻接恢复关系或以删除 carrier 推定可丢失必要依赖。
- **内容、结果与资源**：[结构化参数/结果及独立执行报告](../architecture-v2/client-generation-profile.md)保留 typed owner，但无公开 wire 主链；工具图片仍无 Chat carrier。资源来源坐标、引用依赖、一般 file/resource ID 生命周期、完整工具媒体、结果 Schema adherence 与更广媒体输出/events 尚未闭合。[Chat 生成音频](../architecture-v2/chat-media-profile.md)仅提供有界库级值、控制、显式终态流和受信 scope 的引用投影；公开 Gateway 的无状态来源证明、资源保留/刷新及音频产品激活不由该切片补足。
- **Cache 与 usage**：[前缀证明](../../src/semantic/cache.rs)不承载 Provider 断点/TTL/远端缓存资源，也不保证 wire 字节或命中；[命名 usage view](../../src/semantic/task/generation/usage_views.rs)不是一般计费或完整模态分解。非 operation-final 或不满足标准关系的 scoped reports 没有公开客户端位置；不能合并成虚构 total 或补猜缺失计数，现行目标限制仍有效。
- **文本目标覆盖**：部分 Chat 控制与 history 尚未准入；reasoning、custom/program、phase、概率、reported context 和多 part grouping 并非都有对端位置。按实际消费需求核对[Chat](../architecture-v2/chat-text-profile.md)、[Responses](../architecture-v2/responses-text-profile.md)与[Schema](../architecture-v2/schema-profile.md)，不维护第二份字段清单，也不把单候选 profile 刻意拒绝的多候选当作待补缺口。Schema shell 可表达不证明 strict 缺省语义相同；共享 Schema/adherence 与 fixed profile 配额、reasoning 控制/预算、带单位引用坐标及更广 timestamp/缺失 metadata 的分离仍待后续切片；在具体 Chat 损失规则或其他目标合同落地前，不可表示时仍须拒绝。
- **状态 API 与更广执行域**：活动 continuation/conversation、store/background、资源操作、compaction、WebSocket、其他独立媒体 operation、hosted/dynamic tools 与其他 task family 尚无完整主链。inactive 形式、queued 事件、TaskKind 名称和选定图片输入均不能代表这些能力已实现；未知分支不能塞进 generic extension。Realtime 等详细设计按计划后置，不是当前主线退出条件。

- **独立图片生成产品接线**：独立静态图片生成的当前注册与显式激活需查[图片 catalog](../../src/topology/catalog/images.rs)及 bootstrap；真实账户准入与外部执行仍需独立验证。标准静态控制的表示不证明每个目标都接受，真实控制效果、大图片与跨目标预算仍需独立验证；图片编辑、URL 产物、流式图片与文件服务不由该切片推定。当前接口与资源边界归 [HTTP 指南](../http-gateway.md#独立图片生成)和 [Images codec](../../src/protocol/openai/images.rs)。

## 扩展与执行缺口

- [CustomSections / CodexHeaders](../../src/protocol/extensions.rs)只是低层 carrier；[Adapter request](../../src/adapter/request.rs)限制 body sections，Codex headers 未接入 HTTP 主链。响应自定义段、typed observation headers、body/header 一致性、版本与 turn 生命周期仍需定稿。
- configuration/program/cache 的表示不授权应用设置、执行 program、管理 turn 或扩展 prewarm 执行。Continuation 不提供自动 Agent loop。
- [凭据管理](../credentials.md)与固定 Route/pool 前移不提供请求内登录、自动 refresh/轮换、动态 registry、同候选 retry、负载均衡、健康调度或 session affinity。更广失败分类与长期调度须独立合同；未知 scope 的 429 不授权换凭据。
- canonical model 相同不证明跨候选 opaque replay 安全；内部 scope 也不是客户端 token 的 issuer 真实性证明。多成员入口尚无 client-carried affinity，source-bound replay 仍受现行拒绝边界约束。
- 最小 loopback 网关与有界 probe 元数据不等于生产观测、负载或长期资源保障。

## 验收缺口

固定标准 union 尚无完整逐分支 required/null/跨 kind 与组合审计。独立回归入口见 [semantic](../../tests/semantic.rs)、[transport](../../tests/transport.rs)、[credential](../../tests/credential.rs)与[gateway](../../tests/gateway.rs)；存在测试不证明完整覆盖或最近执行通过。

[固定 SDK gates](../../tests/sdk_loopback.rs)单独显式运行，不随默认 Rust tests 执行。一般 SDK/Agent、真实 Provider/TLS/网络、缓存收益、输出质量、负载和生产稳定性，以及凭据存储的原生平台/文件系统/ACL/断电保证，都需要各自证据，不能由 synthetic loopback 或平台库存在推出。方法归[开发指南](../development.md)与[验收基线](../references/conformance-baseline.md)，结果只在当次交付报告。

新问题区分错误接受、未准入、不可表示、未接线与缺少验收。获准行为修复先写入[current-focus](../implementation-plans/current-focus.md)，再以最低 owning layer 的独立反例保护；闭合后移除对应缺口，不追加完成日志。
