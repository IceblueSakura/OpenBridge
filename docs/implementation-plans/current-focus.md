# 当前开发焦点

当前没有已定稿且未完成的代码行为切片。[后续计划](next-goal.md)从共享内容/资源、Generation/Embedding 承载与标准/兼容投影的设计选择开始；IR 缺口按[决策规则](../architecture-v2/semantic-ir.md#4-ir-不足与标准载体缺口)报告。Chat 允许部分损失是设计方向，具体规则未定稿前不放宽当前 lowering。Realtime 详细设计暂缓。

现有实现边界见[缺口](../implementation-status/generation.md)，当前 HTTP 行为见[网关指南](../http-gateway.md)。

下一片定稿时在此记录可观察行为、需求、不变量、失败用例、非目标和验证边界，再以独立反例推进；不把当前方向当作实例激活、真实请求、凭据操作或部署授权。
