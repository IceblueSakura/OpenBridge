# 实施计划

本目录保留为满足某项功能需求所需的**实施假设**、最小边界与验证思路；它不是需求清单、路线图或兼容承诺。产品目标、非目标和验收行为以[功能需求](../functional-requirements/README.md)为准，已经证明的结论以[实施现状](../implementation-status/README.md)为准。

每次只从一个已定义的功能需求选择一个可观察行为，并在[当前开发焦点](current-focus.md)中先写失败测试。没有进入当前焦点的计划文档不构成必须完成的工作。

## 计划生命周期

计划必须服从实际代码基线，而不是按文档自动串行推进：

1. 读取 live source、工作区状态、测试与[实施现状](../implementation-status/README.md)，只选择一个最小可观察行为。
2. 在 `current-focus.md` 写入该行为、对应需求、先失败的测试、明确不做项和验证边界。
3. 完成代码后，仅将有测试或受控真实验证证据的结论写入 `../implementation-status/`。
4. 删除已完成焦点的计划条目和被实现推翻的设计细节；`current-focus.md` 回到“暂无活动焦点”。
5. 重新从 live baseline 规划下一项。后续候选不是承诺，不得因为排在队列中自动开始。

计划文档可以保留未实施的设计假设，但必须标明为候选；不得混入完成记录、历史阶段或未经验证的
Provider/硬件结论。

| 功能需求域 | 实施假设与设计材料 | 使用方式 |
|---|---|---|
| 网关 API、原生流与目标客户端 | [客户端兼容](client-compatibility.md)、[服务架构](service-architecture.md)、[Provider 适配与数据流](provider-adapters-and-dataflow.md) | 只在实现对应 endpoint、SSE、tool 或 client corpus 时查阅。 |
| 配置、凭证与受信运行 | [配置与路由](configuration-and-routing.md)、[OAuth 凭证边界](oauth-credential-boundary.md) | 配置文件优先和 API-key 核心需求优先；OAuth 仍是可选适配器。 |
| 路由、状态亲和与恢复 | [服务架构](service-architecture.md)、[Provider 适配与数据流](provider-adapters-and-dataflow.md)、[Agent Loop Bridge](agent-loop-bridge.md) | 只为当前 candidate、fallback、continuation 或 tool-loop 行为选择最小假设。 |
| 跨协议兼容 | [协议桥](protocol-bridge.md)、[Agent Loop Bridge](agent-loop-bridge.md) | 后续方向；没有明确 feature、ConversionPlan 和 fixture 时不进入实现。 |

当某项计划被实现并验证后，将可证明的结论转入 `../implementation-status/`；不在本目录保留目标变迁、淘汰方案或旧阶段记录。若计划与功能需求冲突，先修订或废弃计划，而不是扩大产品范围。
