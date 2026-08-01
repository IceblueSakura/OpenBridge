# 实施计划

本目录保留为满足某项功能需求所需的**实施假设**、最小边界与验证思路；它不是需求清单、路线图或兼容承诺。产品目标、非目标和验收行为以[功能需求](../functional-requirements/README.md)为准，已经证明的结论以[实施现状](../implementation-status/README.md)为准。

每次只从一个已定义的功能需求选择一个可观察行为，并在[当前开发焦点](current-focus.md)中先写失败测试。没有进入当前焦点的计划文档不构成必须完成的工作。

## 文档权威顺序

架构相关材料按以下顺序解释，后项不得覆盖前项：

1. [功能需求](../functional-requirements/README.md)：定义客户端可观察行为、安全边界与非目标；
2. [当前代码架构](../implementation-status/current-architecture.md)和[当前实现说明](../implementation-status/current-implementation.md)：定义 live source 已经实现什么；
3. [目标服务架构](service-architecture.md)：定义希望达到的稳定分层和终态概念，不决定实施次序；
4. [架构迁移总计划](registry-architecture-migration.md)：唯一维护 M0–M7 的依赖、切片、退出条件和专项计划映射；
5. 专项计划：只展开总计划中的一个边界，不得自行增加前置阶段或形成另一条路线图；
6. [当前开发焦点](current-focus.md)：唯一表示当前获准实施的一个可观察行为。

发生冲突时，先以 live source 和功能需求修订总计划，再同步目标架构和专项计划；不能通过选择另一份计划绕过总计划的前置条件。

## 计划生命周期

计划必须服从实际代码基线，而不是按文档自动串行推进：

1. 读取 live source、工作区状态、测试与[实施现状](../implementation-status/README.md)，只选择一个最小可观察行为。
2. 在 `current-focus.md` 写入该行为、对应需求、先失败的测试、明确不做项和验证边界。
3. 完成代码后，仅将有测试或受控真实验证证据的结论写入 `../implementation-status/`。
4. 删除已完成焦点的计划条目和被实现推翻的设计细节；`current-focus.md` 回到“暂无活动焦点”。
5. 重新从 live baseline 规划下一项。后续候选不是承诺，不得因为排在队列中自动开始。

计划文档可以保留未实施的设计假设，但必须标明为候选；不得混入完成记录、历史阶段或未经验证的
Provider/硬件结论。

| 计划角色 | 文档 | 与总计划的关系 |
|---|---|---|
| 当前基线说明 | [当前代码注册表与原生路由](configuration-and-routing.md)、[Provider 适配与数据流](provider-adapters-and-dataflow.md) | 记录已切换的 M1–M4 类型和调用路径，并明确尚未补做的行为验收。 |
| 目标结构 | [目标服务架构](service-architecture.md) | 定义 M1–M7 的终态分层、实体和运行边界；不重复维护阶段。 |
| 迁移总控 | [架构迁移总计划](registry-architecture-migration.md) | 唯一维护 M0–M7 的顺序、退出条件、兼容和清理规则。 |
| Native 客户端验收 | [客户端兼容](client-compatibility.md) | 横跨 M0、M4 和 M6，保护 OpenAI SDK/Codex 可见行为。 |
| Bridge 语义 | [协议桥](protocol-bridge.md)、[Agent Loop Bridge](agent-loop-bridge.md) | 只展开 M5；必须先补做 M0–M4 行为回归门再接入生产路径。 |
| Bridge 测试证据 | [协议测试语料构建](protocol-test-corpus.md)、[Mock Server/Client 测试工具](protocol-testkit.md) | 为 M0/M5 提供独立 corpus/testkit；M5 只依赖所选 slice 已稳定的 fixture。 |
| Credential 扩展 | [OAuth 凭证边界](oauth-credential-boundary.md) | 本次 M0–M7 之外的独立后续方向，不是任何迁移切片的前置条件。 |

当某项计划被实现并验证后，将可证明的结论转入 `../implementation-status/`；不在本目录保留目标变迁、淘汰方案或旧阶段记录。若计划与功能需求冲突，先修订或废弃计划，而不是扩大产品范围。
