# 实施现状

本目录只记录已由当前代码、测试或明确验证记录支持的事实；未实施的设计和后续设想不在这里作状态声明。

| 功能 | 文档 | 内容 |
|---|---|---|
| 全局运行行为 | [当前实现说明](current-implementation.md) | 已实现路径、已证明范围、限制与验证命令 |
| 当前代码架构 | [当前代码架构](current-architecture.md) | 按运行、注册表、接入、路由、Provider、Transport/SSE 和验证层描述 live source |
| 上游能力发现 | [上游模型发现与能力探测](capability-probing.md) | 探测 CLI 的行为、边界和输出处理 |
| 独立协议语料 | [协议测试语料与工具](protocol-test-corpus.md) | 已构建的 corpus、独立 Python 工具、验证结果与尚未集成边界 |
| Mock 协议两端 | [Mock Server/Client 测试工具](protocol-testkit.md) | 增量 SSE parser、scenario/plan 编译、HTTP/1.1 mock 两端、observation 与验证边界 |

更新实现现状前，先明确证据来自哪项代码、测试或脱敏验证；SDK、mock 与真实客户端/Provider 的结论必须分开表述。
