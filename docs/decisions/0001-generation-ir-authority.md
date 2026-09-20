# ADR-0001：Generation IR 作为语义权威

## 状态

- **决策：已接受。** 用户确定的下一步方向是双向 decode → 富语义 IR → encode，而不是扩大任意请求的跨源兼容。
- **实现：按语义域收敛中。** Native 普通采样控制与受限静态内容已由 IR 驱动；其余语义和生产顺序尚未闭合。逐项覆盖由[实施状态](../implementation-status/current-boundaries.md)维护。
- **授权边界。** 本决策不表示 hook、工具执行器或管线重构已完成，也不独立授权代码变更。
- **关联决策。** [ADR-0002](0002-task-ir-and-semantic-ownership.md) 补充任务边界；[ADR-0003](0003-ir-pipeline-and-target-compilation.md)、[ADR-0004](0004-source-records-and-fidelity.md)、[ADR-0005](0005-event-ir-and-delivery-lifecycle.md) 分别维护阶段、来源保真与流式细则。IR 语义权威原则继续有效。

## 背景与问题

OpenBridge 同时处理下游协议、Provider 协议差异、固定路由与响应生命周期。若只在 Chat ↔ Responses 转换时使用语义 IR，而同协议请求仍由原始 JSON 决定输出，后续语义分析、工具注入或拦截就必须维护两套处理路径。

作出本决策时，Native 请求和响应虽已进入 IR 解码与校验，大量输出语义仍由源 envelope 保留路径决定。这有利于保存尚未建模的字段，却不能保证 IR 修改、删除和约束反映到最终 wire。问题不是缺少 IR 类型，而是 IR 尚未成为完整的输出权威；该背景不代替当前实现状态。

## 决策

### 1. 双向语义管线

“全流程 IR”指：对当前产品支持范围内的每个推理任务，请求及对应响应在协议边界之间以任务 IR 为语义权威。Native 与跨协议 Bridge 使用相同语义处理边界；最终输出由最终 IR、合法来源记录、受信目标契约及显式转换策略共同生成，原始 wire 不得作为覆盖 IR 的平行语义来源。

```text
请求：下游 wire → 任务 decode → Request IR → 验证/受信处理 → 固定计划/目标 lowering → encode → 上游
响应：上游 wire → 任务 decode → Response/Event IR → 验证/适用处理 → 下游 lowering/encode → 客户端
```

完整阶段顺序和输入输出由 [ADR-0003](0003-ir-pipeline-and-target-compilation.md#1-目标数据流)维护。这是目标，不是当前调用顺序的声明；全流程不等于任意跨源兼容、所有接口都进入 Generation IR，或每条路径只允许一次 JSON 解析。

### 2. IR 是语义权威，不是原 JSON 的包装

- 已建模内容只能由 IR 决定最终编码。删除或替换过的语义不能被保留的 source envelope 恢复。
- Native 与跨协议使用相同语义边界；Native 仅表示上下游协议一致，不表示绕过 decode/encode。
- 可保留原协议的表现形式、未知字段或 opaque 内容，但必须有明确所有权、大小界限、来源和目标适用性。保留信息不能覆盖已建模字段，不能作为未经验证的透传通道。
- IR 不拥有 Public Model 解析、Route、凭据、endpoint 或网络 I/O。执行上下文与语义数据分离。

### 3. 富语义与保真

IR 表达支持范围内的语义并集，不收缩为 Chat/Responses 的最小公分母。内容、工具调用及结果、reasoning、refusal、资源、引用、usage 和终态保持独立语义与身份关系。

编码结果区分：精确保留、等价规范化、仅来源兼容目标可保留、目标不可表达。目标不可表达时默认拒绝；已有明确允许的参数省略或降级必须继续有显式策略，不能由 encoder 静默决定。无损指语义保真，不要求 JSON 字节或 SSE 分片一致。

同协议也可能受 Provider 差异限制；跨协议只对可表达子集成立。opaque reasoning、私有引用和扩展不得仅因 JSON 形状相似就跨 Provider 重放。

### 4. Codec、Provider 与执行职责

协议 codec 只处理 wire 与任务 IR 的纯转换；registry/planner 拥有固定接口和候选；Provider 映射只作用于已选目标；ingress/execution/transport 拥有网络、凭据绑定、attempt、取消和 commit。IR 本身不拥有这些执行能力。

同一语义不能同时由 IR 和后置 JSON hook 独立决定。默认、约束及授权省略归受信语义处理或目标 lowering；表现形式归目标编码；认证、URI 与网络归执行上下文。详细阶段权限见 [ADR-0003](0003-ir-pipeline-and-target-compilation.md#2-阶段输入输出与权限)，来源合并规则见 [ADR-0004](0004-source-records-and-fidelity.md)。

### 5. 流式响应

Event IR 也必须决定增量输出，而不只是校验待重发的原事件。默认逐事件 decode/encode，仅保留必要有界状态；Static/Event 在支持语义上保持一致，不以完整流缓存换取形式统一。

取消、背压及提交后不可拼流的边界继续有效，EOF 或 body error 不能伪造成功 terminal。详细理由与约束见 [ADR-0005](0005-event-ir-and-delivery-lifecycle.md)；未来完整工具调用拦截必须另定缓冲和提交边界。

### 6. 范围与非目标

本 ADR 拥有 Generation 的 Chat/Responses 请求、JSON 响应与 SSE 响应，包括其支持范围内的多模态内容。Embeddings、Images Generations 与专用 Speech 任务按 [ADR-0002](0002-task-ir-and-semantic-ownership.md) 建立各自语义，不因复用 Chat wire 就并入对话 Generation；Models、健康检查和 MCP 不进入模型推理 IR。

现在不实现通用 hook 注册、动态脚本、工具注入或拦截执行器、工具续轮循环、任意跨 Provider 转换，也不改变固定路由、状态契约或凭据边界。已有局部 ToolPlan/test-only 机制不等于生产 hook 平台。

## 替代方案与理由

| 方案 | 结论 |
|---|---|
| Native 透传，仅跨协议使用 IR | 放弃作为目标架构；语义修改与拦截必须维护两条路径，难以保证一致性 |
| 把全部原始 JSON 包在 IR 中再转发 | 不能解决语义权威问题；只可作为受约束的保留元数据 |
| 只建模所有协议共有字段 | 丢失原生丰富语义，违背保真目标 |
| 全部响应先聚合再转换 | 不适用于默认流式路径，会改变延迟和资源生命周期 |
| 富语义 IR + 受约束扩展 + 目标可表达性检查 | 采用；维护成本更高，但语义变换只有一个权威位置 |

## 影响与落实

主要成本在补齐当前依赖 source 保留的语义、定义扩展合并规则和收敛 Provider JSON 变换，不在新增一套平行 Generation IR。先完成任务表达与语义所有权核查，再以同协议保真和 IR 修改生效为门槛迁移 codec；请求与对应静态/流式响应必须在支持范围内闭合。

当前模块与数据流见[架构](../architecture.md)，实现差距见[当前状态边界](../implementation-status/current-boundaries.md)，阶段目标与验收见[下一步目标](../implementation-plans/next-goal.md)。本 ADR 不维护逐模型能力表或验证来源目录。
