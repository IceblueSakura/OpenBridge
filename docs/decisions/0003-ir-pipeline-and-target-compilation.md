# ADR-0003：IR 驱动的阶段管线与候选目标编码

## 状态

- **决策：承接已接受的 IR 管线方向，作为阶段契约维护。** 本页细化并接替 [ADR-0001](0001-generation-ir-authority.md) 与 [ADR-0002](0002-task-ir-and-semantic-ownership.md) 中的完整阶段顺序和跨模块归属；不改变任务边界、固定路由、公开能力或失败策略。
- **实现：生产接线尚未闭合。** 当前顺序由[架构](../architecture.md#62-generation)描述，具体差距由[实施状态](../implementation-status/current-boundaries.md#generation-与-bridge与-ir-权威目标的差距)维护。
- 本页不独立授权代码变更，不引入通用 hook、平行 planner 或新的公共 Rust 接口。

## 背景与问题

在以 wire facts 为起点的管线中，预检、JSON 归一化、候选省略、codec 和 Provider JSON 变换可能分别决定同一字段。即使每个候选都调用 decode，IR 仍可能只是后置校验结果；未来语义处理改变内容后，旧 requirements 也可能不再反映实际请求。

需要把“经过 IR”具体化为阶段输入、输出和权限，使最终语义、能力需求、候选编码与实际发送的内容保持一致。逻辑归属不要求每个阶段成为独立模块或额外持有一份数据。

## 决策

### 1. 目标数据流

```text
下游 wire
  → admission：认证、有界严格解析、endpoint 与 Public Model 标识
  → 解析固定任务及来源协议/profile 契约，不选择 Provider
  → 按任务和来源契约 decode
  → Task Request IR + 有界来源记录 + 独立交付上下文
  → 语义校验 / 已批准的受信策略处理 / 再校验
  → 从最终 IR 提取 requirements
  → 固定 Public Model 接口预检与既定 Route 计划
  → 每个候选独立 lowering、可表达性及转换策略检查
  → 目标协议/profile encode
  → 受信 transport 与 attempt 生命周期

上游 JSON / SSE
  → 有界读取/framing 与已选任务、来源 profile 下的 decode
  → Task Response IR / Event IR
  → 语义校验、适用 reducer / 受信响应处理位置
  → 下游目标 lowering、转换策略检查与 encode
  → downstream commit / body lifecycle
```

这是目标顺序，不是对当前生产调用路径的完成声明。任务、协议和模态的区分由 ADR-0002 拥有；[ADR-0004](0004-source-records-and-fidelity.md) 拥有来源记录与保真，[ADR-0005](0005-event-ir-and-delivery-lifecycle.md) 拥有 Event 与交付边界。

### 2. 阶段输入输出与权限

| 阶段 | 输入与输出 | 不负责 |
|---|---|---|
| admission / resolve task | 有界 wire 的结构事实、endpoint、Public Model 标识 + 固定契约 → 任务与来源协议/profile | Provider 选择、网络探测、猜测任务 |
| decode | 有界 wire + 明确来源契约 → Task IR、来源记录及交付信息 | registry 查询、凭据、路由、网络 |
| semantic policy / validate | Task IR + 已批准的受信策略 → 重验证后的最终 IR | 动态脚本、改变执行拓扑、任意工具执行 |
| requirements | 最终 IR → 语义与资源需求；与独立交付约束组合供预检 | 从旧 body 恢复已删除需求或沿用过期能力授权 |
| preflight / plan | 最终 requirements + 固定接口 → 既定候选和执行约束 | 按请求筛选、重排或扩张 Route |
| candidate lowering | 同一不可变最终 IR + 合法来源记录 + 目标契约 → 目标可表达表示与转换说明 | 原地污染共享 IR、无授权削弱语义 |
| encode | 已通过策略检查的目标表示 → wire DTO/bytes | 重新决定业务默认值、通过源字段恢复已删除内容 |
| execute | 编码结果 + 受信 transport binding → 有界上游结果 | 另建业务语义、以请求字段覆盖 endpoint/credential |

lowering 指将任务语义投影到确定的目标协议/profile 表示，并判断是否可表达；encode 负责该表示的序列化。两者可以由同一纯 codec 实现，不强制新增“编码计划”类型。复用已有 IR、TargetRequest/TargetResponse 与 BridgePlan 的有效职责，不建立长期平行实现。

### 3. 任务识别与 requirements 的权威

任务解析只消费受信 Public Model 契约。未知模型、任务与 endpoint 不匹配在语义执行前拒绝；不根据正文内容猜任务，不为找到可接受的 decoder 而改选 Provider。profile 是调用方给 codec 的明确协议解释契约，不是 codec 查询 registry 的许可。

admission facts 可以服务结构拒绝、限额和错误定位，但不能成为语义变换后的平行能力权威。改变语义后重新验证身份、关联、presence、资源和任务不变量，再从最终 IR 计算 requirements。需要固定接口事实的默认值或 reasoning 策略可以接收预先解析的受信契约；读取契约不等于选择候选，也不授权在 decode 前改写业务内容。

交付上下文只拥有实际交付选择及其预算，例如 stream/non-stream 的执行约束。具有输出含义的投影、usage 请求及任务参数不能仅因出现在 envelope 中就归入无语义元数据；按具体字段合同归属。Public Model 与 upstream model 的绑定仍由受信执行契约决定，不是客户端可变换的路由指令。

### 4. 候选隔离与固定 Route

每个候选从同一不可变最终 IR 独立生成目标表示，来源记录按目标合法性只读消费。参数省略、reasoning wire 映射和保真记录均属于该候选；后续候选不得继承前一候选的省略结果。所有已批准省略都保留原有适用条件，不借重构扩大权限。

目标不可表达不构成“跳过较弱 Route”的许可。固定接口交集、候选顺序、重试和 fallback 行为继续服从[路由与韧性合同](../functional-requirements/routing-resilience.md)；不得把协议拒绝改写成可重放 transport failure。任务 IR 不解除 Embedding 的 vector identity 限制，也不开放任何上游有状态能力。

### 5. Provider 映射与执行上下文

语义默认、约束、内容/工具变换及已授权省略归受信 IR policy 或 candidate lowering；字段拼写与 profile-specific representation 归目标编码；URI、认证 header、credential 和网络超时执行归独立上下文。

Provider 映射只接收已选目标的必要契约，不选择 Public Model 或 Route。完成目标编码后，可以执行不改变任务含义的封装、安全检查及 transport binding，但不得再由通用 JSON hook 自由恢复、删除或改变 IR 已拥有的业务语义。编码中确定的 model 等受信绑定可以被校验，不能与后续映射相互矛盾。

### 6. 响应反向闭合

上游结果按固定任务和已选来源 profile decode，静态、事件和显式 buffering 后的 materialize 分支使用同一语义所有权。返回值、usage、refusal 和真实任务终态不能仅因交付方式不同而改变含义；编码错误仍受提交边界约束。

HTTP transport failure、Provider 错误 envelope 和任务语义终态保持各自 owner；不要求将所有管理或传输错误塞入 GenerationResponse。错误分类、重试许可和对外错误形状仍由原产品合同拥有。

## 替代方案与理由

| 方案 | 结论 |
|---|---|
| wire facts 与 JSON policy 长期作为权威 | 不采用；IR 变化后容易留下过期 requirements 和第二个语义 owner |
| 每个候选从原 body 重新构建语义 | 可作为明确迁移状态，不作为目标；共享策略与来源解释容易分裂 |
| 编码后用 Provider JSON hook 决定业务值 | 不采用；语义变换无法约束真正发送的内容 |
| 新建完整平行 planner/IR | 不采用；保留既有有效职责，按语义切片迁移 |
| 最终 IR 驱动预检和独立目标编码 | 采用；阶段契约更明确，但需配对迁移 codec、调用方和测试 |

## 影响与落实

按[下一步目标](../implementation-plans/next-goal.md)先收敛来源记录与身份，再按完整语义切片补齐字段及生产接线；不能为图上顺序一致立即删除安全的过渡结构或缩小 Native 接受域。

全流程 IR 是语义所有权要求，不是“每条路径只能调用一次 JSON parser”的性能承诺。消除 encoder 为取得另一份语义基线而重新 decode 的依赖，不禁止严格解析、framing、资源验证和目标合法性检查；解析次数与内存成本另按实际切片验证。

验收方法由 [semantic testing](../../testdata/semantic-testing.md#9-provider-无关的任务-ircodec-验收)维护：最终 IR 必须影响 requirements 和 wire，候选投影互不污染，后置映射不复活源值，生产接线需有独立见证。类型存在或文档更新不能代替运行时验收。
