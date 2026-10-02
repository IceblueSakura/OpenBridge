# OpenBridge 文档

**实现事实优先放在代码、邻近注释和独立测试中。** Markdown 只保留不能由局部实现充分表达的决策、跨模块合同、设计缺口、来源和操作方法。

## 内容所有权

| 入口 | 责任 |
|---|---|
| [根 README](../README.md) | 产品定位、使用和构建入口 |
| [AGENTS.md](../AGENTS.md) | 授权、安全、变更和验证规则；不复制架构或字段表 |
| [当前架构](architecture.md) | 跨模块职责与数据流；细节链接源码 |
| [设计与 ADR](architecture-v2/README.md) | 有效决策、理由和后果；设计不等于已经实现 |
| [Generation 缺口](implementation-status/generation.md) | 尚未准入、不可表示、未接线与验收缺口；不列完成记录 |
| [当前焦点](implementation-plans/current-focus.md) / [下一步目标](implementation-plans/next-goal.md) | 获准行为切片 / 推进方向；不是授权来源 |
| [开发指南](development.md) | 本地检查方法与验证层级，不记录执行结果 |
| [HTTP 指南](http-gateway.md) / [OpenAPI](openapi.json) | 启动与公共 HTTP 接口 |
| [凭据管理](credentials.md) | 多 profile 文件授权组件的登录、refresh、退出与存储恢复边界 |
| [Probe 指南](probes.md) | 显式计划、预算、执行和诊断边界 |
| [来源入口](references/README.md) | 必要标准出处、固定版本和许可；动态信息按需重查 |
| [归档定位](archive.md) | 旧源码的 Git 定位，不是当前兼容要求 |

## 写作与维护

- 新规则先落在 owning type、validation、codec 或独立回归；非显然的协议、兼容、安全和资源理由放在邻近注释。不要用文档弥补缺失的可执行约束。
- ADR 只回答“选什么、为什么、承担什么后果”，并链接实现 owner。字段列表、默认值、映射、预算、注册和测试场景不在 ADR 再抄一份。跨模块设计合同不能仅因实现尚缺而删除。
- 一项事实只保留一个 owner。已有 profile 文档用于解释设计准入，精确 wire 接受/拒绝由源码和独立预期维护；修改行为时检查相关合同，不能把代码偶然行为当成设计。
- 不保留历史分析、项目比较、审计报告、测试结果或完成日记，离线/SDK 结果也不例外。历史问题查 Git；当次结果在交付中报告，获准产物仅留 ignored run，不迁入注释。
- Provider/模型、实例启用及上游可用性按 [查询流程](../AGENTS.md#current-provider-model-and-compatibility-information)核对。资料链接、类型和测试存在都不是成功执行的证明。
- 保留仍使用的外部材料的来源、固定版本和许可；不为本地整理刷新外部验证日期，不复制动态 API 清单。

文档修改检查结构、相对路径、锚点、示例和规则一致性，并运行 `git diff --check`。仅文档修改不宣称运行时或 Agent 行为改善。
