# OpenBridge 文档

当前工作区只维护 v2 语义核心、codec、lowering 与离线验收；没有服务或 Provider 执行入口。旧路线在 [Git 归档](archive.md)中，不保留工作区兼容副本。

| 文档 | 事实所有权 |
|---|---|
| [根 README](../README.md) | 当前可用入口、构建与最小使用范围 |
| [当前架构](architecture.md) | 实际模块结构与依赖 |
| [v2 架构](architecture-v2/README.md)及其 decisions | Responses-first 标准语义 + scoped extensions 的设计、owner 与目标边界；不表示已实现 |
| [迁移计划](architecture-v2/migration.md) | 语义迁移阶段、具体剩余缺口 |
| [Responses text profile](architecture-v2/responses-text-profile.md) | 当前纯文本准入与字段归属 |
| [当前焦点](implementation-plans/current-focus.md) | 已获准切片及验收缺口，不自动授权 |
| [下一步目标](implementation-plans/next-goal.md) | 推进顺序，不写完成日志 |
| [开发指南](development.md) | 本地与固定 SDK 验证入口 |
| [实施边界](implementation-status/README.md) | 验证层级与历史证据解释 |
| [主题化参考](references/README.md) | 已整合的历史结论、Responses 标准、扩展、多模态和验收；不再按来源撰写 |
| [上游同步](references/upstream-sync.md) | 本次官方页面、SDK/Codex 固定版本、差异与证据冲突；旧来源原文只作追溯 |
| [归档说明](archive.md) | 旧源码与合同恢复点，不是 v2 功能承诺 |

一个事实只保留一个权威位置，局部实现细则放源码注释和测试。类型表达、codec 映射、实际执行证据与生产接线分别判断；round trip 或编译通过不能证明语义完整。

文档修改需检查相对路径、锚点、示例和规则一致性，运行 `git diff --check`。协议事实以固定来源为准；不要为本地整理刷新外部验证日期。历史证据中的归档链接不是当前依赖，不恢复旧模块来消除它们。
