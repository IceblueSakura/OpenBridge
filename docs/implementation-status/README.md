# 实施现状

本目录只记录已由当前代码、测试或明确验证记录支持的事实；未实施的设计和后续设想不在这里作状态声明。 同一事实出现冲突时，按“当前
checkout → 对应确定性测试 → 本目录最近一次实际验证记录”的顺序处理；历史计数 或外部观察不得覆盖 live source。

| 功能             | 文档                                            | 内容                                                                         |
|------------------|-------------------------------------------------|------------------------------------------------------------------------------|
| 全局运行行为     | [当前实现说明](current-implementation.md)       | 已实现路径、已证明范围、限制与验证命令                                       |
| 当前代码架构     | [当前代码架构](current-architecture.md)         | 按运行、注册表、接入、路由、Provider、Transport/SSE 和验证层描述 live source |
| 遥测指标         | [遥测指标](telemetry-metrics.md)                | Provider attempt 性能、usage、cache 口径及进程内读取边界                     |
| 上游能力发现     | [上游模型发现与能力探测](capability-probing.md) | 探测 CLI 的行为、边界和输出处理                                              |
| 独立协议测试资产 | [协议测试语料与工具](protocol-test-corpus.md)   | corpus、Python 管理工具、Mock Server/Client、验证结果与尚未集成边界          |

当前可路由 Provider/Public Model 矩阵和最新 Rust 验证统一写在[当前实现说明](current-implementation.md)，避免在
多个专题页维护易漂移副本。更新实现现状前，先明确证据来自哪项代码、测试或脱敏验证；静态源码、确定性 mock/fixture、外部
SDK、独立客户端、目标 Agent 与真实 Provider 的结论必须分开表述。
