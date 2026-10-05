# 当前开发焦点

## 当前范围

当前继续完善 Responses 文件输入，不新增 `/v1/files` 上传、存储、下载、删除或 file_id 服务。Inline 文件合同归 [Responses profile](../architecture-v2/responses-text-profile.md#user-inline-file-input)，后续扩展按[计划](next-goal.md)逐片定稿，不能从 PDF carrier 推定所有格式、来源或工具文件均准入。

文件与必要 opaque 回传的验证应分别覆盖“实际报告且回传”和“未报告”；后者即便内容正确也不证明 opaque 路径。显式 reasoning 控制不充当已生成 reasoning 的事实，有限场景通过不等于一般可靠性。

## 必要 replay 合同

已闭合 item 的完整 opaque 值若与终态不同，现行拒绝继续生效；不得按仅终态新增的方案处理已有值替换。后续片先按[实现缺口](../implementation-status/generation.md#语义与表示缺口)定清 replay 值/凭据身份、最终 authority 与更新的依赖影响，再决定最小事件和具名 wire 规则。未定稿前不增加 replay 事件、放宽 snapshot 或补建通用 attachment/framework。

独立 `_openbridge` 不属于[当前客户端合同](../architecture-v2/client-generation-profile.md)，迁移不恢复隐式兼容入口。允许破坏性重写，但新的 IR 结构缺口仍应报告概念方案和迁移影响；Chat 有损规则逐条定稿，不以方向许可提前丢字段。

下一片实施前在此记录可观察结果、需求、不变量、失败例、非目标与验证边界；计划不授予真实请求、凭据操作、部署或提交权限。实现缺口归[状态文档](../implementation-status/generation.md)。
