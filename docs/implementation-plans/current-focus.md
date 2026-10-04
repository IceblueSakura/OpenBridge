# 当前开发焦点

当前没有已定稿且未完成的代码行为切片。下一片按[后续计划](next-goal.md)聚焦 Agent-first Text/Image/File 的承载与标准 Responses 闭环，先确定文件来源/描述、工具结果和必要续轮依赖的最小场景；不扩展音频、Embedding 或其他端点。

独立 `_openbridge` 不属于[当前客户端合同](../architecture-v2/client-generation-profile.md)，迁移不恢复隐式兼容入口。允许破坏性重写，但新的 IR 结构缺口仍应报告概念方案和迁移影响；Chat 有损规则逐条定稿，不以方向许可提前丢字段。

下一片实施前在此记录可观察结果、需求、不变量、失败例、非目标与验证边界；计划不授予真实请求、凭据操作、部署或提交权限。实现缺口归[状态文档](../implementation-status/generation.md)。
