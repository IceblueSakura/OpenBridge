# OpenBridge 文档

当前工作区只维护 v2 语义核心、codec、lowering 与离线验收；没有服务或 Provider 执行入口。旧路线在 [Git 归档](archive.md)中，不保留工作区兼容副本。

| 文档 | 事实所有权 |
|---|---|
| [根 README](../README.md) | 当前可用入口、构建与最小使用范围 |
| [当前架构](architecture.md) | 实际模块结构与依赖 |
| [v2 架构](architecture-v2/README.md)及其 decisions | 当前设计和后续目标边界；不表示已实现 |
| [迁移计划](architecture-v2/migration.md) | 语义迁移阶段、具体剩余缺口 |
| [Responses text profile](architecture-v2/responses-text-profile.md) | 当前纯文本准入与字段归属 |
| [当前焦点](implementation-plans/current-focus.md) | 已获准切片及验收缺口，不自动授权 |
| [下一步目标](implementation-plans/next-goal.md) | 推进顺序，不写完成日志 |
| [开发指南](development.md) | 本地与固定 SDK 验证入口 |
| [实施边界](implementation-status/README.md) | 验证层级与历史证据解释 |
| [外部参考](references/README.md) | 固定协议/SDK/Provider 快照及必要 attribution |
| [归档说明](archive.md) | 旧源码与合同恢复点，不是 v2 功能承诺 |

一个事实只保留一个权威位置，局部实现细则放源码注释和测试。类型表达、codec 映射、实际执行证据与生产接线分别判断；round trip 或编译通过不能证明语义完整。

文档修改需检查相对路径、锚点、示例和规则一致性，运行 `git diff --check`。协议事实以固定来源为准；不要为本地整理刷新外部验证日期。历史证据中的归档链接不是当前依赖，不恢复旧模块来消除它们。
