# E1+ 核心后增强

增强不自动进入排期，也不阻塞 C0–C6。

| 阶段 | 功能 | 最低启动条件 |
|---|---|---|
| E1 | Usage/成本记录 | usage 归属、隐私字段和失败记录已定义 |
| E2 | 被动健康与冷却 | 不破坏 state affinity 和首输出前 fallback |
| E3 | Provider-hosted tool facade | 有真实 Provider corpus、citation/result contract |
| E4 | 本地/MCP Tool Bridge | sandbox、allowlist、timeout、输出限制明确 |
| E5 | 可选 OAuth | mock issuer 通过，官方契约和条款 preflight 明确 |
| E6 | 简单管理 UI | 不持有 secret，不演化为多租户控制面 |

每个增强必须独立定义：

- 用户价值；
- 功能边界；
- 数据与安全边界；
- 实现依赖；
- 测试；
- 退出条件；
- 不支持项。

关联模块：[M07 工具与增强](../modules/07-tools-and-enhancements.md)。
