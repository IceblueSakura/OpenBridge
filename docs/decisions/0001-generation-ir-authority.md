# ADR-0001：Generation IR 作为语义权威

## 状态

- **决策：已接受。** 用户确定的下一步方向是双向 decode → 富语义 IR → encode，而不是扩大任意请求的跨源兼容。
- **实现：按语义域收敛中。** Native 普通采样控制已由 IR 驱动编码；其余源 envelope 保留路径尚未闭合，不能宣称完整管线已完成。
- **授权边界。** 本决策不表示 hook、工具执行器或管线重构已完成，也不独立授权代码变更。
- **关联决策。** [ADR-0002](0002-task-ir-and-semantic-ownership.md) 补充任务类型族，并收窄本页的任务边界与 decode 顺序；IR 语义权威原则继续有效。

## 背景与问题

OpenBridge 同时处理下游协议、Provider 协议差异、固定路由与响应生命周期。若只在 Chat ↔ Responses 转换时使用语义 IR，而同协议请求仍由原始 JSON 决定输出，后续语义分析、工具注入或拦截就必须维护两套处理路径。

现有代码已 decode Native 请求与响应，但普通采样控制以外的请求内容仍依赖源保留，静态响应仍可重发源 envelope。它有利于保存尚未建模的字段，却不能保证任意 IR 修改、删除和约束都反映到最终 wire。问题不是缺少 IR 类型，而是 IR 尚未成为完整的输出权威。

## 决策

### 1. 双向语义管线

```text
下游 wire
  → admission（认证、大小与协议形状）
  → 解析 Public Model 的固定任务契约（不选择 Provider）
  → 按任务和下游协议 decode
  → 对应任务的 Request IR
  → 语义验证 / 受信策略处理位置
  → 从最终 IR 提取 requirements，执行固定接口预检与 Route 计划
  → 候选 Provider/API 的可表达性检查
  → 协议 encode + Provider 特定映射
  → 受信 transport

上游 JSON / SSE
  → Provider-aware decode
  → Response IR / Event IR
  → 语义验证 / 受信响应处理位置
  → 下游协议 encode
  → downstream commit / body lifecycle
```

这是目标数据流，不是对当前调用顺序的描述。路由候选及顺序仍由既定规则决定；目标不可表达性检查不引入请求期能力筛选、动态选模或任意跨源重试。

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

| 边界 | 职责 | 不负责 |
|---|---|---|
| 协议 codec | wire 与 Static/Event IR 的纯转换、协议校验 | 路由、凭据、网络、重试 |
| Provider 映射 | 已选目标的字段映射、私有扩展约束和能力收窄 | 改选 Public Model 或 Route |
| Registry / planner | 静态实体、固定接口、候选与执行计划 | 修改协议正文来掩盖不可表达语义 |
| 执行与 ingress | attempt、body I/O、取消、资源限制、commit | 自建另一套语义转换逻辑 |
| IR 处理位置 | 对受信语义变换提供清晰输入输出边界 | 当前不引入动态插件或工具运行时 |

实现可保留必要的 wire 适配，但同一语义不能在 IR 和后续 JSON hook 中各自独立决定。Provider 的语义变换应具有明确归属，编码后检查不能使被禁止或删除的字段复活。

### 5. 流式响应

SSE 增量 decode 到 Event IR 后增量 encode，不默认聚合整条流。保留事件顺序、item/call identity、参数增量、usage 和合法终态；只允许确定性规范化所需的有界状态。

保留现有取消、背压和 commit 约束：提交后不可重试拼接另一条响应，EOF 或 body error 不能伪造成功 terminal。未来需要拦截完整工具调用时，必须另行定义缓冲和提交边界。

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
