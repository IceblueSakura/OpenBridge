# 协议与设计参考

**当前参考按语义主题维护，不再按项目／Provider 分块撰写。** Generation 以 OpenAI Responses 标准为主要参考，再以 scoped extensions 承载特殊能力。事实综合、接受的设计、当前实现和执行证据仍分开。

## 当前主题入口

| 入口 | 责任 |
|---|---|
| [语义模型综合](semantic-baseline.md) | 汇总历史 IR、转换、tools、state、runtime 与测试经验；按问题组织而不是逐项目比较 |
| [Responses 标准语义](responses-standard.md) | request/item/output/event、Schema、状态与 operation 的目标边界 |
| [扩展与上下文](extensions-and-context.md) | Codex session/cache/thread/turn、扩展 scope、信任与 replay |
| [多模态与资源](multimodal-and-resources.md) | 标准 image/file、特殊媒体、task/modality/wire 和资源边界 |
| [Codec 验收](conformance-baseline.md) | 准入矩阵、独立 oracle、SDK 和失败/资源边界方法；当前缺口链接到实现 owner |
| [上游同步](upstream-sync.md) | 官方页面日期、SDK/Codex commit、同步差异、证据冲突和重核入口 |

采用决定由 [IR 设计](../architecture-v2/semantic-ir.md)拥有；[迁移基线](../architecture-v2/migration.md)记录当前缺口，[current focus](../implementation-plans/current-focus.md)只记录已批准范围，不自动授权执行。

## 历史材料的角色

现有 `openai/`、`codex/`、`protocol-gateways/`、`providers/` 等来源目录保留为**固定研究原文与出处**，不再作为当前设计的分块入口，不要求继续逐来源维护。旧页面中的“当前”、建议和维护流程只适用于其原快照；当前结论以主题综合和上游同步为准。精确原文可查[整合前 Git 快照](https://github.com/IceblueSakura/OpenBridge/tree/5924f80d9af5a68dbf13185c56942ff9102b3361/docs/references)。

本轮整合的是与 IR/codec 设计相关的研究，不声称重新审计 OAuth grant、MCP server 框架、计费或运营实现。那些既有原文及[历史测试资产登记](topics/test-assets-registry.md)、[语义评测方法](semantic-testing-methods.md)按原版本保留，不因重组刷新外部验证日期。

## 维护规则

1. 新调研直接进入所属主题，同行引用来源 URL、commit/release、读取日期、适用范围与未知项；不再要求先建来源专页或 cross-project 前置页。
2. 新标准字段进入标准语义，特殊能力才进入扩展。SDK、独立开放规范、Codex 产品私有协议和其他 gateway 的容错不能混作 OpenAI 标准。
3. 一个事实只保留一个当前 owner。类型表达、codec 映射、生产接线、实际执行分别举证；不建立按项目重复维护的设计 schema。
4. 上游同步必须固定版本并处理冲突，不把网页整理日期写成外部执行日期；新增领域先核对相关一手 schema，不根据名字猜形状。
5. 保留必要 attribution 与 license。默认提炼场景并自主写 synthetic fixture，不复制企业代码、限制商业使用的数据、敏感 payload 或大段第三方源码。
6. 模型目录、价格和 capability metadata 直接引用官方来源，不维护冗余全量镜像；本地类型不证明真实 Provider 支持。
7. 扩展不能承载 auth/target/script override 或绕过资源与信任边界。真实凭据、私人配置与会话不得进入文档、工具参数或输出。
8. 维护相对链接和锚点；已执行的独立外部证据仍由 [evidence](../implementation-status/evidence/README.md)保存，不把静态规范差异称为实测 discrepancy。
