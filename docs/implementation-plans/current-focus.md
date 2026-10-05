# 当前开发焦点

## 当前范围

**当前没有待实施的已定稿行为切片。** 下一切片按[后续选片条件](next-goal.md#后续选片条件)结合具体标准/消费反例定稿。标准文本/function、类型化交互与[标准模型发现](../http-gateway.md#标准模型发现)作为后续工作的维护和回归边界，不扩大成整个 Responses union 或 Agent runtime。文件扩展重评后继续延期，SIWC 实例准入暂缓；后续选片仍按计划重评。

现有能力维护仍遵守以下合同：

- **基础静态图片生成**：数量、报告与严格交付边界见 [HTTP 指南](../http-gateway.md#独立图片生成)；[具名计量损失](../architecture-v2/protocol-and-lowering.md#独立-images-的计量投影)不授权丢弃请求控制、篡改产物报告或隐藏预算失败。
- **基础文件输入**：仅维持 Responses user [inline](../architecture-v2/responses-text-profile.md#user-inline-file-input) / [URL](../architecture-v2/responses-text-profile.md#user-file-url-input) 输入及必要正确性、安全维护；不从 PDF carrier 推定所有格式、来源或工具文件均准入，也不新增 `/v1/files` 或 file_id 服务。
- **客户端与迁移**：遵守[客户端合同](../architecture-v2/client-generation-profile.md)，不恢复独立 `_openbridge` 或隐式兼容入口；允许破坏性重写不免除 IR 结构缺口报告，也不提前应用未定稿 Chat 损失规则。

文件与必要 opaque 回传的验证分别覆盖“实际报告且回传”和“未报告”；后者即便内容正确也不证明 opaque 路径。显式 reasoning 控制不充当已生成 reasoning 的事实，有限场景通过不等于一般可靠性。新付费测试仍需明确场景，不复用旧矩阵授权扩大控制或模型范围。

## 待决问题与实施边界

等待证据或语义决策的问题归[待决状态](../implementation-status/open-questions.md)。Reasoning opaque 的闭合后权威暂缓，不作为当前行为切片或其他模态的前置；本页不重复其问题清单，文档澄清不制造校验或实现任务。

## 新切片的定稿要求

后续选片复用已有标准边界、类型化消费与回传验证；发现新的标准或消费差异时，以独立反例定位最低 owner，不重复重建已满足的合同。

后续行为实施先在本页定稿可观察结果、需求、不变量、失败例、非目标与验证边界；出现未覆盖的结构或标准分歧时先更新/确认切片，不以计划代替操作授权。文档整理不创建行为切片；实现缺口归[状态文档](../implementation-status/generation.md)。
