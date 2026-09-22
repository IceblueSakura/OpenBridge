# 下一步目标：多任务 IR 与离线 codec 验收

> 本页保留前代 IR/Native 迁移路线作为参考。`semantic-v2` 当前按 [v2 迁移计划](../architecture-v2/migration.md)推进 Generation；本页的 Native 来源记录收尾顺序不作为 v2 工作主线。

## 目标与状态

**先完成任务级设计准入，再推进 codec 迁移与新语料验收。** [ADR-0001](../decisions/0001-generation-ir-authority.md) 定义 IR 语义权威，[ADR-0002](../decisions/0002-task-ir-and-semantic-ownership.md) 定义多任务类型族与设计准入；[ADR-0003](../decisions/0003-ir-pipeline-and-target-compilation.md)、[ADR-0004](../decisions/0004-source-records-and-fidelity.md)、[ADR-0005](../decisions/0005-event-ir-and-delivery-lifecycle.md) 分别维护阶段管线、来源保真与 Event 交付细则。完整管线尚未闭合，接受设计不等于类型与映射已经齐备。

这不是要求每个请求都能跨 Provider 或跨协议，也不包含 hook 实现。具体获准实施切片另记于[当前开发焦点](current-focus.md)。

## 当前起点

已有 Generation Static/Event IR、Chat/Responses codec、固定路由和有界生命周期；Native 普通采样控制与受限静态内容变换已从 IR 编码，消息/part 身份和 function-tool 历史可用于安全的增删改与重排。但其他任务尚未形成完整语义 IR，媒体、presence、扩展与 Event 所有权仍有缺口，生产 decode/requirements 顺序尚未调整。具体边界见[当前状态](../implementation-status/current-boundaries.md)。后续仍按完整语义切片推进，不直接以较窄 Bridge encoder 替换 Native。

## 推进顺序

| 阶段 | 交付结果 | 关键验收 |
|---|---|---|
| 1. 任务设计准入 | 核对 Generation、Embedding、Image Generation 和已支持 Speech 任务；逐语义确定类型、保留元数据、扩展和拒绝边界 | 有意义的输入差异可区分；请求/响应闭合；Static/Event、预算与错误契约一致 |
| 2. 语料与独立 oracle | 设计准入通过后选择最小协议语料；覆盖媒体、工具、向量和事件边界，不引入质量 benchmark | 来源与许可证可用；expected 独立于生产 codec；缺陷负例可检出 |
| 3. 补齐任务类型并迁移 codec | 按完整任务/语义切片补齐缺口；同协议请求与响应由 IR 编码，候选投影独立 | decode/encode 分别正确；未修改时保真；新增、替换、删除有效；无源值复活 |
| 4. 闭合生产管线 | 先解析固定任务再 decode；变换后 requirements 预检与既定路由；响应及事件反向编码 | 候选顺序与能力交集不变；不可表达在安全边界失败；取消、背压与 commit 不退化 |

这些阶段是依赖顺序，不是要求拆成固定数量的 PR。每个实施切片必须同时维护其受影响的 codec、调用方和合同，不能长期保留相互冲突的双路径。

设计阶段可以使用小型反例和候选语料核对假设，但不能以现有测试通过代替语义覆盖审查，也不能把来源预筛写成正式数据集验收。各任务的通过条件见 [ADR-0002](../decisions/0002-task-ir-and-semantic-ownership.md#6-设计准入先于语料扩张)。

## 验收原则

- **同协议保真**：保护已支持的普通内容、工具、reasoning、refusal、媒体与扩展语义，不以跨协议较窄子集裁剪 Native。
- **IR 修改有效**：测试分别覆盖新增、替换、删除；期望值独立于生产 encoder，避免自证。
- **转换边界明确**：精确保留、等价规范化、来源限定保留和拒绝各有明确结果，不静默丢失。
- **资源与失败安全**：覆盖超限、非法扩展、工具身份冲突、异常终态、取消和提交后失败。
- **验证适度**：优先最低 owning layer 的确定性测试，必要时补 production Router 场景；不扩成逐模型全排列验证。
- **Provider 无关**：codec 验收离线运行，但固定 wire 协议与 profile；模型质量、账号可用性与真实 Provider 接入不是其 oracle。方法见 [semantic testing](../../testdata/semantic-testing.md#9-provider-无关的任务-ircodec-验收)。

具体命令见[开发指南](../development.md)。真实 Provider、SDK/Agent 与负载验证按具体变更另定；不以静态文档或 codec 单测声称外部兼容。

### 切片完成判据

每个语义切片按 [semantic testing 的最小验收映射](../../testdata/semantic-testing.md#切片验收映射与完成判据)分别给出类型表达、codec 映射、生产接线与实际执行证据。不变保真、增删改和重排、失败边界及适用 Static/Event/materialize 都有独立 expected；候选隔离和最终 IR requirements 需由对应 owning layer 及必要生产见证证明。

已支持语义仍只能靠未解释的原 JSON 才能输出、或 Event 只校验而仍按旧 payload 编码时，不宣称该范围全流程闭合。移除重复 decode 只是来源记录切片的一项结果，不是整个管线完成或性能提升的证明。文档细化不改变下方既定迁移顺序，也不开放额外任务。

## 本目标不包含

- hook/plugin 系统、动态脚本、工具执行器、工具拦截续轮或分析策略实现；
- 动态能力路由、任意跨源 fallback、上游有状态能力；
- 把 Embeddings、Images 或专用 Speech 任务硬塞进 Generation IR，把 MCP 或管理接口伪装成推理任务；
- 逐模型能力扩张、生产部署或真实付费探测。

## 首个 Generation 实施切片的准入

身份、消息分组、presence 与附属元数据的设计规则由 [ADR-0004](../decisions/0004-source-records-and-fidelity.md)拥有，阶段与流式约束分别见 ADR-0003/0005。规则已明确不等于现有类型、codec 和生产接线已经通过准入；本页不授予代码实施权限。

Generation 文本、消息身份与 function-tool 历史已具有同协议请求和静态响应的受限 IR 权威编码。下面的准入要求继续约束该范围的扩展；当前实现不能安全表达的变换仍拒绝，不能把局部落地视为整个 Generation 已通过设计准入。后续切片继续按支持语义列出类型、decode、encode、保留元数据与失败结果的对应关系，并确定独立 expected：

- 同一消息的正文、reasoning、refusal 和 calls 保持明确归属；只覆盖现有合法组合，不借重构扩展接受域。
- 新增、替换、删除与重排影响真实 wire；call/result 关联重新验证，元数据不因旧数组位置误绑，删除项不复活。
- 未修改内容保持同协议语义；未迁移的媒体或扩展有明确保留边界，不能用较窄 Bridge encoder 替代 Native，也不能在修改其归属后仍整段重放旧源对象。
- 请求与静态响应配对，presence/default 有明确解释；候选数、工具身份及失败终态不出现只验请求的闭合缺口。多候选是否开放仍由既有公共任务合同决定。
- Event 身份与 materialize 结果同该切片兼容；静态迁移不等于 Event 已迁移，受影响的共享类型、事件调用方和合同必须同步维护。

优先使用小型离线反例验证设计，不先扩大正式语料。获准行为实施时才记录 current-focus 并以失败测试推进，不在内容迁移中夹带管线重写。

## 近期迁移收尾顺序

先收敛仍承担实际功能的过渡结构，不把“删除旧代码”作为完成标准。`BridgePlan` 的 Static/Event 编排、来源限定字段的安全保留和失败关闭仍有实际职责，不能仅为减少层数而删除。

| 顺序 | 范围与交付 | 验收边界 |
|---|---|---|
| 1. Native 来源记录 | 从当前 Native encoder 为比较基线而再次 decode 源对象的路径入手，在初次 decode 时形成有界、不可变的来源记录及身份绑定；请求和静态响应一起收敛 | 编码不再为取得基线重复语义 decode；不建立第二个可变语义 owner；未修改内容保真，增删改/重排、新增项不继承旧元数据、未迁移语义拒绝边界均保持。检查来源记录的内存、克隆和生命周期成本，不以无界快照替代重复解析 |
| 2. annotation 身份统一 | 将通用 Responses encoder 的 item ID + 旧 content 下标回填收敛到稳定 item/part 身份，贯通 Native 静态、跨协议与 Event materialize 后的适用路径 | 多 part 与 item 重排不误绑；删除不恢复附属记录；改写被引用内容时重新校验范围，不能沿用旧 offset。不能精确保留时仍拒绝，不靠清空 annotation 让测试通过 |
| 3. 剩余语义与管线 | 来源所有权收敛后，按完整切片补齐 presence/投影、媒体与扩展表达，再闭合 Event 编码和固定任务后的 decode/requirements 顺序 | 不同时重写所有任务；候选独立投影，固定 Route 顺序和能力交集不变；Static/Event 终态、资源预算、取消、背压与 commit 边界继续成立 |

首个收尾切片优先处理来源记录；annotation 可按独立切片推进，但不能留下两套对同一语义互相冲突的合并规则。具体类型与持有位置先对照实际调用方和预算确定，不预设通用 sidecar 框架，也不因来源记录就绪而宣称生产管线已经实现一次 decode。

验证先运行最低 owning layer 的独立 decode/encode 预期及变换负例，再执行[开发指南](../development.md#rust-行为与依赖)的完整 Rust 基线与文档检查。保留有独立价值的 production wrapper、流式终态和资源失败测试；不恢复包装层与其底层同一实现的“新旧对跑”自证。真实 Provider、外部 SDK、负载与部署不在此收尾计划的默认执行范围内。

其余任务继续按任务族明确媒体用途、Embedding 精度和事件表达，不同时实现所有类型。具体 Rust 接口由对应切片及失败测试确定，不为未开放任务提前搭建框架。
