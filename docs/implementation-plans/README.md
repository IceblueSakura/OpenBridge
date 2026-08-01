# 实施计划

本目录保留为满足某项功能需求所需的**实施假设**、最小边界与验证思路；它不是需求清单、路线图或兼容承诺。产品目标、非目标和验收行为以[功能需求](../functional-requirements/README.md)为准，已经证明的结论以[实施现状](../implementation-status/README.md)为准。

每次只从一个已定义的功能需求选择一个可观察行为，并在[当前开发焦点](current-focus.md)中先写失败测试。没有进入当前焦点的计划文档不构成必须完成的工作。

## 文档权威顺序

架构相关材料按以下顺序解释，后项不得覆盖前项：

1. [功能需求](../functional-requirements/README.md)：定义客户端可观察行为、安全边界与非目标；
2. [当前代码架构](../implementation-status/current-architecture.md)和[当前实现说明](../implementation-status/current-implementation.md)：定义 live source 已经实现什么；
3. [服务架构与扩展边界](service-architecture.md)：说明当前分层以及尚未实现能力可接入的位置；
4. 专项计划：描述一个尚未实现功能的边界、前提和验证方式；
5. [当前开发焦点](current-focus.md)：唯一表示当前获准实施的一个可观察行为。

发生冲突时，以 live source 和功能需求修订当前架构及专项计划；计划不得把尚未实现的类型或行为写成当前事实。

## 计划生命周期

计划必须服从实际代码基线，而不是按文档自动串行推进：

1. 读取 live source、工作区状态、测试与[实施现状](../implementation-status/README.md)，只选择一个最小可观察行为。
2. 在 `current-focus.md` 写入该行为、对应需求、先失败的测试、明确不做项和验证边界。
3. 完成代码后，仅将有测试或受控真实验证证据的结论写入 `../implementation-status/`。
4. 删除已完成焦点的计划条目和被实现推翻的设计细节；`current-focus.md` 回到“暂无活动焦点”。
5. 重新从 live baseline 规划下一项。后续候选不是承诺，不得因为排在队列中自动开始。

计划文档可以保留未实施的设计假设，但必须标明为候选；不得混入完成记录、历史阶段或未经验证的
Provider/硬件结论。

| 计划角色 | 文档 | 用途 |
|---|---|---|
| 当前基线说明 | [当前代码注册表与原生路由](configuration-and-routing.md)、[Provider 适配与数据流](provider-adapters-and-dataflow.md) | 记录当前类型、数据所有权和 Native 调用路径。 |
| 服务边界 | [服务架构与扩展边界](service-architecture.md) | 汇总当前分层，并标出尚未实现能力的接入约束。 |
| Native 客户端验收 | [客户端兼容](client-compatibility.md) | 定义 OpenAI SDK/Codex 可见行为的验证方式。 |
| Bridge 语义 | [协议桥](protocol-bridge.md)、[Agent Loop Bridge](agent-loop-bridge.md) | 尚未实现；只描述明确的协议、identity 与 state 约束。 |
| Bridge 测试证据 | [协议测试语料构建](protocol-test-corpus.md)、[Mock Server/Client 测试工具](protocol-testkit.md) | 维护独立 corpus/testkit，不代表运行时已经接入 Bridge。 |
| Credential 扩展 | [OAuth 凭证边界](oauth-credential-boundary.md) | Deferred；不是当前 API-key 数据面的组成部分。 |

当某项计划被实现并验证后，将可证明的结论转入 `../implementation-status/`；不在本目录保留目标变迁、淘汰方案或旧阶段记录。若计划与功能需求冲突，先修订或废弃计划，而不是扩大产品范围。
