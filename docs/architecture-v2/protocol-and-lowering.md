# Protocol Codec、目标投影与语义损失

[Semantic Model](semantic-ir.md)是唯一语义权威。本页定义 wire 映射、能力检查、损失与 fidelity；具体已实现字段归 owning codec/profile，不在这里复制 schema。执行与提交归[execution model](execution-model.md)。

## Codec 与 lowering

```text
外部 wire → codec/profile → typed IR + bounded source records
                                ↓ validation / requirements
                    fixed target + projection policy
                                ↓ lowering
                     validated representation → codec → wire
```

- Codec 拥有语法、envelope、presence、事件 grammar 与已声明 carrier；adapter 组合受信规则，不成为第二份语义模型。完整响应、请求简写和 SDK 派生视图分别验证。
- Lowering 判断最终 typed 值是否能按指定目标和投影策略表达。它不选择 Provider、查 registry、取 credential 或联网，不在 encode 后修改 JSON。
- 等价别名、已验证派生视图、精确数值推导与字段级兼容默认必须具名、有限且有前提。默认不覆盖实际报告或 malformed 值，来源记录在语义外；编码不能重做 intake 默认来恢复删除值。
- 同协议与跨协议使用同一链路。对原始字节必须在丢失键序列前拒绝重复 JSON key，并限制解析深度/节点/bytes；预解析 Value 不能证明原字节合法。共享 parser 归 [JSON owner](../../src/semantic/value/json.rs)。

## 能力与固定目标

四层合同回答不同问题：**semantic** 描述何种任务含义及模型能力，**representation** 能用何种 wire 表达，**execution** 具有什么 I/O/资源属性，**public** 向下游承诺什么。宽泛 tools/media 标志不证明每个值都可用，公共能力不能取全部候选的并集。

Canonical Model 拥有模型语义身份；Public Model 绑定 task 和固定 Route；Route 按顺序列出 Endpoint；Endpoint 绑定 Provider、可信 origin/path、upstream model、协议和表示/执行合同。Provider/auth/selected endpoint 不是 task 内容。编译关系归 [topology](../../src/topology/mod.rs)，现场激活按 [AGENTS](../../AGENTS.md#current-provider-model-and-compatibility-information)查询。

Requirements 从最终值和 delivery 推导，不含路由选择。每个固定候选独立从同一不可变 IR 投影，不能让前一候选的降级污染后一候选。投影若改变值，需重验剩余结构、依赖、大小与要求；它不能扩张已批准公共请求或目标能力，也不能靠换目标绕过授权。

## Semantic loss

目标是**低损而不是任意无损**，不构造通用保真百分比。投影分为：

| 类别 | 合同 |
|---|---|
| 精确映射 | 值、行为、关系及必要依赖保持，仅改变 wire 形式 |
| 等价归一化 | 在已声明前提下含义不变，具名规则和独立反例保护 |
| 有损兼容 | 明确哪些语义被省略/合并/降级，保证剩余结果仍满足目标合同 |
| 不可表示 | 无合法映射或超出允许损失时明确失败，不伪装成成功 |

**公开 Chat Completions 是兼容投影，允许部分语义损失。** 这项产品决策允许后续切片定义默认或显式选择的兼容规则，不要求每次调用重新批准；它不是立即放开所有字段，也不改变当前 strict lowering。Responses 的主要接口和 Embedding 的标准接口不因此继承 Chat 的损失策略。

每条有损规则至少固定：方向（request/history/response/event）、目标 profile、受影响 owner、具体损失、前提、保留的约束、对续轮/依赖的影响及独立预期。非必要展示信息的省略、多个文本单元的目标排列、附加报告的降级可作为分析对象，**不是本页已经批准的字段白名单**。精确取舍由实施切片定稿，不靠碰到错误时临时丢字段。

以下不属于普通 Chat 兼容损失：

- 弱化指令 authority、安全/权限、工具选择或行为约束来让请求通过；
- 改写工具 call identity、参数或结果关联，丢弃必要 opaque/replay 依赖却继续承诺同等续轮；
- 将音频、图片或向量冒充普通文本；未经明确内容变换合同用 transcript/caption 代替媒体；
- 把 refusal、失败、取消、截断或未闭合流改为完整成功；
- 补造 usage、timestamp、resource access、issuer 真实性，或绕过资源/提交边界。

损失应可由类型化投影结果或有界非敏感观察判别，不保存原始正文来说明损失；不能把“被省略”混同于“上游未报告”。观察的具体 API/存储在相应切片定稿，不为通知损失而默认添加私有 wire 字段。静态与流式须使用同一策略，不能在已经发布内容后改换策略或撤回事实。

**标准 wire 正确性、功能覆盖和保真度分别验收。** 有损输出必须仍是规范 Chat，而不是借兼容名义增加任意字段；往返不能被要求恢复已经声明丢失的信息。核心 IR 保留原始权威值，不为 Chat 的限制缩减设计。

## Source records

Source/fidelity records 只保存有界的表示形式、wire identity、来源与依赖证明，不保存能覆盖 typed 值的第二正文。复用要求 owner 仍存在、目标/profile/scope 兼容、依赖未失效，且不能恢复删除值。请求、静态响应与事件分别检查。

Opaque 值归 typed owner；fidelity 只绑定格式、来源/依赖。Replay 同时要求 value 和 owner 满足其格式的最终性及目标 scope；partial intake 不等于 history 可重用。绑定不能通过重新 hash 已修改历史伪造原始完整性，普通 scope 标签也不是 issuer 认证。具体依赖归[交互合同](interaction-contract.md)，实现归 [fidelity](../../src/protocol/fidelity.rs)。

SDK parsed/output-text 等派生视图只在所选合同下验证后丢弃，不覆盖 raw/structured authority。Classified extras 不承载未建模行为；需要 declared profile、预算、owner 和最终语义依赖。它们的省略条件与业务语义损失不能混同。

## 静态、事件与 Provider 边界

- Response requirements 从实际结果推导，不伪造 request 做检查。Public Model 的输入准入不充当响应 reported facts 白名单。
- Event lowering 与静态投影保持一致；terminal snapshot 不得补缺失事件、改写已交付值或完成不合法 partial。`StrictComplete` 可在终态前失败；要求先验证完整结果的消费者使用有界静态交付。
- Provider 只声明可信 origin/path/auth、安全 headers、错误分类与具体 profile；不能在 encode 后手术式改写 JSON，不能通过业务字段切换规则。
- 产品 profile 的事件终态摘要、可读 reasoning 或计量别名只有具名规则允许时成立，不扩张公共标准，也不成为新协议的通用开关集合。

Owners：[Adapter](../../src/adapter/mod.rs)、[WireRules](../../src/protocol/adaptation.rs)、[request/response lowering](../../src/lowering/generation.rs)、[event lowering](../../src/lowering/events.rs)、[shared protocol types](../../src/protocol/mod.rs)。当前 Responses/Chat 拒绝边界归相应 profiles；缺少 IR 概念按[缺口决策](semantic-ir.md#4-ir-不足与标准载体缺口)先报告结构方案，而不是添加兼容特例。
