# Agent-first、协议中立的 IR 设计

这是跨协议语义的设计基线，不是当前 Rust 类型、codec 或执行能力已完整落地的声明。项目尚未上线，处于设计探索阶段；稳定目标是概念、所有权和不变量，不是现有 struct 或 SDK DTO。当前实现边界见[Generation 缺口](../implementation-status/generation.md)，设计推进顺序见[next-goal](../implementation-plans/next-goal.md)。

## 1. 设计目标与参考边界

**以一套语义权威表达 Agent 交互中的内容、行动请求、结果、控制转移与续轮依赖，再按目标协议投影。**

- OpenAI Responses、Google Gemini 和 Anthropic Messages 是设计参照与映射对象，没有一家协议决定 IR 的表达力上限。[Responses 固定来源](../references/upstream-sync.md)仍约束其 codec；其他协议入口见[官方来源](../references/providers/README.md)。参考资料不证明本地支持。
- 保留有序异构 item、稳定 identity、显式状态与 Static/Event 一致性，不照搬任何一家的 envelope、union 或消息容器。
- 不采用多协议最小交集、字段机械并集、任意 JSON 透传或第二套 Provider IR。一个目标不可表示时，拒绝或采用明确获准的具名转换，不能削弱共享语义。
- Agent-first 表示交互，不实现通用 Agent 调度器。表示工具、continuation 或上下文编辑，不授权执行工具、自动续轮或管理远端状态。
- 一套 IR 是共享原则与值类型下的 task family，不是所有任务共用一个万能请求。Generation、Embedding、ImageGeneration、Speech 等分别拥有请求、响应与事件合同；task、modality 与 wire 分开。

架构评审优先级：语义完整性与一致性 → 跨协议映射 → 变换及生命周期可验证性 → wire 和当前实现便利性。未发布类型可以在获准实现切片内替换，但不得以设计更新冒充实现完成，也不为未来能力预建空模块。

## 2. 语义核心、能力域与扩展

| 层次 | 归属原则 |
|---|---|
| 稳定共享语义 | identity、顺序、归属、调用与结果关联、生命周期、来源与依赖；概念必须能脱离某一 wire 独立定义 |
| Typed 能力域 | reasoning、工具、媒体、Schema、cache、context 等有独立约束的子域；属于同一权威，不要求塞入一个大对象 |
| Scoped extension | 仍具有特定 issuer/profile 含义的能力、opaque 值和资源规则；显式 attachment、类型、版本、生命周期与目标策略 |
| Representation fidelity | 等价拼写/形式、wire identity、来源与依赖证明；不是第二份正文，也不承担未建模行为的透传 |
| Execution | 凭据、已选 Endpoint/URL、socket、attempt/retry、downstream commit、实际工具及状态服务 |

某概念是否进入共享域，取决于语义是否明确、能否验证，不取决于已有几家提供，也不取决于 OpenAI 是否公开了同名字段。字段同名不证明等价；语义等价也不要求 wire 同形。已有共享 owner 不因新协议缺少位置而降为扩展或复制一份。

扩展仍是第一等 typed 表示，参与 validation、requirements、变换和 lowering。将扩展提升为共享能力前先证明含义、生命周期与 presence 等价，迁移到唯一 owner；不得只因另一家采用相同名称便合并。

## 3. 表示范围与唯一所有权

以下是概念职责，不是拟定的 Rust 文件树或公共序列化格式：

```text
Generation representation
  task semantics
    instructions + ordered interaction items
    generation / reasoning / tool / output constraints
    typed grouping and call / result / resource relations
  request context
    model binding label + state/resource intent + cache/service/safety hints
  response semantics
    ordered output + result/completeness + continuation requirements
    reported usage/context
  delivery intent
  owner-bound typed extensions
  representation fidelity and provenance
```

请求 hints 与响应 reported facts 分开；不能复制请求设置来补响应事实。Instruction echo 不形成第二份正文。Logical session、thread、turn、cache affinity、response/resource reference 也不互为别名；Codex 等特定上下文的出处见[上下文来源](../references/extensions-and-context.md)。

真实 credential、credential locator、已选上游目标、retry counter、commit flag 和可执行脚本不进入语义表示。可信 provenance label 可以限制 replay，但不选择或授权网络目标；业务 JSON 不能自证 issuer。

## 4. 有序交互与有限关联

### 概念边界

| 概念 | 含义与边界 |
|---|---|
| Item | 有独立语义和生命周期的有序单元，例如 Message、Reasoning、ToolCall、ToolResult 或有合同的控制项 |
| Message / Part | 发言的语义角色及有序内容；不是工具、reasoning 或控制项的万能容器 |
| Response | 一次生成操作报告的输出和状态；不等于 HTTP 连接，也不保证逻辑 turn 已结束 |
| Logical turn | 与当前任务有关的交互连续性范围，可跨多个 response 和工具交换；不是网关 session 对象 |
| Group | 协议或能力要求共同投影/回放的一组 owner；不因相邻或 role 相同就自动建立 |
| Call / Result | 明确身份与关联的行动请求/结果；执行方、调用者与 wire role 分别表达 |
| Resource | 有独立来源、用途与生命周期的内容或引用，不是可自由迁移的字符串 ID |

采用**有序记录 + 必要的 typed 关系**，不采用任意可执行 DAG。分组只在存在具体语义依据时建立；不得假设三家的 message、step、item 与 turn 一一对应。工具结果装在 user message 中不改变它的结果身份；文本和调用装在同一 Part 数组中也不应丢失各自 owner。

保留 instruction authority、作用位置、assistant phase 与 item status。不能将 system/developer 及中途指令任意拼接为一个字符串，也不能以 role 推断执行者。Configuration update 和 compaction 是有序控制含义，不是任意设置 patch 或普通摘要文字；其有效范围与历史替代关系须由对应能力合同定义。

稳定 local ItemId/PartId/ResourceId、wire item ID、call ID、response ID、turn/session 与 stream lane identity 分开。索引是坐标，不是 identity。不要求每个协议提供 turn ID；未知身份不伪造为上游报告。必要的本地合成身份必须与上游 identity 区分，不能修复缺失的 wire 必填字段。

## 5. 响应结果、控制转移与续轮

至少分别建模并校验：

- **响应生命周期**：当前 response 的开始、进行、闭合；transport 是否正确终止另外验证。
- **产物结果与完整性**：完整、截断、失败、取消及相应原因；refusal 内容/决定与传输错误不混同。
- **交互进度**：逻辑 turn 已结束、等待外部结果、需要继续生成，或协议未报告。未报告不能推断为完成。

这些不是可以任意组合的布尔值，必须有合法组合约束。一个正常闭合的响应可以要求工具结果或 continuation；一个完整的工具调用描述不证明工具已执行。工具执行错误可以作为有效结果供模型继续使用，不自动升级为 response failure。

Continuation 表示下一次交互的要求和依赖，例如待回应 call、必须保留的内容组/opaque 值、有效 scope、工具或设置约束。它不是自动发送请求的命令、重试许可或脚本。语义层验证这些要求；执行/调用方另行决定是否继续、预算与权限。

[显式 response 续轮检查](../../src/semantic/task/generation/turn.rs)以调用方提供的 local `TurnId` / `ResponseId` 关联不可变 response，不将它们冒充上游报告。它检查最终 history 中完整输出的 owner、值、顺序、声明的消息组成员及调用对应结果；进行中结果保持未知，齐备仅报告 `ResultsComplete`，不报告工具成功、产物完整、turn 完成或整体执行就绪。显式后继关联只校验当前及直接前驱 identity，不管理全局 identity、session 或调度；调用方负责全链唯一性及其他依赖。既有 wire 不承载这些本地关联，也不因这项只读 API 扩大准入。

单 response reducer 终止后不可复活。后继 response 可以延续同一逻辑 turn，但应以显式关系关联，不能拼接进前一个 response 伪装成一次成功，也不能借 continuation 绕过提交后的禁止 fallback 边界。无状态完整历史、远端 response/conversation 引用、store/background 分别建模；表示它们不隐式启用存储或远端状态解析。

## 6. Reasoning 与 source-bound replay

Reasoning 分为独立维度：启用/自适应等模式、effort、数值预算、公开内容的显示/摘要意图、实际可读内容、reported usage，以及 opaque continuation。预算不是 effort 别名；隐藏摘要不等于未推理，也不意味着没有续轮数据。

Readable reasoning/summary 与普通 assistant text 分开。Opaque 值可以没有可读伴随内容；回放 attachment 也不只限于 reasoning，可能属于 part、call、resource 或有明确合同的内容组。不同协议的 encrypted reasoning、signature、redacted block 与 turn-state 保留不同类型，不能因都是字符串而互换。

共享的是 replay 约束：owner、可信来源/兼容 scope、生命周期、partial/final、可见性、大小预算，以及对正文、顺序、分组或设置的依赖。只绑定单个 owner 不足时，依赖必须覆盖所要求的关系；不能声称本地 fingerprint 已验证 issuer 的密码学真实性。

Opaque 值由一个 owner 持有，来源/依赖 sidecar 不得另存一份可覆盖它的权威值。删除或修改依赖后旧值失效；编码不得从 fidelity 恢复。只有 owner 和 token 都满足最终性与目标合同才允许 replay。下游交付→客户端保留→下一请求必须能闭合；只在本次请求内保存 sidecar 不构成无状态续轮支持。

现有 Responses encrypted replay 的实现归[ADR 0006](decisions/0006-reasoning-ownership.md)及对应 profile；其他 opaque 种类需独立设计，而非套用 Responses 的事件最终性规则。

## 7. 内容、工具、资源与引用

文本、refusal、reasoning、媒体和结构化结果分别有类型。共享媒体来源值不抹平 user perception、tool result、reasoning artifact 与生成媒体的不同用途。资源身份、inline/URL/issuer-bound reference、MIME/编码、所需描述与处理意图分别归属；细节见[资源边界](../references/multimodal-and-resources.md)。

工具声明、选择策略、调用、执行结果与进度分开。Client-executed 和 upstream-executed 工具保留执行方；结果按 call identity 关联而不是按名称或邻接位置猜测。不同种类 hosted/custom/function tool 的专有含义不能强制降成普通函数。工具内容可为文本、结构化值或有序媒体，但只接受所选能力的闭合类型，不允许任意递归容器或嵌套可执行调用。

工具参数区分原始文本/语法输入、结构化值与尚未完整的片段。原始参数字符串若是协议的权威值，就不能被 SDK parsed view 覆盖或重序列化替换；结构化输入同样保留精度和其合同要求的顺序。解析视图与原值不能独立修改形成双份权威；从片段转为完整值需要相应能力的验证，不猜测补齐。

引用同时描述**输出 owner/claim 与来源位置**。来源使用稳定 resource identity；字符、字节、页码、时间或内容块坐标必须带明确定义，不能混用输出偏移与源文档偏移。Wire 文档索引由最终顺序投影；源删除、替换或内容编辑后，引用须重验或失效。不能凭缺失位置猜测精确范围。

下载、redirect、上传、转码、扫描、资源授权与保留由资源/执行服务拥有。纯 codec 不获取资源，不将 transcript 当作原媒体的无损替代，也不因已有资源 ID 而假定另一目标能访问。

## 8. 控制、Schema、缓存与计量

### Presence 与 Schema

逐字段定义 Absent、Null、Value 及空/false/default 的区别。外层缺失不能隐藏有效子字段；协议省略默认值只有在含义等价时才可归一化。数值保留精度，不以通用 float 或零填补未知。

Schema 是带方言、顺序、严格性和有界引用关系的约束，不是无序 JSON。区分声明结构有效性、目标可表示性与生成结果 adherence；工具输入约束与最终输出约束不混同。同名 strict 不保证相同含义。目标不支持时不得删除约束或暗改 required。当前实现规则由[schema profile](schema-profile.md)维护，不是未来共享域的永久上限。

### 缓存意图

区分自动缓存亲和提示、前缀断点/策略、远端缓存资源引用和实际命中事实。已理解的缓存意图有 typed context/attachment owner，不只作为可丢弃 fidelity 保存；具体 TTL、前缀范围、投影顺序和可省略条件属于对应能力/profile。

缓存断点可能依赖整个先行前缀，而不只依赖被标记 part；前缀内容、工具/Schema 顺序和有效设置的变化必须反映到投影及依赖检查。不能把缓存 key、logical session、资源 ID 相互派生。Provider-owned cache 不变成网关回答缓存、负载均衡或粘性路由，见[ADR 0011](decisions/0011-stable-admission-provider-cache.md)。

### Reported usage

共享计量语义应明确计数单位、范围、总量/细分关系、重叠或独立性、报告最终性及缺省含义，不维护 Provider 专属 Usage。Token、工具次数、费用及可见正文长度不是同一种计量。

原始报告与合法派生视图只能有一个权威来源；派生需有命名公式、完整前提和 provenance，不能双存可独立修改的 totals。不存在跨协议通用的原始字段加法公式。累计快照不能逐事件相加；缺省/null 未报告值与显式零分开。来源不足时保留未知或拒绝所需投影，不从正文、请求或重叠细分猜测总量。当前 `Usage` 的字段与校验只是实现 profile，不证明所有计量关系已可表达。

## 9. 变换与依赖合同

最终 typed 值始终是编码权威。每种变换须规定依赖影响，而不是任意保留 source JSON：

| 变换 | 必须维护的约束 |
|---|---|
| 插入 | 新 owner 获得新 identity；检查分组、前缀及引用关系，不继承旧坐标的 metadata |
| 重排 | 保留 local identity，重算 wire 坐标；重验顺序/分组依赖，必要时拒绝 replay |
| 替换正文/资源/参数 | 使依赖旧内容的 signature、annotation、probability 或引用失效，除非有明确可证明的保持规则 |
| 删除 | 删除 owner 附着值；悬空调用、引用或组约束须显式修复或拒绝，不偷偷恢复或级联丢弃其他语义 |
| 修改控制/上下文 | 重算 effective settings、continuation/cache/resource 要求，不能把旧 reported fact 当新设置 |

[进程内依赖证明](../../src/semantic/task/generation/dependency.rs)提供显式 message group / prefix-through-owner 范围及可选 settings 绑定。证明只保留摘要；组外变换或前缀尾后 append 不影响所选范围，范围内的值、顺序、成员、identity 或绑定设置变化必须重验失败。Schema 属性顺序及 redacted opaque/资源正文均参与摘要。范围由能力合同选择，不从相邻 item 猜测；证明不授权 replay，不构成持久化或客户端 carrier。

变换后重新验证整体语义并派生 requirements；每个固定候选从相同不可变输入独立 lowering。若需损失转换，必须具名、限定前提、显式获准并说明可观察后果；没有泛化 `best_effort`。

## 10. Static / Event 与交付

每个准入能力同时定义 request/history、static response 和适用的 event 合同。Event 表达 typed transition，不复制 raw SSE/WS envelope 为权威。值内容停止产生、值完整、类型有效、item 完成、response 终止及 replay 就绪是不同事实，不能互相推断。

部分工具参数允许处于尚未完成的表示状态，但不能被视为可执行值；需要完整结构的目标须拒绝不完整投影。流结束不能修补 JSON，terminal snapshot 不能掩盖缺失的必需事件或推翻已交付语义；合同允许仅在终态出现的报告不属于补救缺失。Signature、citation 和 usage 的增量/最终报告各自按所属合同累积与封闭。

合法事件 materialization 与静态语义一致；错误后的 reducer 不可继续。SSE framing、WebSocket lane multiplexing、backpressure、取消和 I/O commit 在外层。表达 hosted-tool progress 不授权网关执行工具。

## 11. Scoped extension 合同

每项扩展至少定义 namespace/kind/version、typed payload 或有 schema 的 bounded opaque value、attachment、可信来源、生命周期、partial/final、可见性/敏感性、资源预算、依赖与失效规则，以及目标映射/拒绝条件。未知 JSON 不自动成为扩展；诊断保留和发送给 Provider 是不同权限。

不允许覆盖已有 owner、注入 auth/target/script，或仅因目标同名便跨 issuer replay。起步用编译期闭合合同，不预建动态插件平台。具体 downstream carrier 和客户端回传合同须在相应实现切片前定稿。

## 12. 设计定稿与实施边界

评审一个能力时，必须能指出：唯一 owner、概念/类型边界、合法状态组合、依赖集合、presence、Static/Event 关系、变换后行为、目标可表示性和执行权限边界。未知事项显式保留，不先造万能类型。

优先定稿交互/continuation 与 replay 依赖，再展开内容/资源/工具结果、cache/usage 和上下文演进；具体产物和待选边界由[next-goal](../implementation-plans/next-goal.md)维护。独立映射、变换与失败证据的方法归[验收基线](../references/conformance-baseline.md)，不以同一 encoder 的 round trip 自证。

当前 Chat/Responses/Schema profiles 与源码是现有准入边界；本设计不自动扩大它们，也不将它们的限制提升为架构上限。实现新 slice 前固定相关官方 API/schema/SDK 版本与具体 carrier，更新受影响类型、序列化、profile、独立预期和必要公共合同，再按[current-focus](../implementation-plans/current-focus.md)推进。文档定稿不授权 live 调用、工具执行、凭据操作或服务部署。
