# Generation 交互合同

本页维护 Agent-first、协议中立 Generation 的跨模块方案、必要理由与实施边界，不是某个 SDK DTO、代码审计或执行报告。[Semantic IR](semantic-ir.md)拥有总体原则；本页细化交互、值权威、关系、报告与目标投影，实施差距仍由[Generation 缺口](../implementation-status/generation.md)维护。

采用路线是先完善共享 IR，再接入 Gemini Developer API 的原生 Interactions 推理。账号管理、Google OAuth、Vertex、订阅管理与真实凭据迁移不在此路线内。必要 API key 请求认证复用既有受信机制，不成为语义数据。

## 1. 表示结构与边界

```text
GenerationRequest
  History: ordered items + finite typed relations
  GenerationIntent: generation/tool/reasoning/output constraints
  RequestContext: state/resource/cache/identity intent

GenerationResponse
  Output: ordered artifacts and admitted actions/results
  Conclusion: operation result + reported interaction progress
  ReportedFacts: scoped usage and context

Decoded<T>
  final typed values
  bounded representation records
  trusted replay/dependency bindings
```

这是概念职责，不预先固定文件树或公开序列化格式。继续使用有序异构 items，不将它们替换为 Google `steps`、万能 Message、字段并集或可执行 DAG。

请求意图、上游报告、派生事实与执行授权分别拥有权威。业务输入不能指定目标 URL、凭据、脚本或可信 issuer。每个固定候选从同一个不可变最终输入独立投影；同协议也没有绕过 IR 的旁路。

## 2. 响应结果、交互进度与续轮

| 维度 | 合同 |
|---|---|
| 响应生命周期 | 操作开始、进行与闭合；transport EOF 在外层验证 |
| 产物结果 | 完整、截断、失败、取消与对应原因 |
| 交互进度 | turn 已结束、等待外部结果、需要继续生成或未报告 |
| 续轮要求 | 下一交互所需 call/result、opaque、分组、前缀及上下文约束 |
| 执行许可 | 调用方另行决定是否执行工具、发送请求及预算 |

不在 outcome 中加入混合维度的 `RequiresAction`。已闭合响应可以提供完整工具调用并等待外部结果；工具错误可以是有效结果而不是 generation failure。收到完整参数不证明工具已执行。

区分上游明确报告的 progress、从最终 history 派生的 pending-result view，以及检查要求后的 readiness。派生视图借用唯一值权威，不另存可独立修改的 pending-call 列表。所有结果已出现不等于成功、产物完整、turn 结束或整体执行就绪；无报告不推定结束。

合法状态组合由共享 validation 与具体 profile 分层约束。至少保护完整调用等待结果、部分参数不可执行、完成 reasoning owner 后响应截断、结果齐备但其他依赖未满足，以及 progress 指向不存在调用的矛盾。

变换不能把旧报告悄悄改成新观察。删除调用后留下的等待关系须显式修复或拒绝；修改产物不复活已终止 reducer，也不改写 transport 失败为语义成功。后继 response 独立闭合，可由调用方显式关联；不建设 session manager 或自动 Agent loop。

## 3. 工具参数的唯一值权威

完整参数区分原始文本与结构化 JSON。Raw arguments 的文本是权威，解析 view 只能派生；structured arguments 的 JSON 值是权威，不再保存可独立修改的同义原文。

原文完整、JSON 语法有效、符合参数 Schema与获得执行许可是不同判断。不能因 model 报告了已结束的字符串，就把它当作可执行值；不能将合法 JSON 值自动推定为对象，具体目标的 object 要求留在该 profile。

增量片段是值的构造过程。片段尚未闭合时不能作为完整 structured value；完成须经过有界、严格解析和目标 shape 检查。空对象参数与 opening shell 分开，不能从初始 `{}` 猜测调用已完成。

静态值与增量 materialization 保持一致。最终快照不能改写已发布参数；必要的延迟只允许在单值预算内，不收集并回放整条 SSE。来自 string 参数的非法 JSON仍是原文报告，是否允许解析投影由明确目标合同决定，不自动修补。

数值必须定义精度边界。结构化工具值保留整数、十进制与对象顺序，不先舍入后称原始报告。使用成熟解析库的精确 number 能力，并在进入 Value 前解决 duplicate keys、深度/nodes 与字节预算；不能让特殊内部 number 表示把普通对象误解成数字。

## 4. 工具结果与执行报告

工具结果分别拥有 call 关联、结果值、execution report 与 artifact lifecycle。Text、结构化 JSON 与有序内容 parts 是值的不同形式；错误不是排除其他正文的另一种 payload。

允许执行失败同时携带结构化诊断结果。执行成功不保证产物完整，有结果值不保证已报告执行成功。错误结果关联齐备可以满足待结果关联，但不授权继续操作。

Result name 若由协议报告，不能替代 call identity；与已有调用不一致须拒绝。String、空值、单 part、空数组及多 part 的区别按字段合同保留，不拼接或隐式 stringify。

没有 execution-report carrier 的目标拒绝该报告，不能丢掉 error 或 success 标识；没有结构化 carrier 的目标拒绝相应值。表示能力与 Public Model 准入分别检查。

## 5. 身份与有限关系

分别建模 message ownership、operation association 与共同 replay group。它们可以重合，但不能互为别名。关系只解决声明的 membership、call/result、共同回放、prefix 或直接后继要求，不成为通用工作流图。

一项 membership 只有一个权威位置，其他 group views 由它派生；不能维护两份可分别修改的同义成员表。不同含义的组可以有独立关系，但不得从邻接、role 或 step 名称猜测。

Local ItemId/PartId/CallRef、wire call/item ID、step index、resource ID 与 caller turn relation 分开。Index 是坐标，不是身份。Wire ID 的唯一性与关联范围由所选合同确定；跨 history 歧义不能用本地合成身份掩盖。

重排保留 local identity，但重算 wire 坐标并重验顺序依赖。删除、替换或成员变化要求显式修复或拒绝，不能附着到同位置的新 owner。标准 Responses 无 message-call membership carrier 的现有拒绝继续有效；更完整的 IR 不自动增加目标可表示性。

## 6. Typed replay 与独立证明

Replay attachment 拥有格式、唯一 typed/opaque value、owner、partial/final、生命周期及可见性；binding 拥有可信兼容 scope 与依赖证明，不另存正文。Responses encrypted content、Google thought signature 与其他 signature 不因是字符串而可互换。

共同约束可用于 reasoning、call、part 或声明的 group，但只实现已选 attachment，不预建万能容器。Scoped 格式可以有 issuer/profile 含义，不允许选择 credential 或 upstream target。

最终性由格式合同决定。收到 signature、value final、owner completed 与 response closed 分别验证；不能将 Responses item-done 规则复制给 Google step。Opaque-only 内容合法与是否满足必要 replay 分开。

依赖可以涉及 owner 内容、成员与顺序、所选 prefix、工具/Schema、有效设置及资源值。固定能力合同选择范围；业务 JSON 不能传 arbitrary selector 或降低依赖。不要默认永久冻结全部历史与设置，也不要声称选定保守策略就是已验证的上游密码学依赖。

删除 owner 或 value 不得从 fidelity 恢复；修改依赖后旧值不可 replay，除非有显式保持证明。缺少证明不代表可以丢值。缓存证明与 replay binding 复用必要方法而非混用 scope 或 ID。

### 验证层级

1. 当前进程中变换前后的 typed 依赖一致性。
2. 交付给客户端并回传后的完整性。
3. Issuer 对 opaque 值及其用途的真实性验证。

这三层不能互相推导。对客户端修改后的 history 重新计算 hash，不能证明等于原签发内容。跨请求完整性若由网关保证，需另行定稿 authenticated carrier 或受信保留状态；普通摘要和客户端标签不够。

进程内 Debug fingerprint 不是持久化格式。需要跨版本证明时另行固定版本化、有界、域分离的编码，保留合同要求的 JSON/Schema 顺序和 opaque/resource 字节；不能泄漏正文、endpoint 或 credential locator。

## 7. 有范围的计量报告

Usage 描述 scope、basis、单位、计数域及关系，不维护 Provider 专属 Usage。Operation、item/step、session 不是同一范围；delta、累计 snapshot 与 final report 不是同一 basis。

Input、生成输出、thought、cache、模态及工具计数分别明确含义；子集、重叠、互斥与是否穷尽由 typed 合同声明。不存在由字段同名自动得到的通用加法式；不只删除校验，也不引入任意公式 DSL。

Report 保留唯一权威，派生 view 只借用报告及命名公式。相同 scope/unit、明确关系及完整计数前提满足后才允许派生；未知值不补零，兼容默认遵守 [ADR 0008](decisions/0008-stable-core-and-vendor-adapters.md) 的来源与 audit 边界。派生数不是原 wire 字段或实测费用。

同一累计报告替换而不是累加；step report 不自动加入 operation total。Event 需区分报告范围和封闭，不能以一个只允许一次的 Usage 事件代表所有报告。报告不能撤回已交付事实；source 不足时保持未知或拒绝所需投影。

## 8. Schema、控制、内容、引用与 metadata

Schema 值/方言/引用结构、adherence 或 normalization 意图、声明信息及目标词汇/额度分开。源 strict 默认可能改变行为，不能整体移入 fidelity；decode 明确其有效意图，表示记录仅保留形式。目标无法提供同等含义时拒绝，不删 required 或补默认来兼容。

复用现有有界结构、引用和 pattern 验证，不建设第二套 Schema engine。Wire shell 的必填 name、strict 词汇及固定配额不成为共享概念的永久限制；既有 profile 仍须独立保持严格。

Reasoning mode、effort、数值预算、display/summary 与 continuation 要求分开。Minimal 不自动等于关闭；禁用 summary 不代表无需 replay。格式专属 encrypted-output 开关不能成为所有 opaque continuation 的通用表示。

共享内容值不抹平 user 输入、model artifact、reasoning summary 和 tool result 的用途。现有文本/图片切片不代表完整 media union；未实现域明确拒绝，不压成字符串或任意 JSON。

引用区分输出 claim 与来源坐标。Byte、字符、页、时间与 block 坐标必须有单位；跨单位只允许可证明的精确映射，落在 UTF-8 中间的偏移不能近似转换。Source 重排重算坐标，source 删除或替换使引用重验或失效。

Metadata 中 identity、timestamp、reported context 与 serving label 分开。共享时间有明确精度含义；wire 格式/必填性由 profile 拥有。不补造时间或回显，不以当前 Number carrier 限制其他协议。中途 instruction 位置与 authority 不能为了一个 global system 字符串被拼接丢失。

## 9. 纯层与运行时边界

共用 decoded carrier、strict JSON、codec error、representation/replay records 与必要 metadata 不属于 OpenAI 专属命名空间。各协议拥有自己的 grammar、profile/default 与目标 lowering，不把 Google 原生语法作为全局 WireRules 的另一组布尔值。

分别派生 request、response 与 history replay requirements；响应准入不通过临时 request 代替。Event checks 与最终检查一致；known unsupported 分支不能成为扩展旁路。输入 Public Model 准入不限制实际报告事实的独立 downstream contract。

采用闭合 enum 和明确接口，不引入动态插件 registry、反射 mapper、SDK DTO 或新的 Provider IR。先让当前 profiles 在共用边界上成立，不为未来协议创建空模块。

后续 runtime 接入再调整 upstream decoder、downstream encoder、delivery intent、topology/bootstrap 与可信 operation/path。上下游协议独立；强制上游 SSE不改写下游 JSON 意图。Endpoint 路径仍来自受信绑定，compiler 验证 origin/operation/auth 关系；业务数据不能扩张。

固定候选、总预算、增量 intake、publication/commit、backpressure、strict EOF、取消与 late-error 行为保留。实际 Developer 接入仅推理，不建设账号或 session 服务。

## 10. 客户端交付与回传

库级表达不证明客户端 carrier 闭合。必要 signature、分组或 structured authority 不能依赖 SDK 偶然容忍 unknown fields。

后续选择明确 scoped carrier、原生入口或可闭合的严格标准子集；缺失载体的情况保持拒绝。新增扩展必须明确 schema/version、attachment、scope、资源、变换与回传验证，不能伪装成标准 encrypted_content。

Carrier 语法、网关跨请求认证机制及更广状态服务不由本合同自动定案。第一阶段完善 typed 要求与拒绝边界；运行激活须等选定场景能交付→保存→回传。

## 11. 实施与验收

推进切片与非目标归 [next-goal](../implementation-plans/next-goal.md)，当前行为范围归 [current-focus](../implementation-plans/current-focus.md)。每片更新受影响的类型、caller、codec/lowering、序列化、独立预期和公共合同；未发布 API 不保留 legacy alias 或第二权威。

核心验收采用有序输入→opaque thought→并行 structured calls→等待结果→正常与错误结果→依赖检查→后继 response 的闭合场景。它是离线设计 gate，不授权工具执行或真实请求，也不要求提前注册 Google runtime。

独立反例覆盖插入/替换/删除/重排、数值和 duplicate-key 预算、空值与 partial shell、call kind/identity、replay scope/format/attachment、累计计量、坐标单位、Static/Event 终态及发布后失败。最低 owning layer 保护不变量；不复制逐模型矩阵或用 round trip 自证。方法归[验收基线](../references/conformance-baseline.md)，执行命令归[开发指南](../development.md)。

## 12. 来源与未定边界

- [Google Interactions v1](https://ai.google.dev/api/interactions-api-v1)：结构化 steps、调用/结果、计量及引用坐标。
- [Google thinking](https://ai.google.dev/gemini-api/docs/thinking)、[streaming](https://ai.google.dev/gemini-api/docs/interactions/streaming)与[stateless 示例](https://ai.google.dev/gemini-api/docs/quickstart.md.txt)：opaque、增量与完整 history 依赖。
- [Anthropic Messages](https://platform.claude.com/docs/en/api/messages)：独立检验控制转移、structured input 和 signature 概念，不引入其 Provider 接入。
- 现有 OpenAI/Codex 来源保持[固定基线](../references/upstream-sync.md)，不因本方案隐式升级。

Google 网页/schema/示例的 required/optional 与事件内容需要在对应 codec 实施前统一到明确 API/schema/SDK/profile。结构准入、必要续轮与真实执行证据分别判断；未知事项不补猜，也不从动态资料推定实例支持。

Google 页面文字采用 CC-BY-4.0、示例采用 Apache-2.0；这里保留必要来源，不复制 SDK 实现、真实会话、动态库存或测试结果。接受本合同不等于全部落地，不授予 live/付费调用、账号操作或部署权限。
