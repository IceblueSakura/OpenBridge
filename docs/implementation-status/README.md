# 实施边界

[Generation 缺口](generation.md)只维护相对当前设计尚未解决的边界，不保留已完成列表、历史审计或执行报告。

- 跨模块结构见[架构](../architecture.md)，行为细节查 owning code、注释和独立测试。
- 推进方向见[下一步目标](../implementation-plans/next-goal.md)，获准切片见[当前焦点](../implementation-plans/current-focus.md)。
- 检查方法见[开发指南](../development.md)，当前 Provider/模型与实例启用按 [AGENTS](../../AGENTS.md#current-provider-model-and-compatibility-information)现场查询。

类型表达、codec 映射、网关接线、实例启用和实际执行分别判断。测试存在不等于执行通过；synthetic loopback 不证明真实 Provider、一般 Agent、负载或生产兼容。结果只在当次交付与授权 ignored artifacts 中报告。
