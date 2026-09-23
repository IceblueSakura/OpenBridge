# 下一步目标

**在仅保留 v2 的工作区完成 Generation 语义验收，再设计生产接线。** 旧运行时已[归档](../archive.md)，不要求旧入口继续可用，也不以删代码代替验收。

1. 收敛 [Responses 纯文本当前焦点](current-focus.md)：逐字段/事件准入、独立 decode/encode 预期、变换与删除、拒绝与资源边界、SDK/HTTP/SSE 生命周期。
2. 按 [v2 迁移计划](../architecture-v2/migration.md)评估其余 Generation 语义和 Chat/Responses 可表示性；媒体与其他范围的实施另行明确，不自动扩张当前切片。
3. Generation 验收通过后，再推进固定 Public Model、topology、credential、execution 与 ingress。历史 retry/fallback/cancel/commit 证据是设计输入，不是现成 v2 实现。

不重建旧 Native/Bridge 双路径、ToolPlan/gateway-tools 原型或通用插件框架；不恢复仅为兼容旧 crate path 的 shim。仍坚持最终 IR 权威、候选独立投影、固定路由和明确损失/拒绝。

此页记录方向，不授权真实 Provider 调用、付费探测、服务部署、提交或推送。
