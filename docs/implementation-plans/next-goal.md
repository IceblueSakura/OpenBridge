# 后续计划

## 主线与范围

先稳定**可由 Gateway 与未来自研 Agent 复用的 Semantic Model / IR**，再逐片闭合请求型多模态与低损 Provider 映射。设计权威归 [Semantic Model](../architecture-v2/semantic-ir.md)，本页只规定顺序与退出条件，不复制字段或类型方案。

- 输入目标：`text / image / voice / file`；输出目标：`text / image / voice / vector`。
- Generation 的主要公开接口为规范 **OpenAI Responses**；**Chat Completions 必须兼容，允许声明范围内的语义损失**。
- Embedding 是同一语义体系中的独立 task，公开使用标准 **`/v1/embeddings`**，不把向量放进 Responses message。
- Gemini/Anthropic 原生协议接入后置。**Realtime 等差异较大的交互暂不细化设计或实现**，不作为当前稳定化 gate；长期方向保留，不预建状态机或占位类型。
- 未来 Agent 复用语义入口、事件与依赖，不在 Gateway 中提前实现规划、记忆、工具执行或自动循环。

“完整多模态”按明确的 task、用途、来源与目标闭合，不承诺每个模型支持任意输入输出组合。模型注册、wire 可表示性、产品准入和真实可用性分别核对。项目尚未发布，允许有依据的结构替换，不为当前 Rust 类型建立兼容包袱。

## 推进顺序

| 阶段 | 工作与产物 | 退出条件 |
|---|---|---|
| **1. 核心承载边界** | 检查共享内容/产物/资源、Generation 与 Embedding 的 task 边界；明确 identity、presence、数值、依赖与适用事件。报告当前 IR 缺口与结构方案，先定概念再改类型 | 用独立的多模态 Generation 与 Embedding 概念样例证明单一语义权威能表达所需内容；区分 IR 缺口、标准载体缺口、未接线。不要求 Realtime 或全部 Provider 设计完成 |
| **2. 标准投影与 Chat 损失合同** | 固定所需官方 schema/SDK/profile；确定标准 Responses 与现行本地扩展的隔离/替换方案；为 Chat 定稿有损投影方向、条件、损失与受保护约束 | 标准客户端不依赖私有字段；每条 Chat 规则有独立正反预期、静态/事件一致性及续轮后果说明。未批准损失仍拒绝，不以默认丢字段修复失败 |
| **3. 请求型多模态 Generation** | 按独立切片推进文件/音频输入、图片/音频产物及必要资源引用，保留文本/图片输入、工具结果与失败回归；优先标准 Responses 可闭合场景，并维护相应 Chat 兼容投影 | 每片请求→IR→目标→实际 JSON/适用事件交付闭合；涉及引用/history 时再验证保留→回传。预算、截断、取消、引用失效与严格终态有反例 |
| **4. Embedding 闭环** | 定稿输入/批次关联、向量数值/维度/编码、报告与预算；实现 task、标准 codec、可信绑定及入口 | 独立输入/输出 oracle、批次关联、维度/数值错误、编码预算与提交失败通过；不复用 Generation 的消息/终态假设，也不隐式提供向量数据库 |
| **5. 语义消费者稳定化** | 用独立的类型化消费者验证无需 protocol DTO、网络或私有配置即可构造/检查/编辑模型交互；收敛必要公共类型与演进规则 | Gateway 与 Agent 消费方式共享同一模型，不双写正文/事件/usage；需要跨请求保存的场景有明确版本、完整性与恢复边界，不把内存 fingerprint 当持久化协议 |

阶段 1 是必要结构选择的入口，不是全量大重构；阶段 2 随具体能力定稿，不要求先设计全部降级规则。阶段 3/4 在共享 owner 明确后可按独立性安排，不把所有媒体完成作为 Embedding 的前置。每次只实施具有独立退出条件的切片，未完成方向保留于本页。

## 下一片：先作承载与投影设计选择

下一片应集中解决以下结构问题，而不是先新增 Provider adapter：

1. **共享内容与资源**：区分媒体值、生成产物、可选 transcript、远端引用及 expiry；文件描述与音频属性不能沿用图片预算或 Chat 必填组合充当通用合同。
2. **Embedding 最小任务合同**：明确首批输入、结果关联、向量类型/维度及编码范围，使用标准 Embeddings 来源核对，不预建 sparse/multi-vector 或存储检索能力。
3. **标准 Generation 与兼容 Chat**：为一个具体场景标出标准载体、可接受损失和不可损失的行为/依赖。将现行 `_openbridge`、本地事件/枚举与标准路径分开评估，不能靠文档改名宣称已经标准化。

分析发现 IR 无法承载时，按 [IR 缺口规则](../architecture-v2/semantic-ir.md#4-ir-不足与标准载体缺口)向用户报告最小反例、受影响 owner、方案和迁移/验证后果，再选择结构。媒体没有标准 Responses 载体时，不暗加字段；提出所需标准媒体 operation 或保留明确缺口，不削掉整体目标。

## 保留但不扩张的工作

- 工具/turn/replay、Schema/adherence、reasoning 控制、usage scope/单位、引用坐标和 metadata presence 按当前切片依赖推进，不以整套 Agent 交互完成作为媒体前置。
- Provider 原生缓存保持独立意图与报告；无回答缓存、跨请求粘性路由或一般 session 管理承诺。Realtime 后续所需状态另行设计，不从当前约束推导永久排除。
- 凭据生命周期、订阅用途、观测与运行保障按实际主线需求扩展，维持[现行凭据](../credentials.md)和[执行](../architecture-v2/execution-model.md)合同，不混入语义数据。
- 既有非标准客户端 carrier 只维护现行合同；新扩展不作为缺失 IR 或 Chat 兼容的默认补丁。任何替换/删除同时更新消费者与 API 合同。

## 行为切片与验收

[current-focus](current-focus.md)只记录获准且未完成的具体行为：可观察结果、要求、失败例、非目标与验证边界。设计/计划不授予代码重构、部署、凭据或 live 权限。

每片先建立独立 synthetic 反例，再验证语义、codec/字节、实际交付与固定消费者。Chat 额外断言允许损失及保护的不变量，不用 round trip 要求恢复已丢信息，也不以它代替独立 oracle。真实调用另行固定目标、矩阵、总预算与脱敏范围；Provider 成功、缓存收益、负载和一般 Agent 兼容不由离线检查推定。

稳定合同留在 owning 文档/代码，执行结果只在交付与获准 ignored run 中报告；不建立完成日志、动态能力矩阵或第二份计划。
