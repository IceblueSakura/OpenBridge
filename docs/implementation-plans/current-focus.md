# 当前开发焦点

## 当前范围

当前没有待实施的已定稿行为切片。图片方向维持基础静态生成，数量、报告与严格交付边界归 [HTTP 指南](../http-gateway.md#独立图片生成)及其 owning code；SSE 与高级图片功能均按[计划](next-goal.md#推进顺序与退出条件)延期，不作为当前收口前置，也不自动追加模型验证。文件扩展仍延期，下一次选片时重评，不以所有图片能力完成为前置。

独立 Images 的具名计量损失只按[投影合同](../architecture-v2/protocol-and-lowering.md#独立-images-的计量投影)适用，不授权丢弃请求控制、篡改产物报告或隐藏预算失败。新付费测试需要明确场景，不复用旧矩阵授权扩大控制或模型范围。

文件仅维持既有 Responses user inline/URL 基础输入及必要正确性、安全维护；其他文件功能暂停，恢复评估按[计划的有限首批范围与重评节点](next-goal.md#推进顺序与退出条件)执行，不等待所有模态完成，也不自动恢复实施。不新增 `/v1/files` 上传、存储、下载、删除或 file_id 服务。Inline 与 URL 文件合同分别归 [inline profile](../architecture-v2/responses-text-profile.md#user-inline-file-input)和 [URL profile](../architecture-v2/responses-text-profile.md#user-file-url-input)，其他模态的下一片按[计划](next-goal.md)单独定稿；文件扩展延期，不能从 PDF carrier 推定所有格式、来源或工具文件均准入。

文件与必要 opaque 回传的验证应分别覆盖“实际报告且回传”和“未报告”；后者即便内容正确也不证明 opaque 路径。显式 reasoning 控制不充当已生成 reasoning 的事实，有限场景通过不等于一般可靠性。

## 必要 replay 合同

已闭合 item 的完整 opaque 值若与终态不同，现行拒绝继续生效；不得按仅终态新增的方案处理已有值替换。该缺口保留但不作为下一片或其他模态的前置；重新选片时先按[实现缺口](../implementation-status/generation.md#语义与表示缺口)定清 replay 值/凭据身份、最终 authority 与更新的依赖影响，再决定最小事件和具名 wire 规则。未定稿前不增加 replay 事件、放宽 snapshot 或补建通用 attachment/framework。

独立 `_openbridge` 不属于[当前客户端合同](../architecture-v2/client-generation-profile.md)，迁移不恢复隐式兼容入口。允许破坏性重写，但新的 IR 结构缺口仍应报告概念方案和迁移影响；Chat 有损规则逐条定稿，不以方向许可提前丢字段。

后续实施先在本页定稿可观察结果、需求、不变量、失败例、非目标与验证边界；出现未覆盖的结构或标准分歧时先更新/确认切片，不以计划代替操作授权。实现缺口归[状态文档](../implementation-status/generation.md)。
