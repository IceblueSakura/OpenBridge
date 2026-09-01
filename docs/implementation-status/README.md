# 实施现状

本目录按职责只保留六类内容：

| Owner | 只回答什么 |
|---|---|
| [当前实现](current-state.md) | 实现进度仪表盘、源码 owner 与确定性证据入口（细节由代码与模块注释拥有） |
| [Model 与 Provider 映射](model-provider-mapping.md) | 当前 canonical Model、Provider Target 与 Public Model 关系 |
| [当前状态边界](current-boundaries.md) | 全局未实现、未验证及各证据层不能证明什么 |
| [Provider 接入进度](providers/README.md) | 每个 Provider family 的接入进度、验证状态与未证明边界 |
| [当前架构](current-architecture.md) | 稳定模块责任、依赖方向与请求数据流 |
| [带日期的外部验证](evidence/README.md) | 固定日期、账号、网络、模型与 payload 下的真实 Provider/SDK/Agent 观察 |

功能合同由[功能需求](../functional-requirements/README.md)拥有，实施授权只来自
[当前开发焦点](../implementation-plans/current-focus.md)，外部协议事实由[references](../references/README.md)拥有。

同一实施事实冲突时，以当前 checkout 和对应确定性测试为准。外部记录不能覆盖后续源码，也不能替代其他账号、SDK、Agent、
fallback、负载、长期运行或生产验收。

实现细节不在本目录展开：模块行为与收窄理由写在源码 `//!`/`///` 注释和测试中，本目录只保留进度、关系、边界与证据指针。
文档不复制单模型 capability metadata。模型能力由 `src/models/`、`src/providers/` 和运行中的扩展 Models API 自描述；外部动态能力以 Provider 官方文档为准。
