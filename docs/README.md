# OpenBridge 文档

当前 `main` 以原 `semantic-v2` 实现为基线，维护语义核心、双向 adapters、codec/lowering、固定 topology、caller-driven execution 和最小 loopback HTTP 网关；已有 synthetic 全链验收，不等于生产验收。主线按产品目标独立演进，不以旧版迁移或功能对等为目标；旧版只作[历史参考](archive.md)，不保留工作区兼容副本。

| 文档 | 事实所有权 |
|---|---|
| [根 README](../README.md) | 当前可用入口、构建与最小使用范围 |
| [当前架构](architecture.md) | 实际模块结构与依赖 |
| [v2 架构](architecture-v2/README.md)及其 decisions | Responses-first 标准语义 + scoped extensions 的设计、owner 与目标边界；不表示已实现 |
| [Generation 当前能力与边界](implementation-status/generation.md) | 文本输出/选定图片输入的分层完成度、已闭合正确性边界、未准入/不可表示/未接线与验收缺口；当前状态的唯一汇总 |
| [Responses text profile](architecture-v2/responses-text-profile.md) | 当前 Responses 文本输出与 user 图片输入 slice 的准入/字段归属 |
| [Chat text profile](architecture-v2/chat-text-profile.md) | 单候选 Chat envelope/SSE 准入与双协议验证边界 |
| [AGENTS 查询指南](../AGENTS.md#current-provider-model-and-compatibility-information) | 如何查询当前 Provider/模型、启动准入、adapter 与运行实例；不保留清单 |
| [Adapter 源码](../src/adapter/mod.rs)与[具名规则](../src/protocol/adaptation.rs) | 厂商 wire 规则由实现、邻近注释及独立测试维护，不另建 Markdown 适配表 |
| [Schema profile](architecture-v2/schema-profile.md) | Schema 结构、strict/default 模式、本地引用与独立资源预算 |
| [当前焦点](implementation-plans/current-focus.md) | 已选定行为切片的范围与验收条件；无进行中切片时保持空，不把建议变成授权 |
| [下一步目标](implementation-plans/next-goal.md) | 推进顺序、推荐切片与进入下一阶段的门槛，不重复能力清单或完成日志 |
| [开发指南](development.md) | 本地与固定 SDK 验证入口 |
| [Probe 指南](probes.md) | 显式 live 计划、共享预算、SDK/pi/library 执行与原调用诊断边界 |
| [HTTP 网关指南](http-gateway.md) / [OpenAPI](openapi.json) | 最小环境变量启动、HTTP 外层接口与实际 I/O 边界；字段准入仍归各 profile |
| [实施边界](implementation-status/README.md) | 验证层级与历史证据解释 |
| [主题化参考](references/README.md) | 已整合的历史结论、Responses 标准、扩展、多模态和验收；不再按来源撰写 |
| [上游同步](references/upstream-sync.md) | 本次官方页面、SDK/Codex 固定版本、差异与证据冲突；旧来源原文只作追溯 |
| [归档说明](archive.md) | 旧源码与合同恢复点，不是 v2 功能承诺 |

**代码、邻近注释和独立测试是实现事实的权威。** Markdown 只保留稳定架构、跨模块合同、设计理由、操作方法与必要来源归属，不复述可从代码查询的注册和映射。ADR 保留有效决策；固定标准/SDK 研究保留版本和许可。Provider 的易变 API 细节按需查官方来源，不另存本地快照。

不在文档维护 Provider 测试结果、当前适配模型/协议矩阵、账号可用性或临时失败状态，也不将这些清单搬到 AGENTS、代码注释或记忆中。测试结果在当次交付中报告，授权产生的脱敏产物只留 ignored run 目录；从发现中提炼出的稳定不变量和拒绝理由落在 owning code 与 synthetic 回归。类型表达、codec 映射、实例启用与实时可用性分别判断；round trip 或编译通过不能证明语义完整。

文档修改需检查相对路径、锚点、示例和规则一致性，运行 `git diff --check`。协议事实以固定来源为准；不要为本地整理刷新外部验证日期。历史证据中的归档链接不是当前依赖，不恢复旧模块来消除它们。
