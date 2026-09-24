# 下一步目标

**按 Responses-first 标准语义 + scoped extensions 基线完善 Generation，而不是追求 Chat/Responses 最小交集。** 设计见 [IR owner](../architecture-v2/semantic-ir.md)，主题综合与固定上游版本见[主题参考](../references/README.md)。旧运行时已[归档](../archive.md)，不要求恢复旧入口。

当前阶段以纯文本 Generation 为主，暂不推进多模态。Responses 是语义主干，Chat Completions 作为同一 IR 的第二协议投影验证稳定性；不另建 Chat IR，也不以 Chat 的能力上限收缩 Responses。

## 推进顺序

1. 基于[同步基线](../references/upstream-sync.md)补 request/item/part/tool/response/event/context 的逐分支准入矩阵；标准、特殊扩展与当前实现三者分开。
2. 继续关闭已有纯文本子集的缺口：Chat response_format 映射、SDK 派生 view；按实际目标完善 [Schema profile](../architecture-v2/schema-profile.md)，审查其余字段/事件的必填性和拒绝边界。具体反例见[迁移缺口](../architecture-v2/migration.md#当前已知闭合缺口)。
3. 补 `phase`、configuration update 等标准续轮语义，再按明确切片推进标准媒体/tools/state 表示。每域覆盖 request/response/event、变换/删除与失败边界，不机械照搬 SDK DTO。
4. 为 Codex context 或特殊多模态确定 extension schema、attachment、可信 scope、生命周期与公开 wire 位置，再实施。不能用 generic extra JSON 代替设计，也不恢复旧 Gateway tools executor。
5. 对已验收语义域单独设计 topology、credentials、resource/state、execution 与 ingress。类型支持不自动授权网络工具或存储；WS lane/steering 的外层状态机不塞进单 response reducer。

## 范围与未决项

[当前焦点](current-focus.md)收敛为下一步 Chat response_format 映射及其 JSON/SSE 验收；SDK 派生视图回放另开切片，其他纯文本缺口不随当前切片完成而消失。当前 Responses 与单候选 Chat 的固定 SDK 局部验收范围见[开发指南](../development.md)和 [Chat profile](../architecture-v2/chat-text-profile.md)；按域补独立双向映射和静态/事件一致性，不扩大为重复模型矩阵。

downstream 扩展字段位置/版本、Codex turn 管理模式、特殊媒体具体 profile 和各状态资源的执行 owner 需在对应实现前定稿；不可先做通用插件框架再找使用场景。Embedding、专用 Speech 等继续有独立任务合同。

此页只记录方向，不授权真实 Provider/付费调用、部署、提交或推送。
