# 架构决策（ADR）

ADR 记录影响模块边界和数据流的设计决定、理由与代价，不作为任务流水账或能力验证清单。阅读顺序：
[当前架构](../architecture.md) → 本目录的有效决策 → [下一步目标](../implementation-plans/next-goal.md)。

| 决策 | 决策状态 | 实现状态 |
|---|---|---|
| [ADR-0001：Generation IR 作为语义权威](0001-generation-ir-authority.md) | 已接受 | 部分基础已存在，统一管线待实施 |
| [ADR-0002：多任务 IR 类型族与语义所有权](0002-task-ir-and-semantic-ownership.md) | 已接受；拥有任务边界与设计准入 | 类型族方向已确定，任务级准入与实现待完成 |
| [ADR-0003：IR 驱动的阶段管线与候选目标编码](0003-ir-pipeline-and-target-compilation.md) | 承接 ADR-0001/0002 已接受的顺序，维护阶段细则 | 最终 IR 驱动 requirements 的生产顺序尚未闭合 |
| [ADR-0004：来源记录、语义所有权与保真结果](0004-source-records-and-fidelity.md) | 承接既有所有权决定，接替身份/presence/元数据细则 | Native 静态内容部分迁移，来源与其余语义仍待收敛 |
| [ADR-0005：Event IR 权威、静态一致性与交付生命周期](0005-event-ir-and-delivery-lifecycle.md) | 承接既有 Event 与生命周期约束 | Native Event 仍有来源事件编码路径 |

## 决策归属与阅读

先读 ADR-0001 的“全流程 IR”定义，再按任务问题读 ADR-0002，按处理顺序读 ADR-0003，按来源保留和变换读 ADR-0004，按流式交付读 ADR-0005。ADR-0003–0005 是已有决定的职责细化，不新增任务、动态路由、hook 或运行时实施授权。

ADR-0001/0002 的既有入口保留原则摘要与链接，详细阶段和元数据规则不再双份维护。实现差距归[当前状态边界](../implementation-status/current-boundaries.md)，验收方法归 [semantic testing](../../testdata/semantic-testing.md#9-provider-无关的任务-ircodec-验收)；本索引的短状态不代替二者。

## 格式与维护

每篇 ADR 使用以下结构：

- **状态**：分开说明决策是否接受、实现是否完成；接受方向不等于授权执行代码变更。
- **背景与问题**：需要解决的具体矛盾，不复制整个项目介绍。
- **决策**：数据流、职责、约束及明确不做什么。
- **替代方案与理由**：只记录真正影响选择的方案和代价。
- **影响与落实**：链接当前差距、下一步目标和验收原则，不复制测试报告。

当前结构由架构页维护，产品行为由需求页维护；ADR 负责解释跨模块决定。决策被替代时标明替代关系，不保留两套同时有效的规则。外部协议资料仍放在 [references](../references/README.md)，执行记录仍放在 [evidence](../implementation-status/evidence/README.md)。
