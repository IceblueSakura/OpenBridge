# Responses-first IR 与扩展设计

这是 Generation IR 的设计基线，不是当前 Rust 类型已完整落地的声明。主要依据为 [Responses 标准语义](../references/responses-standard.md)、[历史调研综合](../references/semantic-baseline.md)及[固定上游快照](../references/upstream-sync.md)。当前缺口由 [migration](migration.md)维护。

## 1. 核心决定

**以 OpenAI Responses 的有序 item、content、tool、reasoning、状态与 event 定义作为 Generation 的语义主干，结合显式、受约束的扩展表达特殊能力。**

- 不以 Chat 或多协议最小公分母限制 IR 表达力。
- 不把 SDK DTO/原始 JSON 直接作为 IR；core 类型自行拥有验证、presence、identity 和变换不变量。
- 标准已经定义的能力进入标准域；当前 codec 不支持不构成降格为 extension 的理由。
- Chat、其他 wire 与 Provider profile 是对同一 IR 的映射；不可表示时明确拒绝或使用获准的命名转换，不能削减 IR。
- 任务、上下文、交付、来源保真和真正的执行状态仍分层。Responses-first 不把所有字段塞进 Message，也不把全部任务强塞进 Generation。

## 2. “IR” 的范围与层次

内部请求表示由下列有明确 owner 的部分构成，具体 Rust facade 名称在实施切片中确定：

```text
Generation request representation
  standard task semantics
    instructions + ordered input items
    generation/reasoning/tool/output controls
  standard request context
    model binding label + state/resource intent
    cache/service/safety/metadata hints
  delivery intent
    JSON / SSE / WebSocket operation and lane intent
  scoped extensions
    task/item/part/resource capabilities
    request/session context extensions
  fidelity
    equivalent spelling/form, wire identity, owner-bound opaque replay
```

这些部分共同构成内部请求的权威表示。`session_id` 一类扩展可以属于 **IR 的上下文扩展层**，不再用“不是 task content”当理由将其整体排除在 IR 设计之外。

但这不包含运行时对象：真实 credential、选中的 upstream URL/EndpointId、socket、retry counter、downstream commit flag 和可执行脚本不进入 IR。Provider namespace、可信 origin label 与已经选定的路由/网络目标不是同一概念。

响应相应拥有标准 ordered output、status/details/usage、reported context 与 scoped output extensions。request hint 不能冒充实际 response fact。

## 3. 标准语义域

### Ordered items 与内容

Message 保留 role、phase、item status、ordered content；Instruction authority 与位置不被统一拼接成一个 system 字符串。Tool call/result、Reasoning、approval、compaction、configuration update、resource reference 等按 Responses union 分别建模，不伪装成 message。

`phase` 是标准 assistant 语义，不是单纯 UI 标签或 Codex 私有 metadata；`commentary` 与 `final_answer` 不能合并后丢失。标准 configuration update 是有序历史中的控制变化，影响后续 effective settings，不是任意 JSON 配置 patch。

文本、refusal、image、file、引用/概率，以及标准工具结果保持各自类型。特殊音频/视频等经明确扩展表达；媒体来源值可以共享，但任务/用途/生命周期不折叠。

### 控制与 presence

按字段表达 Absent、Null、Value，以及显式空/false/default 的差别。只有来源证明等价时才规范化。受验证的设置不能同时声明“容器不存在”与“子字段存在”；TextOptions 的 presence=false 必须同时要求 format 和 verbosity 为 Absent，包括不能隐藏显式 Null。

Schema 不是通用无序 JSON：保留定义的属性顺序、strictness、固定方言、局部引用和有界图结构；当前共享验证与默认模式见 [schema profile](schema-profile.md)。不在 pure codec 下载 `$ref` 或执行 schema 程序。声明结构验证、目标 strict 子集准入与最终输出 adherence 各有独立责任。

### State 与资源意图

无状态完整历史、previous response、conversation、store/background、prompt、compaction 等标准意图应有可表达的位置，不永久用 unit/null stub 代表。纯 codec 不解析远程 state，也不隐式开启存储；实际操作需要对应执行/权限 owner。

“当前只支持 stateless profile”是阶段性实现限制，不是标准目标设计边界。

## 4. Identity 与依赖

稳定 local ItemId/PartId/ResourceId、wire item ID、call ID、response ID、conversation、session/thread/turn 与 WS stream ID 分别建模。索引是 wire 坐标，不是语义 identity。

- 重排保持 identity；新对象获得新 identity，不能继承旧位置的 metadata。
- 删除 owner 就删除相关输出，fidelity/extension 不可恢复旧值。
- 修改正文会使旧 annotation offsets、probabilities 或 signature 失效，除非有明确可验证的保持规则。
- output item done 和 response terminal 是不同事实；完整上游对象缺必填 id/status 不可通过合成掩盖。
- 明确自主构造输出时可分配新 wire ID，但应视为合成表示，不伪称保留了上游 ID。

## 5. 扩展合同

扩展是标准域之外的第一等内部表示，但默认不可随意迁移。每一项至少具有：

| 维度 | 要求 |
|---|---|
| 身份 | namespace、kind、schema version；不覆盖标准字段 |
| Attachment | request context、item、part、resource、event 的明确 owner |
| Payload | typed 内容或已定义 schema 的有界 opaque value，不是任意 `extra_body` |
| 来源 | 可信 origin、profile/scope 与信任来源；客户端不能自证 issuer |
| 生命周期 | session/thread/turn/response/item、partial/final、replayable 或不可 replay |
| 可迁移性 | 显式目标映射、同 scope 保留或拒绝；不能只凭两个 Provider 都叫 Responses |
| 安全 | visibility、敏感性、预算、禁止 auth/target/script override |
| 变换 | requirements、owner 修改/删除、fallback/重试的失效规则 |

已理解的特殊能力以 typed extension 建模；未知字段不是自动可执行 extension。受限诊断保留与重新发往 Provider 是不同权限。起步使用编译期闭合类型/注册合同，不建立通用动态插件平台。

当扩展语义被标准化，迁到标准 owner；兼容 wire spelling 留在 profile codec，不保留双份语义或 legacy alias。

## 6. Codex 与特殊多模态

Codex context 至少区分 logical session、cache affinity、thread、context window、turn-state 和 agent lineage。`session-id` 在固定 ChatGPT profile 上可能是 cache affinity 的投影，不能直接等同内部 logical session。

turn-state 是 server-issued opaque context，有同 turn、auth owner 与来源限制；不由 session UUID/hash 派生。body `client_metadata` 和 headers 是同一上下文的投影，而非多个独立权威。没有真实 owner 时不伪造 thread/turn 字段。详细事实见[上下文基线](../references/extensions-and-context.md)。

多模态标准 image/file 优先进入标准 content；Provider-specific speech/video/config 进入对应 task/part/resource 扩展。专用 Speech、Embedding、ImageGeneration 仍是 task family，不因 wire 使用 Chat/Responses 改成聊天任务。来源安全与验收见[媒体基线](../references/multimodal-and-resources.md)。

## 7. Reasoning 与 opaque 值

request effort/summary/context/mode、readable reasoning/summary、reported reasoning usage、encrypted/signature replay 分开。标准允许值与具体模型支持分开，不以 Chat 的可表示性收窄 Responses。

Opaque 是有类型且受约束的值，不必全部塞进任意扩展：标准 encrypted reasoning 保持标准字段的所有权，纯表示来源约束由 sidecar 记录；其他 Provider signature 由 scoped extension 持有。核心不能解密、重签或用旧来源恢复被删除值。

当前 encrypted replay 采用 [reasoning ownership](decisions/0006-reasoning-ownership.md) 的 owner/origin/finality 规则；其他 opaque 类型需逐项建立合同，不机械共用 token 字符串。

## 8. Static / Event 与 transport

同一标准分支的 request、response、event 要闭合。Event IR 表达 typed transition，不存 raw SSE envelope 作为权威。纯 reducer 管理单 response 的 identity、partial state、usage、terminal 和 materialization。

HTTP framing 与 WS multiplex/steering 在外层：每个 lane/response 分派到对应 reducer。连接还活着不等于当前 response 未结束；当前 response incomplete 后继新 response 不允许复活已经终止的 reducer。

支持标准 hosted-tool progress 不等于 Gateway 执行工具；表示中保留执行者与生命周期，实际 tool orchestration 单独授权和实现。

## 9. 验收与实施边界

按照 [验收基线](../references/conformance-baseline.md)逐域实现，先建立独立 wire/IR oracle，再实现与变换、失败反例。测试覆盖不足只能说明验收缺口，不能成为永久缩减 IR 表达力的理由。

downstream 扩展的 wire 位置/namespace 版本、Codex turn 管理模式、特殊多模态具体 profile、完整 state 解析执行需在对应实现切片前定稿。设计目标不代表类型、codec 或执行已经实现；具体缺口由 [migration](migration.md)维护，当前切片及完成条件由 [current-focus](../implementation-plans/current-focus.md)维护。
